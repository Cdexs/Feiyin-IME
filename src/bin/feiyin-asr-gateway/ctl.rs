//! 本机控制通道服务端（S4b，协议见 `asr_shared/control.rs`）。
//!
//! 独立线程阻塞接入：命令都是瞬时操作（改内存标志 / 读写一次终端表），一问一答后断开。

use std::sync::Arc;

use crate::asr_shared::control::{read_json, write_json, GatewayInfo, Request, Response};
use crate::transport::CtlListener;
use crate::ws::Shared;

/// 绑定控制端点、写 `asr-gateway.json`、起服务线程。
pub fn start(shared: Arc<Shared>) -> std::io::Result<()> {
    let ctl = CtlListener::bind("AsrGateway")?;
    GatewayInfo {
        pid: std::process::id(),
        control: ctl.endpoint().to_string(),
    }
    .save()?;
    std::thread::Builder::new()
        .name("asr-gateway-ctl".into())
        .spawn(move || loop {
            match ctl.accept() {
                Ok(mut s) => {
                    let resp = match read_json::<Request>(&mut s) {
                        Ok(req) => handle(&shared, req),
                        Err(e) => Response::err(format!("bad request: {e}")),
                    };
                    let _ = write_json(&mut s, &resp);
                }
                Err(e) => {
                    log::warn!("[asr-gateway] 控制通道接入失败：{e}");
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        })?;
    Ok(())
}

fn handle(shared: &Shared, req: Request) -> Response {
    match req {
        Request::Status => Response {
            status: Some(shared.status()),
            ..Default::default()
        },
        Request::Pause | Request::Resume => {
            shared.set_paused(req == Request::Pause);
            Response {
                status: Some(shared.status()),
                ..Default::default()
            }
        }
        Request::AddTerminal => match shared.add_terminal() {
            Ok(a) => Response {
                added: Some(a),
                ..Default::default()
            },
            Err(e) => Response::err(format!("add terminal failed: {e}")),
        },
        Request::Revoke { terminal_id } => {
            if shared.revoke(&terminal_id) {
                Response::default()
            } else {
                Response::err("no such terminal")
            }
        }
    }
}
