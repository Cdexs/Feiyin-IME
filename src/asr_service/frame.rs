//! ASR-SERVER-491 S3：内核 ↔ 网关的 IPC 帧（设计 §3）。
//!
//! 帧 = `长度 u32 LE`（其后字节数）+ `类型 u8` + `会话号 u64 LE` + 内容。一条连接上多路会话复用。
//!
//! | 方向 | 类型 | 内容 |
//! | --- | --- | --- |
//! | 网关 → 内核 | `Hello` | 握手口令（UTF-8；内核拉起网关时下发，防本机其他程序冒充网关） |
//! | 网关 → 内核 | `Open` | 会话参数 JSON（[`OpenParams`]） |
//! | 网关 → 内核 | `Audio` | 16kHz 单声道 PCM，i16 LE |
//! | 网关 → 内核 | `Finish` / `Cancel` | 说完了 / 取消（无内容） |
//! | 内核 → 网关 | `Result` | 识别结果 JSON（[`ResultPayload`]） |
//! | 内核 → 网关 | `End` | 会话结束 JSON（[`EndPayload`]；出错也走这里） |

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

/// 单帧内容上限（防坏帧把内存撑爆）。100ms 音频 = 3200 字节，结果文本远小于此。
pub(crate) const MAX_FRAME: usize = 1 << 20;

const HEADER: usize = 1 + 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Kind {
    Hello = 0x01,
    Open = 0x02,
    Audio = 0x03,
    Finish = 0x04,
    Cancel = 0x05,
    Result = 0x81,
    End = 0x82,
}

impl Kind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0x01 => Self::Hello,
            0x02 => Self::Open,
            0x03 => Self::Audio,
            0x04 => Self::Finish,
            0x05 => Self::Cancel,
            0x81 => Self::Result,
            0x82 => Self::End,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub kind: Kind,
    pub session: u64,
    pub body: Vec<u8>,
}

impl Frame {
    pub(crate) fn new(kind: Kind, session: u64, body: Vec<u8>) -> Self {
        Self {
            kind,
            session,
            body,
        }
    }

    pub(crate) fn json<T: Serialize>(kind: Kind, session: u64, v: &T) -> Self {
        Self::new(kind, session, serde_json::to_vec(v).unwrap_or_default())
    }
}

/// 写一帧（调用方保证同一写端串行）。
pub(crate) fn write_frame(w: &mut impl Write, f: &Frame) -> io::Result<()> {
    if f.body.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    let mut buf = Vec::with_capacity(4 + HEADER + f.body.len());
    buf.extend_from_slice(&((HEADER + f.body.len()) as u32).to_le_bytes());
    buf.push(f.kind as u8);
    buf.extend_from_slice(&f.session.to_le_bytes());
    buf.extend_from_slice(&f.body);
    w.write_all(&buf)?;
    w.flush()
}

/// 读一帧。对端关闭 ⇒ `Ok(None)`；未知类型 / 超长 ⇒ `InvalidData`（连接按坏处理）。
pub(crate) fn read_frame(r: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len) as usize;
    if !(HEADER..=HEADER + MAX_FRAME).contains(&len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad frame length",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let kind = Kind::from_u8(buf[0])
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown frame kind"))?;
    let session = u64::from_le_bytes(buf[1..HEADER].try_into().unwrap_or_default());
    Ok(Some(Frame {
        kind,
        session,
        body: buf.split_off(HEADER),
    }))
}

/// `Audio` 帧内容（i16 LE）⇒ f32 样本（与本机录音同口径 [-1, 1)）。奇数尾字节丢弃。
pub(crate) fn pcm_i16le_to_f32(body: &[u8]) -> Vec<f32> {
    body.chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

/// `Open` 帧参数。终端名称 / 设备代码由终端传来（§16.3），其余可缺省。
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct OpenParams {
    /// 终端设备代码（配对时绑定；统计按它归属）。
    #[serde(default)]
    pub device_id: String,
    /// 终端名称（展示用）。
    #[serde(default)]
    pub device_name: String,
    /// 客户端追加词表（阿里协议 `vocabulary`）；追加在本机词库之后。
    #[serde(default)]
    pub vocabulary: Vec<String>,
}

/// `Result` 帧内容。`text` 恒为**本会话至今的完整文本**（不是增量），终端整段替换显示即可。
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub(crate) struct ResultPayload {
    /// `true` = 终稿（会话最后一帧结果，此后不再改）。
    pub is_final: bool,
    pub text: String,
    /// 末尾尚未稳定的字数（预览浅色；终稿恒 0）。
    #[serde(default)]
    pub tentative: usize,
}

/// `End` 帧内容。`error` 为空 = 正常结束。
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub(crate) struct EndPayload {
    /// 机器可读错误码：`not_ready`（本机未加载本地实时模型）/ `capacity`（推理通道已满）/
    /// `bad_request` / `internal`。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    /// 本会话收到的音频秒数（统计用，S5）。
    #[serde(default)]
    pub audio_secs: f32,
    /// 终稿字数（统计用，S5）。
    #[serde(default)]
    pub chars: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_kinds() {
        let mut buf = Vec::new();
        let frames = [
            Frame::new(Kind::Hello, 0, b"token".to_vec()),
            Frame::new(Kind::Audio, 7, vec![1, 2, 3, 4]),
            Frame::new(Kind::Finish, 7, Vec::new()),
            Frame::json(
                Kind::Result,
                7,
                &ResultPayload {
                    is_final: true,
                    text: "你好".into(),
                    tentative: 0,
                },
            ),
        ];
        for f in &frames {
            write_frame(&mut buf, f).unwrap();
        }
        let mut r = buf.as_slice();
        for f in &frames {
            assert_eq!(read_frame(&mut r).unwrap().as_ref(), Some(f));
        }
        assert_eq!(read_frame(&mut r).unwrap(), None, "读完 ⇒ None");
    }

    #[test]
    fn rejects_bad_length_and_kind() {
        let mut r: &[u8] = &[0, 0, 0, 0];
        assert!(read_frame(&mut r).is_err(), "长度不足头部");
        let mut bad = Vec::new();
        bad.extend_from_slice(&9u32.to_le_bytes());
        bad.push(0x7f);
        bad.extend_from_slice(&0u64.to_le_bytes());
        assert!(read_frame(&mut bad.as_slice()).is_err(), "未知类型");
    }

    #[test]
    fn pcm_conversion() {
        let s = pcm_i16le_to_f32(&[0x00, 0x80, 0xff, 0x7f, 0x01]);
        assert_eq!(s.len(), 2, "奇数尾字节丢弃");
        assert_eq!(s[0], -1.0);
        assert!((s[1] - 32767.0 / 32768.0).abs() < 1e-6);
    }
}
