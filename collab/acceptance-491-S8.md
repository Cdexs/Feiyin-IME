# 验收清单 · ASR-SERVER-491 S8（电脑客户端连自家识别服务）

> 依据：`docs/ASR-SERVER-DESIGN.md` §5.1 / §8.2 / §8.3 / §8.4 / §8.5 / §8.8 / §13.3 第 4 条 / §16.3；Gavin 10-17 配对原话（§8 引文）。
> 判定：✅ 符合 / ⚠️ 偏离（附理由）/ ❌ 缺失。行号以 2026-10-17 验收时为准。

## A. 设计逐条对照

| # | 设计要求（原文要点） | 代码位置 | 判定 |
| --- | --- | --- | --- |
| 1 | §8.2「没摄像头的电脑：**连接码**（二维码的文字版），复制粘贴一次即可」，内容 `feiyin-asr://pair?v=1&lan=..&wan=..&key=..&fp=..` | `src/asr_shared/remote.rs:163` `from_pair_uri`（版本须为 1、未知字段忽略、必须带指纹）；与服务端 `pairing::pair_uri` 往返测试 `pair_uri_round_trip_with_server_format` | ✅ |
| 2 | §8.2「想手输的：分开显示 内网地址:端口、外网地址:端口、key」 | `remote.rs:193` `manual`；界面 `FeiyinServerPairing.tsx` 手动输入三栏 | ✅ |
| 3 | §8.3「终端可手动选内网或外网（Gavin 要求，保留）」 | `remote.rs:234` `candidates`（Lan / Wan 只给一个地址）；界面「只用内网 / 只用外网」 | ✅ |
| 4 | §8.3「默认用自动：先试内网，连不上再试外网…离开 Wi-Fi 自动切外网，回来再切回内网」 | `NetMode` 默认 `Auto`（`remote.rs` `#[default]`）；`feiyin_remote.rs:91` `race_tcp`：两路同时试 TCP，内网通了即用，外网先通再等内网 300ms（`:31` `LAN_GRACE`）。每次录音重新挑（不跨录音复用连接，沿用 040-C） | ⚠️ 实现比原文「先试内网、连不上再试外网」更快：原文串行在出门时要干等内网超时（5 秒），改并发试，内网优先的语义不变 |
| 5 | §8.4「首次使用即绑定那台终端」——终端须稳定提供设备代码 | `remote.rs:222` `adopt_identity`：设备代码首次随机生成后固定，重新配对沿用 | ✅ |
| 6 | §8.5「终端只认这一张证书（防冒充）」 | `src/asr_shared/tls_pin.rs:48` `verify_server_cert` 比对指纹；签名仍按 TLS 规范校验（`:77` / `:83`）；只走 TLS 1.3（`:99`，网关同） | ✅ |
| 7 | §8.5「手输方式：第一次连接时终端显示指纹让用户确认一次（同 SSH 首次连接）」 | `tls_pin.rs:151` `probe_fingerprint`；`src-tauri/src/asr_server.rs:291` `asr_remote_probe` → 界面核对 → `:315` `asr_remote_pair_manual` 保存 | ⚠️ 确认时机从「第一次连接时」提前到「配对保存时」：语音识别热键路径上不能弹确认框；效果相同（未确认的指纹不落盘，`is_paired` 为假不会连） |
| 8 | §8.5「内网也走同一条加密通道」 | `feiyin_remote.rs:51` `connect` 内外网同一路径（`wss` + 指纹） | ✅ |
| 9 | §5.1「鉴权放在 `Authorization: Bearer <访问码>`」 | `feiyin_remote.rs` `connect` 请求头 | ✅ |
| 10 | §5.1 / §16.3「`run-task` 的 `parameters.feiyin.device_id / device_name` 必填」；§16.3「终端名称对方传来（可在对方改名）」 | `feiyin_remote.rs:42` `run_task_ext`；`qwen_inference.rs:1177` 注入；名称可在语音页改，留空取计算机名（`remote.rs:261`） | ✅ |
| 11 | §13.3 第 4 条「客户端连我们的服务时请求 `postprocess: ["homophone","itn"]`…客户端在这一点上不用改」 | `feiyin_remote.rs:46`；客户端跳过 ITN 主通道逻辑（V091-ITN-SKIP-ONLINE-215）未改，`is_online_streaming` 已含 `FeiyinServer`（`transcription/mod.rs:123`） | ✅（**依赖 S10**：服务端处理 `postprocess` 之前，连自家服务的结果缺数字规整。S10 不完成不得出包） |
| 12 | §8.8「已配对终端…存独立数据文件，不进 config.toml」 | `remote.rs:140` `asr-remote.json`；`asr_model` 只多了一个取值 `feiyin_server`，无新配置项 | ✅ |
| 13 | §10 一期「我们的电脑客户端支持配对接入…内网或外网另一台电脑上的飞音能用这台的本地实时识别」 | 档位 `AsrModel::FeiyinServer`（`transcription/mod.rs:96`），主流程分流 `main.rs:8835`；非流式兜底 `transcription/mod.rs:2142` | ✅（端到端待两台电脑 / S11 参考客户端实测） |

## B. 不改坏现有功能

| 检查项 | 结论 |
| --- | --- |
| 阿里在线识别（qwen / FunASR）路径 | `transcribe_streaming_realtime` 签名不变；建连代码原样搬进 `connect_dashscope`（`qwen_inference.rs:1056`），会话循环 `realtime_session` 逐行未改，只多一个 `if let Feiyin` 注入扩展字段。唯一可见差异：一条 info 日志 `url=` 改为 `target=`（全仓无解析依赖，已查） |
| 在线档无 API Key 拦截 | 只对 `FeiyinServer` 豁免（`transcription/mod.rs:1892`），其余档逐位不变 |
| 错误提示归类 | 新码在最前面单独认（`main.rs:6996` 起），码名与 DashScope 错误不重叠（`invalidkey` 不是 `invalidapikey` 的子串）；原 E1/E2 护栏用例保留并扩充 |
| 网关锁定 | 内外网赛跑输掉的那路只建 TCP 不发 key；网关只在握手里 key 校验失败时计数（`ws.rs` `fail`），不会误锁 |
| 「测试连接」副作用 | 只到 WebSocket 握手（`tls_pin.rs:176` `check_access`），登录记录在 `run-task` 才产生，不污染对方统计；错误 key 会计入对方锁定（正确行为） |

## C. 安全

| 项 | 结论 |
| --- | --- |
| key 落盘 | 客户端 `asr-remote.json` 存 key 明文——本机连接的长期凭证，同保存的密码；服务端仍只存哈希（§8.4 指服务端）。key 不回传设置界面（`RemoteView` 无 key 字段） |
| 指纹校验 | 握手失败时区分「证书换了」（`mismatched`）→ 提示重新配对 |

## D. 界面 / i18n / 风格

| 项 | 结论 |
| --- | --- |
| 文案 | 主程序 3 条错误文案 + 设置界面 35 条，三语齐；用词与识别服务页一致（连接码 / 密钥 / 证书指纹） |
| 组件 | 只用现有类（`radio-card` / `input` / `btn` / `form-hint` / `code`），未新增样式 |
| 目视确认 | ⏳ **待 Gavin** |

## E. 测试

| 项 | 结论 |
| --- | --- |
| 新增单测 | remote.rs 5 个、tls_pin.rs 3 个、feiyin_remote.rs 3 个（含真实端口的内外网赛跑）、档位映射 + 错误归类护栏扩充 |
| 自验结果 | 首轮 2 处失败，均判为**测试表随设计变更未同步**（非实现缺陷）：① E3 护栏「上屏文案必须在 i18n 表内」未登记新增的 3 条 `error_feiyin_*` ⇒ 补登记；② ASR-UI-002 断言下拉框恰 2 项 ⇒ 改为断言选项值 `[performance, fun_asr_realtime, feiyin_server]`（更严：qwen 仍不得出现、本地实时仍默认隐藏）。复跑：主程序 42 通过（`asr_shared` / `feiyin_remote` / `friendly_error` / 档位映射 / `asr_056` / 实时入口）；网关 22 通过；设置界面 `asr_shared` 16 通过；vitest 133 通过；rustfmt 全部干净 |
