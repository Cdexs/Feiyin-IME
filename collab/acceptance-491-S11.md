# 验收清单 · ASR-SERVER-491 S11（协议文档 + Python 参考客户端）

> 依据：todo S11「协议文档 + Python 参考客户端」；Gavin 10-17「安卓和 iOS 端……我会另起项目开发」⇒ 协议文档须是**独立契约**（不依赖本仓库源码即可对接）。
> 判定：✅ / ⚠️ / ❌。2026-10-17。

## A. 协议文档 `docs/ASR-PROTOCOL.md`（主控写，逐条对网关代码核实）

| # | 章节 | 核实来源 | 判定 |
| --- | --- | --- | --- |
| 1 | §1.1 连接码字段 / v=1 / 未知字段忽略 / 缺端口补 7860 / IPv6 方括号 | `asr_shared/pairing.rs` `pair_uri`；`asr_shared/remote.rs` `from_pair_uri` / `validated` | ✅ |
| 2 | §1.1 key = base32 26 字符；fp = DER SHA-256 小写 hex | `pairing.rs` `base32(random_bytes::<16>)`、`sha256_hex_bytes`（`{b:02x}`） | ✅ |
| 3 | §1.2 key 规整（去空白和 `-`、转大写），服务端同规则比对 | `pairing.rs` `normalize_key`、`check_key` 先规整再哈希 | ✅ |
| 4 | §1.3 10 分钟绑定窗口 / 首次绑定 device_id / 撤销 | `pairing.rs` `authorize`、`BIND_WINDOW_SECS`；网关 `kick` | ✅ |
| 5 | §2.2 TLS 1.3、钉扎指纹、**先验指纹再发 key** | `asr_shared/tls_pin.rs`（`TLS13`、自定义校验器） | ✅ |
| 6 | §2.3 401 正文错误码 / 503 `ServicePaused` / 5 次锁 10 分钟 / 握手 10 秒 | `ws.rs` `serve_conn`、`reject`、`LOCKOUT`、`HANDSHAKE_TIMEOUT` | ✅ |
| 7 | §3 一连接多段、同时一段、60 秒空闲（Ping 算活动）、1MB、64 连接 | `ws.rs` `run` 的 `deadline` 在任何消息时重置；`MAX_FRAME = 1<<20`；`main.rs` `MAX_CONNS` | ✅ |
| 8 | §4.1 run-task 字段与缺省、鉴权类错误后断开 | `ws.rs` `run_task`（format/rate 校验、三项 S10 参数、`vocabulary` 两种写法、`authorize` 失败 `return false`） | ✅ |
| 9 | §4.2 无进行中识别时音频丢弃 | `on_client` `Binary` 分支 `if let Some(t)` | ✅ |
| 10 | §4.3 finish-task 对不上 ⇒ `InvalidParameter` 连接保持；§4.4 关连接即取消 | `on_client`；`run` 结束 `cancel_task` | ✅ |
| 11 | §5.2 sentence / words / tentative / ts_source 语义 | `ws.rs` `result_json`；`session.rs` 预览 / 终稿；`char_times::group_words` 拼接不变量 | ✅ |
| 12 | §5.3 译文形状与失败不给 | `result_json` S10 段；`text_stage::translate` | ✅ |
| 13 | §5.4 错误码全表 | `end_code`、`AuthError::code`、`run_task` 各分支 | ✅ |

子 Agent 写客户端时提出的文档问题（8 条）处理：指纹宽容比对、key 先解码再规整、同名字段取后者、手输地址规则同连接码、被拒正文读法、识别中不必另发 Ping —— 已补进文档；`end_time` 可为 `null` 文档原已写；任务书里 RFC 6455 示例向量是主控笔误（客户端按 RFC 真实值 `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=` 实现，正确）。

## B. 参考客户端 `tools/asr-client/feiyin_asr_client.py`（子 Agent）

| 要求 | 结论 |
| --- | --- |
| 只用标准库 | ✅ 自实现 RFC 6455 最小客户端（掩码、126/127、分片、Ping/Pong、Close） |
| 指纹先验再升级 | ✅ `connect_pinned`：`TLSv1_3` 上下限、`CERT_NONE` + 握手后 `getpeercert(binary_form=True)` 比对，通过才发升级（:576 注释点明）；集成测试断言指纹不符时假网关 TLS 后收到 0 字节 |
| `--probe` 不发 key；手输无指纹且非 probe ⇒ 拒绝 | ✅（贴合 §1.2「未经确认的指纹不得带 key 连接」） |
| auto 选址 | ✅ 并发试 TCP、内网优先、外网先通等 300ms；6 场景单测 |
| 退出码区分 | ✅ 0 / 2 参数 / 3 task-failed / 4 握手被拒 / 5 指纹不符 / 6 网络 / 7 音频格式 / 8 协议异常 |
| 每个对外行为标章节号 | ✅ |

## C. 测试

| 项 | 结论 |
| --- | --- |
| `py -3 -m unittest discover -s tools/asr-client` | **45 过 / 0 失败 / 0 跳过**（主控复跑确认）；含本地回环 TLS 1.3 假网关集成测试 7 个（完整流程、`--json`、指纹不符、错 key 401、task-failed、probe、TLS 1.2 服务端不降级） |
| 真实服务端端到端 | ⬜ 需本机开「识别服务」并添加终端拿到 key 后跑：`py -3 tools/asr-client/feiyin_asr_client.py --code "<连接码>" --wav <16k 单声道 wav>`。并入 T 阶段 tester-1 / Gavin 两台电脑实测 |

## D. 跨平台

文档与 Python 客户端平台无关；无 Rust 改动。MACOS-HANDOFF 记一条（macOS 侧可用同一脚本冒烟）。
