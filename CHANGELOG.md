# CHANGELOG - 变更日志 (voice-ime)

> 任务编号 | 简要说明 | 负责人 | 完成时间
> 详细记录见 logs/YYYYMMDD.md

---

## v0.7.1 - Bug 修复 + 优化批次（2026-07-24）

> 本批次含 REBUILD-LOST-001（07-24 git 事故重做）+ v0.7.1 新增项。coder-1 负责的后端部分代码在 07-24 前已全部落地并验收，本表为文档闭环补录（上次 session 在验收记录环节被中断）。三处版本号已对齐 0.7.1。

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| REBUILD-LOST-001-BACKEND | git 事故后端重做 7 项（按原始顺序）：① FORMAT-LLM-001-CORE（build_format_instruction_block F1/F2/F3 + flatten_multiline + FormatFailed 事件 + i18n 三语 + version 0.6.2→0.7.0）② ITN-SMART-002（consumed>=2 算法根治单字保护 + itn-rules.toml [protect.historical] 95 条历史词）③ SCENE-SENSE-001-CORE（F4 场景注入+三道防线裁决+SceneConfig 隐藏字段）④ FMT-LLM-002（超时 [6s]→[8s,15s] + F3 OVERRIDES/MUST 两分支）⑤ FMT-LLM-004（防编造守卫 strip_fabricated_email_lines/is_fabricated_salutation/is_fabricated_closing 中英文）⑥ FMT-LLM-005（normalize_script_only 替代 fix_asr_english_case 保留 LLM 大小写）⑦ LANG-AUTO-001-CORE（contains_han 内容检测替代配置门控 + SCENE-AI-AGENT-001 CORE 4 处硬编码替换）；主控独立核实 7 项全命中 + cargo check 0 errors | coder-1 | 2026-07-24 |
| TEMP-CELSIUS-001 | 摄氏度符号识别：语音含"摄氏"关键词时输出 °C 符号，裸"度"不变；复核确认重做过程中未受影响 | coder-1 | 2026-07-24 |
| FMT-EMPTY-CORRECTED-001 | Bug 修复：空/噪声语音格式化返回字面量 "<corrected></corrected>"；实现位置在 optimize() 而非 try_once()（功能等价且避免与重试逻辑冲突，更合理）；单测 parse_empty_corrected_tag_returns_full_response_text 通过 | coder-1 | 2026-07-24 |
| FMT-EMAIL-I18N-001 | 邮件称呼/祝福语中英日韩优化：scene-rules.toml email style 补日韩称呼（拝啓 X様/X様/〇〇様 + X님/안녕하십니까 X님）+ 日韩祝福（よろしくお願いいたします/敬具 + 감사합니다/이상）+ 标点规则（日韩 ':' 而非逗号）；src/llm/mod.rs:757-843 防编造守卫 is_fabricated_salutation/closing 补日韩模式（四语言防护对称）+ 8 条新增单测（日韩各 4 条：salutation/closing 检测 + strip + keep_input）；依赖 FMT-LLM-004 先落地（已重做完成） | coder-1 | 2026-07-24 |
| FIX-REBUILD-REGRESSION-001 | ASR-HIDE-ACCURACY-001-CORE 迁移逻辑补回：REBUILD-LOST-001-BACKEND 重做过程中 asr_model_accuracy_roundtrip 测试断言方向被意外改回"保留accuracy"；src/config/mod.rs:370-434 补回 accuracy→performance 静默迁移逻辑（load + load_from 两路径，日志 "ASR-HIDE-ACCURACY-001: migrating legacy asr_model='accuracy' -> 'performance'"）+ 单测 asr_model_accuracy_migrates_to_performance / asr_model_performance_unchanged_by_migration 断言迁移方向正确 | coder-1 | 2026-07-24 |
| FORMAT-LLM-001-UI | LLM 设置页格式化输出 UI：Llm.tsx 更名"格式化输出"区块 + connectivity_verified 开启门槛校验（连接验证通过后才允许启用格式化）+ api_url/api_key/model 改动重置校验状态（防配置漂移后沿用过期验证）；src-tauri/src/llm.rs probe() 删除 enabled 死锁检查（格式化开关与连接校验解耦） | coder-2 | 2026-07-24 |
| FORMAT-UI-POLISH-001 | LLM 设置页样式精修：Llm.tsx API URL/Key 输入框独占整行（避免拥挤）+ 状态条独立成块 + styles.css 新增 llm-api-card 等 CSS 类（卡片化布局，视觉层次清晰） | coder-2 | 2026-07-24 |
| LANG-AUTO-001-UI | 语言自动 UI 清理：Voice.tsx 删输入语言区块 + HotkeySettings.tsx 删目标语言派生逻辑 + i18n 三语删 11 key + src-tauri/src/config.rs 加 Deprecated 注释 + **修复 src/crash:114 asr_model 误填 transcription_language 的 bug**（崩溃报告字段错填修复）+ Voice.test.tsx combobox 索引调整适配删除后的选项顺序 | coder-2 | 2026-07-24 |
| GIT-AUDIT-001 | git 仓库同步状态只读审计：核对本地代码是否完整提交并推送到 GitHub；严格 5 类只读命令（status/diff/log/show/fetch）无任何写操作；结论：远程领先本地=空✅，本地领先远程=1 提交 f2240b7 未推送，未提交改动经 -w 核实仅 CHANGELOG +3 行真实（其余 15 文件系 CRLF↔LF 行尾符差异误报），未跟踪文件 nth=1（0 字节误产物）；git 事故重做成果已完整落盘无丢失，唯一缺口=f2240b7 未推送 | coder-1 | 2026-07-24 |

---

## v0.6.2 - 词库单词化 + 智能数字规整（2026-07-10）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| ITN-SMART-001 | 智能数字规整模块：中文数字→阿拉伯数字（进位组合/小数点/逐位串），含时间/货币/单位/百分比/分数/序数/经纬度语境引擎 + 成语/专名/虚词保护 + 规则数据文件分离（itn-rules.toml 外置，DEC-030） | coder-1 | 2026-07-10 |
| WORDBOOK-SINGLEWORD-001-CORE | 词库单词化改造·后端核心：词对(raw→corrected)→单词(word)模式；migration 003 幂等迁移（corrected 侧去重导入）+ finalize 条件替换旧表；wordbook cache/db/mod 全部单词化 + 删除 apply() 文本替换；LLM SuggestionEntry 单词化 + prompt 词汇表 + 向后兼容旧格式解析；transcription curate/build/version 签名 &[String]；main.rs 移除 apply 步骤 + hotwords/learn 适配单词；8 文件改动，cargo check 0 errors + cargo test 357+9+24 全绿（含 4 迁移幂等性单测）；src-tauri 编译损坏属预期（Phase2 coder-2 跟进） | coder-1 | 2026-07-10 |
| WORDBOOK-SINGLEWORD-001-UI | 词库单词化改造·Tauri+UI 层：WordbookEntry 结构改为 {id,word,source,created_at}；add_wordbook_entry 单参数 word，删除无调用 delete_wordbook_entry 统一走 delete_wordbook_entry_by_id；src-tauri/src/i18n.rs wordbook 文案单词化；Wordbook.tsx 原词+修正词双输入框改为单「词条」输入框 + invoke 单参数；三语言 i18n 同步（wordbook_word/wordbook_word_placeholder/wordbook_add_hint 语义更新，删除原词/修正词相关 key）；Wordbook.test.tsx 适配 + 新增 ADD-UNIT-005 单参数断言；cargo check src-tauri 0 errors / Vitest 44/44 / npm run build PASS；未碰版本号与主 crate | coder-2 | 2026-07-10 |
| TEST-SYNC-062-001 | WORDBOOK-SINGLEWORD-001 + ITN-SMART-001 测试用例对齐：修复 wordbook_delete_tests.rs 4 处旧断言（validate_pair→validate_word/delete 锚点）；更新 test_wordbook_integration/llm/i18n 3 文件残留旧格式断言；新增 test_webview_ui.py TestWordbookPage 6 条 E2E 用例；残留扫描 7 模式全部清理；生产代码零改动 | tester-1 | 2026-07-10 |
| TEST-EXEC-062-001 | 0.6.2 全量出包：cargo test 404/0/6 + Tauri 38/0/0 + Vitest 44/44 全绿；构建三件套 0 errors；Publish 三 exe+itn-rules.toml 字节一致核验通过；ProductVersion 0.6.2.0/0.6.2；冒烟 Responding=True WS 309MB；migration 003 生效确认 (word 列, 4 条目)；Playwright 20 SKIP (CDP 未就绪) | tester-1 | 2026-07-10 |

## v0.6.1 - ASR accuracy 调参 E1+E2（2026-07-08）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| ASR-ACC-TUNE-001 | E1 temperature 0.3→0.1（Gavin 直接指定终值）+ E2 HOTWORDS_MAX_ENTRIES 50→20；仅 src/transcription/mod.rs 单文件，注释含决策出处，单测符号断言自动适配；performance/qwen3 零影响；自验 cargo test 369 全绿 + 主控独立 cargo check | coder-1 | 2026-07-08 |
| TEST-SYNC-ACC-TUNE-001 | 零改动——6 模式残留扫描零命中（测试资产无 hotwords 上限/temperature 硬编码），E2E 缺口论证无需补；主控独立扫描核验一致；result.md 首轮空文件（MSYS 路径写入静默失败 [COLLAB-WRITE-001]，随 EXEC 补填） | tester-1 | 2026-07-08 |
| TEST-EXEC-ACC-TUNE-001 | cargo test 405/0/8 无回归 + 仅主程序重建（1m51s 0 errors）+ Publish 同步 SHA256 一致 + ProductVersion 0.6.1.0 不变 + 冒烟 Responding=True（456MB，qwen3 配置零本地模型属预期）；UI/crash-reporter 未触碰 | tester-1 | 2026-07-08 |
| ~~RESEARCH-TEMP-SWEEP-001~~ | 已取消（Gavin 拍板直接定 0.1，不等扫描；中间数据保留 collab/research/） | - | 2026-07-08 |

## v0.6.1 - ASR accuracy 质量研究勘误（2026-07-08）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| RESEARCH-ASR-ACCURACY-003 | Gavin 真实语料（56s 自录朗读）双模型同源 A/B（Windows 生产同栈）：**未复现体感**——A2 native+VAD CER 0.0271 优于 A1 CTC 0.0724（低 63%），专有名词 93% vs 67%；实锤 native >28s 整段空输出（A3 para3 26.5s max_total_len 截断），反证生产 VAD 分段是必要防护（naive_chunk ≤20s 有单测）；归因第三分支，剩余嫌疑三层（a 应用采集链/b 短指令语料形态【最高】/c 标点后处理路径差异）；F1 DEBUG-AUDIO-DUMP + F2 短指令多样本待 Gavin 拍板，E1-E4 继续挂起；方法论教训：Python sherpa max_new_tokens=0 语义陷阱（Python=0 token vs Rust=不限） | coder-1 | 2026-07-08 |
| RESEARCH-ASR-ACCURACY-002 | accuracy vs performance 深挖研究（纯研究零生产改动）：**推翻 001 核心结论**——001 实验脚本漏传 --temperature（PoC 默认 1.0 vs 生产 0.3），native 数据被低估 20pp；生产等价条件实测 accuracy CER 0.15/first 85% 全面优于 CTC 0.3553/70%；生效性审计 6 项全 PASS（ASR-ACC-OPT-001 优化真实生效，todo 遗留项5"降级用 accuracy 参数"描述不符实际无 bug）；参数扫描给出 E1 temp 0.3→0.2（+2.5pp 低风险）/E2 hotwords 上限 50→20/E3 backtrack 100→50ms/E4 temp→0.0（需大样本验证）四方案待 Gavin 拍板；001 报告已加勘误注；方法论教训：PoC 脚本必须对照生产代码逐参数核对 | coder-1 | 2026-07-08 |

## v0.6.1 - DEC-028 Qwen3 在线 ASR 接入（2026-07-08）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| ASR-QWEN3-BACKEND-001 | qwen3-asr-flash-realtime WS 后端（qwen3_online 模块 457 行）：官方 Realtime 协议（session.update pcm+sample_rate+modalities+language 条件传参）+ 整段上传手动 commit + 静默超时 10s/硬上限 max(30s,音频×0.5) + 连接 5s 超时 + fail-fast 不降级；config 四字段；AsrModel 三值；qwen3 模式零本地模型（recognizer Option 化）；热重载感知 qwen3 配置变更+None 自愈（R2-4）；含 R1/R2 两轮修订 | coder-1 | 2026-07-08 |
| ASR-QWEN3-UI-001 | Voice 页单选框→下拉三选项（本地-快速/本地-长音频/Qwen3在线）+ 品牌色说明 + API Key 密码框/测试连接三态按钮 + 空 key 离开自动回退（prevModelRef+latestRef）+ 三语 i18n 10 键 + src-tauri config 同步与 test_qwen3_asr_connection command + accuracy 文案改长音频定位（消化 todo 遗留④）；含 R1（前端未落盘打回）/R2（回退双缺陷）两轮修订 | coder-2 | 2026-07-08 |
| BUG-QWEN3-CRYPTO-001 | P0 崩溃修复：rustls 0.23 CryptoProvider 未确定 panic（src-tauri 零 provider feature）；两 manifest +rustls ring 直接依赖 + 两进程 main() 入口 install_default + 幂等单测 | coder-1 | 2026-07-08 |
| BUG-QWEN3-WS-HANDSHAKE-001 | P0 修复：src-tauri 测试连接手工 Request::builder 缺 WS 握手头（Sec-WebSocket-Key 协议错误）→ into_client_request 标准写法 + build_ws_request 纯函数 + 握手头存在性单测 | coder-2 | 2026-07-08 |
| BUG-QWEN3-STATUS-001 | P0 修复：qwen3_online.rs 握手后 status.is_success()（仅 2xx）把 WS 升级成功 101 误判为"连接被拒绝"→ 删除误判块 + debug_assert(101)；Gavin debug 日志钉死 | Orchestrator | 2026-07-08 |
| TLS-ROOTS-FIX | P0 修复：两处 tungstenite 内部 feature __rustls-tls 无根证书库（RootCertStore 空，运行时 UnknownIssuer 必失败）→ rustls-tls-webpki-roots | Orchestrator | 2026-07-08 |
| TEST-SYNC-QWEN3-001 | 测试同步：残留扫描 4 模式全零；6 缺口 4 补 2 免（config serde 5 + preprocessing Qwen3Online 1 + from_config 2 断言 + Playwright 下拉 2 + conftest 三字段），生产零改动 | tester-1 | 2026-07-08 |
| TEST-EXEC-QWEN3-001~005 | 五轮测试执行与出包：终态 405/0/8 + Vitest 43/43 + 完整构建（001/003）与增量重建（002 根证书 / 004 UI 握手 / 005 主程序 101）+ Publish 同步 + 冒烟；004/005 期间守护 Gavin 端测现场（决策 B 不杀实例不碰 config）；构建中修 4 处编译阻塞（user-event 依赖/tsc 未用变量/feature 名/close API） | tester-1 | 2026-07-08 |
| UI-INPUT-STYLE-FIX | API Key 输入框 className input-field（无定义类）→ input，与下拉框同宽等高 | coder-2 | 2026-07-08 |
| 端测 | Gavin 全链端测通过（测试连接 + 热键转录，工作空间 endpoint wss://<workspace>.cn-beijing.maas.aliyuncs.com）；后续使用中发现问题另行立项 | Gavin | 2026-07-08 |

## v0.6.1 - B-002-FIX 下载引导卡修复（2026-07-07）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| ASR-DUAL-B-002-FIX | `<a target="_blank">` → invoke('open_url_in_browser') + URL 文本 `<code>` 渲染 + 独立复制按钮状态（copiedField） + 三语 i18n + 4 Vitest 用例（ASR-UI-009/010/011），npm build ✅ 35/35 PASS | coder-2 | 2026-07-07 |
| RESEARCH-ASR-ACCURACY-001 | accuracy 模型实测准确率反低于 performance 根因研究：16 组 A/B + 6 种前导静音曲线 × 3 模型，证实 R1 主因（生产前处理为 CTC 调优伤 native，silence curve native 50ms 掉 10pp vs CTC 2.5pp）+ R2 hotwords 全量灌入副作用（精选 80% vs 全量 60% vs 220条 0%）+ R3 native 固有 hallucination；上游调研 PR #3122 确认 hotwords prompt-based 吃 context 预算；优化方案 A（hotwords 精选）+B（accuracy 前处理适配）预期 ~57→~70% 与 CTC 持平；生产代码零改动 | coder-1 | 2026-07-07 |
| ASR-ACC-OPT-001 | accuracy 模型优化实施（方案 A+B）：方案 A build_hotwords_string 新增 curate_hotwords_entries（过滤纯 ASCII/超长词条 + 上限 50 按 id DESC 截断）+ 9 单测；方案 B 新增 select_preprocessing_params（Performance 50ms/200ms 字面零改动 / Accuracy 0ms/100ms）+ run_pipeline 运行时分支 + 4 单测；cargo check 0 errors / cargo test 306 passed 0 failed 3 ignored；自验方案 B native+hw 65→77.5%（+12.5pp 达标），方案 A 精选 ≥ 全量；performance 红线零改动 | coder-1 | 2026-07-07 |
| RESEARCH-ASR-CTC-OPT-001 | CTC 模型优化空间研究（纯研究 7 方向）：C1 silence head 0ms 比 50ms 高 2.5pp（72.5% vs 70%，证实 50ms 是旧 SenseVoice 遗产可落地+2.5pp）/ C2 blank_penalty 五档全 75% 无影响 / C3 CTC 不支持 hotwords（c-api 无字段）/ C4 ITN rule-fsts 未生效可落地体验收益 / C5 70% 错误是同音字 CTC 天花板 / C6 offline CTC 仅 greedy 无 beam / C7 无新模型版本；方案 P1 silence head 50→0ms 强烈推荐 + P2 ITN 中风险 + P3 清理遗产值零收益；生产代码零改动 | coder-1 | 2026-07-07 |
| ASR-CTC-OPT-001 | CTC 优化实施（P1+P3 交付，P2 撤销）：P1 silence head 50→0ms（select_preprocessing_params 800→0 + 6 测试更新 + 注释，+2.5pp 达标）/ P3 blank_penalty 0.5→0.0（C2 证实无影响零风险清理）/ P2 ITN rule-fsts 撤销（自验发现七→7 副作用对输入法有害，移除 rule_fsts 设置 + fst 资产移至 collab/research/ 留存 + resolve_itn_fst_path 函数+3 测试保留 #[allow(dead_code)] 供未来复用；智能 ITN 另行立项）；cargo check 0 errors / cargo test 313 passed 0 failed 3 ignored 无回归；accuracy 分支零改动 | coder-1 | 2026-07-07 |
| RESEARCH-ASR-HALLUC-ROOT-001 | native decoder 幻觉根因研究（6 方向纯研究）：根因 R1=LLM decoder 在声学不确定性下的 LM prior 接管+default temperature=1.0 放大（与 Whisper hallucination 同构）；D1 temperature=0.1 PoC 改善质量/0.3 推荐值；D2 VAD 质量正常；D3 上游调研网络受限；D4 Whisper 通法迁移可行（temp=0/compr_ratio/logprob/no_speech）；D5 段级语义校验不可行（logits 未暴露）；D6 hotwords 非根因；缓解 H1 temp=0.3+H2 阈值 12→8 强烈推荐；TTS 无法复现幻觉是核心瓶颈，需 Gavin 真实录音样本；生产代码零改动 | coder-1 | 2026-07-07 |
| ASR-HALLUC-FIX-001 | 幻觉缓解实施（H1+H2'）：H1 temperature 1.0→0.3（create_funasr_nano_recognizer 降温）；H2' is_language_anomaly 英文成分检测（zh 模式，≥20 chars 短文本跳过，长英文词≥4字母≥3个 或 超长英文词≥10字母≥1个→fallback）；校准数据：Gavin 幻觉样本触发/正常中英混说（iPhone/WiFi/bug/API/TODO/Windows/2品牌并列）放行/en/auto/ja 跳过；8 新增单测；原 12 字/s 阈值保留 | coder-1 | 2026-07-07 |
| TEST-SYNC-ASR-ACC-OPT-001 | 测试同步：审查方案 A 9 个 + 方案 B 4 个已有单测，评估 4 个候选缺口确认全部缺失；`src/transcription/mod.rs` 追加 4 个新测试用例（过滤不变性→版本号稳定 / 长度边界 10/11 字 / curate 层中英混合保留 / 上限截断保序 61→50），生产代码零改动 | tester-1 | 2026-07-07 |
| TEST-SYNC-CTC-OPT-001 | 测试同步：审查 P1（src/main.rs select_preprocessing_params PERF_SILENCE_HEAD 800→0） + P3（src/transcription/mod.rs blank_penalty 0.5→0.0）测试改动，评估 4 个候选缺口全部已覆盖，无需新增用例 | tester-1 | 2026-07-07 |
| TEST-EXEC-ASR-OPT-MERGED-001 | 合并测试执行+出包：cargo test 368/0/5（≥基线348）- cargo build --release 1m51s 0 errors - Publish 同步时间戳15:28一致 - ProductVersion 0.6.1.0 - 冒烟PID 24876 Responding=True 无crash - 全检通过产物就绪 | tester-1 | 2026-07-07 |
| TEST-SYNC-HALLUC-FIX-001 | 测试同步：审查 is_language_anomaly 8单测，评估4候选缺口（边界值/撇号词/兜底reason/数字串），补3测试函数5断言（边界值 / 撇号词不误杀 / 数字串token行为），缺口3无需补充（boolean即为fallback判据），生产代码零改动 | tester-1 | 2026-07-07 |
| TEST-EXEC-HALLUC-FIX-001 | 测试执行+出包：cargo test 379/0/5（≥基线368+11），仅主程序cargo build --release 1m16s 0 errors，ProductVersion 0.6.1.0不变，crash-reporter同步Publish成功，冒烟PID 25256 Responding=True；feiyin-ime.exe因Gavin -debug实例锁定未覆盖（红线不杀） | tester-1 | 2026-07-07 |
| HALLUC-FIX-PUBLISH-SYNC | 收尾同步+新包真实冒烟：Gavin关闭-debug后补Publish同步（19:36），启动无参实例PID 27012 Responding=True 759.4MB，测试实例已清理；红线全遵守 | tester-1 | 2026-07-07 |
| TEST-EXEC-VAD-SINGLEMODEL-001 | 测试执行+出包：cargo test 366/0/7（VAD reset + 单模型+去CTC兜底），仅主程序build --release 1m44s 0 errors，Publish同步21:07，冒烟PID 26648 Responding=True 759.1MB，红线全遵守 | tester-1 | 2026-07-07 |
| ASR-QWEN3-UI-001 | Qwen3 在线 ASR UI（DEC-028）：Voice 页 radio-group → select 三选项下拉（performance/accuracy/qwen3_online）+ 品牌色描述文字 + 消化 todo 遗留④ accuracy 文案改"长音频"定位 + Qwen3 API Key 输入（password）+ 测试连接按钮（invoke test_qwen3_asr_connection）+ 空 key 回退 prevModel（unmount 检测）+ 三语 i18n + 5 新增 Vitest 用例（ASR-UI-012~016），npm test 35/35 PASS | coder-2 | 2026-07-07 |

## v0.6.0 - ASR 双模型 + 正式出包（2026-07-06）

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| ASR-SWAP-A-001 | 默认模型直换 179MB FunASR Nano CTC（ensure_sensevoice_model 目录名更换），PoC bin 对照验证 blank_penalty 0.5 无副作用保留，5 语识别正常首字完整，cargo check 0 errors | coder-1 | 2026-07-06 |
| ASR-DUAL-B-001 | 后端双模型架构：AsrModel enum（performance/accuracy）+ Transcriber 重构 + 异步热重载（channel+后台线程+unsafe impl Send）+ accuracy 分支 native 模型 + config 层 hotwords（词库哈希版本号感知）+ hallucination 兜底（常驻 performance recognizer 重转）+ 14 单测，cargo test 314/0/4 | coder-1 | 2026-07-06 |
| ASR-DUAL-B-003 | Tauri 后端同步：src-tauri/config.rs AudioConfig 加 asr_model 字段（防 round-trip 丢字段）+ check_accuracy_model_ready command（{ready,model_dir,download_url} 接口契约），i18n 评估无需改动，cargo check src-tauri 0 errors | coder-1 | 2026-07-06 |
| HOTRELOAD-FIX-001 | 热重载并发防护修正（Orchestrator 验收发现并直接修正）：asr_reload_in_flight 标志 + channel 传 Result（失败回信号）+ active 状态统一 swap 更新 + Transcriber::language() getter，防 6s 构建窗口重复 spawn 并发加载 972MB 模型 | Orchestrator | 2026-07-06 |
| TEST-SYNC/EXEC-ASR-DUAL-001 | Rust 单测审查无缺口 + cargo test 314/0/4 + Vitest 32/0 + 完整出包 + Publish 同步 254MB 新模型；产物同步错误被验收拦截，BUILD-FIX 修正（feiyin-ime-ui.exe 正确同步、三处陈旧 voice-ime-ui.exe 删除） | tester-1 | 2026-07-06 |
| VERSION-BUMP-003 | 版本号 0.5.4 → 0.6.0（Gavin 指示）：根 Cargo.toml + src-tauri/Cargo.toml + tauri.conf.json，中文 productName 完好，cargo check 双侧 0 errors | coder-1 | 2026-07-06 |
| BUILD-RELEASE-0.6.0 | 0.6.0 完整出包：npm build + Tauri UI + 主程序全量构建，Step 4 cp 正确（feiyin-ime-ui.exe），Publish 三 exe 同步，ProductVersion 0.6.0 核实通过，冒烟 13s 无崩溃；文档债务补齐（result.md BUILD-FIX 记录 + troubleshooting BUILD-FIX-SYNC-001 + CHANGELOG） | tester-1 | 2026-07-06 |
| ASR-NATIVE-LONG-001 | accuracy 长音频空输出根因调查（max_total_len=512 KV cache 限制，~28s 以上截断生成 0 token）+ 兜底加固（空输出/hallucination/n-gram 环路检测→fallback→Err，绝不静默注入垃圾）+ is_repetitive_garbage 函数 + 8 单测 + 分段转录调研报告（推荐 VAD 分段），cargo check 0 errors / cargo test 323/0/4 | coder-1 | 2026-07-06 |
| ASR-LONG-AUDIO-001 | VAD 分段转录根治 native 长音频上限（DEC-026 路径A）：新增 src/transcription/vad.rs（VadSegmenter 懒加载 + 分段纯函数 + 14 单测）+ mod.rs accuracy 长音频(>24s) VAD 切分逐段转录拼接（段≤20s+200ms padding+三重兜底+拼接），performance 分支不碰，VAD 缺失降级单次转录，silero VAD 模型 643KB，PoC bin 验证 30/60/90s 切分正常，cargo test 337/0/4；附 RESEARCH-ASR-PUNCT-001 标点研究结论（CTC 无标点不可替代，native 自带标点 accuracy 可省） | coder-1 | 2026-07-06 |
| ASR-PUNCT-OPT-001 | accuracy 模式启用模型自带标点（跳过标点引擎）：transcribe_with_punct_info 返回(text,native_punctuated) + 标点决策追加!native_punctuated + strip_punctuation 函数（中英标点剥离，保守不剥小数点/URL）+ 12 单测，performance 零改动，兜底来源判定基于文本出处非配置，cargo test 348/0/5 | coder-1 | 2026-07-06 |
| TEST-EXEC-NATIVE-LONG-001 | 测试执行 + 出包：cargo test 323/0/4（+8 repetitive_garbage 单测）✅，仅主程序出包（11,065,856 B, 21:06），ProductVersion 0.6.0 核实通过，冒烟 13s 无崩溃；仅更新 feiyin-ime.exe，Tauri UI 沿用现有 | tester-1 | 2026-07-06 |

## v0.5.4 - research (2026-07-06)

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| RESEARCH-QWEN3ASR-001 | Qwen3-ASR-0.6B 替换可行性研究：结论观望（有条件 go），发现更优候选 FunASR Nano int8（179MB，支持 hotwords），sherpa-onnx 1.12.38 原生支持无需升级依赖；Qwen3-ASR 体积 938MB 超红线；附阶段二 PoC 设计 | coder-1 | 2026-07-06 |
| POC-QWEN3ASR-002A | FunASR Nano PoC：下载两套模型（179MB CTC+802.7MB原生）+ PoC bin（src/bin/poc_funasr_nano.rs）+ V1 RTF（native 4线程 0.185✅/2线程 0.223⚠️）+ V3 内存（native 1.6GB✅远低于预估4-5GB）+ hotwords 通路验证（config层实证纠正紫菜→酯✅，create_stream_with_hotwords对非transducer报错）+ 研究报告勘误 | coder-1 | 2026-07-06 |
| POC-QWEN3ASR-002A-FIX | PoC bin 增加 --model-dir 参数（002B 基线组加载生产模型），0 warnings，生产模型加载验证（zh.wav 丢"开"复现 FIRSTCHAR 痛点） | coder-1 | 2026-07-06 |
| POC-QWEN3ASR-002B | V2 hotwords 送气短词纠偏对比（40 wav × 4 组）：✅ PASS——(d) native+hotwords 首字 80% vs (a) 生产 70%（+10pp 达标）vs (c) native 62.5%（+17.5pp）；(b) 179MB CTC 黑马 75% 零风险；发现 native decoder hallucination 风险（qidian_v1 RTF 2.66 乱码）+ hotwords 推理延迟翻倍；报告 collab/research/poc-funasr-nano-B.md | tester-1 | 2026-07-06 |
| ASR-DUAL-B-002 | 配置界面 ASR 模型选择 + 下载引导：Voice.tsx 新增 performance/accuracy 单选 + check_accuracy_model_ready 调用 + 未下载提示卡（下载链接/目标目录/一键复制）+ 三语 i18n + 7 项 Vitest 用例，npm build 通过；Note：全量 Vitest 仍有 2 个 About.test.tsx 既有失败与本任务无关 | coder-2 | 2026-07-06 |
| TEST-SYNC-ASR-DUAL-B002 | 测试同步：Voice.test.tsx 审查补缺（新增 ASR-UI-008 useEffect 重检测）+ About.test.tsx 产品名期望修正（飞音智能语音输入/Feiyin Smart Voice Input）+ E2E 评估无需改动；边界遵守 ✅ | tester-1 | 2026-07-06 |
| TEST-SYNC-ASR-DUAL-001 + TEST-EXEC-ASR-DUAL-001 | 测试同步（Rust 单测 4 检查点全覆盖无缺口）+ 测试执行（cargo test 314/0/4 ✅ + Vitest 32/0 ✅）+ 完整出包（npm+Tauri UI+主程序）+ Publish 同步（含 254MB 新模型目录）+ 运行时冒烟通过 | tester-1 | 2026-07-06 |

## v0.5.4 - patch (2026-05-28)

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
| BUILD-RELEASE-VERSION-REVERT-001 | 版本号回退到 0.5.4 后完整出包（前端+Tauri UI+主程序全重建），两 exe winres ProductVersion 均为 0.5.4，cargo test 295/0/2，smoke 4/4，feiyin-ime.exe + feiyin-ime-ui.exe (16:43)，Publish/已同步 | tester-1 | 2026-05-28 |
| VERSION-REVERT-001 | 版本号回退 0.5.5 → 0.5.4（Cargo.toml + src-tauri/Cargo.toml + tauri.conf.json），cargo check 0 errors | coder-1 | 2026-05-28 |
| TEST-EXEC-VERSION-BUMP-002 | 0.5.5 完整出包（前端+Tauri UI+主程序全重建），两 exe winres 属性版本号均确认 0.5.5，cargo test 295/0/2，smoke 4/4，feiyin-ime.exe + feiyin-ime-ui.exe (21:05)，Publish/已同步 | tester-1 | 2026-05-27 |
| VERSION-BUMP-002 | 版本号 0.5.4 → 0.5.5（Cargo.toml + src-tauri/Cargo.toml + tauri.conf.json），cargo check 0 errors | coder-1 | 2026-05-27 |
| FIRSTCHAR-FIX-006 | R2+R3打包：前导静音规整（find_speech_onset_with_backtrack+静音头200→50ms）+ find_speech_anchor 回溯150ms，6新增测试+7旧测试更新，cargo check 0 errors / cargo test 295/0/2 | coder-1 | 2026-05-27 |
| FIRSTCHAR-FIX-005 | 降采样抗混叠根治送气清声母首字识别错误：resample_anti_alias（Hann窗sinc低通+FIR多相），RecordingState改为存储原生采样率，collect_recording末尾整段重采样，max_frames/log/capacity修复，find_speech_anchor窗口缩放，7新增测试+1重命名+6参数更新，cargo check 0 errors / cargo test 289/0/2 | coder-1 | 2026-05-27 |
| TEST-EXEC-FIRSTCHAR-004 | FIRSTCHAR-FIX-004 出包：feiyin-ime.exe 10.99MB（12:47 构建，含修正）+ Publish 同步；orchestrator 独立验证 cargo test 282/0/4，audio 32/32 含 2 个 D3 新测试全 PASS（tester-1 kimi-k2.6 卡死，由 orchestrator 接管验证） | orchestrator | 2026-05-26 |
| FIRSTCHAR-FIX-004-REVIEW | 验收修正：drain cutoff 由 record_start（ensure_stream 后捕获）改为 t_record（函数开头/热键触发时刻），消除冷启动重建期间首字被误清隐患，cargo check 0 errors | orchestrator | 2026-05-26 |
| FIRSTCHAR-FIX-004 | D3 时间戳精确清空：channel chunk 携带 Instant 时间戳（type AudioChunk），idle drain 只清热键触发前 chunk，精确保留热键后首字音频，collect_recording 新增 post_hotkey_chunks 参数，5 测试更新+2 新增 | coder-1 | 2026-05-26 |
| TEST-EXEC-FIRSTCHAR-003 | 仅主程序出包：FIRSTCHAR-FIX-003，cargo test 282/0/2，smoke 4/4，feiyin-ime.exe 10.99MB，Publish/已同步 | tester-1 | 2026-05-26 |
| TEST-EXEC-FIRSTCHAR-002 | 仅主程序出包：FIRSTCHAR-FIX-002，cargo test 282/0/2，smoke 4/4，feiyin-ime.exe 10.99MB，Publish/已同步 | tester-1 | 2026-05-25 |
| BUILD-RELEASE-20260525 | 出包：FIRSTCHAR-FIX-001 + TEST-WRITE-FIRSTCHAR-001 + I18N-FIX-EN-001，286 PASS / 0 FAIL / 2 IGNORED，smoke 4/4，feiyin-ime.exe 10.99MB / feiyin-ime-ui.exe 8.56MB / crash-reporter.exe 23.68MB，Publish/已同步 | tester-1 | 2026-05-25 |
| TEST-EXEC-FIRSTCHAR-001 | 首字识别全量测试执行：cargo test 286/0/2，smoke 4/4，时间戳验证 ✅，全回归通过 | tester-1 | 2026-05-25 |
| TEST-WRITE-FIRSTCHAR-001 | 首字识别测试收紧：1 旧断言收紧(cleared_samples<budget) + 4 边界测试，cargo test 261/0/2 | coder-1 | 2026-05-25 |
| TEST-SYNC-FIRSTCHAR-001 | 首字识别测试同步审查：审查 5 个现有用例，建议收紧 1 断言 + 设计 4 新边界用例 | tester-1 | 2026-05-25 |
| FIRSTCHAR-FIX-003 | idle_clear 改为无限清空（full drain），消除 256-channel 满载时 196 陈旧 chunk 残留，cargo check 0 errors，test 261/0/4 | coder-1 | 2026-05-25 |
| FIRSTCHAR-FIX-002 | idle_clear 从样本预算改为 chunk 数量匹配（消除 WASAPI chunk size 不对齐导致多清一个 chunk），cargo check 0 errors，test 261/0/4 | coder-1 | 2026-05-25 |
| I18N-FIX-EN-001 | 修复 EN Strings 缺少 8 字段导致 Tauri 编译失败，cargo check 0 errors | coder-1 | 2026-05-25 |
| TEST-WRITE-FIRSTCHAR-001 | 首字识别测试收紧：1 旧断言收紧(cleared_samples<budget) + 4 边界测试(short_buffer/exact_boundary/all_speech/exact_budget)，cargo test 261/0/2 | coder-1 | 2026-05-25 |
| FIRSTCHAR-FIX-001 | 首字识别修复：idle_cleared 限量清空 + prime trim 保头部 + find_speech_anchor 函数，cargo check 0 errors，test 259/0/2 | coder-1 | 2026-05-25 |
| RESEARCH-FIRSTCHAR-001 | 首字识别不稳定根因研究：6候选+5方向，C1竞争窗口最高概率，报告输出 result.md | coder-1 | 2026-05-25 |
| TEST-EXEC-PREROLL-001 | PREROLL-RINGBUF-001 全量测试 + 出包：273 PASS / 0 FAIL / 2 IGNORED，smoke 4/4，feiyin-ime.exe 10.99MB，Publish/ 同步 | tester-1 | 2026-05-23 |
| TEST-SYNC-PREROLL-001 | 新增 3 个 audio 单元测试（ring buffer 淘汰最旧/channel drain 语义验证） | tester-1 | 2026-05-23 |
| PREROLL-RINGBUF-001 | 首字丢失根治修复：WarmInputStream 引入环形缓冲区，drain_pre_roll 改读 Mutex<VecDeque>，record() 开始时清空录音 channel，cargo check 0 errors | coder-1 | 2026-05-23 |
| TEST-SYNC-RENAME-001 | 测试文件同步旧 exe 名替换（voice-ime→feiyin-ime），4 个 Python 测试文件精确替换，config 目录路径保护，未执行构建/测试 | tester-1 | 2026-05-14 |
| TITLEBAR-ICON-FIX-001 | Tauri setup hook 加 set_icon() 设置标题栏橙色麦克风图标，cargo check 0 errors | coder-1 | 2026-05-14 |
| ICON-EMBED-001 | feiyin-ime.exe 嵌橙色麦克风图标 + feiyin-ime-ui.exe 用齿轮 ICO，cargo check 0 errors | coder-1 | 2026-05-14 |
| VERSION-BUMP-001 | 版本号 0.5.3 → 0.5.4（Cargo.toml + src-tauri/Cargo.toml + tauri.conf.json），cargo check 0 errors | coder-1 | 2026-05-14 |
| VERSIONINFO-FIX-001 | 移除 src-tauri/build.rs winres（CVT1100 资源冲突），cargo check 0 errors | coder-1 | 2026-05-14 |
| RENAME-AND-VERSIONINFO-001 | exe 重命名 voice-ime→feiyin-ime + Windows 版本信息嵌入（winres），cargo check 0 errors | coder-1 | 2026-05-14 |
| UI-CHECK-BTN-COLOR-001 | 检查更新按钮文字改橘色 #ff6b35，npm build/tsc 通过，暂不出包 | coder-2 | 2026-05-14 |
| UI-VERSION-CARD-HEIGHT-001 | About 版本卡片增加 minHeight:150px，npm build/tsc 通过，暂不出包 | coder-2 | 2026-05-14 |
| UI-VERSION-CARD-SIZE-001 | About 版本卡片放大：fit-content → minWidth:240px + justifyContent:center，npm build/tsc 通过，暂不出包 | coder-2 | 2026-05-14 |
| UI-ABOUT-STRINGS-001 | About 页 i18n 文案更新：3 语 × 3 key 共 9 处替换（品牌名加 Smart + 副标题改新文案），npm build/tsc 通过，暂不出包 | coder-2 | 2026-05-14 |
| UI-VERSION-CARD-SPACING-001 | About 版本卡片内部间距收窄：fit-content + gap:8px，移除嵌套层，npm build/tsc 通过，暂不出包 | coder-2 | 2026-05-14 |
| BUILD-RELEASE-20260514H | 出包：ICON-EMBED-001 + UI-VERSION-CARD-SIZE-001，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，feiyin-ime.exe 10.98MB (19:28) / feiyin-ime-ui.exe 8.56MB (19:22) / crash-reporter.exe 24.84MB (19:27)，Publish/已同步 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514I | 出包：TITLEBAR-ICON-FIX-001 + UI-VERSION-CARD-HEIGHT-001 + UI-CHECK-BTN-COLOR-001，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，feiyin-ime.exe 10.98MB (21:03) / feiyin-ime-ui.exe 8.76MB (21:04) / crash-reporter.exe 24.84MB (21:03)，Publish/已同步 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514G | 出包：RENAME + UI-STRINGS + UI-SPACING + LOGO + TEST-SYNC-RENAME，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，feiyin-ime.exe 10.89MB (18:26) / feiyin-ime-ui.exe 8.65MB (18:24) / crash-reporter.exe 24.74MB (18:25)，Publish/已同步 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514F | 最终合包：OVERLAY-FOCUS-FIX-001 + UI-ABOUT-FIX-001，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，voice-ime-ui.exe 18.66MB (16:00) / voice-ime.exe 10.89MB（沿用）/ crash-reporter.exe 24.74MB（沿用），Publish/已同步 | tester-1 | 2026-05-14 |
| OVERLAY-FOCUS-FIX-001 | 录音 overlay 加 WS_EX_NOACTIVATE + SW_SHOWNA，弹出录音窗口不再抢焦，目标应用焦点正常保持 | orchestrator | 2026-05-14 |
| UI-ABOUT-FIX-001 | About 版本卡片 280→380px + 移除侧边栏齿轮图标按钮，npm build/tsc 通过 | coder-2 | 2026-05-14 |
| BUILD-RELEASE-20260514E | 出包：OVERLAY-FOCUS-FIX-001（后端），270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，voice-ime.exe 10.89MB (15:52)，Publish/已同步 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514D | 出包：ESC-CANCEL-FIX-001 + open_url_in_browser macOS 修复，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，voice-ime.exe 10.89MB / crash-reporter.exe 24.74MB (14:03)，Publish/已同步 | tester-1 | 2026-05-14 |
| ESC-CANCEL-FIX-001 | GetAsyncKeyState ESC 检测位修复：0x0001→0x8000u16，消除录音前 ESC 残留 bit 导致 cancel_signal 误触发 | coder-1 | 2026-05-14 |
| CROSSPLATFORM-FIX-001 | open_url_in_browser 加 macOS cfg 分支：Windows 用 cmd /C start，macOS 用 open <url>，cargo check 0 errors | orchestrator | 2026-05-14 |
| BUILD-RELEASE-20260514C | 出包：PIPELINE-CANCEL-FIX-001（诊断日志），270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，voice-ime.exe 10.89MB / voice-ime-ui.exe 18.66MB / crash-reporter.exe 24.74MB (13:32)，Publish/已同步 | tester-1 | 2026-05-14 |
| PIPELINE-CANCEL-FIX-001 | 录音后 cancel_signal 静默跳过转录的诊断日志：worker cancel warn + run_pipeline debug + pipeline cancel warn，cargo check 0 errors | coder-1 | 2026-05-14 |
| BUILD-RELEASE-20260514B | 出包：VERSION-CHECK + MIC-MUTE-DETECT-001，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，voice-ime.exe 10.88MB / voice-ime-ui.exe 18.66MB / crash-reporter.exe 24.74MB (12:52~12:54) | tester-1 | 2026-05-14 |
| TEST-EXEC-VERSION-CHECK-001 | 版本检查全量测试：270 PASS / 0 FAIL / 2 IGNORED，version_check 13 新增单测全 PASS，npm build 0 errors，无回归 | tester-1 | 2026-05-14 |
| TEST-SYNC-VERSION-CHECK-001 | 版本检查测试同步：主程序补 4 单测（共 12）+ Tauri 侧新建 9 单测，cargo check 0 errors | tester-1 | 2026-05-14 |
| VERSION-CHECK-BACKEND | 后端版本检查：主程序后台线程 GitHub API 检查 + Tauri 3 个 IPC command + 8 个单测，cargo check 0 errors | coder-1 | 2026-05-14 |
| PIPELINE-CANCEL-FIX-001 | cancel_signal 静默跳过转录修复：3 处诊断日志，cargo check 0 errors | coder-1 | 2026-05-14 |
| ESC-CANCEL-FIX-001 | GetAsyncKeyState ESC 检测位修复：0x0001 → 0x8000u16，消除残留 bit 误触发 cancel_signal | coder-1 | 2026-05-14 |
| TASK-UI-I18N-BACKEND | i18n 测试补充：TraditionalChinese 序列化往返 + 三语字符串覆盖，253 PASS / 0 FAIL | coder-1 | 2026-05-14 |
| I18N-ZH-FIX-001 | ZH 静态字串 error_transcription_empty 繁→簡修復驗證：cargo check 0 errors | coder-1 | 2026-05-13 |
| VERSION-CHECK-UI | About 页面集成版本检查 UI：状态机 + get_version_info 缓存读取 + force_check_latest_version 手动重检 + open_url_in_browser 下载，三语 i18n，npm build/tsc 通过 | coder-2 | 2026-05-14 |
| MIC-MUTE-DETECT-001 | 麦克风静音探测：is_mic_muted() Win32 API 检测 + Start 处拦截 + 录音中周期检测 + i18n 三语 + convert_to_friendly_error 匹配 | coder-1 | 2026-05-13 |
| TEST-SYNC-MIC-MUTE-001 | 麦克风静音探测测试同步：补充 3 个单测（50 chunk 间隔/非 Windows 返回 false/三语 i18n 非空），cargo check 0 errors | tester-1 | 2026-05-13 |
| TEST-EXEC-MIC-MUTE-001 | 麦克风静音探测测试执行：cargo test 250 PASS / 0 FAIL / 2 IGNORED，新增 3 个测试全部 PASS，无回归 | tester-1 | 2026-05-14 |
| TASK-UI-OPT-005 | 5项UI优化：LLM提示文字+语音标签文字+About改造+繁体中文+i18n重构（7文件+4新文件），npm build/test 0 errors 24 PASS | coder-2 | 2026-05-07 |
| I18N-TW-001 | 后端繁体中文支持：UiLanguage 枚举新增 ChineseTraditional + src/ src-tauri/ 双端 ZH_TW i18n 字符串表 + crash/reporter 繁体文案 | coder-1 | 2026-05-07 |
| CS-OPT-002 | 语音输入设置页增加输入语言选项：Voice.tsx 新增输入语言 section（中/英/日/韩/粤），复用 audio.transcription_language 字段 | coder-2 | 2026-05-07 |
| CS-OPT-001 | 中英混合识别优化：ASR language 传递（config→Transcriber→SenseVoice）+ blank_penalty=0.5 + LLM CODESWITCH_FIX（全语言英文拼写还原） | coder-1 | 2026-05-07 |
| TEST-FIX-SETUP-MOCK | 测试 mock 修复：transcription_language "auto"→"zh" 匹配 UI 选项 | coder-1 | 2026-05-07 |
| BUILD-RELEASE-20260507D | 前后端完整构建链：cargo test 230 PASS + Vitest 24 PASS + 冒烟 4/4，voice-ime.exe 10.25MB / voice-ime-ui.exe 17.69MB / crash-reporter 23.59MB (23:49) | tester-1 | 2026-05-07 |
| BUILD-RELEASE-20260507C | 输入语言 UI radio-card→select 下拉框（纯前端）：Vitest 24 PASS + Tauri 15 PASS，voice-ime-ui.exe 17.69MB (20:57)，主程序沿用 | tester-1 | 2026-05-07 |
| BUILD-RELEASE-20260507B | CS-OPT-001/002 代码切换优化出包：cargo test 230 PASS + Vitest 24 PASS，voice-ime.exe 10.24MB (19:42)，冒烟 4/4 PASS | tester-1 | 2026-05-07 |
| BUILD-RELEASE-20260507A | Release 出包：TRANS-REGRESSION-001 + RECORDING-PARAMS-001，cargo test 230 PASS，voice-ime.exe 10.24MB (14:06)，冒烟 4/4 PASS | tester-1 | 2026-05-07 |
| OVERLAY-LOCK-SCOPE-001 | overlay 锁范围缩小：draw_recording_overlay 波形绘制段锁内只做快照+衰减，GDI 绘制释放锁后执行；麦克风颜色段锁内只读布尔状态 | coder-1 | 2026-05-08 |
| HOTKEY-STREAM-PREWARM-001 | 流预热检测：AudioCapture.check_stream_health() + worker 线程 recv→recv_timeout 空闲期周期性预重建失败 WASAPI 流 | coder-1 | 2026-05-08 |
| RESEARCH-CS-001 | SenseVoice 中英混合优化研究：hotwords 不可用(CTC)，推荐参数调优 "auto"→"zh" + Paraformer trilingual 替代 + LLM 提示词修复 | coder-1 | 2026-05-07 |
| RECORDING-PARAMS-001 | 录音时长+静默超时调整：MAX_RECORD_SECONDS 180→300、SILENCE_DURATION_MS 8000→30000 | coder-1 | 2026-05-07 |
| TRANS-REGRESSION-001 | 本地翻译两回归修复：①空格丢失改用 tokenizer.decode() 替换 join+normalize；② MAX_DECODE_STEPS 256→512 修复截断 | coder-1 | 2026-05-07 |
| HOTKEY-LATENCY-FIX-001 | 热键录音视觉延迟修复：HotkeyEvent::Start 立即 show_overlay(Recording)（消除 200ms 卡顿）+ drain_pre_roll 改为循环收集至 PRE_ROLL_MS 目标量或 350ms 超时（改善偶发首字丢失） | coder-1 | 2026-05-06 |
| TEST-SYNC-HOTKEY-LATENCY-001 | 热键延迟修复测试同步：提升 2 个模块常量至模块级（PRIME_TIMEOUT_MS/TICK_MS）+ 新增 3 个单测锁定常量契约，HOTKEY-LATENCY-FIX-001 覆盖完成 | tester-1 | 2026-05-06 |
| TEST-SYNC-PUNCT-SUGGEST-001 | PROMPT-PUNCT-REVAMP + WORDBOOK-SUGGEST-FIX 测试同步：更新/新增 5 个单测（标点语义精确匹配、MUST 指令无条件追加、last-line fallback），导入扩增 | tester-1 | 2026-05-06 |
| TEST-EXEC-PUNCT-SUGGEST-001 + BUILD-RELEASE-20260506D | 全量测试 174 PASS + Release 出包：voice-ime.exe 10.23MB + crash-reporter.exe 23.58MB（23:58-23:59），冒烟 4/4 PASS | tester-1 | 2026-05-06 |
| BUILD-RELEASE-20260506E | Release 构建：HOTKEY-LATENCY-FIX-001 + cargo test 229 PASS，voice-ime.exe 10.24MB (01:00) + crash-reporter 23.58MB (00:59)，冒烟 4/4 | tester-1 | 2026-05-07 |
| PUNCT-INTEGRATION-001-UI |
| PUNCT-INTEGRATION-001 |
| WAVEFORM-FIX-002 + SHIMMER-SPEED-002 + PROMPT-PUNCT-FIX-001 | 波形索引修复(center=newest)+边缘先落加权衰减+shimmer 800ms+LLM标点开关 | coder-1 | 2026-05-06 | 标点补全后端集成：PunctuationConfig+PunctuationEngine模块+pipeline条件调用(LLM未处理+未翻译时)+LLM提示词Rule2降级+模型部署 | coder-1 | 2026-05-06 | 标点补全 UI 开关+Tauri 配置同步：Voice 页面 toggle 开关+AppConfig/PunctuationConfig 双端配置 | coder-2 | 2026-05-06 |
| MIC-ICON-ENLARGE-001 | 录音 overlay 麦克风图标放大：circ_size 14→18px, 胶囊体 22→28px, 4x超采样 56→72, 左分隔线适配 | coder-1 | 2026-05-06 |
| AUDIO-PREROLL-FIX-001 | 录音首字丢失修复：PRE_ROLL_MS 300→500ms + drain空时200ms prime等待 + transcribe前1600零样本静音头 | coder-1 | 2026-05-06 |
| RESEARCH-PUNCT-001 | 本地标点符号补全方案研究：4方向评估+对比表+TOP2推荐（ct-transformer+规则引擎） | coder-1 | 2026-05-06 |
| RESEARCH-PUNCT-002 | CT2格式中文标点模型调查：不存在CT2标点恢复模型，推荐sherpa-onnx ONNX路径（72MB INT8） | coder-1 | 2026-05-06 |
| POC-PUNCT-001 | anchor-flux 72MB标点模型PoC验证：sherpa-onnx OfflinePunctuation兼容成功，加载229ms推理2ms，9/9通过 | coder-1 | 2026-05-06 |
| TEST-SYNC-MIC-AUDIO-001 | MIC-ICON-ENLARGE + AUDIO-PREROLL-FIX 测试同步：5 个新增单测（图标布局/静音头/prime条件/常量值），cargo check ✅ | tester-1 | 2026-05-06 |
| TEST-SYNC-PUNCT-001 | 标点集成测试同步：Rust 10 个单测（英文半角转换/配置序列化/旧配置兼容）+ Vitest 7 个（Voice toggle 渲染/交互），cargo check + 16/16 PASS + 7/7 PASS | tester-1 | 2026-05-06 |
| TEST-EXEC-PUNCT-001 | 全量 cargo test 208 PASS / 0 FAIL + 16/16 标点专项 + 7/7 Vitest，无回归 | tester-1 | 2026-05-06 |
| BUILD-RELEASE-20260506B | Release 出包（前后端）：voice-ime.exe 10.21MB + crash-reporter 23.59MB + voice-ime-ui 17.69MB（均 18:46/18:47），冒烟 4/4 PASS | tester-1 | 2026-05-06 |
| BUILD-RELEASE-20260506A | Release 出包：voice-ime.exe 10.21MB (13:51) + crash-reporter.exe 23.58MB (13:51)，冒烟 4/4 PASS | tester-1 | 2026-05-06 |
| TEST-SYNC-OVERLAY-FIX-006 | OVERLAY-FIX-006 测试同步：13 个新增单测（边框加深/动效重写/按钮尺寸/颜色分离），等待 TEST-EXEC | tester-1 | 2026-05-05 |
| --- | --- | --- | --- |
| OVERLAY-FIX-007 | 处理中动效v3（2px橘色底边扫描线）+ 四窗口边框再加深70%（0x171513→0x060607） | coder-2 | 2026-05-05 |
| OVERLAY-FIX-006 | 7项 Overlay 视觉修复：边框加深一倍/3层实色矩形动效/shimmer单向0→1/预览按钮边框统一/按钮缩小25%/关闭按钮文字改灰 | coder-2 | 2026-05-05 |
| WAVEFORM-FIX-001 | 波形自由落体过渡 + 中心频谱加权：FallingToProcessing 状态 + GRAVITY_RATE=0.25 衰减 + cos² 中心权重 | coder-1 | 2026-05-04 |
| TEST-EXEC-OVERLAY-FIX-003 | 测试执行：149 PASS / 0 FAIL（+3 band_width 单测） | tester-1 | 2026-05-04 |
| TEST-SYNC-OVERLAY-FIX-003 | 测试用例编写：OVERLAY-FIX-003 band_width 计算 3 个单测 + 目视验收建议 | tester-1 | 2026-05-04 |
| BUILD-RELEASE-20260504C | Release 构建 + 测试：OVERLAY-FIX-002 出包，146 tests PASS，voice-ime.exe 10.20MB (18:09) | tester-1 | 2026-05-04 |
| TEST-EXEC-OVERLAY-FIX-002 | 三角波 shimmer 相位计算单测：新增 4 个 Rust 单测，146 PASS / 0 FAIL | tester-1 | 2026-05-04 |
| TEST-SYNC-OVERLAY-FIX-002 | 测试用例对齐分析：麦克风图标 GDI 参数不可自动化，三角波计算可提取单测 | tester-1 | 2026-05-04 |
| BUILD-RELEASE-20260504B | Release 构建 + 测试：补完 UI 改动出包，142 tests PASS，voice-ime.exe 10.20MB (17:33) | tester-1 | 2026-05-04 |
| I18N-ERROR-001 | 错误提示文本本地化：convert_to_friendly_error 接入 i18n，4 类错误按 UI 语言显示 | coder-2 | 2026-05-04 |
| UI-OPT-004 | 录音窗口指示灯改为麦克风图标（GDI 手绘）：胶囊体+弧形支架+杆+底座，三态颜色不变 | coder-1 | 2026-05-04 |
| UI-OVERLAY-OPT-001 | Overlay 窗口视觉优化：波形中间展开、边框统一加深、处理中 GradientFill 光扫、错误窗口小红圆+橘色文字 | coder-2 | 2026-05-04 |
| OVERLAY-FIX-005 | 处理中动效同步修复(phase移入WM_PAINT)+光带恢复2/3宽+预览窗口5项橘色优化 | coder-2 | 2026-05-04 |
| OVERLAY-FIX-004 | 麦克风图标简化(宽椭圆22×38+杆+底座)+处理中光晕端点改背景色(黑→#181A18) | coder-2 | 2026-05-04 |
| OVERLAY-FIX-003 | 四项 Overlay 修复：窄光带(1/5宽)+Chord填实麦克风图标+错误/预览圆角对齐(16→10) | coder-2 | 2026-05-04 |
| OVERLAY-FIX-002 | 麦克风图标断裂修复 + 处理中动效淡银白光晕平滑滑动：坐标对齐+三角波周期2.0+AlphaBlend 140 | coder-2 | 2026-05-04 |
| BORDER-DARKEN-001 | 录音窗口边框加深一倍：BORDER_GRAY/CIRC_BORDER 0x181910 → 0x0C0C08 | coder-2 | 2026-05-04 |
| PERF-BATCH-001 | 性能优化 6 合 1：Transcriber 预初始化 / MsgWaitForMultipleObjects / show_overlay 去 RwLock / 托盘 i18n / 三态指示灯 / prewarm 启动预热 | coder-1 + 咖啡 | 2026-05-03 |
| PERF-INIT-001 | LlmClient + TranslationEngine 预初始化优化：复用 reqwest Client、热重载缓存避免重复加载 CT2 模型 | coder-1 | 2026-05-04 |
| TEST-SYNC-PERF-INIT-001 | 预初始化测试同步：新增 LlmClient update_config 3 个测试 + TranslationEngine needs_reload 6 个测试 | tester-1 | 2026-05-04 |
| BUILD-PERF-BATCH-001 | Release 构建 + 测试：PERF-BATCH-001 5 项优化，133 tests PASS，voice-ime.exe 10.20MB (22:11) | tester-1 | 2026-05-03 |
| TEST-SYNC-PERF-BATCH-001 | 性能优化测试同步：修复 i18n 字段缺失，133 tests PASS | tester-1 | 2026-05-03 |
| BUILD-RELEASE-20260502E | 集成构建验证：WAVEFORM-HEIGHT-FIX-001，133 tests PASS，voice-ime.exe 10.20MB (00:58) | tester-1 | 2026-05-03 |
| WAVEFORM-HEIGHT-FIX-001 | 波形条高度增大 + 处理中边框恢复固定灰色 | coder-2 | 2026-05-02 |
| OVERLAY-UI-TUNE-001 | 录音窗口5项UI调整 | coder-2 | 2026-05-02 |
| PROCESSING-SHIMMER-001 | 处理中窗口 Slim Shimmer 效果 | coder-2 | 2026-05-02 |
| WAVEFORM-001 | 波形峰值保持 + 60fps | coder-2 | 2026-05-02 |
| LATENCY-001 | Controller 响应延迟优化 | coder-1 | 2026-05-02 |
| STOP-BUTTON-CENTER-FIX-001 | 停止按钮居中修复 | coder-2 | 2026-05-02 |
| OVERLAY-ADJUST-001 | 录音窗口7项精细调整 | coder-2 | 2026-05-02 |
| OVERLAY-UI-FIX-001 | 录音窗口UI基础优化 | coder-2 | 2026-05-02 |
| OVERLAY-FINAL-TUNE-001 | 录音窗口5项最终调整 | coder-2 | 2026-05-02 |
| HALFTONE-AA-001 | 实心圆HALFTONE抗锯齿 | coder-2 | 2026-05-02 |
| OVERLAY-SPECTRUM-FIX-001 | 频谱效果恢复 | coder-2 | 2026-05-02 |
| OVERLAY-WAKE-001 | overlay线程响应优化 | 咖啡 | 2026-05-02 |
| PROCESSING-UI-001 | 处理中窗口UI优化 | 咖啡 | 2026-05-02 |
| OVERLAY-FLICKER-FIX-001 | overlay闪烁根治 | 咖啡 | 2026-05-02 |
| RECORDING-OVERLAY-REDESIGN-002 | 录音窗口重设计 | coder-2 | 2026-05-02 |

---

## v0.5.3 - 已完成（2026-04-28）

| 功能 / 修复 | 完成日期 |
| --- | --- |
| 翻译热键 + 翻译功能 | 2026-04-28 |
| opus-mt 双向离线翻译引擎 | 2026-04-28 |
| UI 热键设置页重构 | 2026-04-28 |
| BUILD-RELEASE-20260505A | OVERLAY-FIX-006 出包：voice-ime.exe 10.21MB (11:15)，crash-reporter.exe 23.58MB (11:15)，voice-ime-ui.exe 沿用 17.68MB | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505B | OVERLAY-FIX-006 v2 出包：voice-ime.exe 10.20MB (12:02)，crash-reporter.exe 23.58MB (12:01)，cargo test 142 PASS/0 FAIL | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505C | OVERLAY-FIX-007 + I18N-EMPTY-001 出包：voice-ime.exe 10.20MB (16:51)，cargo test 142 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505D | SHIMMER-VISUAL-003 | 处理中动效：4层离散AlphaBlend → 30薄条高斯渐变软光晕(GLOW_HALF=45,alpha=exp(-3t²)*200) | coder-1 | 2026-05-05 |
| SHIMMER-VISUAL-002 | 处理中动效：3层实色矩形 → 4层 AlphaBlend 半透明银白光晕(30/90/160/220) | coder-1 | 2026-05-05 |
| SHIMMER-VISUAL-001 | 处理中动效：底边2px橘色线 → 全高度三层银白光晕滑动(±35/0x606060+±20/0x909090+±8/0xD8D8D8) | coder-1 | 2026-05-05 |
| SHIMMER-FIX-002 | 处理中动效闪烁根治：shimmer_phase 改为时间戳驱动，与 WM_PAINT 频率解耦 | coder-1 | 2026-05-05 |
| SHIMMER-FIX-001 出包：voice-ime.exe 10.20MB (18:32)，cargo test 142 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505E | SHIMMER-FIX-002 + SHIMMER-VISUAL-001 出包：voice-ime.exe 10.20MB (19:04)，cargo test 142 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505F | SHIMMER-VISUAL-002 出包：voice-ime.exe 10.20MB (19:26)，cargo test 140 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505G | SHIMMER-VISUAL-003 出包：voice-ime.exe 10.20MB (21:18)，cargo test 140 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| BUILD-RELEASE-20260505H | 透明度200→150 + 掃動速度2000ms週期：voice-ime.exe 10.20MB (21:43)，cargo test 140 PASS/0 FAIL/2 IGNORED | tester-1 | 2026-05-05 |
| MIC-ICON-ENLARGE-001 | 录音窗口麦克风图标放大：circ_size 14→18px，circ_l rect.left+8→+6，胶囊体 28px 宽 | coder-1 | 2026-05-06 |
| AUDIO-PREROLL-FIX-001 | 热键录音首字丢失修复：PRE_ROLL_MS 300→500ms + WASAPI idle prime 200ms 等待 + transcribe 前插入 100ms 静音头（1600 样本） | coder-1 | 2026-05-06 |
| BUILD-RELEASE-20260506A | 出包：183 PASS / 0 FAIL，voice-ime.exe 10.21MB (13:51)，crash-reporter.exe 23.58MB (13:51)，voice-ime-ui.exe 17.68MB（沿用） | tester-1 | 2026-05-06 |
| PUNCT-INTEGRATION-001 |
| WAVEFORM-FIX-002 + SHIMMER-SPEED-002 + PROMPT-PUNCT-FIX-001 | 波形索引修复(center=newest)+边缘先落加权衰减+shimmer 800ms+LLM标点开关 | coder-1 | 2026-05-06 | 标点符号自动补全后端集成：PunctuationEngine + pipeline 三条件调用 + LLM 提示词降级 + 英文半角转换 | coder-1 | 2026-05-06 |
| PUNCT-INTEGRATION-001-UI | 标点补全 UI 开关（Voice 页转录设置，默认 ON，双语）+ Tauri config 同步 | coder-2 | 2026-05-06 |
| SHIMMER-SPEED-001 | 处理中动效扫光周期 2000ms → 1200ms（快约 40%） | 咖啡 | 2026-05-06 |
| BUILD-RELEASE-20260506B | 全链出包：voice-ime.exe 10.21MB / voice-ime-ui.exe 17.69MB / crash-reporter.exe 23.59MB (18:47)，208 PASS / 0 FAIL，冒烟 4/4 | tester-1 | 2026-05-06 |
| WAVEFORM-FIX-002 | 波形索引反转(中心=最新)+FallingToProcessing边缘先落(0.125~0.5x)，音频开始时中心先振，停止时边缘先落 | coder-1 | 2026-05-06 |
| SHIMMER-SPEED-002 | 处理中动效周期 1200ms → 800ms | coder-1 | 2026-05-06 |
| PROMPT-PUNCT-FIX-001 | LLM提示词标点开关：enabled=false时注入"Do NOT add punctuation"，enabled=true时正常加标点 | coder-1 | 2026-05-06 |
| BUILD-RELEASE-20260506C | 出包：voice-ime.exe 10.22MB / voice-ime-ui.exe 17.69MB / crash-reporter.exe 23.59MB，276 PASS / 0 FAIL，冒烟 4/4 | tester-1 | 2026-05-06 |

| PROMPT-PUNCT-REVAMP-001 | LLM 标点指令重构：ON 时追加"Add appropriate punctuation"，OFF 时不追加任何标点指令（移除旧双指令逻辑） | coder-1 | 2026-05-06 |
| WORDBOOK-SUGGEST-FIX-001 | 词条自动学习修复：SUGGESTION_INSTRUCTION 强化为 MUST + 始终注入 + optimize_and_translate last_line fallback + 诊断日志 | coder-1 | 2026-05-06 |
HOTKEY-LATENCY-FIX-001 | 热键录音视觉延迟 + 偶发首字丢失修复：热键 Start 立即 show_overlay + drain_pre_roll 循环收集至目标样本量 | coder-1 | 2026-05-06
| TEST-SYNC-OVERLAY-PREWARM-001 | OVERLAY-LOCK-SCOPE-001 + HOTKEY-STREAM-PREWARM-001 测试同步：新增 2 单测（check_stream_health 安全短路 + warm_stream_match stream_failed 决策逻辑），cargo check 0 errors | tester-1 | 2026-05-08 |
| BUILD-RELEASE-20260508A | Release 出包：cargo test 184 PASS / 0 FAIL / 2 IGNORED，voice-ime.exe 10.76MB (21:44)，voice-ime-ui.exe 18.55MB (21:42)，crash-reporter.exe 24.74MB (21:43)，冒烟 4/4 PASS | tester-1 | 2026-05-08 |
| OVERLAY-LOCK-SCOPE-001 | overlay 锁范围缩小：audio_buf 快照模式，锁内仅拷贝+decay，GDI 绘制移到锁外（持锁 2-8ms→<1ms） | coder-1 | 2026-05-08 |
| HOTKEY-STREAM-PREWARM-001 | ensure_stream 预热检测：空闲态 recv_timeout(500ms) 周期性检查 stream_failed + 预重建，避免热键 Start 路径 50-500ms 阻塞 | coder-1 | 2026-05-08 |
| BUILD-RELEASE-20260509A | Release 出包：cargo test 187 PASS / 0 FAIL / 2 IGNORED，voice-ime.exe 10.76MB (00:24)，voice-ime-ui.exe 18.55MB (00:22)，crash-reporter.exe 24.74MB (00:23)，冒烟 4/4 PASS | tester-1 | 2026-05-09 |
| TRUNCATION-FIX-001 | 翻译截断修复：max_input_length=0 解除 CT2 输入长度限制，由 MAX_RECORD_SECONDS 300s 限制 | coder-1 | 2026-05-09 |
| BUILD-RELEASE-20260509A | Release 出包：187 PASS / 0 FAIL / 2 IGNORED，voice-ime.exe 10.76MB / voice-ime-ui.exe 18.55MB / crash-reporter.exe 24.74MB，冒烟 4/4 | tester-1 | 2026-05-09 |
| TRANS-SEGMENT-001 | opus-mt 长文本分段翻译：segment_text() + translate_segment()，LENGTH_PENALTY 1.2→1.5，COVERAGE_PENALTY=0.05，MIN/MAX_SEGMENT_CHARS=120/200，MAX_SENTENCES_PER_SEGMENT=3，9 新增单测，cargo test 244 PASS | coder-1 | 2026-05-09 |
| TEST-SYNC-TRANS-SEGMENT-001 | 翻译分段测试同步：补充 3 个缺口单测（短文本不分段/单句长文本不分段/MAX_SENTENCES_PER_SEGMENT=3 边界），cargo check 0 errors | tester-1 | 2026-05-13 |
| TEST-EXEC-TRANS-SEGMENT-001 | 翻译分段测试执行：cargo test 247 PASS / 0 FAIL / 2 IGNORED，新增 3 个缺口测试全部 PASS，无回归 | tester-1 | 2026-05-13 |
| BUILD-RELEASE-20260513A | Release 出包：TRANS-SEGMENT-001 + TEST-SYNC + I18N-ZH-FIX，cargo test 247 PASS，voice-ime.exe 10.77MB / crash-reporter.exe 24.74MB / voice-ime-ui.exe 沿用 18.55MB，冒烟 4/4 PASS | tester-1 | 2026-05-13 |
| BUILD-RELEASE-20260513B | Release 出包：EXE-DIR-PATHS-001（资源路径统一exe目录）+ model_dir简化 + default-config.toml，cargo test 247 PASS，voice-ime.exe 10.77MB / voice-ime-ui.exe 18.55MB / crash-reporter.exe 24.74MB（均今日），冒烟 4/4 PASS | tester-1 | 2026-05-13 |
| I18N-ZH-FIX-001 | i18n ZH 简体段落 error_transcription_empty 繁体→简体（"識別結果為空。"→"识别结果为空。"），cargo check 0 errors | coder-1 | 2026-05-13 |
| EXE-DIR-PATHS-001 | 统一所有外部资源路径为 exe 所在目录：config/wordbook/crash/debug.log → {exe}/；model_dir() 移除 dev fallback；debug 日志也从 AppData 移至 exe 目录 | coder-1+orchestrator | 2026-05-13 |
| BUILD-RELEASE-20260513B | 出包：EXE-DIR-PATHS-001，cargo test 247 PASS / 0 FAIL，冒烟 4/4 PASS，voice-ime.exe 10.77MB / voice-ime-ui.exe 18.55MB / crash-reporter.exe 24.74MB (21:37-21:39) | tester-1 | 2026-05-13 |
| MIC-MUTE-DETECT-001 | 麦克风静音探测：热键前检测（IAudioEndpointVolume COM）+ 录音中每 1s 检测，静音时 Error overlay，i18n 三语错误提示 | coder-1 | 2026-05-13 |
| TEST-SYNC-MIC-MUTE-001 | 静音探测测试同步：3 个新单测（检测间隔/非Windows返回false/三语i18n非空），cargo check 0 errors | tester-1 | 2026-05-14 |
| TEST-EXEC-MIC-MUTE-001 | 静音探测测试执行：cargo test 250 PASS / 0 FAIL / 2 IGNORED，3 个新测试全部 PASS，无回归 | tester-1 | 2026-05-14 |
| TEST-SYNC-VERSION-CHECK-001 | 版本检查模块测试同步：主程序补 4 单测（边界输入/多段版本/serde 往返）+ Tauri 侧新增 9 单测，cargo check 0 errors | tester-1 | 2026-05-14 |
| TEST-EXEC-VERSION-CHECK-001 | 版本检查模块测试执行：270 PASS / 0 FAIL / 2 IGNORED，version_check 12/12 全 PASS，无回归，npm build + Tauri check 0 errors | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514B | Release 出包：MIC-MUTE + VERSION-CHECK + I18N，270 PASS / 0 FAIL，冒烟 4/4，voice-ime.exe 10.88MB / voice-ime-ui.exe 18.66MB / crash-reporter.exe 24.74MB (12:52-12:54) | tester-1 | 2026-05-14 |
| BUILD-PUBLISH-FIX-001 | 构建流程 Publish 同步修复：build-test-guide.md 增强 Step 4（PowerShell+不可跳过）+ build.bat 修复（无条件复制+mkdir 兜底）+ troubleshooting.md 追加条目 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514C | Release 出包：PIPELINE-CANCEL-FIX-001，270 PASS / 0 FAIL，冒烟 4/4，voice-ime.exe 10.89MB / voice-ime-ui.exe 18.66MB / crash-reporter.exe 24.74MB (13:32) | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514D | Release 出包：ESC-CANCEL-FIX-001 + PIPELINE-CANCEL-FIX-001，270 PASS / 0 FAIL，冒烟 4/4，voice-ime.exe 10.89MB / crash-reporter.exe 24.74MB (14:03) | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514E | Release 出包：OVERLAY-FOCUS-FIX-001（WS_EX_NOACTIVATE + SW_SHOWNA），270 PASS / 0 FAIL，冒烟 4/4，voice-ime.exe 10.89MB / crash-reporter.exe 24.74MB (15:52) | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514F | 最终合包：OVERLAY-FOCUS-FIX-001 + UI-ABOUT-FIX-001，270 PASS / 0 FAIL，冒烟 4/4，voice-ime-ui.exe 新构建 18.66MB (16:00)，voice-ime.exe/crash-reporter 沿用 | tester-1 | 2026-05-14 |
| UI-ABOUT-FONT-GAP-001 | About 页版本卡片 gap 8→48px + 检查更新按钮 fontFamily:inherit，npm build 通过，暂不出包 | coder-2 | 2026-05-14 |
| BUILD-RELEASE-20260514J | 出包：LOGO-REPLACE-001 + UI-ABOUT-STRINGS-001 + UI-ABOUT-FONT-GAP-001，270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，feiyin-ime.exe 10.98MB (22:49) / feiyin-ime-ui.exe 8.75MB (22:47) / crash-reporter.exe 24.84MB (22:49)，Publish/已同步 | tester-1 | 2026-05-14 |
| BUILD-RELEASE-20260514K | 出包：VERSION-BUMP-001（0.5.4），270 PASS / 0 FAIL / 2 IGNORED，冒烟 4/4，feiyin-ime.exe 10.98MB (23:27) / feiyin-ime-ui.exe 8.75MB (23:25) / crash-reporter.exe 24.84MB (23:27)，Publish/已同步 | tester-1 | 2026-05-14 |
| TASK-UI-I18N-BACKEND | 后端 UiLanguage::TraditionalChinese + i18n 完整性审查，新增 4 单测（save/load 往返 + ZH/ZH_TW/EN 覆盖），cargo test 253 PASS | coder-1 | 2026-05-14 |
| VERSION-CHECK-BACKEND | GitHub 版本检查后端：主程序后台线程 + Tauri 3 IPC command（get_version_info/force_check/open_url），缓存到 exe 同级 version_check.json | coder-1 | 2026-05-14 |
| VERSION-CHECK-UI | About 页版本检查 UI：状态机 idle/checking/latest/failed/has_update，自动读缓存，手动重检，下载按钮，3 语 i18n | coder-2 | 2026-05-14 |
| PIPELINE-CANCEL-FIX-001 | 录音结束后 cancel_signal 竞态诊断日志，src/main.rs 3 处日志，行为不变 | coder-1 | 2026-05-14 |
| ESC-CANCEL-FIX-001 | GetAsyncKeyState VK_ESCAPE 检测位修复：0x0001→0x8000u16（按住状态位），消除 ESC 残留 bit 导致跳过转录的 bug | coder-1 | 2026-05-14 |
| CROSSPLATFORM-FIX-001 | open_url_in_browser 新增 macOS cfg 分支（open 命令），跨平台同步修复 | coder-1 | 2026-05-14 |
| OVERLAY-FOCUS-FIX-001 | 录音 overlay WS_EX_NOACTIVATE + SW_SHOWNA，不再抢焦导致失焦预览窗口 | coder-2 | 2026-05-14 |
| UI-ABOUT-FIX-001 | About 版本卡片 280→380px + 移除侧边栏底部齿轮图标 | coder-2 | 2026-05-14 |
| LOGO-REPLACE-001 | 全量替换橙色复古麦克风图标：src-tauri/icons + ui/public/icons 共 19 处，ICO/ICNS 全套，WSL Python Pillow 处理 | orchestrator | 2026-05-14 |
| UI-VERSION-CARD-SPACING-001 | About 版本卡片布局重构：width fit-content + flex row + gap 8px，消除 space-between 导致标签/值两端拉开问题 | coder-2 | 2026-05-14 |
| UI-ABOUT-STRINGS-001 | About 页品牌文案更新：app_title/about_title/about_subtitle，3 语言 × 3 key（飞音智能语音输入 / 解放双手提升交互效率） | coder-2 | 2026-05-14 |
| UI-VERSION-CARD-SIZE-001 | About 版本卡片 minWidth 240px + justifyContent center | coder-2 | 2026-05-14 |
| UI-VERSION-CARD-HEIGHT-001 | About 版本卡片 minHeight 150px | coder-2 | 2026-05-14 |
| UI-CHECK-BTN-COLOR-001 | About 检查更新按钮文字 color #ff6b35（品牌橘色） | coder-2 | 2026-05-14 |
| RENAME-AND-VERSIONINFO-001 | exe 重命名 voice-ime→feiyin-ime（8处）+ Windows 版本信息 winres 嵌入 ProductName/FileDescription/Version（build.rs 新建） | coder-1 | 2026-05-14 |
| TEST-SYNC-RENAME-001 | 测试文件 exe 名同步：conftest.py + test_tauri_v2_commands.py + test_tray.py + test_webview_ui.py，旧名 0 残留 | tester-1 | 2026-05-14 |
| VERSIONINFO-FIX-001 | 移除 src-tauri/build.rs winres（与 tauri_build 自动生成的 VERSION 资源冲突 CVT1100）| coder-1 | 2026-05-14 |
| BUILD-RELEASE-20260514G | 出包：RENAME-AND-VERSIONINFO-001 + VERSIONINFO-FIX-001，270 PASS / 0 FAIL，feiyin-ime.exe 10.89MB / feiyin-ime-ui.exe 8.65MB（废弃图标清理后体积归正）/ crash-reporter.exe 24.74MB | tester-1 | 2026-05-14 |
| BUILD-SCRIPT-UPDATE-001 | 构建脚本全量修正：build-test-guide.md exe 名替换 + docs/RUNTIME-DEPS.md 更新 + build.bat Publish 同步块重写 | tester-1 | 2026-05-14 |
| ICON-EMBED-001 | exe 图标嵌入：build.rs 追加 set_icon(app.ico)（feiyin-ime.exe 显橙色麦克风）+ tauri.conf.json bundle.icon 末项→icon-settings.ico（feiyin-ime-ui.exe 显齿轮） | coder-1 | 2026-05-14 |
| BUILD-RELEASE-20260514H | 出包：ICON-EMBED-001 + UI-VERSION-CARD-SIZE-001，270 PASS / 0 FAIL，feiyin-ime.exe 10.98MB / feiyin-ime-ui.exe 8.56MB / crash-reporter.exe 24.84MB | tester-1 | 2026-05-14 |
| TITLEBAR-ICON-FIX-001 | Tauri setup hook set_icon() 强制标题栏显示橙色麦克风（include_bytes! 128x128.png），tauri feature 新增 image-png | coder-1 | 2026-05-14 |
| BUILD-RELEASE-20260514I | 出包：TITLEBAR-ICON-FIX-001 + UI-VERSION-CARD-HEIGHT-001 + UI-CHECK-BTN-COLOR-001，270 PASS / 0 FAIL，feiyin-ime.exe 10.98MB / feiyin-ime-ui.exe 8.76MB / crash-reporter.exe 24.84MB | tester-1 | 2026-05-14 |
| UI-ABOUT-FONT-GAP-001 | About 版本卡片 gap 8→48px（6倍间距）+ 检查更新按钮 fontFamily:inherit（对齐侧边栏 Segoe UI Variable 字体） | coder-2 | 2026-05-14 |
| VERSION-BUMP-001 | 版本号 0.5.3 → 0.5.4（Cargo.toml / src-tauri/Cargo.toml / tauri.conf.json 三处） | coder-1 | 2026-05-14 |
| BUILD-RELEASE-20260514K | 出包：VERSION-BUMP-001（v0.5.4），270 PASS / 0 FAIL，feiyin-ime.exe 10.98MB / feiyin-ime-ui.exe 8.75MB / crash-reporter.exe 24.84MB，git push 34331c1 | tester-1 | 2026-05-14 |
| TEST-SYNC-FIRSTCHAR-001 | FIRSTCHAR-FIX-001 测试审查：5 个现有用例审查、1 处断言收紧建议、4 个新增边界用例设计、3 项不可自动化目视验收建议 | tester-1 | 2026-05-25 |
| TEST-EXEC-FIRSTCHAR-001 | FIRSTCHAR-FIX-001 全量构建出包：Step 1~7 完整执行，cargo test 286 PASS / 0 FAIL / 2 IGNORED，smoke 4/4 PASS，feiyin-ime.exe 10.99MB / feiyin-ime-ui.exe 8.56MB / crash-reporter.exe 23.68MB | tester-1 | 2026-05-25 |
| TEST-EXEC-FIRSTCHAR-002 | FIRSTCHAR-FIX-002 构建出包（仅 Rust 主程序，无前端改动）：cargo test 282 PASS / 0 FAIL / 2 IGNORED，smoke 4/4 PASS，feiyin-ime.exe 10.99MB / crash-reporter.exe 23.68MB，供 Gavin 端测"派发"识别 | tester-1 | 2026-05-25 |
| TEST-EXEC-FIRSTCHAR-003 | FIRSTCHAR-FIX-003 构建出包（idle_clear 改为 full drain）：cargo test 282 PASS / 0 FAIL / 2 IGNORED，smoke 4/4 PASS，feiyin-ime.exe 10.99MB / crash-reporter.exe 23.68MB，供 Gavin 端测"派对"/"派发"短词识别 | tester-1 | 2026-05-26 |
| RESEARCH-ACC-CRASH-001 | accuracy 长音频静默崩溃根因审计（纯研究）：审计 VAD 分段/录音缓冲/unwrap 点/sherpa-onnx 已知 issue/内存峰值；Top 3 候选（VAD 降级单次转录超 max_total_len / OOM alloc abort / 双 stream+ORT arena 累积）；产出 collab/research/acc-crash-001.md | coder-1 | 2026-07-07 |
| FIX-VAD-STATE-RESET-001 | 修复 accuracy 长音频第二次转录 VAD detector 游标未重置致 slice 越界 panic（P0）：segment() 末尾 clear 后加 reset() + build_padded_segments 纵深防御（越界段丢弃/clamp）+ 6 新增单测；cargo check 0 errors / cargo test 384 passed 0 failed 6 ignored | coder-1 | 2026-07-07 |
| ASR-SINGLE-MODEL-001 | 实施 DEC-027（accuracy 单模型加载 + 去 CTC 兜底 + 删异常检测链）：Transcriber 去 fallback_recognizer + 删 need_fallback 兜底链 + 删三函数及 26 测试 + VAD 降级重设计（分段全空 bail / VAD 不可用朴素 20s 等分）+ 6 naive_chunk 单测；cargo check 0 errors / cargo test 364 passed 0 failed 6 ignored | coder-1 | 2026-07-07 |
| ASR-SINGLE-MODEL-001-R2 | 验收第 2 轮修订：R1 lock poisoned 走 naive_chunk 分支（抽 transcribe_segments_chunked 辅助函数消除重复）+ R2 build_recognizer 返回 effective_model（降级 CTC 语义归位三处）+ 顺手注释更新 + 3 新增单测；cargo check 0 errors / cargo test 366 passed 0 failed 7 ignored | coder-1 | 2026-07-07 |
| ASR-QWEN3-BACKEND-001 | Qwen3 在线 ASR 后端续做：模型 ID 从硬编码改为配置文件读取（`qwen3_asr_model` 字段）+ WS 读写超时 10s 保护（防 worker 线程无限阻塞）+ 审计逐条 DEC-028 7 条通过；cargo check 0 errors / cargo test qwen3_online 21/0/1 | coder-1 | 2026-07-07 |

## [0.6.0] - 2026-07-07 (work-in-progress)

### Added (ASR Qwen3 UI + Backend)
- Frontend: ASR model radio-group replaced with `<select>` dropdown; brand-color description text for each model
- Frontend: Qwen3 Online model section: password-input API Key + Test Connection button (testing/success/failed states) + empty-key hint
- Frontend: 16 ASR UI tests (ASR-UI-001~016 incl. 5 Qwen3 tests) + 7 PUNCT UI tests
- Backend: `qwen3_asr_model` config field with serde default
- Backend: `test_qwen3_asr_connection` now reads URL + model from config instead of hardcoding

### Fixed (R2)
- [R2-1 P0] Empty-key fallback: fixed prevModelRef recording timing (moved to handleAsrModelChange) + stale closure (use latestRef)
- [R2-2 P1] Cleaned up 3 temp files left from R1 delivery
- [R2-3 P2] Fixed Voice.tsx indentation for new code (2-space consistent)

### Tests
- Added 3 FALLBACK tests covering real user-flow: switch-to-qwen3→unmount scenarios
- Total: 43 tests (26 Voice, 2 App, 9 Wordbook, 6 other)

### Fixed (BUG-QWEN3-WS-HANDSHAKE-001)
- src-tauri/src/qwen3.rs: use `IntoClientRequest::into_client_request()` to generate proper WebSocket handshake headers (Sec-WebSocket-Key, Upgrade, Connection, etc.)
- Removed manual `Request::builder()` path that caused "Missing, duplicated or incorrect header sec-websocket-key"
- Added `build_ws_request()` helper with unit test verifying handshake headers presence


### 2026-07-08 — RESEARCH-ASR-ACCURACY-002 完成
- **任务**：accuracy 优化落地后仍不敌 performance 的深挖研究（调用层生效性审计 + CER 对比 + 调优空间扫描）
- **负责人**：coder-1
- **性质**：纯研究，生产代码零改动
- **核心结论**：推翻 RESEARCH-ASR-ACCURACY-001 结论——001 PoC 脚本漏传 --temperature（默认1.0）与生产 temp=0.3 不可比；生产等价条件下 accuracy CER=0.15/first=85% 优于 CTC CER=0.3553/first=70%；生效性审计6项全PASS无BUG；建议 temp 0.3→0.2（+2.5pp低风险）


### 2026-07-08 — RESEARCH-ASR-ACCURACY-003 完成
- **任务**：Gavin 真实语料双模型同源 A/B（分化根因终审）
- **负责人**：coder-1
- **性质**：纯研究，生产代码零改动
- **核心结论**：未复现 Gavin performance 更优体感——A2 native+VAD CER=0.0271 优于 A1 CTC CER=0.0724（低63%），专有名词 93% vs 67%；A3 para3 空输出实锤 max_total_len 截断（反向证明 VAD 分段是 native 长音频必要条件）；剩余嫌疑三层（采集链/语料形态/后处理），建议 DEBUG-AUDIO-DUMP-001 排查采集链


### 2026-07-08 — ASR-ACC-TUNE-001 完成
- **任务**：E1 temperature 0.3→0.1 + E2 HOTWORDS_MAX_ENTRIES 50→20（Gavin 拍板）
- **负责人**：coder-1
- **改动**：src/transcription/mod.rs（L641 temp + L447 hotwords 上限 + 注释/单测同步）
- **自验**：cargo check 0 errors / cargo test 全绿（345+24 passed 0 failed）
- **影响**：仅 accuracy 分支；performance/qwen3 零影响
