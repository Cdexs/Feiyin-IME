//! 对外 `wss://` 识别协议：照阿里云百炼 `run-task` 双工协议（设计 §4），第三方客户端照官方文档即可接入。
//!
//! 一条连接的流程：
//! 1. TLS 1.3 握手 → WebSocket 升级；升级请求须带 `Authorization: Bearer <key>`，key 不对直接 401。
//! 2. 客户端发 `run-task`（`parameters.feiyin` 带设备代码 / 终端名称）→ 设备绑定校验 → 回 `task-started`。
//! 3. 客户端发二进制音频（16kHz 单声道 PCM i16 LE）→ 转内核；内核结果 → `result-generated`。
//! 4. 客户端发 `finish-task` → 内核出终稿 → `task-finished`。同一连接可再发 `run-task` 开下一段。
//!
//! 防护：同一 IP 连续 5 次鉴权失败锁 10 分钟；握手 10 秒超时；60 秒无任何消息断开；
//! 单条消息 ≤ 1MB；并发连接上限见 `main.rs`。

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

use crate::asr_shared::control::{Added, Online, Status};
use crate::asr_shared::now_ts;
use crate::asr_shared::pairing::{pair_uri, AuthError, TerminalStore};
use crate::asr_shared::settings::ServiceSettings;
use crate::core_link::CoreLink;
use crate::frame::{EndPayload, Frame, Kind, OpenParams, ResultPayload, MAX_FRAME};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FAILS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(600);
/// 失败记录表上限（大量不同 IP 乱试时防内存增长）：超出只留锁定中的。
const FAILS_CAP: usize = 4096;

pub struct Shared {
    pub core: CoreLink,
    /// 监听端口与证书指纹（控制通道「状态」/「添加终端」用）。
    pub port: u16,
    pub fingerprint: String,
    /// 暂停（设计 §16.2：只在内存里，重启恢复运行）：进行中的识别做完；新的 `run-task` 回
    /// `ServicePaused`；新连接握手回 503。
    paused: AtomicBool,
    lockout: Lockout,
    /// 终端表「读-改-写」串行。网关运行期间终端表只由网关写（设置界面经控制通道）。
    store: Mutex<()>,
    /// 已通过终端鉴权的在线连接（连接号 ⇒ 信息）。
    online: Mutex<HashMap<u64, OnlineConn>>,
    next_conn: AtomicU64,
}

struct OnlineConn {
    terminal_id: String,
    net: &'static str,
    login_ts: i64,
    /// 撤销时叫醒该连接让它断开。
    kick: Arc<Notify>,
}

#[derive(Default)]
struct Fails {
    count: u32,
    until: Option<Instant>,
}

/// 按来源 IP 的鉴权失败计数与锁定。
#[derive(Default)]
struct Lockout(Mutex<HashMap<IpAddr, Fails>>);

impl Lockout {
    fn locked(&self, ip: IpAddr, now: Instant) -> bool {
        lock(&self.0)
            .get(&ip)
            .and_then(|f| f.until)
            .is_some_and(|u| now < u)
    }

    /// 记一次失败；本次触发锁定 ⇒ `true`。锁定期满后重新计数。
    fn fail(&self, ip: IpAddr, now: Instant) -> bool {
        let mut map = lock(&self.0);
        if map.len() >= FAILS_CAP {
            map.retain(|_, f| f.until.is_some_and(|u| now < u));
        }
        let f = map.entry(ip).or_default();
        if f.until.is_some_and(|u| now >= u) {
            *f = Fails::default();
        }
        f.count += 1;
        let hit = f.count == MAX_FAILS;
        if hit {
            f.until = Some(now + LOCKOUT);
        }
        hit
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    pub fn new(core: CoreLink, port: u16, fingerprint: String) -> Self {
        Self {
            core,
            port,
            fingerprint,
            paused: AtomicBool::new(false),
            lockout: Lockout::default(),
            store: Mutex::default(),
            online: Mutex::default(),
            next_conn: AtomicU64::new(1),
        }
    }

    fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
        log::info!("[asr-gateway] {}", if p { "已暂停" } else { "已继续" });
    }

    /// 在线终端（同一终端多条连接只列一次，取最早登录）。
    pub fn status(&self) -> Status {
        let mut by_id: HashMap<&str, Online> = HashMap::new();
        let map = lock(&self.online);
        for c in map.values() {
            let e = by_id.entry(&c.terminal_id).or_insert_with(|| Online {
                terminal_id: c.terminal_id.clone(),
                net: c.net.to_string(),
                login_ts: c.login_ts,
            });
            if c.login_ts < e.login_ts {
                e.login_ts = c.login_ts;
                e.net = c.net.to_string();
            }
        }
        let mut online: Vec<Online> = by_id.into_values().collect();
        online.sort_by_key(|o| o.login_ts);
        Status {
            paused: self.paused(),
            port: self.port,
            fingerprint: self.fingerprint.clone(),
            online,
        }
    }

    /// 添加终端：生成 key 并给出二维码内容（外网地址每次从设置文件现读，设置页改了立即生效）。
    pub fn add_terminal(&self) -> std::io::Result<Added> {
        let now = now_ts();
        let (terminal_id, key) = self.with_store_io(|st| {
            st.prune_expired(now);
            st.create(now)
        })?;
        let settings = ServiceSettings {
            port: self.port,
            ..ServiceSettings::load()
        };
        let lan = settings.lan_endpoint().unwrap_or_default();
        let wan = settings.wan_endpoint().unwrap_or_default();
        let uri = pair_uri(
            &lan,
            (!wan.is_empty()).then_some(wan.as_str()),
            &key,
            &self.fingerprint,
        );
        Ok(Added {
            terminal_id,
            key,
            lan,
            wan,
            uri,
        })
    }

    /// 撤销：key 作废 + 该终端所有在线连接断开。返回是否找到该终端。
    pub fn revoke(&self, terminal_id: &str) -> bool {
        let found = self.with_store(|st| {
            let exists = st.get_mut(terminal_id).is_some();
            (exists, st.revoke(terminal_id))
        });
        for c in lock(&self.online).values() {
            if c.terminal_id == terminal_id {
                c.kick.notify_one();
            }
        }
        found
    }

    fn locked_out(&self, ip: IpAddr) -> bool {
        self.lockout.locked(ip, Instant::now())
    }

    fn fail(&self, ip: IpAddr) {
        if self.lockout.fail(ip, Instant::now()) {
            log::warn!("[asr-gateway] {ip} 连续鉴权失败，锁定 10 分钟");
        }
    }

    fn succeed(&self, ip: IpAddr) {
        lock(&self.lockout.0).remove(&ip);
    }

    /// 终端表读-改-写。`f` 返回 `(结果, 是否改了)`；改了才落盘。
    fn with_store<R>(&self, f: impl FnOnce(&mut TerminalStore) -> (R, bool)) -> R {
        let _g = lock(&self.store);
        let mut st = TerminalStore::load();
        let (r, dirty) = f(&mut st);
        if dirty {
            if let Err(e) = st.save() {
                log::warn!("[asr-gateway] 保存终端表失败：{e}");
            }
        }
        r
    }

    /// 同上，但修改步骤可能失败（失败不落盘）。
    fn with_store_io<R>(
        &self,
        f: impl FnOnce(&mut TerminalStore) -> std::io::Result<R>,
    ) -> std::io::Result<R> {
        let _g = lock(&self.store);
        let mut st = TerminalStore::load();
        let r = f(&mut st)?;
        st.save()?;
        Ok(r)
    }

    /// 启动时清掉过期未绑定的 key。
    pub fn prune(&self) {
        self.with_store(|st| ((), st.prune_expired(now_ts())));
    }
}

/// 内网 / 外网：来源地址是私网 / 本机 / 链路本地 ⇒ `lan`，否则 `wan`（设计 §8.5）。
pub fn net_kind(ip: IpAddr) -> &'static str {
    let lan = match ip.to_canonical() {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let s0 = v6.segments()[0];
            v6.is_loopback() || (s0 & 0xfe00) == 0xfc00 || (s0 & 0xffc0) == 0xfe80
        }
    };
    if lan {
        "lan"
    } else {
        "wan"
    }
}

fn bearer(req: &Request) -> Option<String> {
    let v = req.headers().get("authorization")?.to_str().ok()?.trim();
    let (scheme, key) = v.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| key.trim().to_string())
}

fn reject(status: StatusCode, code: &str) -> ErrorResponse {
    let mut r = ErrorResponse::new(Some(code.to_string()));
    *r.status_mut() = status;
    r
}

pub async fn serve_conn(
    shared: Arc<Shared>,
    acceptor: TlsAcceptor,
    tcp: TcpStream,
    peer: SocketAddr,
) {
    let ip = peer.ip().to_canonical();
    if shared.locked_out(ip) {
        return; // 锁定期内直接断 TCP，连 TLS 都不做（省 CPU）。
    }
    let _ = tcp.set_nodelay(true);
    let mut key = None;
    let hs = async {
        let tls = acceptor.accept(tcp).await.ok()?;
        let cfg = WebSocketConfig::default()
            .max_message_size(Some(MAX_FRAME))
            .max_frame_size(Some(MAX_FRAME));
        let sh = shared.clone();
        let key_out = &mut key;
        tokio_tungstenite::accept_hdr_async_with_config(
            tls,
            |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
                if sh.paused() {
                    // 暂停中不接受新终端接入（§16.2）；不算鉴权失败。
                    return Err(reject(StatusCode::SERVICE_UNAVAILABLE, "ServicePaused"));
                }
                let Some(k) = bearer(req) else {
                    sh.fail(ip);
                    return Err(reject(
                        StatusCode::UNAUTHORIZED,
                        AuthError::InvalidKey.code(),
                    ));
                };
                let checked = {
                    let _g = lock(&sh.store);
                    TerminalStore::load().check_key(&k, now_ts()).map(|_| ())
                };
                match checked {
                    Ok(()) => {
                        *key_out = Some(k);
                        Ok(resp)
                    }
                    Err(e) => {
                        sh.fail(ip);
                        Err(reject(StatusCode::UNAUTHORIZED, e.code()))
                    }
                }
            },
            Some(cfg),
        )
        .await
        .ok()
    };
    let ws = match tokio::time::timeout(HANDSHAKE_TIMEOUT, hs).await {
        Ok(Some(ws)) => ws,
        _ => return,
    };
    let Some(key) = key else { return };
    let conn_id = shared.next_conn.fetch_add(1, Ordering::Relaxed);
    Conn {
        shared,
        ip,
        key,
        conn_id,
        kick: Arc::new(Notify::new()),
        terminal: None,
        task: None,
    }
    .run(ws)
    .await;
}

struct Task {
    id: String,
    session: u64,
    rx: UnboundedReceiver<Frame>,
}

struct Conn {
    shared: Arc<Shared>,
    ip: IpAddr,
    key: String,
    conn_id: u64,
    kick: Arc<Notify>,
    /// 鉴权通过后的终端内部编号（首个 `run-task` 时确定；之后同一连接沿用）。
    terminal: Option<String>,
    task: Option<Task>,
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;

/// 发给客户端的事件（百炼格式：`header.event` + `payload`）。
fn event(task_id: &str, name: &str, payload: Value) -> Message {
    Message::text(
        json!({
            "header": { "task_id": task_id, "event": name, "attributes": {} },
            "payload": payload,
        })
        .to_string(),
    )
}

fn failed(task_id: &str, code: &str, msg: &str) -> Message {
    Message::text(
        json!({
            "header": {
                "task_id": task_id, "event": "task-failed",
                "error_code": code, "error_message": msg, "attributes": {},
            },
            "payload": {},
        })
        .to_string(),
    )
}

/// 内核错误码 ⇒ 对外错误码。
fn end_code(e: &str) -> &'static str {
    match e {
        "not_ready" => "ModelNotReady",
        "capacity" => "ServiceBusy",
        "bad_request" => "InvalidParameter",
        _ => "InternalError",
    }
}

/// `vocabulary` 两种写法都收：百炼的 `{词: 权重}` 对象，或字符串数组。
fn vocabulary(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Object(m)) => m.keys().cloned().collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

async fn next_core(task: &mut Option<Task>) -> Option<Frame> {
    match task {
        Some(t) => t.rx.recv().await,
        None => std::future::pending().await,
    }
}

impl Conn {
    async fn run(mut self, mut ws: Ws) {
        let kick = self.kick.clone();
        let mut deadline = tokio::time::Instant::now() + IDLE_TIMEOUT;
        loop {
            let ok = tokio::select! {
                msg = ws.next() => match msg {
                    Some(Ok(m)) => {
                        deadline = tokio::time::Instant::now() + IDLE_TIMEOUT;
                        self.on_client(&mut ws, m).await
                    }
                    _ => false,
                },
                f = next_core(&mut self.task) => match f {
                    Some(f) => self.on_core(&mut ws, f).await,
                    None => false, // 路由没了（内核那边出问题）
                },
                // 撤销配对：立即断开（进行中的识别一并取消）。
                _ = kick.notified() => {
                    if let Some(t) = &self.task {
                        let _ = ws.send(failed(&t.id, AuthError::Revoked.code(), "terminal revoked")).await;
                    }
                    false
                }
                _ = tokio::time::sleep_until(deadline) => false,
            };
            if !ok {
                break;
            }
        }
        self.cancel_task();
        let _ = ws.close(None).await;
        lock(&self.shared.online).remove(&self.conn_id);
        if let Some(id) = &self.terminal {
            self.shared
                .core
                .send(Frame::new(Kind::Logout, self.conn_id, Vec::new()));
            self.shared.with_store(|st| match st.get_mut(id) {
                Some(t) => {
                    t.last_logout_ts = now_ts();
                    ((), true)
                }
                None => ((), false),
            });
        }
    }

    fn cancel_task(&mut self) {
        if let Some(t) = self.task.take() {
            self.shared
                .core
                .send(Frame::new(Kind::Cancel, t.session, Vec::new()));
            self.shared.core.close(t.session);
        }
    }

    async fn on_client(&mut self, ws: &mut Ws, m: Message) -> bool {
        match m {
            Message::Binary(b) => {
                if let Some(t) = &self.task {
                    self.shared
                        .core
                        .send(Frame::new(Kind::Audio, t.session, b.to_vec()));
                }
                true
            }
            Message::Text(s) => {
                let Ok(v) = serde_json::from_str::<Value>(s.as_str()) else {
                    return ws
                        .send(failed("", "InvalidParameter", "bad json"))
                        .await
                        .is_ok();
                };
                let header = &v["header"];
                let task_id = header["task_id"].as_str().unwrap_or_default().to_string();
                match header["action"].as_str() {
                    Some("run-task") => self.run_task(ws, task_id, &v["payload"]).await,
                    Some("finish-task") => {
                        match &self.task {
                            Some(t) if t.id == task_id => self.shared.core.send(Frame::new(
                                Kind::Finish,
                                t.session,
                                Vec::new(),
                            )),
                            _ => {
                                return ws
                                    .send(failed(&task_id, "InvalidParameter", "no such task"))
                                    .await
                                    .is_ok()
                            }
                        }
                        true
                    }
                    _ => ws
                        .send(failed(&task_id, "InvalidParameter", "unknown action"))
                        .await
                        .is_ok(),
                }
            }
            Message::Close(_) => false,
            _ => true, // Ping / Pong 由 tungstenite 自动处理
        }
    }

    async fn run_task(&mut self, ws: &mut Ws, task_id: String, payload: &Value) -> bool {
        let fail = |code: &str, msg: &str| failed(&task_id, code, msg);
        if task_id.is_empty() {
            return ws
                .send(fail("InvalidParameter", "missing task_id"))
                .await
                .is_ok();
        }
        if self.task.is_some() {
            return ws
                .send(fail("InvalidParameter", "a task is already running"))
                .await
                .is_ok();
        }
        if self.shared.paused() {
            return ws
                .send(fail("ServicePaused", "service paused"))
                .await
                .is_ok();
        }
        let p = &payload["parameters"];
        let format = p["format"].as_str().unwrap_or("pcm");
        let rate = p["sample_rate"].as_u64().unwrap_or(16000);
        if !format.eq_ignore_ascii_case("pcm") || rate != 16000 {
            return ws
                .send(fail(
                    "UnsupportedFormat",
                    "only pcm 16000Hz mono 16-bit is supported",
                ))
                .await
                .is_ok();
        }
        let device_id = p["feiyin"]["device_id"].as_str().unwrap_or_default();
        let device_name = p["feiyin"]["device_name"].as_str().unwrap_or_default();
        let terminal = match self.authorize(device_id, device_name) {
            Ok(id) => id,
            Err(e) => {
                let _ = ws.send(fail(e.code(), "terminal not authorized")).await;
                return false; // 鉴权失败断开
            }
        };
        let params = OpenParams {
            device_id: device_id.trim().to_string(),
            device_name: device_name.trim().to_string(),
            terminal_id: terminal,
            vocabulary: vocabulary(p.get("vocabulary")),
        };
        let (session, rx) = self.shared.core.open(&params);
        self.task = Some(Task {
            id: task_id.clone(),
            session,
            rx,
        });
        ws.send(event(&task_id, "task-started", json!({})))
            .await
            .is_ok()
    }

    /// 首个 `run-task`：key + 设备代码校验（未绑定则绑定）并记登录；之后沿用（设备代码不得中途变）。
    fn authorize(&mut self, device_id: &str, device_name: &str) -> Result<String, AuthError> {
        let now = now_ts();
        let net = net_kind(self.ip);
        let first = self.terminal.is_none();
        let shared = &self.shared;
        let (conn_id, kick) = (self.conn_id, &self.kick);
        let r = shared.with_store(|st| {
            match st.authorize(&self.key, device_id, device_name, now) {
                Ok((id, mut dirty)) => {
                    if first {
                        if let Some(t) = st.get_mut(&id) {
                            t.last_login_ts = now;
                            t.last_net = net.to_string();
                            dirty = true;
                        }
                        // 在终端表锁内登记在线：与「撤销」互斥，不会出现撤销后才登记、踢不掉的连接。
                        lock(&shared.online).insert(
                            conn_id,
                            OnlineConn {
                                terminal_id: id.clone(),
                                net,
                                login_ts: now,
                                kick: kick.clone(),
                            },
                        );
                    }
                    (Ok(id), dirty)
                }
                Err(e) => (Err(e), false),
            }
        });
        match &r {
            Ok(id) => {
                if self.terminal.as_ref().is_some_and(|t| t != id) {
                    return Err(AuthError::DeviceMismatch);
                }
                self.shared.succeed(self.ip);
                if first {
                    // 统计「登录 / 退出」记录由内核写（§16.4）；会话号字段放连接号。
                    self.shared.core.send(Frame::new(
                        Kind::Login,
                        self.conn_id,
                        id.clone().into_bytes(),
                    ));
                }
                self.terminal = Some(id.clone());
            }
            Err(_) => self.shared.fail(self.ip),
        }
        r
    }

    async fn on_core(&mut self, ws: &mut Ws, f: Frame) -> bool {
        let Some(task_id) = self.task.as_ref().map(|t| t.id.clone()) else {
            return true;
        };
        match f.kind {
            Kind::Result => {
                let Ok(r) = serde_json::from_slice::<ResultPayload>(&f.body) else {
                    return true;
                };
                let payload = json!({
                    // 一期整段一句（sentence_id 恒 1，text 为全文）；扩展字段按设计 §5.1 放 output.feiyin。
                    "output": {
                        "sentence": {
                            "sentence_id": 1,
                            "begin_time": 0,
                            "end_time": null,
                            "text": r.text,
                            "sentence_end": r.is_final,
                            "words": [],
                        },
                        "feiyin": { "tentative": r.tentative },
                    },
                    "usage": null,
                });
                ws.send(event(&task_id, "result-generated", payload))
                    .await
                    .is_ok()
            }
            Kind::End => {
                self.shared.core.close(f.session);
                self.task = None;
                let end: EndPayload = serde_json::from_slice(&f.body).unwrap_or_default();
                let msg = if end.error.is_empty() {
                    event(
                        &task_id,
                        "task-finished",
                        json!({ "output": {}, "usage": { "duration": end.audio_secs.ceil() as u64 } }),
                    )
                } else {
                    failed(&task_id, end_code(&end.error), &end.error)
                };
                ws.send(msg).await.is_ok()
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_wan_classification() {
        for (ip, want) in [
            ("192.168.1.5", "lan"),
            ("10.0.0.1", "lan"),
            ("172.16.3.4", "lan"),
            ("127.0.0.1", "lan"),
            ("169.254.1.1", "lan"),
            ("8.8.8.8", "wan"),
            ("::1", "lan"),
            ("fd00::1", "lan"),
            ("fe80::1", "lan"),
            ("::ffff:192.168.1.5", "lan"),
            ("2001:db8::1", "wan"),
        ] {
            assert_eq!(net_kind(ip.parse().unwrap()), want, "{ip}");
        }
    }

    #[test]
    fn vocabulary_shapes() {
        assert_eq!(vocabulary(Some(&json!({"飞音": 5}))), vec!["飞音"]);
        assert_eq!(vocabulary(Some(&json!(["a", 1, "b"]))), vec!["a", "b"]);
        assert!(vocabulary(None).is_empty());
    }

    #[test]
    fn lockout_after_five_failures() {
        let lo = Lockout::default();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        let other: IpAddr = "5.6.7.8".parse().unwrap();
        let t0 = Instant::now();
        for i in 1..MAX_FAILS {
            assert!(!lo.fail(ip, t0), "第 {i} 次不锁");
            assert!(!lo.locked(ip, t0));
        }
        assert!(lo.fail(ip, t0), "第 5 次锁定");
        assert!(lo.locked(ip, t0 + Duration::from_secs(599)));
        assert!(!lo.locked(other, t0), "只锁该 IP");
        let after = t0 + LOCKOUT;
        assert!(!lo.locked(ip, after), "10 分钟后解锁");
        assert!(!lo.fail(ip, after), "解锁后重新计数");
    }
}
