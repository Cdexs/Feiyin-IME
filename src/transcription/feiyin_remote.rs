//! ASR-SERVER-491 S8：本机连另一台电脑的飞音识别服务（设计 §8.3 / §8.5 / §13.3）。
//!
//! 只管「建连」和 `run-task` 扩展字段；会话收发与阿里在线识别共用
//! [`super::qwen_inference::transcribe_streaming_feiyin`]（同一个 `run-task` 协议）。
//!
//! - **内外网**：「自动」= 两个地址同时试 TCP（只试 TCP，不带 key，不惊动对方鉴权）；
//!   内网通了直接用；外网先通则再等内网 [`LAN_GRACE`]，内网没来就用外网。
//!   在家时内网几毫秒就通，出门时内网不通也不用干等超时。
//! - **证书**：只认配对时记下的指纹（`asr_shared::tls_pin`）；对不上 ⇒ `FingerprintMismatch`，提示重新配对。
//! - **错误**：对方的错误码原样带进错误串（`服务端错误：<码>`），由 `convert_to_friendly_error` 转人话。

use std::net::TcpStream;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tungstenite::http::Uri;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{
    client_tls_with_config, ClientRequestBuilder, Connector, Error as WsError, HandshakeError,
    WebSocket,
};

use crate::asr_shared::remote::RemotePairing;
use crate::asr_shared::tls_pin::{self, PinnedCert};

/// `run-task` 的 `model`（对方服务端不看，只进日志 / 统计摘要）。
pub const MODEL_NAME: &str = "feiyin-local-realtime";

/// 自动模式下外网先连上后，再给内网的等待时间（内网优先，§8.3）。
const LAN_GRACE: Duration = Duration::from_millis(300);

/// 本机没配对 / 配对不全（界面提示去设置里配对）。
pub const ERR_NOT_PAIRED: &str = "NotPaired";
/// 对方证书与配对时记下的指纹不一致（对方重装 / 被冒充 ⇒ 重新配对）。
pub const ERR_FINGERPRINT: &str = "FingerprintMismatch";

/// FLASH-P3-B：连 flashasr 时本次识别的处理开关（每段录音按当时设置现算，见 FlashASR `collab/p3b-design.md` §1）。
/// 文字处理全在服务端做，输入法只传开关、不再自己处理。
#[derive(Debug, Clone, Default)]
pub struct FlashOptions {
    /// 补标点（关 ⇒ 服务端全去标点，同本机「自动标点」关）。
    pub punctuation: bool,
    /// 输出优化（服务端统一大模型）。
    pub optimize: bool,
    /// 繁简：`"simplified"` / `"traditional"`。
    pub chinese_script: &'static str,
    /// 去句末标点。
    pub strip_trailing: bool,
    /// 场景（`{kind, style_hint, multiline_safe}`）；场景感知关 ⇒ `None` 不传。窗口标题 / 程序名不传。
    pub scene: Option<serde_json::Value>,
    /// 设置里开了翻译功能（翻译热键可用）。
    pub translation_enabled: bool,
    /// 本次是否按了翻译热键 —— 可在录音中途才按，**说完时**读（随 `finish-task` 发，见 [`Self::translate_now`]）。
    pub translate_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl FlashOptions {
    /// 此刻是否要翻译（开始时 = `run-task` 的初值；说完时 = `finish-task` 带的终值）。
    pub fn translate_now(&self) -> bool {
        self.translation_enabled
            && self
                .translate_flag
                .as_ref()
                .is_some_and(|f| f.load(std::sync::atomic::Ordering::Acquire))
    }
}

/// `run-task` 的 `parameters.feiyin`：本机身份 + flashasr 扩展开关。
pub fn run_task_ext(p: &RemotePairing, o: &FlashOptions) -> serde_json::Value {
    let mut v = serde_json::json!({
        "device_id": p.device_id,
        "device_name": p.effective_device_name(),
        "output_optimization_enabled": o.optimize,
        "strip_trailing_punct": o.strip_trailing,
        "chinese_script": o.chinese_script,
    });
    if let Some(scene) = &o.scene {
        v["scene"] = scene.clone();
    }
    v
}

/// 文字处理开关里与阿里同名的三项（放 `parameters` 顶层，不在 `feiyin` 里）。
/// 输入法没有数字规整开关 ⇒ 恒请服务端做；翻译是开始时的初值，说完时以 `finish-task` 为准。
pub fn text_params(o: &FlashOptions) -> serde_json::Value {
    serde_json::json!({
        "inverse_text_normalization_enabled": true,
        "punctuation_prediction_enabled": o.punctuation,
        "translation_enabled": o.translate_now(),
    })
}

/// 场景 ⇒ 协议取值（`kind` 用 flashasr 认的名字；输入法内部名字是给大模型看的，不能直接发）。
pub fn scene_param(s: &crate::scene::SceneContext) -> serde_json::Value {
    use crate::scene::SceneKind;
    let kind = match s.scene {
        SceneKind::Chat => "chat",
        SceneKind::Agent => "agent",
        SceneKind::Email => "email",
        SceneKind::Doc => "doc",
        SceneKind::IdeTerminal => "ide_terminal",
        SceneKind::Browser => "browser",
        SceneKind::Unknown => "unknown",
    };
    serde_json::json!({
        "kind": kind,
        "style_hint": s.style_hint,
        "multiline_safe": s.multiline_safe,
    })
}

/// 建连：挑地址 → TCP → TLS（指纹钉扎）→ WebSocket（Bearer key）。
pub fn connect(
    p: &RemotePairing,
    timeout: Duration,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>> {
    if !p.is_paired() || p.device_id.is_empty() {
        bail!("服务端错误：{ERR_NOT_PAIRED}");
    }
    let t0 = Instant::now();
    let (tcp, endpoint, net) = race_tcp(&p.candidates(), timeout)?;
    tcp.set_read_timeout(Some(timeout))
        .context("网络失败：设置读取超时失败")?;
    tcp.set_write_timeout(Some(timeout))
        .context("网络失败：设置写入超时失败")?;
    log::info!(
        "[Latency] ASR feiyin TCP connected via {net} ({endpoint}) in {:.0}ms",
        t0.elapsed().as_millis()
    );

    let t_tls = Instant::now();
    let verifier = PinnedCert::new(Some(&p.fingerprint));
    let config = tls_pin::client_config(Arc::clone(&verifier))
        .map_err(|e| anyhow!("网络失败：TLS 配置失败 - {e}"))?;
    let uri: Uri = format!("wss://{endpoint}/")
        .parse()
        .context("网络失败：地址无效")?;
    let request =
        ClientRequestBuilder::new(uri).with_header("Authorization", format!("Bearer {}", p.key));
    let (ws, _) = client_tls_with_config(request, tcp, None, Some(Connector::Rustls(config)))
        .map_err(|e| match e {
            HandshakeError::Failure(e) => map_handshake_error(&e, &verifier),
            HandshakeError::Interrupted(_) => anyhow!("网络失败：TLS 握手意外中断"),
        })?;
    log::info!(
        "[Latency] ASR feiyin TLS+WS handshake in {:.0}ms",
        t_tls.elapsed().as_millis()
    );
    Ok(ws)
}

/// 候选地址同时试 TCP；排在前面的优先（自动模式 = 内网在前）。返回 `(连接, 地址, "lan"|"wan")`。
fn race_tcp(
    candidates: &[(&str, &'static str)],
    timeout: Duration,
) -> Result<(TcpStream, String, &'static str)> {
    if candidates.is_empty() {
        bail!("服务端错误：{ERR_NOT_PAIRED}");
    }
    let (tx, rx) = mpsc::channel();
    for (rank, (endpoint, net)) in candidates.iter().enumerate() {
        let (tx, endpoint, net) = (tx.clone(), endpoint.to_string(), *net);
        // 输了的那路连上后随线程结束自然关闭（只建了 TCP，对方看到的是一次空连接）。
        std::thread::spawn(move || {
            let r = tls_pin::tcp_connect(&endpoint, timeout);
            let _ = tx.send((rank, endpoint, net, r));
        });
    }
    drop(tx);
    let mut best: Option<(TcpStream, String, &'static str)> = None;
    let mut deadline: Option<Instant> = None;
    let mut last_err = String::new();
    loop {
        let msg = match deadline {
            None => rx.recv().ok(),
            Some(d) => rx
                .recv_timeout(d.saturating_duration_since(Instant::now()))
                .ok(),
        };
        let Some((rank, endpoint, net, r)) = msg else {
            break;
        };
        match r {
            Ok(s) if rank == 0 => return Ok((s, endpoint, net)),
            Ok(s) => {
                if best.is_none() {
                    best = Some((s, endpoint, net));
                    deadline = Some(Instant::now() + LAN_GRACE);
                }
            }
            Err(e) => {
                log::info!("[ASR-SERVER-491] {net} {endpoint} connect failed: {e}");
                last_err = e.to_string();
            }
        }
    }
    best.ok_or_else(|| anyhow!("网络失败：连接失败 - {last_err}"))
}

fn map_handshake_error(e: &WsError, verifier: &PinnedCert) -> anyhow::Error {
    if verifier.mismatched() {
        return anyhow!("服务端错误：{ERR_FINGERPRINT}");
    }
    if let WsError::Http(resp) = e {
        // 网关握手拒绝时正文就是错误码（InvalidKey / KeyRevoked / KeyExpired / ServicePaused）。
        let code = resp
            .body()
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        return anyhow!(
            "服务端错误：{} (HTTP {})",
            code.trim(),
            resp.status().as_u16()
        );
    }
    anyhow!("网络失败：连接失败 - {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn run_task_ext_carries_identity_and_text_params_at_top_level() {
        let p = RemotePairing {
            device_id: "dev1".into(),
            device_name: "书房".into(),
            ..Default::default()
        };
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let o = FlashOptions {
            punctuation: false,
            optimize: true,
            chinese_script: "traditional",
            strip_trailing: true,
            scene: None,
            translation_enabled: true,
            translate_flag: Some(Arc::clone(&flag)),
        };
        let v = run_task_ext(&p, &o);
        assert_eq!(v["device_id"], "dev1");
        assert_eq!(v["device_name"], "书房");
        assert_eq!(v["output_optimization_enabled"], true);
        assert_eq!(v["strip_trailing_punct"], true);
        assert_eq!(v["chinese_script"], "traditional");
        assert!(v.get("scene").is_none(), "场景感知关 ⇒ 不传");
        assert!(v.get("postprocess").is_none(), "S10 起不再发 postprocess");
        // 三项在 parameters 顶层（经 Feiyin 分支合并），不在 feiyin 里。
        for k in [
            "inverse_text_normalization_enabled",
            "punctuation_prediction_enabled",
            "translation_enabled",
        ] {
            assert!(v.get(k).is_none(), "{k} 不应在 feiyin 里");
        }
        let t = text_params(&o);
        assert_eq!(t["inverse_text_normalization_enabled"], true);
        assert_eq!(t["punctuation_prediction_enabled"], false);
        assert_eq!(t["translation_enabled"], false, "开始时还没按翻译键");
        // 录音中途按了翻译键 ⇒ 说完时为真。
        flag.store(true, std::sync::atomic::Ordering::Release);
        assert!(o.translate_now());
        // 设置里没开翻译 ⇒ 按了也不翻。
        let off = FlashOptions {
            translation_enabled: false,
            ..o.clone()
        };
        assert!(!off.translate_now());
    }

    #[test]
    fn scene_param_uses_protocol_kind_names() {
        let mut s = crate::scene::SceneContext::unknown();
        s.scene = crate::scene::SceneKind::IdeTerminal;
        s.multiline_safe = true;
        s.style_hint = "简洁".into();
        let v = scene_param(&s);
        assert_eq!(v["kind"], "ide_terminal");
        assert_eq!(v["multiline_safe"], true);
        assert_eq!(v["style_hint"], "简洁");
        assert!(
            v.get("window_title").is_none() && v.get("app_exe").is_none(),
            "不外发窗口信息"
        );
    }

    #[test]
    fn unpaired_fails_before_network() {
        let err = connect(&RemotePairing::default(), Duration::from_millis(50))
            .unwrap_err()
            .to_string();
        assert!(err.contains(ERR_NOT_PAIRED), "{err}");
    }

    /// 内网优先：两路都通 ⇒ 用排在前面的；前面不通 ⇒ 用后面的。
    #[test]
    fn race_prefers_first_candidate_and_falls_back() {
        let a = TcpListener::bind("127.0.0.1:0").unwrap();
        let b = TcpListener::bind("127.0.0.1:0").unwrap();
        let (ea, eb) = (
            a.local_addr().unwrap().to_string(),
            b.local_addr().unwrap().to_string(),
        );
        let t = Duration::from_secs(2);
        let (_, ep, net) = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap();
        assert_eq!((ep.as_str(), net), (ea.as_str(), "lan"));
        // 关掉内网那个端口 ⇒ 落到外网。
        drop(a);
        let (_, ep, net) = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap();
        assert_eq!((ep.as_str(), net), (eb.as_str(), "wan"));
        drop(b);
        let err = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap_err();
        assert!(err.to_string().starts_with("网络失败"), "{err}");
    }
}
