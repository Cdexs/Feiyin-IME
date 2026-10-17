//! ASR-SERVER-491：识别服务的内核侧（S3）—— 本机 IPC 服务 + 远程会话。
//!
//! 结构（设计 §3）：网关进程（S4）对外讲 `run-task` 等协议，经本机 IPC 把各终端的音频送进来；
//! 本模块在内核里为每路终端开一路远程会话（[`session::run_remote_session`]），结果经同一条连接回网关。
//!
//! 资源承诺（设计 §7）：
//! - 服务没启动 ⇒ 不建线程、不建管道；
//! - 不为远程加载模型：用本机本地实时档已加载的那一份（[`transcription::remote_engines`]），
//!   本机不在本地实时档 ⇒ 回 `not_ready`；
//! - 每路终端一路推理通道（约 73MB），路数按空闲显存封顶（[`transcription::AccEngine::max_remote_slots`]），
//!   超了回 `capacity`。

mod compose;
mod frame;
mod session;
mod stats;
pub(crate) mod supervisor;
mod transport;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use frame::{EndPayload, Frame, Kind, OpenParams};

use crate::config::AppConfig;
use crate::transcription;

/// 远程会话结束错误码。
const ERR_NOT_READY: &str = "not_ready";
const ERR_CAPACITY: &str = "capacity";
const ERR_BAD_REQUEST: &str = "bad_request";

/// 每路会话的音频缓冲（100ms 一帧 ⇒ 约 25s）。满了说明会话卡住，丢帧并记日志，不拖累其他会话。
const AUDIO_QUEUE: usize = 256;

/// 运行中的识别服务（内核侧）。`stop` 或 drop 时结束：断开网关、取消所有远程会话。
pub(crate) struct AsrService {
    stop: Arc<AtomicBool>,
    listener: Arc<transport::Listener>,
    handle: Option<JoinHandle<()>>,
}

impl AsrService {
    /// 启动 IPC 服务。`token` = 拉起网关时下发给它的口令（网关首帧须带上）。
    pub(crate) fn start(
        runtime_config: Arc<RwLock<AppConfig>>,
        token: String,
    ) -> std::io::Result<Self> {
        let listener = Arc::new(transport::Listener::bind(token)?);
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let (listener, stop) = (Arc::clone(&listener), Arc::clone(&stop));
            std::thread::Builder::new()
                .name("asr-ipc".into())
                .spawn(move || serve_forever(&listener, &stop, &runtime_config))?
        };
        log::info!(
            "[ASR-SERVER-491] IPC service listening: {}",
            listener.endpoint()
        );
        Ok(Self {
            stop,
            listener,
            handle: Some(handle),
        })
    }

    /// 网关连接用的端点（Windows 管道基名 / macOS socket 路径），拉起网关时作参数传过去。
    pub(crate) fn endpoint(&self) -> &str {
        self.listener.endpoint()
    }

    pub(crate) fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.listener.wake();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AsrService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve_forever(
    listener: &transport::Listener,
    stop: &AtomicBool,
    runtime_config: &Arc<RwLock<AppConfig>>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok(conn) if !stop.load(Ordering::Acquire) => {
                log::info!("[ASR-SERVER-491] gateway connected");
                serve_conn(conn, runtime_config);
                log::info!("[ASR-SERVER-491] gateway disconnected");
            }
            Ok(_) => {}
            Err(e) => {
                if !stop.load(Ordering::Acquire) {
                    log::warn!("[ASR-SERVER-491] gateway accept failed: {e}");
                    // 失败多为握手不对 / 对端立刻断开；稍等再接，避免异常时空转。
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
    }
}

type Writer = Arc<Mutex<Box<dyn Write + Send>>>;

fn send(writer: &Writer, f: &Frame) {
    let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = frame::write_frame(&mut *w, f) {
        log::debug!("[ASR-SERVER-491] write frame failed: {e}");
    }
}

fn end_with(writer: &Writer, session: u64, error: &str) {
    send(
        writer,
        &Frame::json(
            Kind::End,
            session,
            &EndPayload {
                error: error.into(),
                ..Default::default()
            },
        ),
    );
}

struct Live {
    audio_tx: Option<crossbeam_channel::Sender<Vec<f32>>>,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// 一条网关连接：读线程（本线程）按会话号分发帧；每路会话一个线程。连接断开 ⇒ 取消全部会话。
fn serve_conn(conn: transport::Conn, runtime_config: &Arc<RwLock<AppConfig>>) {
    let transport::Conn { mut reader, writer } = conn;
    let writer: Writer = Arc::new(Mutex::new(writer));
    // 先建后删：会话线程在下面 join 完才轮到它 drop（drop 时补记仍在线终端的退出时间）。
    let stats = stats::Stats::start();
    let mut live: HashMap<u64, Live> = HashMap::new();
    loop {
        let f = match frame::read_frame(&mut reader) {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                log::warn!("[ASR-SERVER-491] read frame failed: {e}");
                break;
            }
        };
        live.retain(|_, s| !s.handle.is_finished());
        match f.kind {
            Kind::Open => {
                if live.contains_key(&f.session) {
                    end_with(&writer, f.session, ERR_BAD_REQUEST);
                    continue;
                }
                let params: OpenParams = match serde_json::from_slice(&f.body) {
                    Ok(p) => p,
                    Err(e) => {
                        log::warn!("[ASR-SERVER-491] bad open params: {e}");
                        end_with(&writer, f.session, ERR_BAD_REQUEST);
                        continue;
                    }
                };
                match open_session(
                    f.session,
                    params,
                    live.len(),
                    &writer,
                    runtime_config,
                    stats.sender(),
                ) {
                    Ok(s) => {
                        live.insert(f.session, s);
                    }
                    Err(code) => end_with(&writer, f.session, code),
                }
            }
            Kind::Audio => {
                if let Some(tx) = live.get(&f.session).and_then(|s| s.audio_tx.as_ref()) {
                    if tx.try_send(frame::pcm_i16le_to_f32(&f.body)).is_err() {
                        log::warn!(
                            "[ASR-SERVER-491] session {} audio queue full, frame dropped",
                            f.session
                        );
                    }
                }
            }
            // 说完了：关音频通道 ⇒ 预览线程收尾 ⇒ 会话出终稿后自己发 End。
            Kind::Finish => {
                if let Some(s) = live.get_mut(&f.session) {
                    s.audio_tx = None;
                }
            }
            Kind::Cancel => {
                if let Some(s) = live.get_mut(&f.session) {
                    s.cancel.store(true, Ordering::Release);
                    s.audio_tx = None;
                }
            }
            Kind::Login => stats.send(stats::StatsOp::Login {
                conn: f.session,
                terminal: String::from_utf8_lossy(&f.body).into_owned(),
            }),
            Kind::Logout => stats.send(stats::StatsOp::Logout { conn: f.session }),
            Kind::Hello | Kind::Result | Kind::End => {
                log::warn!("[ASR-SERVER-491] unexpected frame {:?}", f.kind);
            }
        }
    }
    for s in live.values_mut() {
        s.cancel.store(true, Ordering::Release);
        s.audio_tx = None;
    }
    for (_, s) in live {
        let _ = s.handle.join();
    }
}

/// 开一路远程会话：查模型就绪 → 查容量 → 开推理通道 → 起会话线程。失败返回错误码。
fn open_session(
    id: u64,
    params: OpenParams,
    active: usize,
    writer: &Writer,
    runtime_config: &Arc<RwLock<AppConfig>>,
    stats_tx: Option<crossbeam_channel::Sender<stats::StatsOp>>,
) -> Result<Live, &'static str> {
    let engines = transcription::remote_engines().ok_or(ERR_NOT_READY)?;
    // 空闲显存是「此刻」的：已开的通道已经占掉了，故直接看还能不能再开一路。
    if active >= transcription::AccEngine::SLOT_MAX || engines.acc.max_remote_slots() == 0 {
        log::warn!("[ASR-SERVER-491] remote capacity reached (active={active})");
        return Err(ERR_CAPACITY);
    }
    let slot = engines.acc.create_slot().map_err(|e| {
        log::warn!("[ASR-SERVER-491] create slot failed: {e:#}");
        ERR_CAPACITY
    })?;
    let config = crate::clone_runtime_config(runtime_config);
    let (audio_tx, audio_rx) = crossbeam_channel::bounded::<Vec<f32>>(AUDIO_QUEUE);
    let cancel = Arc::new(AtomicBool::new(false));
    let handle = {
        let (writer, cancel) = (Arc::clone(writer), Arc::clone(&cancel));
        std::thread::Builder::new()
            .name(format!("asr-remote-{id}"))
            .spawn(move || {
                log::info!(
                    "[ASR-SERVER-491] remote session {id} open: device={} name={}",
                    params.device_id,
                    params.device_name
                );
                // 每路会话自己一份 VAD（几十 ms；不碰本机的进程级缓存）。
                let vad = transcription::VadSegmenter::try_new_for_local_silence(
                    &transcription::model_dir(),
                );
                let out = session::run_remote_session(
                    &engines,
                    &slot,
                    vad.as_ref(),
                    &config,
                    &params,
                    audio_rx,
                    cancel,
                    |r| send(&writer, &Frame::json(Kind::Result, id, &r)),
                );
                // 推理通道（约 73MB）先还，再回网关、记统计。
                drop(slot);
                drop(engines);
                let end = &out.end;
                log::info!(
                    "[ASR-SERVER-491] remote session {id} end: error={:?} audio={:.1}s chars={}",
                    end.error,
                    end.audio_secs,
                    end.chars
                );
                send(&writer, &Frame::json(Kind::End, id, end));
                if let Some(tx) = stats_tx {
                    if !out.text.trim().is_empty() && !params.terminal_id.is_empty() {
                        let _ = tx.send(stats::StatsOp::Final {
                            terminal: params.terminal_id.clone(),
                            text: out.text,
                            pcm: out.pcm,
                        });
                    }
                }
            })
            .map_err(|_| session::ERR_INTERNAL)?
    };
    Ok(Live {
        audio_tx: Some(audio_tx),
        cancel,
        handle,
    })
}
