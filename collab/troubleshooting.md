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
| [FMT-COLLATERAL-001] | 只改 2 文件却冒出 5-9 个 modified → `cargo fmt` 全量连带格式化，去空白 / 去逗号 md5 比对可证清白 |

## 协作 / Worker

| ID | 现象 → 判据 |
| --- | --- |
| [REPLACE-WORKER-INJECT-LOST-001] | 重启脚本打印「注入完成」但 Worker 零字未收到 → 就绪判据命中太早且零校验，须认输入框占位符 + 打字回读验证 |
| [REPLACE-WORKER-TASKFILE-WIPED-001] | 重启会清空 `inbox/task.md`，Worker 转去读同目录陈旧 `task_*.md` 执行 → **先重启后写任务书**，inbox 只留三文件 |
| [WORKER-RESTART-MODEL-RESET-001] | 重启后模型回落到额度耗尽的免费档，看着活着实则永不响应 → 先切模型再注入；ACK_FAIL 先看是否 `Insufficient balance` |
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
| [E2E-COLD-START-RACE-001] | cold 启动后热键停止 / PTT 释放卡在 RECORDING → 首个 Start 清空 stop 信号的竞态，warm 态无故障，harness 预热即可 |
| [PYTEST-MACOS-COLLECT-001] | 仓库根裸跑 pytest 直接 INTERNALERROR → 递归进 CT 源码树 + Windows-only 导入，须限定 `tests/` 路径 |
| [PLAYWRIGHT-FIX-001] | 9 个 UI 测试全 ScopeMismatch → session 作用域 fixture 依赖了 module fixture，改同级 scope |
| [HAPPYDOM-ALTGR-INDISTINGUISHABLE-001] | happy-dom 把 AltGraph 直接映射到 `altKey` → 真按 Ctrl+Alt 与 AltGr 单测里无法区分，只能靠端测 |
| [TESTER-SCREENSHOT-FAIL] / [SCREENSHOT-METHOD-001] | 连续截到桌面背景或别的窗口 → 像素统计自验不可靠；须 MoveWindow + 置顶 + ShowWindow 三连后截固定区域 |

## ASR / overlay / 产品行为

| ID | 现象 → 判据 |
| --- | --- |
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
