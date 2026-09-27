# 技术坑索引 · voice-ime

> **本文件是索引，一条一行「现象 → 判据」。** 全文在 `troubleshooting-archive.md`，按 `[条目ID]` 搜索。
> 🔴 **用法**：启动只读本文件。看到自己正在踩的现象 → 去 archive 读全文再动手。
> **索引里没写的细节不等于不存在**，不要凭一行就下「原文没提」的结论。
> 🔴 **新增条目**：全文写进 archive，同时在下表补一行。
> ⚠️ archive 里有同 ID 多次出现的条目（同一个坑的复发记录），搜索时看全部命中。

---

## 🔴 必读（踩下去代价最大的）

| ID | 现象 → 判据 |
| --- | --- |
| [SECRET-IN-REPO-001] | API key 明文入库并 push 到公网、20 天后被盗刷 → **删文件不等于止血，唯一真止血是后台吊销 key**，之后才清仓库 |
| [GATE-MATCHES-SHAPE-NOT-CONTENT-001] | 密钥闸门全绿却让口述语料 / 实验 dump 原样入公开库 → 正则只认「形态」，须靠 `.gitignore` 范围规则兜底，不能靠扫描 |
| [HOOK-FAIL-OPEN-001] | pre-push 闸门在根提交无父 / diff 取失败时**静默放行** → 安全闸门必须 fail-closed，`rev-parse` 要加 `--verify` |
| [GIT-RESET-INCIDENT-001] | 排查测试失败误执行批量 `git checkout --` / `stash` → **抹掉 11 个已验收批次**；破坏性 git 命令全面禁用 |
| [TOML-ALL-NUL-001] | 出包后规则词表整文件变全 NUL、exe 照常启动但功能静默失效 → **大小与好文件完全相等，只比大小检不出来**，须做内容 hash |
| [BINARY-PROBE-FALSE-HIT-001] | 二进制探针 grep 得 18912 这种超大命中 → BRE 把方括号当字符类了，必须 grep -F；🔴 命中数异常大是红旗不是好消息。同批：PowerShell 内联 foreach 跨层转义被吃 → 三行空值，宁可逐条调用。共同形态 = **工具语法问题伪装成数据结论** |
| [VERIFY-CHECKLIST-UNWRITTEN-001] | 「N 项核验全 PASS」长期无成文清单，条数从七→五→六自由漂移，`BUILD-159` 把 toml hash 丢进「跳过」隔天就炸 `[TOML-ALL-NUL-001]` → **编号会自动补位所以逐条读报告看不出缺项**；收到「N 项全 PASS」先问 **N 等于文档里的几项**。清单落点 `build-test-guide.md` §一·五 |
| [DISK-CLEANUP-001] | 清理 target 磁盘占用 → **禁用 `cargo clean`**（会删端测数据 + 可能穿透 models 符号链接），须逐目录 `rm` |
| [ENCODING-UTF8-001] | PowerShell 改 UTF-8 源码变乱码报错；**Python 不声明编码同样会被 GBK 截断** → 任何语言写中文源文件必须显式 utf-8 |
| [WORKER-DOC-ENCODING-002] | Worker 用「追加」语义写文档仍毁 UTF-8 且内容不可逆丢失 → 根因是 PowerShell 默认按 GBK 写，须显式 `-Encoding utf8` |
| [WORKER-DOC-OVERWRITE-001] | Worker 报「五文档已更新」实为整份覆盖、他人条目连表头一起消失 → 提交前必查 `git diff --stat` 有无删除行 |
| [EVIDENCE-LOG-VOLATILE-001] | 唯一未污染的关键 debug.log 躺在会被下次运行覆写的路径 → 判据成立那刻立即 `cp` 进 `evidence/`，**读进上下文不等于已保全** |
| [PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001] | 修了「能解释症状」的缺陷就宣布已修复，一天踩三次 → 因果链再顺也需**修复前后同场景实测对照**，否则只能说「待确认」 |

## 判断方法类（容易得出「看起来对但其实错」的结论）

| ID | 现象 → 判据 |
| --- | --- |
| [DOC-STATE-DRIFT-001] | todo 写「未做」实际已做，照派发也不报错 → 派发前必须凭 git / 文件系统重新取证 |
| [CONFIG-MIRROR-DRIFT-001] | 用户手写的 config.toml 隐藏字段，设置界面一保存就静默消失 → 主配置与 `src-tauri/src/config.rs` 是两份独立结构体，镜像没有的字段序列化时直接蒸发；`#[serde(default)]` 只兜读不兜写。**加字段必改两处**；现有 `mirror_*_match_main_config_literals` 只锁默认值（value 级），抓不住新字段漏镜像（schema 级）——为何不补 schema 快照见全文 |
| [VERSION-DRIFT-001] | 根 `Cargo.toml` 版本号与 handoffs 记载不符 → 验收须实际核对三处文件，不信文字记录 |
| [TESTER-FABRICATED-REPORT-001] | tester 自报已完成截图 / 进程运行，实际文件进程均不存在 → 验收逐项独立取证，不信表格 |
| [WORKER-WRITE-SILENT-FAIL-001] | Worker 报「已写入 N 字节」但文件实际 0 字节且 mtime 不变 → 验收必须 `wc -c` + 查 mtime |
| [STATIC-PROOF-MISSED-CALLGATE-001] | 绘制原语 diff 为空就判「零影响」，其实窗口根本没上屏 → 还须核查调用点是否被新增门控恒假挡住 |
| [ABLATION-MODEL-TOO-LIGHT-001] | 主控数值推演预告消融结果系统性偏轻 → 消融点若是多处引用的中间变量，须逐引用点核对，预期只写下限 |
| [LOG-FIELD-IS-OUTPUT-NOT-INPUT-001] | 日志里 `display`/`words` 其实是**聚合输出不是服务端入参** → 拿它反推入参会编出虚构机制 |
| [ASR-SERVER-REWRITES-TIMELINE-001] | 服务端整段重写词时间轴（词数回撤）而文本长度严格单调不减 → 别把「词数回撤」和「文本回撤」混为一谈 |
| [SAFECRLF-WARNING-MISREAD-001] | 把 `LF will be replaced by CRLF` 的警告行数当成「N 个文件被改动」上报 → 用 `--numstat` 才是真实改动数 |
| [CRLF-CROSSPLAT-001] | `git diff` 显示 4800 行改动像大重构 → 实为 CRLF↔LF，用 `--ignore-cr-at-eol` 证伪 |
| [NPM-LOCK-CROSSPLAT-001] | lock diff 显示 +39/−462 像 win32 条目被删 → 实为嵌套副本去重，判据是顶层包名不是行数 |
| [SCENE-OBSERVABILITY-001] | grep 场景日志 0 命中，误判场景感知未生效 → 实为零 log 可观测性缺口，用长度反演证明功能正常 |
| [BUGREPORT-SELFCORRUPT-001] | 口述 bug 报告可能被 bug 本身污染（出现语法突兀词）→ 用错误值反推真实输入，勿照抄报告文本复现 |
| [SESSION-CRASH-RECOVERY-001] | session 崩溃后产物与文档脱节 → sha256 + mtime 链 + 正反向探针三件套，全过则只补文档不重建 |
| [FMT-COLLATERAL-001] | 只改 2 文件却冒出 5-9 个 modified → `cargo fmt` 全量连带格式化，去空白 / 去逗号 md5 比对可证清白。🔴 2026-09-17 定规：**`src-tauri` crate 本身非 fmt-clean，Worker 一律不许对它跑 `cargo fmt`**；核验连带是否清干净用 `git diff --numstat` 与 `--numstat -w` 两份输出是否逐字相同 |
| [PROVIDER-SILENT-FALLBACK-001] | `provider` 设了不支持的值（如未编译 DirectML 时设 `"directml"`）**不报错不警告静默回退 CPU** → 以为开了 GPU 实际全程 CPU，据此做的判断全错。**判据不能是「设了参数」必须「验证实际生效」**（看 ORT 日志启用的 EP / 耗时差异反证）。🔴 同族：**DEC-069**（撞顶静默截断）、**DEC-073**（生成饿死无日志）—— 第三次踩「没报错≠成功」 |

## 协作 / Worker

| ID | 现象 → 判据 |
| --- | --- |
| [REPLACE-WORKER-INJECT-LOST-001] | 重启脚本的注入结论**两个方向都会错**：早期报「完成」实为零字未收到；2026-09-17 报「三次均失败」实为三次全部提交成功 → **报错不携带真相**，必须 `capture-pane` 看状态行：有 `esc interrupt` = 已在跑别动；只有占位符 = 补一个 Enter |
| [REPLACE-WORKER-TASKFILE-WIPED-001] | 重启会清空 `inbox/task.md`，Worker 转去读同目录陈旧 `task_*.md` 执行 → **先重启后写任务书**，inbox 只留三文件 |
| [WORKER-RESTART-MODEL-RESET-001] | 重启后模型回落到额度耗尽的免费档，看着活着实则永不响应 → 先切模型再注入；ACK_FAIL 先看是否 `Insufficient balance` |
| [FIRST-MARKER-BOUNDARY-001] | 一天内同一形态连爆四次（endpoint 块用「首个 `} else`」截断切中嵌套 if；`audio/mod.rs` 中途 `#[cfg(test)]`；`main.rs` 被新增 test mod 打穿；`punctuation/mod.rs` 跨文件扫 main.rs 同样被打穿）→ 根因是**用「第一个出现的某标记」当区域边界**，任何人日后在它前面插一个同类标记就塌缩。🔴 判据：写下「扫到第一个 X 就停」时立刻问**「谁能在 X 前面再插一个 X」**；答得出就必须改成花括号配平或「剔除全部 X 项」。改法不许是「再加一个排除条件」（下次换个嵌套又中）。**假红吵闹但安全，假绿静默且致命**，谓词宁可漏剔不可多剔 |
| [ORCH-GIT-ADD-ALL-SWEEPS-WIP-001] | 主控 docs commit 里混进 378 行生产代码，Worker 交付时才发现 → `git add -A` 在多 Agent 工作区会扫走**在途 Worker 的半成品**；最坏把编译不过的中间态钉进历史，且逼 Worker 的「未 commit」判据失真。主控 commit 一律显式列路径，提交前 `git status --porcelain` 查有无 `src/`。已扫入的**不 rewrite**，在 CHANGELOG/logs 更正归属 |
| [COLLAB-ACK-001] | Worker 正常干活却漏写 ack 文件触发假警报（3 次记录）→ `capture-pane` 确认活着就别重发重启，重发反而拖慢 Worker |
| [WORKER-HANG-001] | Worker 屏幕完全冻结连计时器都不走 → 两次 `capture-pane` 的 md5 比对判定，Escape / C-c 唤醒 |
| [COLLAB-WRITE-001] | OpenCode 写新文件到 `/d/...` MSYS 路径静默失败无报错 → 新建文件须用 Windows 风格 `D:\` 路径 |
| [COLLAB-PATH-SPLIT-001] | Worker 称已写非零字节，主控 collect 却读到 0 字节 → 实为**两套 collab 目录路径分叉**，不是写入失败 |
| [WORKER-GIT-AUTOCRLF-ENV-DIFF-001] | 同一工作区主控看 clean 而 Worker 看 44 个文件 M → 两侧 git 读到的 autocrlf 配置不同，非文件真被改 |
| [OPENCODE-BROKEN-BIN-001] | 三 Worker 全停裸 bash，敲 `opencode` 却弹出 bun 帮助 → 官方 Windows 产物退化成裸 bun，须校验 version 号 |
| [HEREDOC-EXPANSION-001] | bash heredoc 分隔符未加引号，反引号 / `$()` 被当命令执行写进文档 → 必须 `<<'EOF'` 或改用 Write/Edit |
| [MSYS-GIT-ADD-FORCE-NOOP-001] | MSYS 下 `git add -f` 对被忽略文件偶发静默 no-op（exit 0 却没进索引）→ 测闸门前须 `git ls-files` 确认 |
| [TELEGRAM-CHANNEL-001] | Telegram 消息收不到而本地链路全健康 → 实为 Claude Code 服务端 feature gate 拦截，本地无解 |
| [RESEARCH-001] | Worker 调研可自行 WebSearch / WebFetch，无需请示；讨论 ≤3 轮由主控拍板 |

## 构建 / 出包 / 测试

| ID | 现象 → 判据 |
| --- | --- |
| [BUILD-002] | 端测窗口尺寸还是旧值像没生效 → 根 `target/release` 残留旧 exe，构建后必须 `cp` 覆盖 |
| [BUILD-003] | 构建后 exe 时间戳还是旧的 → tester 漏跑构建步骤，须完整执行并核对产物时间戳 |
| [BUILD-PUBLISH-001] | 出包后 `Publish/` 的 exe 未更新（2 次记录）→ 出包流程强制含 Publish 同步 + 时间戳验证 |
| [BUILD-FIX-SYNC-001] | 改名后 target 残留旧 exe，`cp` 刷新 mtime 骗过时间戳检查 → 须核对文件名与 package name |
| [TOML-STALE-001] | 外置规则 toml 改后只同步两处 → `target/release` 残留旧版覆盖内置新默认，须**三副本 sha256 核验** |
| [VERIFY-001] | 窗口丢失最小化 / 关闭按钮，自动化测试仍报 PASS → 原生窗口行为改动必须人工启动实机验收 |
| [TEST-001] | 进程启动正常但 UI 实为空白页（2 次记录）→ 必须截图核实标题 / Tab / 控件，**进程存活不代表 UI 对** |
| [SMOKE-VANISH-001] | 冒烟 exe 无声消失，无崩溃弹窗无 crash.json → 根因未定位，**进程存活不能当稳定性证据** |
| [SENDINPUT-001] | 热键测试全失败、overlay 一直 hidden → 是旧常驻进程抢占按键，测试前须先 kill 旧实例 |
| [D2D-HANG-001] | `cargo test` 里 D2D 用例挂死、kill 都杀不掉 → COM 对象放 thread_local 在线程退出 loader lock 下 Release 自锁 |
| [E2E-CONFIG-PATH-STALE-001] | pytest 热键测试全 FAIL 像产品回归（含修复实证）→ 实为 harness 写 APPDATA 而程序只读 exe_dir |
| [TESTENV-SHARED-DIR-RACE-001] | config 测试并行随机 1 例红、`os error 3`，**且每次红的用例不同** → 判据「谁抢输谁红」= 共享资源竞态，不是新增用例写错。根因是 `TestEnv` 全用例共享 `voice-ime-test-{pid}` 且 Drop 删共享目录；23/25 靠 `TEST_MUTEX` 串行掩盖，无锁用例增至 2 个即必现。**别靠补锁了事**（那 2 例走显式路径本不该取锁），要改 TestEnv 每实例唯一目录 |
| [E2E-COLD-START-RACE-001] | cold 启动后热键停止 / PTT 释放卡在 RECORDING → 首个 Start 清空 stop 信号的竞态，warm 态无故障，harness 预热即可 |
| [PYTEST-MACOS-COLLECT-001] | 仓库根裸跑 pytest 直接 INTERNALERROR → 递归进 CT 源码树 + Windows-only 导入，须限定 `tests/` 路径 |
| [PLAYWRIGHT-FIX-001] | 9 个 UI 测试全 ScopeMismatch → session 作用域 fixture 依赖了 module fixture，改同级 scope |
| [HAPPYDOM-ALTGR-INDISTINGUISHABLE-001] | happy-dom 把 AltGraph 直接映射到 `altKey` → 真按 Ctrl+Alt 与 AltGr 单测里无法区分，只能靠端测 |
| [TESTER-SCREENSHOT-FAIL] / [SCREENSHOT-METHOD-001] | 连续截到桌面背景或别的窗口 → 像素统计自验不可靠；须 MoveWindow + 置顶 + ShowWindow 三连后截固定区域 |
| [CT2-DLL-SHADOW-397] | 换 native DLL（CT2/oneDNN）后行为或性能**逐毫秒没变** → `cargo test` exe 在 `target/<profile>/deps/`，Windows 先加载 exe 同目录 ⇒ deps 里的旧 DLL 静默盖住新库。`ls -la` + `sha256sum` 两处对比即坐实；`build.rs` shared 分支须同时拷 `deps/` |
| [BUILD-RS-RERUN-452] | 改了 build.rs 编译的 native 源码（shim.cpp）却链接旧 .lib、功能静默不生效 → winres 已输出 rerun-if-changed ⇒ cargo 只盯它声明的文件；native 源码必须自行声明 `rerun-if-changed`，怀疑没生效先比 out/ 产物时间戳 |

## ASR / overlay / 产品行为

| ID | 现象 → 判据 |
| --- | --- |
| [VOICEPRINT-OVERLAP-001] | 旁放他人语音、声纹不剔除 → 干扰多与本人**重叠 / 混在同一 VAD 段**或太轻不成段，按段判定的声纹原理上分不开（段分仍 ≥0.84）；调低判定时长门无效。本人 1.0~1.5s 短片最低 0.658，门槛下调本身不伤本人（POC-444，2026-09-26） |
| [FINAL-CLEAN-NOT-VOICEPRINT-001] | 旁有他人说话、最终上屏干净 → **不等于声纹剔除生效**：开着格式化 LLM 时是 LLM 把不通顺的干扰句删了。判据：看 `filler dedup` / LLM `user (len=…)` 行里**送进 LLM 之前**的文字，以及 `DBG-412 verdict` 是否出现 `DropNonUser`（2026-09-26，443） |
| [SLICE-VS-DISPATCH-001] | 以为「10s 找切点」在派发时起作用，实际只在**派发后解码前**切片 → 连续说话 / 背景声不停时整段到松键才派发。判据：区分「**何时派发**」（`should_dispatch_acc`）与「**派出后怎么切**」（`plan_gap_cuts`），任何切片机制上线前先写清它挂在哪一步（2026-09-26，DEC-088） |
| [OVERLAY-FLUSH-TEXT-DROP-001] | 松手后 overlay 预览尾字/尾段不上屏（最终注入文本完整）→ **flush 帧 StreamingText 结构性必丢**：`STREAMING_STOPPED`（`main.rs` 松手即置 true）+ `should_ignore_streaming_text`（= stopped）⇒ flush 必在松手后 ⇒ 必被丢。**已修（LOCALRT-FIRSTCHAR-282）**：**不改共享 latch**，新开本地档专用事件 `StreamingFinalPreview`（只有本地档发/收）；在线档代码路径结构上不变 |
| [ASR-SAMPLERATE-STREAM-001] | 边录边发后识别结果变成完全不相干的客服话术 → 采样率写死 16kHz 未重采样 48kHz，看日志秒数是否恰为实际 3 倍 |
| [ASR-LONG-AUDIO-001] / [ASR-NATIVE-LONG-001] | accuracy 长音频空输出 / 乱码 / 截断 → native `max_total_len=512` 硬限制，VAD 切段 ≤20s 逐段转录拼接根治 |
| [ASR-HALLUC-SEGMENT-001] | VAD 分段长音频中段被整段幻觉替换 → 流畅低速率幻觉是三重兜底盲区，需 CTC 交叉验证 |
| [ASR-RELOAD-001] | 模型热重载并发触发重复加载 972MB 模型 → channel 非空 ≠ 构建在途，须独立 `in_flight` 标志 |
| [FIRSTCHAR-001] / [FIRSTCHAR-002] | 热键触发首字丢失；送气清声母字（派/对）首字反复错 → 根因是 channel 清空竞争 + Prime 截头 + 降采样无抗混叠，**不是纯模型局限** |
| [OVERLAY-POS-HARDCODED-ZERO-001] | 在线流式 overlay 先跳左上角 `[0,0]` 再跳到正确位置 → 硬编码坐标此前因参数未生效而无害，参数生效后须全文件回查传参点 |
| [EDIT-CONTROL-NO-FONT-001] | 进入编辑态文字突然变粗糙有锯齿 → EDIT 控件从未收到 `WM_SETFONT` 退回点阵字体，别把圆角裁剪当根因 |
| [F3-UNORDERED-LIST-001] | 无序枚举（比如…）不出列表、11 个假设全证伪 → 根因是模型对无主语短句保守不判；按 DEC-049 不修，改说「第一 / 第二」可靠 |
| [ITN-PREFIX-SHADOW-001] | 保护词表短词条前缀匹配会遮蔽 / 撕裂后续文本 → 新增前必问「会遮蔽什么」，规则性语法族交 ITN 文法不进词表 |
| [ITN-LOCAL-RULE-OVERREACH-001] | 为短表达设计的 ITN 特例规则未声明边界，更长上下文里越界改错（「一三班」→「13班」）→ 须配边界外护栏测试 |
| [WORDBOOK-AUTOLEARN-001] | 词库自动学习看似不生效 → 阈值（同词 ≥2 次）与 LLM 一次性建议词分布不匹配，非链路故障 |
| [WORDBOOK-SCHEMA-BREAK-001] | 词库操作全部报错 `no such column raw` → migration 在已迁移库上必然失败，须按 schema 三态处理 |
| [LLM-COT-LEAK-001] | 换 DeepSeek 后偶发整句被替换成三个点 → 我方代码错把 `reasoning_content` 回落当 answer |
| [PERF-001] | LLM 优化要等好几秒 → 推理模型默认开思维链，须关 `enable_thinking` 并压低 `max_tokens` |
| [BUG-PTT] | PTT 松键后录音不停止 → `SetTimer` 部分场景不可靠，改轮询线程 + `AtomicBool` |
| [BUG-025] | 只能捕单键，Ctrl+A 等组合键失效 → `keyCode` 不可靠，改 `e.code` + VK 表；Win 键系统级拦截放弃 |
| [BUG-018] | 换目录启动后热键延迟，卡在联网下载 → 模型路径是相对路径，须用 exe 所在目录拼绝对路径 |

## 早期 / 已收敛（多为考古用）

| ID | 现象 → 判据 |
| --- | --- |
| [WINDOW-TITLEBAR-BUG] · [WINDOW-TITLEBAR-REVERT] · [WINDOW-TITLEBAR-RESEARCH-001] | `decorations:false` 设了标题栏仍在 → Tauri v2 Windows 已知能力缺口，保留原生标题栏；回滚须同时去掉 `transparent` |
| [BUILD-001] | 装了 tauri-cli v2 后 `cargo tauri build` 报错无法构建 → 项目是 Tauri v1 不向下兼容，直接用 `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` 跳过 tauri-cli |
| [ARCH-001] | 多 viewport / 多线程窗口显隐不稳定甚至崩溃 → 禁止同进程多个 `run_native`，改 Win32 主控 + 子进程 |
| [BUG-027] | 托盘「配置」二次点击无反应（2 次记录）→ overlay 隐藏窗口令进程不退出，关窗须一并 exit |
| [TAURI-001] / [TAURI-002] | 打开空白或「localhost 拒绝连接」；加了 `custom-protocol` 仍连 localhost → release 必须带该 feature，切 feature 后须先 `cargo clean` |
| [CRASH-001] | exe 崩溃不生成报告、不弹 reporter → Tauri 子进程缺 panic hook，须补注册复用主程序 crash 逻辑 |
| [EXE-SIZE-001] | exe 体积膨胀，`ui/dist` 仅 193KB 并非主因 → 真凶是 release profile 缺失 / feature 过宽 |
| [TRANS-CT2-001] · [TRANS-CT2-DEBUG-001] · [TRANS-CT2-EMPTY-002] | CT2 找不到头文件 / 无输出回退原文 / token 正确却零结果 → 官方 build.rs 路径算错一层须 patch；改走 SentencePiece + 底层 API；rust 包装层有 bug 须直调 C FFI |
| [NLLB-EVAL-001] | 换 NLLB 翻译看似能跑但结果不对 → 高层 `Translator2` 没把 prefix 传给底层解码，须走 `translate_batch2` |
| [CT2-SUBMODULE-DEADLOCK-001] | ctranslate2-sys 构建报「目录已存在」，重试永远同样报错 → tarball 缺 submodule，须删残缺子目录重 clone |
| [TOML-SECTION-DRIFT-001] | macOS `cargo check` 报 3 个共享 crate 找不到 → 段头插到表中间致依赖被静默划入 Windows 专属 |
| [SHELL-BASHSOURCE-ZSH-001] | macOS 默认 zsh 下脚本自定位不报错却解析错误 → `BASH_SOURCE` 在 zsh 下为空，需兜底 `$0` |
| [NPM-CI-LOCK-DESYNC-001] | `npm ci` 两平台都报 EUSAGE 缺失依赖 → lock 长期失同步非新改动引入，禁 `npm install` 静默裁剪 |
| [TRANS-REGRESSION-001] | 翻译空格丢失 + 输出截断回归 → 任何任务须加回归检查项，防修复被后续改动打回 |
| [UI-DEBUG-001] / [UI-DEBUG-002] | 改了滚动条 CSS 仍看得见；`.main-content` 无滚动条用户却仍看到 → webkit fallback 写法本身强制显示；真凶是 `.sidebar::after` 伪元素溢出 |
| [UI-001] | `Llm.tsx` 里被加了 `system_prompt` 输入框 → 该入口已随 PROMPT-BASE-207 整体移除，提示词现为内置常量 |
| [ENCODING-FIX-001] | 配置里中文标题变乱码致断言失败 → 以 UTF-8 重存并重新构建覆盖旧 exe |
| [MAC-011] / [MAC-012] / [ENV] / [ENV-002] | 本机装 Darwin target / cc 失败、cmake 找不到、Tauri 构建缺环境 → 环境记录，报告中须明确写「未验证 Darwin」不谎称已验证 |
| [POC-BYPASSES-PROD-WRAPPER-001] | PoC 报出「产品级严重缺陷」（如 accuracy >28s 空输出）→ **先核对它调的是不是生产同一条代码路径**，不是核对数据。数据全真但路径不同 ⇒ 结论完全无效。同批教训：判断既有模块行为前先按 ID 搜 `decisions-archive.md` 全文（`use_itn`/`itn:1` 实际无效已载于 DEC-030 背景，主控却凭字段值推断出不存在的「双重 ITN」） |
| [ENUM-EQ-CHECK-MISSES-NEW-VARIANT-001] | 新增枚举变体后出现两个看似无关的 bug（标点重复 + 长句无输出）→ 根因是 `== Enum::Variant` **相等比较**漏改，**编译器不报错**（只有穷举 `match` 有保护）。派单只补 match arm 不够，**必须全仓 grep `==`/`!=` 逐个判断**。修法：在枚举上加语义化判定方法作收敛点（`matches!` 穷举形式），禁散落写 `== A \|\| == B`。同族 [CONFIG-MIRROR-DRIFT-001] |
| [BINARY-PROBE-SYMBOL-INLINED-001] | 用**函数名**做 release 二进制探针，命中 0 被误判为「代码没进包」→ 小函数（尤其 `matches!` 展开的判定方法）release 下必被内联，符号根本不入二进制。**只有字符串字面量才进 .rdata**，探针必须选字面量（模型目录名、config 键名、日志前缀），禁用函数/方法/类型名。主控 2026-09-20 出题即犯此错，tester-1 发现 |

## [FMT-COLLATERAL-001] rustfmt 吃 crate 根 main.rs 会递归进所有子模块（多人并行时隐性触碰他人在飞文件）

**现象**：对 crate 根 `src/main.rs` 跑 `rustfmt src/main.rs`，rustfmt 会沿 `mod xxx;` 声明
**递归格式化整棵模块树**（`src/audio/mod.rs`、`src/itn.rs` …），而不是只格式化你给的那个文件。

**为什么危险**：多 Worker 并行时，A 只被授权动 `src/main.rs`，一次 rustfmt 却可能改写
B 正在编辑的 `src/audio/mod.rs`。改动是纯 whitespace，**编译照过、测试照绿**，
但会污染 B 的 diff，且在 `numstat == -w` 自证里暴露成「B 夹带了无关空白改动」——**冤枉 B**。
更糟的情况是 B 正在写盘，两边互相覆盖。

**判据（谁都能跑，读操作）**：
```bash
git diff --numstat -- <文件>      # 含空白
git diff -w --numstat -- <文件>   # 忽略空白
```
两者**逐字相同 ⇒ 无 whitespace-only 改动 ⇒ 没有夹带**；不同则差额即为纯空白行数。

**正确做法**：
```bash
rustfmt --config skip_children=true <具体文件>   # 禁止递归子模块
cargo fmt -- <具体文件>                          # 或只点名文件
```
🔴 **不要对 crate 根 `main.rs` 裸跑 rustfmt**，除非你确实持有整棵树。

**实例（2026-09-21）**：coder-2 做 ACC-PREVIEW-REFLOW-325 时为恢复编译跑了
`rustfmt main.rs local_stream.rs`，同期 coder-1 正在 `src/audio/mod.rs` 上做 PREROLL-DEAD-322。
coder-2 **自查发现风险并主动上报**，主控实测 `357/11` vs `-w 357/11` 逐字相同 ⇒ 本次未实际夹带
（`rustfmt --check` 当时仍报 audio/mod.rs 两处未格式化，与「没写进去」一致）。
**记录本条是因为风险真实存在且下次未必落空**，不是因为这次出了事。

## [AUTOLEARN-REACH-001] 🔴 已作废（2026-09-21 实测证伪）—— 原记录「本地档自学习不可达」是错的

**原记录内容**：「只有在线流式路径写 `last_streaming_text`，本地模型路径不写 ⇒ 自学习拿不到原文 ⇒ 整条路不可达」。

**证伪**（BUILD-321，Gavin 的 LocalRealtime 会话，`collab/evidence/20260921-build321-e2e/debug-build321.log`）：

```
08:42:56 DEBUG wordbook] Auto-learn candidate rejected (contains sentence-ending punctuation): "指导灵的信息吗？"
08:43:14 DEBUG wordbook] Auto-learn candidate rejected (contains sentence-ending punctuation): "指导灵的信息吗？"
08:43:45 WARN  wordbook] [AUTOLEARN] candidate observed: '指导灵' (1/2)
08:44:10 DEBUG wordbook] Auto-learn candidate rejected (contains sentence-ending punctuation): "指导灵的信息吗？"
```

`learn_correction` **确实被调用**，原文非空，比对跑出了差异 ⇒ LocalRealtime 的学习镜像是通的
（写入点 `main.rs:8167`）。**原记录对 LocalRealtime 不成立。**

非流式本地档（Accuracy / Performance / 批处理）走 `record()`，不发 `StreamingText`，
**且没有编辑入口**（`text_hit_rect` 仅在 `RecordingWithText` 下设置）⇒ 对它们而言不是「不可达」，
而是**本就没有这条交互**，改镜像也不会触发 `SubmitRequested`。

**真因**（`src/wordbook/mod.rs:203` `extract_correction_word`）：`tokenize` 按**字符**切，
候选 = 掐公共前缀 + 公共后缀后的中间段。**ASR 若把句子后半段也听错，公共后缀为空，
候选就一路吞到句尾**（连标点），被 `is_valid_candidate` 的句末标点校验整体拒掉。
4 次尝试只有 1 次 ASR 尾部恰好听对、抽出干净的「指导灵」⇒ 计数 1，门槛 2 永远够不着。

⇒ 后续单 **AUTOLEARN-CANDIDATE-327**：修抽取（收窄跨度），**不降门槛、不放宽校验**。

**教训**：主控凭「字段在某路径没写」就断定整条链路不可达，**没有翻运行日志核实**。
`AUTOLEARN-REACH-001` 这条错记录还被写进了任务书当前提派发，Worker 停手反证才拦住。
🔴 **下结论前先 grep 运行日志**——功能有没有跑过，日志比代码阅读更直接。

## [TOML-STALE-001] 补充（2026-09-21）：新增外置规则表时，**漏拷**与**拷旧**是两种不同的坑

原条目讲的是「exe 同级存在**旧副本**，覆盖编译期内置默认 ⇒ 修复静默失效」。
2026-09-21 同步四张表时发现**另一种形态**，症状相反、同样要防：

| 形态 | 现象 | 后果 |
| --- | --- | --- |
| **拷旧**（原条目） | 三副本内容不一致 | 🔴 修复静默失效（旧表盖掉新内置默认） |
| **漏拷**（本次新增） | 输出目录**根本没有该文件** | ⚠️ 行为正确（回退内置默认），但**用户改不了这张表** —— 外置的初衷（DEC-011）落空 |

**本次实况**：`homophone-rules.toml`（318 新增）与 `wordbook-rules.toml`（327 新增）
在 `Publish/` 与 `target/release/` 下**都不存在**。行为无 bug，但等于白外置。

**规矩**：**新增任何 `*-rules.toml` 外置表，同批把它拷到 `Publish/` 与 `target/release/`**，
并在交付自证里列出三处 sha256。出包流程同样逐张核对。

**对账命令**（四张表一次核完）：
```bash
for f in itn-rules.toml scene-rules.toml homophone-rules.toml wordbook-rules.toml; do
  sha256sum "$f" "Publish/$f" "target/release/$f"
done
```
🔴 **覆盖前先 `diff`**：确认旧副本与根目录的差异只是「缺新内容」，而不是含本地定制。
本次 diff 确认两个旧 `itn-rules.toml` 仅缺 `[protect.degree_adverbs]` 一块，其余逐字相同，
故覆盖安全。用 `cp`（字节复制），**不要用 PowerShell Set-Content**（会把 UTF-8 写成 UTF-16 LE）。

## [FILTERED-TEST-BLINDSPOT-001] 🔴 过滤跑测试查不出源码级护栏 —— 红了两次提交没人发现

**事故**（2026-09-21）：`flicker_130_guard_tests::f1_mirror_before_gate` 在
`4d49252`(329) 与 `7d3a3a5`(331) **两次提交中一直是红的**，主控与 coder-2 都没发现，
直到 coder-1 做 332 时跑全量才暴露。

**成因**：329 把镜像早写由 `*mirror = Some(text.clone())` 改成 `*mirror = Some(composed.clone())`。
F1 是**源码级护栏**——它 `include_str!` 读自身源码、按**字面量**定位那行来验证
「镜像早写先于 043 门闩」这个不变量。字面量一变，锚点就找不到，测试红。

**为什么两道验证都漏掉**：
- coder-2 自证跑的是 `reflow_persist_329` / `preview_reflow_325` / `053-B`（过滤）
- 主控复核跑的是同一批过滤用例（`cargo test --bin feiyin-ime -- <filter>`）
- 🔴 **过滤跑根本不会执行到 F1** —— 它不在过滤名单里，`filtered out` 数字里静静躺着

**规矩**：
1. 🔴 **改动落在源码级护栏覆盖区（main.rs 的 flicker_130 / overlay_075 / f1~f4 等模块）时，
   提交前必须跑全量 `cargo test`，或至少整个护栏模块** —— 不能只跑本任务的目标用例；
2. 过滤跑只适合「快速看本次改动对不对」，**不能当作提交判据**；
3. 源码级护栏一旦因**正当改动**失配，**只同步锚点、不改断言语义**，并在注释写明何时为何同步
   （332 即如此处理；人工复核不变量仍成立：早写 :6835 先于门 :6843）。

**识别特征**：测试名里带 `guard` / `f1~f4` / 断言里出现 `count_startswith` `block_line_of`
`main_prod_lines` `concat!("…", "…")`（故意拆字符串防自匹配）⇒ 就是源码级护栏，
**它对无关改动敏感，这是设计使然，不是脆弱**。

## [FMT-COLLATERAL-001] 再补（2026-09-21）：`skip_children=true` 可以**用来格式化**，但**不能用来验收**

本条是主控给错指引后的订正，**责任在主控**。

**背景**：为防止 rustfmt 吃 crate 根 `main.rs` 时沿 `mod` 递归污染他人在飞文件（本条第一节），
主控要求 Worker 一律用 `rustfmt --config skip_children=true`。**防污染这个目的是对的。**

🔴 **但把它同时当作验收命令就是假通过**：`skip_children=true` 会**跳过所有经 `mod` 到达的子文件**，
于是 `rustfmt --config skip_children=true --check src/main.rs` 只检查了 `main.rs` 自己。
`src/audio/mod.rs` 正是 `main.rs` 的子模块 ⇒ 它不 clean 也照样报 clean
（coder-1 2026-09-21 自查发现并上报）。

**规矩（两条分开，别混用）**：

| 目的 | 命令 |
| --- | --- |
| **格式化**（避免碰他人文件） | `rustfmt --config skip_children=true <自己的文件>` 或 `cargo fmt -- <文件>` |
| **验收/提交前**（必须覆盖全仓） | 🔴 `cargo fmt --check`（**不带** skip_children） |

⇒ Worker 自证里写「fmt clean」时**必须注明用的哪一条**；写 `skip_children` 的那条只能证明
「我自己的文件干净」，**不能证明仓库干净**。主控提交前自己跑一次不带参数的 `cargo fmt --check`。

**与本文件 `[FILTERED-TEST-BLINDSPOT-001]` 同型**：两者都是「**为缩小范围而加的参数，
被当成了全量验收**」。识别特征：命令里出现任何 **缩小检查面** 的开关
（`--config skip_children`、`cargo test -- <filter>`、`grep` 带限定路径）⇒
它的绿色只覆盖被检查的那部分，**不能推广为整体结论**。

## [SPEC-PARAPHRASE-001] 🔴 主控把需求转述成自己的说法，转述错了就成了「设计」

**事故**（2026-09-22 Gavin 指出）：298 并行派发的设计意图是
「静默 ≥800ms **或** 连续说话累计 ≥5s，任一成立即切片送 accuracy」。
主控写任务书时转述成了 **AND**，实现照做，注释还写死「三个条件**同时**成立」。

**后果链**（这才是重点，不是单个 bug）：
1. 连续说话时 `silent_ms` 永远到不了 800ms ⇒ **accuracy 全程拿不到音频**，只在停顿时才工作；
2. 短句说完停顿，因不足 5s **也不派** ⇒ 预览永远补不上尾字；
3. 于是回灌表现为「时有时无」，Gavin 反复端测、主控连查四轮
   （291 补 EOS / 307 重解码 / 340 补静音 / 342 查 get_result 空），
   每轮都查出**真实但次要**的问题，**根因始终没被碰到**；
4. 错误的注释「三个条件同时成立」**把误解固化成了文档**，后人只会以为本来就这么设计。

🔴 **根因不是写错一个逻辑运算符，是转述**：
主控习惯把需求「理解后用自己的话」写进任务书。转述一旦偏离，它就成了唯一的权威记录 ——
Worker 照它实现、注释照它写、测试照它锁死，**从此没人再回头看原话**。

**规矩**：
1. 🔴 **任务书必须原样引用 Gavin 的原话**（带引号），再附主控的理解；两者并列，**不得只留转述**。
   今天 337/340 的任务书做到了，298 没有。
2. 涉及**判据/阈值/条件组合**（且-或、上限-下限、先-后）时，**逐条对照原话复述一遍**再落笔
   —— 这类错误单看代码永远发现不了，因为代码与注释会自洽。
3. Worker 若发现任务书与既有设计文档矛盾，**停手报主控**，不要「按任务书为准」硬做。
4. 🔴 **被指出曲解后，不许用「这是个权衡」「会增加开销」之类说法软化** ——
   照原设计实现，代价另开单优化。

---

## [QWEN3-PREFIX-LEAK-371] 1.7B 语种前缀漏进正文 —— 研究结论被实现时私自收窄

**现象**：BUILD-349 端测，最终输出开头带 26 字前缀
`language chinese<asr_text>明天天气好的话，…`。预览与最终文本都有。

**判据**：`[LocalRT-DBG-320]` 里 `out_chars` 比该片音频应有字数多出约 26；
日志 grep `asr_text` 命中用户可见文本（而非只在 PoC 日志里）。

**根因**：`collab/research/qwen3-1.7b-capability-355.md:16,185` 定的规则是
**「无条件截断到第一个 `<asr_text>`（含）」**；359 落地时（`mod.rs:522`）自行加了两道闸 ——
① 前缀必须「标签样」(`is_qwen3_language_label`)；② `<asr_text>` 必须紧贴标签（前缀不得以空白结尾）。
355 观测前缀时走的是 `raw_decode`/`make_qwen3`，**没有 `set_option("hotwords", …)`**；
生产 `decode_accuracy_once`(`mod.rs:381`) 每次注入 ctx(112字)+词库(76字)，
注入改变了模型自造前缀的形态 ⇒ 两道闸任一拦不住就整条漏出。

🔴 **教训**：研究报告里带「必须 / 不得」字样的判据，落地时**只能放宽不能收紧**。
要收紧必须回头改报告并说明理由，否则实现与结论静默分叉，单测还会照着收紧后的形态写、
把错误锁死（本例 `mod.rs:4059/4145` 两条单测对**没加空格**的形态是通过的，正因如此没拦住）。

---

## [ALIGN-PERIODIC-EAT-371] 重复语句下滑窗对齐吃掉中间文本

**现象**：同一句连说 4-5 遍，最终文本中间少一段，只剩半句残片
（实测：5 遍中第 3 遍只剩「来看电影吧。」，前面「明天天气好的话，可以一起出」没了）。

**判据**：内容具周期性（用户重复、口头禅、复述）+ 最终文本**整段或半句消失**，
而 `join:` 行的 `committed` 明显小于各窗 `out_chars` 之和。

**根因**：`align_overlap`(`mod.rs:1343`) 从 `max_k` **往短找、取第一个过质量门的 k**
（原注释：「长重叠优先 ⇒ 边界更靠前、更稳」）。周期性内容下这个假设崩塌：
设 S 为重复句，`prev = S1 S2 S3`、`new = S2 S3 S4`，因 S1=S2=S3=S4，
`k = max_k` 时 `p_tail = S1S2S3` 与 `new[..k] = S2S3S4` **逐字相等** ⇒ dist=0 先命中 ⇒
切点落到 prev 开头 ⇒ `committed_prefix` 为空 ⇒ **整窗文本被丢**。
ASR 变体字（明甸/明天、一起出来/解出来）使重复不完全相同，但质量门
`ALIGN_MAX_EDIT_RATIO=0.15` 仍放行，于是表现为「吃掉半句」而非整段。

🔴 **方向性铁律**（必须写进代码注释）：
k 越小 ⇒ 切点越靠后 ⇒ committed 越长 ⇒ 最坏是**重复**；
k 越大 ⇒ 切点越靠前 ⇒ **丢字**。
Gavin 的优先级是**丢字 P0、重复可容忍**，所以任何退化/兜底分支只能往**小 k**偏。

🔴 **正解不是调阈值**：重叠长度不是自由变量 —— `OrderedReflow::push` 已带切片区间、
`main.rs:8290-8325` 有每片音频时长，**期望重叠可以算出来**，应据此把 k 约束在期望值附近。
在全区间自由搜索 + 靠编辑距离兜底，遇到周期性内容必然失手。

🔴 **补充（Gavin 2026-09-22 追问后修正）**：按时长比率估字数**不精确**
（语速不匀：停顿、语气词 ⇒ 秒数涨字数不涨 ⇒ 估值虚高），**只能做粗筛，不能当判据**。
真正精确、零假设的是下面这条硬约束：

> 窗 N 覆盖片 `[ws_N, we_N)`、窗 N+1 覆盖 `[ws_N1, we_N1)`。若 `ws_N1 > ws_N`，
> 说明窗 N 比窗 N+1 **多出 m 片有声切片**（VAD 切出来的，必定对应非空文本）
> ⇒ `committed_prefix` **必须非空** ⇒ `k` 必须**严格小于** prev 的有效字数。

实例（Gavin 原话那句连说 5 遍）：窗N=[0,3)、窗N+1=[1,4)，多出片0（4.48s 有声）；
`k = max_k` 给出 `committed_prefix = ""`，与「片0 有 4.48 秒话音」直接矛盾 ⇒ 当场否决。

**判据必须四层，缺一不可**：
① 硬约束（精确，最先执行）→ ② 时长比率软范围（粗筛，容差宁宽勿窄）
→ ③ 编辑距离质量门（在候选区间内定切点，沿用现有）→ ④ 兜底偏**小** k。
只有②挡不住 `k=53` 这类（提交 1 字、仍吃 17 字）；只有①只挡得住最极端那个。
②宁可放宽：把真值挡在外面（回落兜底 ⇒ 重复）比放进一个错值（⇒ 丢字）代价小。

---

## [PROMPT-SLOT-ABUSE-377] 把自由文本塞进专用模型的固定模板槽位

**现象**（同一病根的三个症状）：① 输出开头出现 `language chinese<asr_text>`；
② 末尾整串吐出用户词库；③ 末尾吐出上一次录音文本的变形（「上一下文传路的」）。

**判据**：输出里出现的内容**能在我们注入的 prompt 里逐字或近似找到** ⇒ 就是这一类。

**根因**：Qwen3-ASR 是从 Qwen3-Omni 微调的**专用 ASR 模型**，模板固定三段
（system 放偏置词 → user 放音频 → assistant 出「语种 + 正文」）。
sherpa 的 `hotwords` 被**原样**塞进 `<|im_start|>system` 段，C++ 注释写死期望形态是
**`"foo,bar,baz"` 这样的 ASCII 逗号分隔词表**。而我们塞了：两句英文指令 + `Context:` 整段中文散文
+ `Terms:` 标签 + 词表 —— **四样里只有词表合规**。
LLM 的输出本质是 prompt 的续写 ⇒ 超规格的自由文本会被当成「要输出的内容」。

**POC-376 实证**：英文指令句与 `Context:` 散文**各自独立**都能被吐出来
（ja·不设 language ⇒ 吐 68 字英文指令句；ja·Auto ⇒ 吐 82 字 Context 散文，LCS 77/81）。
设 `language`（含 `Auto`/空串）**不能**消除回显，只是换了回显的内容。

🔴 **三条可复用的判据**：
1. **接口同名 ≠ 语义相同**。`set_option("hotwords", …)` 在 CTC/transducer 上是**解码打分偏置**
   （词条物理上出不来），在 Qwen3 上是**prompt 文本**（可以原样回来）。
   换模型时，**同名接口的语义可能整个变了**，代码一行不改也会静默漂移。
2. **专用模型不是 chat 模型**。往固定模板里塞自由指令，模型不会「遵守」，只会「转写」。
   实证：模型把 `Transcribe cleanly: drop stutters...` 这句指令**原样吐了出来**。
3. **研究结论里带「必须/不得」的判据，落地时只能放宽不能收紧**（见 `[QWEN3-PREFIX-LEAK-371]`）。

**修法（377）**：注入内容砍成**纯 ASCII 逗号分隔词表**；词表为空 ⇒ 不调 `set_option`
（等同官方示例的「无 system 段」）。`language` **保持不设**——官方 `language=None` 即自动语种判别，
其评测原文「none of the tests specified a language parameter」。

---

## [DEAD-FEATURE-CLEANUP-INSTR] 加提示词前先问「管线上是不是已经有人在干这事」

`CLEANUP_INSTR_EN`（"Transcribe cleanly: drop stutters, repeated words, and filler words."）
由 `7cc7cea`（2026-09-21）**夹带**在一个护栏修复单里加入（commit 标题第三项「输出清理指令恒发」），
**无单独立单、无 A/B 实证** —— 违反 Gavin 既定的「提示词是独立模块，禁夹带顺手改」。

**它是三重多余**：
1. **模型根本没执行**（POC-376：模型把这句话原样吐了出来 ⇒ 当成转写内容而非指令）；
2. **管线上早有专门节点**：`apply_filler_strip`（`main.rs`，`enabled = !llm_handled`，
   调 `text_normalizer::strip_fillers_conservative`）+ LLM 路径的 **F1 Filler Removal**，两条互补全覆盖；
3. **白占 KV 预算**（指令句 + 标签约 55 token）。

🔴 **教训**：加提示词之前必须回答「**现有管线上是不是已经有节点在干这件事**」。
单独立单会被迫回答这个问题；夹带进别的单子就不会 —— 这正是「禁夹带」规矩要防的。

## [GUARD-SKIP-BRACE-IN-STRING-382] `prod_lines_excluding_cfg_test` 的花括号计数会被**字符串里的 `{`/`}`** 带偏 —— 一次假红 5 条跨文件护栏

**现象**：FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382 新增若干 `#[cfg(test)]` 模块后，全量回归出现 6 条 FAILED，全部是**跨文件结构护栏**：
`punctuation::tests::guard_214_215::{g7,g9,g10,g11}`（扫 `main.rs`）、`nospeech_122_guard_tests::h4`（扫 `main.rs`）、
`testsync371_window_counter_guard_tests::counters_are_pushed_together`。报错都是「锚点必须存在」——但锚点明明在源码里。

**根因**：`guard_prod_lines::prod_lines_excluding_cfg_test`（`main.rs`）跳过 `#[cfg(test)]` 项时用**朴素的逐字符花括号计数**
（`'{' → depth++`、`'}' → depth--`，`opened && depth<=0` 停）**来决定「该项本体到哪里结束」**。
它**不区分字符串字面量 / 注释**：我在新测试模块里写了 `assert!(..., l.contains("for _ in 0..concurrency {"))` ——
那个 `{` 令 `depth` 多算一层，模块真正的收尾 `}` 之后 `depth` 仍为 1，于是扫描器**越过模块边界继续吞**，
一路吃到下一个平衡点（实测 `11353→15268`，把整个 `run_pipeline_core` 都当成了测试区剔除）⇒ 其后所有生产锚点「消失」。

**判据**：护栏报「锚点必须存在」但 `grep` 源码明明有 ⇒ 先怀疑**扫描器把生产区误剔了**，而不是锚点真丢了。

**修法（本单）**：把测试字符串里的 `{` 去掉（`"for _ in 0..concurrency"` 即可）⇒ 6 条立刻转绿。

**🔴 2026-09-25 复发（第二次）⇒ 422 根治**：TEST-SYNC-420 给 `fix_reflow_raw_base_420_tests` 加 `fn_body`
helper，内含 `.find('{')` 与 `.expect("函数体 `{` 缺失")` 等**非代码 `{`** ⇒ 朴素计数再次被带偏，
收尾 `depth>0` ⇒ 把其后生产区（L11144~L12409）整段误剔（唯一失败 `testsync421_tests::ts421g_source_anchors`）。
**根治**：`prod_lines_excluding_cfg_test` 的项体定界改为**字符串 / 字符 / 注释感知**的共用状态机
（`next_top_token` / `brace_match`，显式处理普通串·原始串 `r#"…"#`·字符 `'{'` 与生命周期 `'a` 区分·
行/块注释可嵌套），`fn_body` 也改用它（**禁两份**）。**不再靠「别写裸 `{`」约定**。

**可复用规则**：
1. **写 `#[cfg(test)]` 模块内的字符串时禁含裸 `{`/`}`**（要含就保证成对，但这很脆——最好直接避免）；注释同理（注释里的花括号也会被算）。
2. 新增/移动 `#[cfg(test)]` 模块后，**必跑全量**（`cargo test --no-fail-fast`）—— 跨文件护栏不在 `cargo check` 覆盖内。
3. 诊断方法（Think-in-Code）：用脚本**复刻** `prod_lines_excluding_cfg_test` 并打印每个 skip 的 `[start,end]`，
   一眼看出哪个 skip 跨越了几百行（正常测试模块多在几十~百余行；跨越 >400 行即高度可疑）。

---

## [CT2-DESTROY-DEADLOCK-394] CTranslate2 `translator_destroy` 在**已完成推理**的 translator 上死锁 —— 退出卡死 / 运行中换向卡死

**现象**（TRANS-394-REWORK R1-a，真模型 NLLB-200 int8，`#[ignore]` 普通线程取证）：
线程内 `load → translate → drop(engine)` 后 `drop` 永不返回：

```
[A:load+translate+drop] t=5s 未返回；进程 CPU 增 16.48s
[A:load+translate+drop] t=10s 未返回；进程 CPU 增 22.11s
[A:load+translate+drop] t=15s..30s 未返回；进程 CPU 增 22.11s   ← CPU 停止增长（= 死锁，非自旋）
🔴 [A:load+translate+drop] 30s 内 drop 未返回
```

`timeout 75 cargo test …` 退出码 **124** ⇒ 测试进程**自身也无法退出**（进程 teardown 同样卡住）。
`main.rs` 全文件**没有** `process::exit`；退出序列 `worker_tx.send(WorkerCommand::Shutdown)` → `worker_join.join()`，
worker 返回时 drop 局部 `cached_translation` ⇒ 触发 `translator_destroy` ⇒ **退出程序卡住**。

**关键判据（本次实测最大的增量）**：挂死**不是**「create 后 destroy 就挂」——

- **A 组**（load → **translate** → drop）：🔴 30s 不返回，CPU 停止增长，进程不能退出；
- **B 组**（load → **不翻译** → drop）：✅ 1.28~1.64s 返回，退出码 0（复现 3 次一致）。

⇒ 死锁**只在 translator 跑过推理之后**发生（CT2 CPU 线程池已启动/执行过计算，析构此处 join 卡死）。
只按 `create → destroy` 写的最小复现**测不出来**，必须带一次 `translate`。

**根因**：CTranslate2 `Translator` 析构在其 CPU 线程池已运行过后发生 join/锁死锁（本机 int8 CPU 路径实测）。

**修法（R1-b，`src/translation/mod.rs`）**：模型是**进程级资源** ⇒ **加载一次、永不析构**：
`thread_local! { static NLLB_MODEL: RefCell<Option<(PathBuf, &'static NllbModel)>> }` +
`shared_model()`（命中同路径复用，否则 `Box::leak` 常驻）；`TranslationEngine` 持 `&'static NllbModel`；
`impl Drop for Ct2Translator` 保留但**生产路径永不触发**。
代价：关闭翻译后模型仍驻留（int8 ≈ 600MB）直到进程退出。

**可复用规则**：

1. **外部 C/C++ 资源的析构若不可靠，就不要析构** —— 进程级单例 + `Box::leak`；别把「释放 600MB」当收益（释放失败 = 卡死，代价远大于内存）。
2. 复现「析构挂死」：**必须在真正用过的资源上试**；用 `recv_timeout` + 进程 CPU 采样区分「自旋（CPU 涨）/ 死锁（CPU 平）」；泄漏挂死线程、**不 join**，再用 `timeout` 看进程能否自行退出。
3. 别写「生产走 `process::exit` 跳过析构故无碍」这类**未核实**的免责声明 —— 先 `grep` 调用方确认退出路径（本单即因该假设错误被验收退回）。
