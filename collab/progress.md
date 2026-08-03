# 里程碑进度 · voice-ime

## 文档更新规则

1. **所有功能/优化/BUG修复/研究任务必须归入对应版本表格**，不在底部增加散落内容块
2. **每个条目必须包含完成日期**，格式：`功能描述 | 日期`
3. **条目描述简洁**：一句话说清楚完成了什么功能任务
4. **版本构建产物统一记录在底部产物表**
5. **与 CHANGELOG/logs 的职责区分**：
   - progress.md：按版本的功能级汇总（看版本就看到全部功能）
   - CHANGELOG.md：按版本的任务级记录（编号+负责人+时间）
   - logs/YYYYMMDD.md：按日期的代码级详细日志（变更细节+验证结果）
6. **新任务完成时立即更新**，不批量补
7. **测试用例同步、构建和出包任务不记录到 progress**，这些属于 CHANGELOG/logs 范畴

---

## 关键架构决策

| ID | 决策 |
| --- | --- |
| DEC-000 | 目标平台 Win10/Win11（移除 Win7） |
| DEC-001 | Win32 controller 主控（非 eframe 宿主） |
| DEC-002 | 设置窗口独立 `--settings-ui` 入口 |
| DEC-003 | 录音悬浮层原生 Win32 overlay（GDI） |
| DEC-004 | RegisterHotKey 全局热键 |
| DEC-005 | controller 统一 shutdown 协议 |
| DEC-013 | Settings UI 迁移至 Tauri+React（渐进式） |
| DEC-015 | macOS 事件循环采用 Tauri 作为主机 |

---

## v0.1.0（2026-04-13）

| 功能 | 说明 |
| --- | --- |
| 语音转录 | Whisper 模型 + WASAPI 录音 |
| LLM 优化 | OpenAI 兼容 API 文本纠错 |
| 全局热键 | RegisterHotKey Toggle/PTT |
| 系统托盘 | 托盘图标 + 菜单 |
| 配置界面 | eframe 首版 |

## v0.2.0（2026-04-15）

| 功能 | 说明 |
| --- | --- |
| Win32 架构重构 | controller + settings-ui + overlay 三进程 |

## v0.3.0（2026-04-15）

| 功能 | 说明 |
| --- | --- |
| 配置界面改版 | 左侧 Tab 导航工业级 UI |

## v0.3.1（2026-04-15）

| 功能 | 说明 |
| --- | --- |
| 开机自启 | 注册表启动项 |
| 设备选择 | 音频输入设备列表 |
| 品牌统一 | 产品名+图标 |
| ESC 中断 | 录音中途取消 |
| 自动保存 | 配置实时持久化 |

## v0.3.2（2026-04-15）

| 功能 | 说明 |
| --- | --- |
| ASR 升级 | Whisper → Paraformer，CER 18%→3% |

## v0.3.3（2026-04-16）

| 功能 | 说明 |
| --- | --- |
| 热键延迟修复 | 消除响应卡顿 |
| LLM 自动禁用 | 连接失败自动回退 |
| 多语言支持 | 中英双语 i18n |
| crash 模块 | 崩溃检测+报告 |

## v0.3.4（2026-04-16）

| 功能 | 说明 |
| --- | --- |
| 模型路径修复 | exe 相对路径 |
| UI 精修 | 细节优化 |
| 自动化测试框架 | pytest+pywinauto |

## v0.3.5（2026-04-17）

| 功能 | 说明 |
| --- | --- |
| 双语 ASR | 中英识别 |
| 系统提示词多语言 | i18n 同步 |
| E2E 测试 | 端到端验证 |
| 代码清理 | 移除废弃代码 |

## v0.3.6（2026-04-17）

| 功能 | 说明 |
| --- | --- |
| PTT 松键修复 | 释放即停止录音 |
| 配置界面小标题优化 | 分组标签 |

## v0.4.0（2026-04-17）

| 功能 | 说明 |
| --- | --- |
| UI 框架升级 | eframe → Tauri+React |

## v0.4.2.x（2026-04-18~19）

| 功能 | 说明 |
| --- | --- |
| PTT 取消优化 | 松键即停 |
| Overlay 边框 | 视觉优化 |
| 热键捕获修复 | 右 Ctrl+e.code 映射 |
| 热键重注册 | 配置变更热更新 |
| 窗口尺寸 | 1025×730 / 1179×720 |
| 崩溃报告 UI | 中文+橘色按钮+图标 |
| BUG 修复 | UIPATH+PROMPT-REVERT |

## v0.5.0（2026-04-20）

| 功能 | 说明 |
| --- | --- |
| BUG-027 修复 | 托盘菜单二次点击配置窗口 |
| macOS 基础 | 跨平台架构抽象 |
| 框架优化 | FRAMEWORK-001+UI-044 |

## v0.5.1（2026-04-20）

| 功能 | 说明 |
| --- | --- |
| Tauri v2 升级 | CONFIG/RUST/FRONTEND+回归验证 |

## v0.5.2（2026-04-23~27）

| 功能 | 说明 |
| --- | --- |
| SQLite 词库数据库 | 内存缓存+持久化 |
| LLM 词库注入 | Rule 5/6 系统提示词 |
| 用户词库 UI | 添加/删除/双 Tab |
| LLM 主动建议词条 | Rule 7+suggestions 解析 |
| 频率阈值自动学习 | 候选表+阈值晋升 |
| 词条删除修复 | 按 ID 删除+旧表持久化 |
| 词库页视觉优化 | 橙色高亮/28px 按钮/白色卡片 |
| LLM suggestions 修复 | 旧 config 兜底 |
| 词条弹窗优化 | modal-header+×关闭 |
| LLM 输出结构化 | `<corrected>` 标签 |
| 配置 UI 启动守卫 | 主程序未运行时 exit 1 |
| 热键首字丢失修复 | WASAPI 流预热常驻 |
| 热键首字预卷保留 | 300ms 预卷+drain 空过滤 |
| 热键配置同步 | notify watcher+debounce+Arc 即时同步 |
| 配置内存一致性 | notify+debounce+atomic save 方案 |
| 热键 Arc 共享 | RwLock AppConfig 即时同步 |
| Hotkey 线程优化 | MsgWaitForMultipleObjects 零 CPU |

## v0.5.3（2026-04-28~05-07）

| 功能 | 日期 |
| --- | --- |
| 翻译热键 Ctrl+T | 2026-04-28 |
| opus-mt 离线翻译 zh-en/en-zh 双向 | 2026-04-28 |
| UI 翻译热键设置页 | 2026-04-28 |
| 翻译热键优化 Arc\<AtomicBool\>+150ms 轮询 | 2026-04-29 |
| ORT 内存优化（关闭 Arena/MemPattern+移除无效 Session） | 2026-04-29 |
| 单向翻译引擎热加载+语言跳过过滤 | 2026-04-29 |
| Beam Search beam=6+长度归一化 | 2026-04-29 |
| KV-cache warm-start 修复 | 2026-04-29 |
| no-repeat-3gram 防 beam 重复死循环 | 2026-04-29 |
| CT2 引擎切换 ORT→CTranslate2（exe 缩至 10MB） | 2026-04-30 |
| SentencePiece tokenizer 替换 Xenova（修复空结果） | 2026-05-01 |
| CT2 FFI 直调修复（单批路径） | 2026-05-01 |
| 翻译空格修复回归：tokenizer.decode() 替换 join+normalize | 2026-05-07 |
| 翻译截断修复回归：MAX_DECODE_STEPS 256→512 | 2026-05-07 |
| 录音时长 180s→300s | 2026-05-07 |
| 静默超时 8s→30s | 2026-05-07 |
| 标点符号自动补全（72MB CT-Transformer+英文半角） | 2026-05-06 |
| 标点 UI 开关（Voice 页 toggle+Tauri 配置同步） | 2026-05-06 |
| LLM 标点指令重构（ON→追加/OFF→不追加） | 2026-05-06 |
| 词条自动学习修复（SUGGESTION_INSTRUCTION MUST+fallback） | 2026-05-06 |
| 热键视觉延迟修复（overlay 立即变橘+pre-roll 循环） | 2026-05-06 |
| 录音 overlay 优化系列（实心圆/波形/停止按钮/麦克风图标） | 2026-05-02~04 |
| overlay 唤醒优化（WM_APP_OVERLAY_WAKE 替代 100ms sleep） | 2026-05-02 |
| overlay 闪烁修复（style:0+WM_ERASEBKGND） | 2026-05-02 |
| 处理中动效（Shimmer 扫光+800ms 周期） | 2026-05-04 |
| 边框加深（0x0C0C08→0x2E2A26→0x171513） | 2026-05-04~05 |
| 6合1性能优化（预初始化/阻塞等待/去锁/i18n/三态灯/prewarm） | 2026-05-03 |
| LLM/翻译引擎预初始化（TCP 连接池+热重载缓存） | 2026-05-04 |
| 托盘冻结根治（TrackPopupMenu TPM_RETURNCMD） | 2026-05-04 |
| 错误提示本地化（网络超时/服务不可用/模型/麦克风） | 2026-05-04 |
| 波形索引反转（中心=最新+边缘先落） | 2026-05-06 |
| 波形频谱优化（PeakLevel+60fps+32条） | 2026-05-02 |
| 麦克风图标 GDI 手绘（18px 放大+4x 超采样抗锯齿） | 2026-05-06 |
| 首字丢失修复（PRE_ROLL_MS 500ms+WASAPI prime+静音头） | 2026-05-06 |
| 中英混合识别研究（P0 参数+P1 LLM 提示词方案选定） | 2026-05-07 |
| 输入语言 UI（Voice 页中/英/日/韩/粤选项） | 2026-05-07 |
| ASR language 传递（配置→SenseVoice 模型参数） | 2026-05-07 |
| blank_penalty 优化（0.5 降低空白帧概率） | 2026-05-07 |
| LLM code-switching 规则（所有语言英文拼写还原） | 2026-05-07 |
| 输入语言 UI 改为下拉选框（select-input 统一风格） | 2026-05-07 |
| LLM 开关提示文字（橘色小字说明） | 2026-05-07 |
| 语音页文字修改（输入语言提示+识别输出） | 2026-05-07 |
| About 页改造（版本号从 Tauri API 读取+去掉构建日期/引擎/版权） | 2026-05-07 |
| 界面语言添加繁体中文（简体/繁体/English 三选） | 2026-05-07 |
| 前端 i18n 重构（7 页面字符串提取到翻译资源文件） | 2026-05-07 |
| 后端 UiLanguage 新增 TraditionalChinese 枚举 | 2026-05-07 |
| 后端 i18n 新增 ZH_TW 繁体字符串（overlay/错误/托盘/崩溃报告全覆盖） | 2026-05-07 |
| overlay 锁范围缩小（audio_buf 快照模式，持锁 2-8ms→<1ms） | 2026-05-08 |
| ensure_stream 预热检测（空闲态周期性检查 stream_failed + 预重建） | 2026-05-08 |
| 翻译截断修复（max_input_length=0 解除输入长度限制） | 2026-05-09 |
| 长文本分段翻译（segment_text + translate_segment，MIN=120/MAX=200字符，≤3句/段） | 2026-05-09 |
| 麦克风静音探测（IAudioEndpointVolume COM，热键前+录音中双场景，fail-open）| 2026-05-14 |
| GitHub 版本自动探测（主程序后台线程静默检查 + About 页展示新版本 + 手动重检 + 一键打开下载链接）| 2026-05-14 |
| 录音后 cancel_signal 竞态诊断日志（PIPELINE-CANCEL-FIX-001，消除静默跳过转录问题）| 2026-05-14 |
| ESC-CANCEL-FIX-001：GetAsyncKeyState 0x0001→0x8000u16，消除 ESC 残留 bit 导致录音结束后跳过转录的 bug | 2026-05-14 |
| CROSSPLATFORM-FIX-001：open_url_in_browser 加 macOS cfg 分支（open 命令）| 2026-05-14 |
| OVERLAY-FOCUS-FIX-001：录音 overlay WS_EX_NOACTIVATE + SW_SHOWNA，不再抢焦导致失焦预览窗口 | 2026-05-14 |
| UI-ABOUT-FIX-001：About 版本卡片 280→380px + 移除侧边栏底部齿轮图标 | 2026-05-14 |

## v0.5.4-patch（2026-05-23~25）

| 功能 | 日期 |
| --- | --- |
| FIRSTCHAR-FIX-006：R2+R3 打包改善孤立短词首字。R3 转录前规整前导静音（语音起点回溯 200ms margin 裁多余静音 + silence head 200ms→50ms，前导 ~800ms→~250ms）；R2 find_speech_anchor 回溯 150ms 保护送气声母（冷启动）。强约束绝不削声母（回溯 margin + saturate 保护）。+6 单测，cargo test 295/0/2 | 2026-05-27 |
| FIRSTCHAR-FIX-005：降采样抗混叠根治送气清声母首字错误（派/对/七）。resample_anti_alias（Hann 窗 sinc 低通+多相 FIR，截止 7.2kHz）替代裸线性插值，改为整段重采样消除 chunk 边界 glitch 与高频混叠；附带修复 48kHz 下 max_frames 录音时长被截 1/3 的隐藏 bug。+7 单测，cargo test 289/0/2 | 2026-05-27 |
| PREROLL-RINGBUF-001：首字丢失根治，pre-roll 环形缓冲区（Mutex<VecDeque>）替代 bounded channel，消除热键触发时首段语音丢弃问题 | 2026-05-23 |
| FIRSTCHAR-FIX-004（D3）：channel chunk 携带 Instant 时间戳，idle drain 按热键触发时刻（t_record）精确区分——陈旧背景丢弃、热键后首字保留，根治 full drain 误清热键后首字问题；冷启动重建期间首字亦完整保留 | 2026-05-26 |
| FIRSTCHAR-FIX-003：idle_clear 改为无限清空（full drain），消除 256-channel 满载时 196 陈旧 chunk（~2s背景噪音）污染短词录音导致完全识别错误 | 2026-05-26 |
| FIRSTCHAR-FIX-002：idle_clear 改为 chunk 数量匹配，消除 WASAPI chunk size 不固定导致多清一个 chunk 吞首字 | 2026-05-25 |
| FIRSTCHAR-FIX-001：首字识别不稳定二次修复，bounded idle_clear（消除 C1 竞争窗口）+ find_speech_anchor 保头部 prime trim（消除 C2 截断），+9 单测，cargo test 261/0/2 | 2026-05-25 |
| I18N-FIX-EN-001：EN Strings 补齐 8 个字段（preview_title_bar / overlay_error / error_* 系列），修复 Tauri UI 编译失败 | 2026-05-25 |

---

## v0.6.0~0.6.1 · ASR 双模型（2026-07-06）

| 功能 | 说明 |
| --- | --- |
| 默认模型直换 | SenseVoice-Small(237MB) → FunASR Nano CTC(179MB)，首字 70%→75%，五语正常（DEC-025 路线 A）|
| 可选 accuracy 模型 | FunASR Nano native 972MB（0.8B，Qwen3-0.6B decoder），不随包，配置界面下载引导（DEC-025 路线 B）|
| hotwords 词库注入 | accuracy 模式词库 corrected 词条自动灌入，len+哈希版本号感知变更后台重建（~6s 异步）|
| Transcriber 热重载 | 模型切换/语言/词库变更后台重建 + channel 替换，in_flight 防并发，失败保旧实例 |
| 三重兜底 | 空输出/hallucination(>12字/s)/n-gram 环路乱码 → fallback CTC 重转 → Err，绝不注入垃圾 |
| VAD 长音频分段 | silero VAD(643KB) >24s 切段（段≤20s+200ms padding），根治 native max_total_len=512 的 28s 上限（DEC-026）|
| accuracy 自带标点 | native_punctuated 来源标记，native 成功时跳过标点模型推理；关开关时剥标点（修复缺口）|
| UI 模型选择 | Voice 页 ASR 模型区块（性能最优/准确率更高）+ 未下载提示卡（链接/目录/一键复制）+ 三语 i18n |
| GitHub | commit 81304f7 推送 main（21 files，+2296/-49）|
| 下载引导卡修复 | B-002-FIX（2026-07-07）：下载按钮改 invoke(open_url_in_browser) 修复 Tauri 外链拦截 + URL 文本渲染可复制 |
| accuracy 根因研究 | RESEARCH-ASR-ACCURACY-001（2026-07-07）：证实生产前处理为 CTC 调优伤 native（50ms 静音头 native 掉 10pp）+ hotwords 全量灌入副作用 + PoC 80% 为理想化假象 |
| hotwords 精选 | ASR-ACC-OPT-001 方案 A（2026-07-07）：curate_hotwords_entries 过滤纯 ASCII/超长词条 + 上限 50，防大词库撑爆 context |
| accuracy 前处理适配 | ASR-ACC-OPT-001 方案 B（2026-07-07）：accuracy 分支 silence head 0ms + backtrack 100ms，PoC native+hw 65→77.5% |
| CTC 优化研究 | RESEARCH-ASR-CTC-OPT-001（2026-07-07）：同音字 70% 错误为 CTC 天花板；blank_penalty 无影响；CTC 不支持 hotwords |
| CTC 前处理优化 | ASR-CTC-OPT-001 P1（2026-07-07）：CTC silence head 50→0ms（+2.5pp，50ms 为旧 SenseVoice 遗产）+ P3 blank_penalty 0.5→0 清理；P2 ITN 因"七→7"副作用撤销 |

## v0.6.2 · 词库单词化 + 智能数字规整（2026-07-10）

| 功能 | 说明 |
| --- | --- |
| 词库单词化 | 词对(raw→corrected)→单词(word)模式（DEC-029）：migration 003 幂等迁移（corrected 侧去重导入，真实 DB 运行时验证生效）；删除 apply() 文本替换；hotwords（仅 accuracy）改读单词表 |
| LLM 词汇表纠偏 | prompt 从 XML 映射表改为用户词汇表语义（发音相近误写→修正为标准写法）；suggestions 自动学习改单词格式 {"suggestions":["word"]}，旧对象格式向后兼容（raw-only 条目丢弃防污染） |
| 词库 UI 单词化 | Tauri command add_wordbook_entry(word) 单参数 + 删除无调用旧 delete command；添加弹窗改单输入框；三语 i18n 同步 |
| 智能数字规整 ITN | 自研规则模块 src/itn.rs（DEC-030）：多位数字/计量语境转阿拉伯（金额/电话/日期时间/经纬度/温度/压力/百分比/小数/分数/序数/单位/逐位串 12 类），单字数字+成语/专名/量词保护；规则数据外置 itn-rules.toml（内置默认+exe 同级覆盖+损坏降级），转录后/LLM 前三模型统一生效；50 单测 |
| P0 词库 migration 修复 | init_schema 二次执行崩溃修复（2026-07-11）：migration 003 后重跑 MIGRATION_001 索引重建引用已删除 raw 列致词库永久失败；增加已迁移检测跳过旧 migration + 2 条回归单测；附带词库添加弹窗说明三语文案更新 |

## v0.7.0 · 格式化输出 + 场景感知 + ITN 历史词修复（2026-07-13）

| 功能 | 说明 |
| --- | --- |
| 格式化输出（更名+指令集） | 「LLM 优化」整体更名「格式化输出」（DEC-031 单开关，llm.enabled 零迁移）：prompt 新增 F1 语气词去除/F2 改口修正/F3 结构重组固定指令段（F3 硬约束禁压缩/禁删语义/禁添加）；三语 UI+Tauri i18n 全面更名 |
| 多行输出安全网 | Phase 1 无场景感知，LLM 输出兜底单行化（换行→"；"，仅 optimize 路径），规避聊天框误发送/终端逐行执行风险 |
| 失败报错不关开关 | LLM 调用失败→原文照常注入（语音不丢）+ 注入后 2.5s overlay 错误条提示检查配置 + tray 复位 Idle；不再有自动禁用 |
| 开启门槛校验 | UI 开启格式化输出须 api_url/api_key/model 齐全且连接测试通过；配置修改自动重置验证态；修复 probe() 未开启不能测试的死锁 |
| ITN 历史词误转修复 | 端测 bug"五代十国→五代10国"根治：多位数判定改源汉字数（单字十/百/千回归保护路径）+ protect.historical 词表 95 条（历史/典籍/民俗五分类，外置可热修） |
| 场景感知（Phase 2，零配置） | 录音启动瞬间采集前台窗口进程名+标题（微秒级 Win32），本地分类六类场景（chat/email/doc/ide_terminal/browser/unknown，词表外置 scene-rules.toml 可热修）+ 浏览器细分（标题兜 Gmail/Docs）；LLM prompt 注入 F4 场景风格段；multiline_safe 三道防线（F3 禁用/输出单行化/含换行强制剪贴板）防聊天框误发送；隐私默认只上送场景类别，窗口标题不上送；**无独立 UI 开关（DEC-031 勘误，Gavin 端测拍板）——随「启用格式化输出」单开关生效**，scene.enabled/send_window_title 仅为 config.toml 隐藏字段 ｜ 2026-07-13 |
| LLM 输出大小写保护 | 端测 bug "Dear Mr. Wang,"→"mr. wang" 根治：LLM 成功路径不再过 ASR 全大写后处理（fix_asr_english_case），改 normalize_script_only 仅保留简繁转换兜底，LLM 修正的大小写完整注入 ｜ 2026-07-13 |
| 邮件称呼冒号规则 | scene-rules.toml email style 改中英文称呼一律冒号结尾（原英文用逗号，Gavin 端测拍板），热修免构建 ｜ 2026-07-13 |
| AI Agent 场景支持 | 场景感知词表新增 AI Agent 专属块（9 个核实进程名：Claude/ChatGPT/Codex/OpenCode/CherryStudio/Chatbox/jan/AnythingLLM/元宝 + 13 个网页版/PWA 标题关键词）+ ide_terminal 补 conhost/OpenConsole 经典控制台盲区（CLI Agent 命中路径），热修免构建 ｜ 2026-07-14 |

## v0.7.2 增补 · 双平台兼容重构 + 思维链泄漏修复（2026-07-29~30，未升版）

| 功能 | 说明 |
| --- | --- |
| 双平台兼容接缝重构（DEC-033） | 为 macOS 团队接手做 A 阶段适配，硬红线「不影响任何 Windows 功能」：`mod hotkey`/`mod injection` 加 `#[cfg(windows)]`（全仓零引用实证）｜`get_windows_version()` cfg 拆两版、调用点一行未改｜**`platform/mod.rs` glob 导出改 15 符号显式清单 + 契约注释块**（漏列即响亮编译失败，替代无法奏效的 trait 抽象）｜macOS 侧补 `notify_config_changed`/`capture_scene_signals` stub。Tauri 侧：`windows` 依赖挪 target 段 ｜ `check_hotkey_available` cfg 隔离 ｜ `overlay.rs` `.transparent(true)` cfg 拆链 ｜ 新增 `scripts/fetch-sherpa-onnx.ps1`（解决全新 checkout 构建不了的既有问题）。按 DEC-033 附则二取消 CI 相关改动 ｜ 2026-07-29 |
| macOS 交接文档与分支审计 | `docs/MACOS-HANDOFF.md`（242 行，平台契约/协作硬约定/CT2 构建陷阱/checkout 缺口/TODO 索引）+ `docs/MACOS-BRANCH-AUDIT.md`（15 处 cfg 分支静态审计：**P0×1** `crash/reporter.rs:369` 调用 `egui 0.29.1` 不存在的 `FontData::from_bytes`，macOS 必然编译失败；P1×8 含 `main.rs:3419-3475 mod macos_stubs` 空实现；P2×4；P3×2）｜ 2026-07-30 |
| ITN 顺序反转 + 摄氏度符号独立通道 | Gavin 端测「说摄氏度不出 ℃」根治（DEC-035）：**①ITN 从「LLM 前」移到「三分支后、标点前」** —— ASR 把「摄氏」误听成「摄息/摄斯/摄四」（实测 11 次仅 2 次听对），只有 LLM 纠正后 ITN 才可能匹配；落点选标点之前而非管线末端，是为了不让标点模型（CT-Transformer）吃到分布外的阿拉伯数字+符号输入，把不确定性留在可控的规则引擎一侧；三条路径（LLM 成功/失败兜底/关闭）由一处调用统一覆盖 **②新增独立于中文数字路径的单位符号通道**（`40摄氏度`/`40°C`→`40℃`，`itn-rules.toml` 新增 `[[unit_symbols.rules]]` 三条，最长匹配优先 + 必须有数字前缀），因 LLM 会自行把中文数字转阿拉伯数字，仅靠移位置不足以修复；**`44度` 绝不转**（角度/温度同形，Gavin 2026-07-27 拍板） ｜ 2026-07-30 |
| 翻译热键双向化 | Gavin 端测「开了翻译按热键不译成英文」根治：方向改由**内容自动判定**（含汉字→英文，否则→中文），删除原 `should_translate_for_language` 配置门控（该门控 + UI 未暴露 `target_language` 导致翻译热键对中文输入**永久无效**）；离线引擎单槽位换向（非双向常驻，守 DEC-027 省内存，每模型 153MB），重建失败回落注入原文而非交出方向不符的引擎；`target_language` 语义改为「上次使用方向缓存」供启动预载，不新增任何用户可见配置（守 DEC-031） ｜ 2026-07-30 |
| 跨平台共享化重构 | `derive_translation_target` / `ensure_translation_direction` / `remember_translation_direction` 三个纯业务逻辑函数从 `#[cfg(target_os="windows")]` 的 `main.rs` 移入平台中立模块（`translation/mod.rs` / `config/mod.rs`），macOS 侧可直接复用、零重写；配套测试随之搬入中立模块，macOS 侧也能跑到（DEC-033 附则三） ｜ 2026-07-30 |
| 思维链泄漏五环修复（P0） | DeepSeek 推理模型把 CoT 泄漏进输入框（实测约 7 次 1 次，最坏整句变 `...`）根治：**P0-1** 请求体双发 `thinking:{"type":"disabled"}`（DeepSeek 官方开关）+ 保留 `enable_thinking`（SiliconFlow/Qwen3 用，避免回归），4 处注入点 ｜ **P0-2** `extract_text` 移除「content 空回落 `reasoning_content`」（这是 CoT 被当答案的真正入口）｜ **P0-3** `extract_corrected_tag` 改 `rfind` 取末对标签，防 CoT 里的模板占位劫持 ｜ **P0-4** 新增 `lacks_any_substantive_char()` 拒绝纯标点结果；**比例判据经实跑证伪后降级为只观测不拒绝**（合法重度压缩 9.7% vs 故障 6%，仅差 3.7pp 划不出可靠边界，误伤代价不对称）｜ **P0-5** 补 `finish_reason` + `usage`（含 `reasoning_tokens`）日志，`length` 截断今后可直接从日志判定 ｜ 2026-07-30 |

## ✅ v0.7.3 增补 · ITN 二代重构（2026-07-31 起，2026-08-01 闭环并出包）

| 功能 | 说明 |
| --- | --- |
| ITN 二代设计研究（RESEARCH-ITN-V2-001） | Gavin 四项需求的双轨并行研究（coder-1 主交付 `itn-v2-design-001.md` + 主控独立稿 `itn-v2-orchestrator-001.md` → 合并终稿 `itn-v2-merged-final.md`）。**三项关键结论**：**①** `十一块九毛二`→`十一块9毛2` 的根因是保护词表**撕裂**（`十一` 国庆节义命中白名单，`check_protection` 只前移游标不锁定后续 → 语义单元后半段照转），这是 `[ITN-PREFIX-SHADOW-001]` 此前未识别的**第三种失败模式**，主控 07-30「误保护=优雅降级」结论据此补充适用条件（→ DEC-038）｜ **②** 主控独立取证发现保护词表对**规则性语法族的覆盖是随机的**——`一/六/八/九点半` 在表内而 `二/三/四/五/七/十点半` 不在，用户看到同一表达因数值不同行为完全相反，根因是 1386 条机器词频派生词表把语法族切成随机子集（→ DEC-038）｜ **③** `UNIT_SYMBOL_PROTECTION` 指令（`src/llm/mod.rs:29`）正文前提「input already contains normalized numbers」在 DEC-035 反转顺序后**已为假**，反转时未同步修订——这构成支持 ITN 回移的独立论据（→ DEC-036） ｜ 2026-07-31 |

**本批次新增决策**：DEC-036（ITN 双通道，部分推翻 DEC-035）｜ DEC-037（输出形态按单位族分治，货币归一）｜ DEC-038（保护词表不得承载规则性语法族）

**实施批次**（按**文件域**切分，非按阶段——`src/itn.rs` 被 P1/P2/P3/P4 共同触及，按阶段并行会撞同一文件）：

| 批次 | 任务 | Worker | 内容 | 状态 |
| --- | --- | --- | --- | --- |
| P1 | ENGINE-001 | coder-1 | **ITN 双通道**（DEC-036）：主通道 `normalize_numbers` 移到 LLM 前、补丁通道 `normalize_unit_symbols_only` 留在 LLM 后捞 ℃；缺陷A 撕裂修复（③块级 + ①右邻否决，①用「含进位单位」判据精确区分 `十一`(撤销) 与 `五一`(不撤销)）；语法族词条盘点 | ✅ |
| P1 | PROMPT-001 | coder-2 | `UNIT_SYMBOL_PROTECTION` 追加**事实保全**（禁止重算/取整/重述数值时间日期，`4:45`不得变`4:30`、`明天`不得变`今天`）；**F3 列表四象限**（有序×多行`1. `／无序×多行`• `／有序×单行内联保序号／无序×单行「、」「；」）；`build_output_format` 补 bullet 防契约压制；`scene-rules.toml` 审查后**零改动**（notepad/wordpad 早已在 doc 块内） | ✅ |
| P2 | ENGINE-002 | coder-1 | 修 ①引入的**输出不确定性**：`check_protection` 五 set 中三个从 `find_map`(首个) 改 `filter+max()`(确定性最长匹配)。5 次独立进程实证恒定 | ✅ |
| P2 | PROMPT-002 | coder-2 | `flatten_multiline` **分隔符叠加守卫**，消除 `；；`/`、；`/`。；` 三类畸形 | ✅ |
| P3 | ENGINE-003 | coder-1 | **甲型文法**（半/刻）：`四点半`→`4:30`、`五点三刻`→`5:45`、`一吨半`→`1.5吨`、`一个半小时`→`1.5小时`（量词穿透）；**成对移除 9 条**保护词条；`is_real_unit` 守卫（通用量词不算单位）；idioms 收口 max()；新增 `[units.time]` | ✅ |
| P4 | ENGINE-004 | coder-1 | **乙型**（隐式小数 `一米二`→`1.2米`，边界护栏）+ **丙型**（多级链 `十一块九毛二`→`11.92元`）+ **单位层级表** + **`分`族属消歧**（前驱单位决定货币/时间）+ **全或无**（`三年二班` 整段不转）+ **删除 ③** | ✅ |
| P5 | ENGINE-005 | coder-1 | 含数字地名白名单 **+60 条 ≥3 字**（行政区划 24 + 景点 36），`proper_nouns` 69→129；反向护栏 60/60；零 Rust 改动 | ✅ |

**累积**：`src/itn.rs` +845 ｜ `src/llm/mod.rs` +112 ｜ `src/main.rs` +56 ｜ `itn-rules.toml` +69 ｜ **版本号 0.7.3 全程未动**

### 主控验收中查出、Worker 汇报未覆盖的五项（未采信汇报表格）

1. **保护词表对语法族的覆盖是随机的**（→ DEC-038）：`一/六/八/九点半` 在表内而 `二/三/四/五/七/十点半` 不在，用户看到同一表达因数值不同行为相反。根因是 1386 条机器词频派生词表把规则性语法族切成随机子集。**Gavin 报的 `四点半` 只是露出水面的那一个**
2. **`UNIT_SYMBOL_PROTECTION` 指令前提自 DEC-035 起为假**：正文写「input already contains normalized numbers」，而 ITN 当时跑在 LLM 之后。反转顺序时未同步修订 → 构成支持 ITN 回移的独立论据
3. **HashSet 迭代顺序不确定性被 ①激活**：`proper_noun_set` 是 `HashSet` + 首个匹配，`十一`/`十一月` 前缀重叠 → 同一输入两次运行可能给出 `11月` 或 `十一月`。改动前两分支可见输出相同（隐患潜伏），①让它变成用户可见
4. **`flatten_multiline` 分隔符叠加**：coder-2 用推理（「语义互斥」）代替验证得出「不会叠加」的错误结论。实际只需某行以分隔符结尾且后有行即产出 `；；`，而 PROMPT-001 教 LLM 用「；」**恰好提高了该畸形的概率**
5. **③ 在旗舰用例上从未触发**：`match_unit_word(...)?` 用 `?` 而非 `break`，`十一块九毛二` 末尾 `二` 无单位即整体返回 None。`11块9毛2` 实由 ①+逐字路径产出。该发现直接决定「全或无」不能以「③返回 None」为信号

### 遗留（非本批引入，已记录）

- `三年二班` 类撕裂已由 DEC-037 附则「全或无」解决
- `七星`（2 字既有条目）遮蔽面开放（`七星级`/`七星彩`），本批未处理
- 未经请求的范围扩张：`[units.time]` 致 `三小时`→`3小时`、`五分钟`→`5分钟`，已列入 TEST-SYNC 专项覆盖

**后续**：✅ 已于 2026-08-01 12:51 闭环出包（BUILD-RELEASE-20260801-001），Gavin 端测反馈见下一批次

---

## ✅ v0.7.3 增补二 · 格式分流 + ITN 数值静默改错 + 提示词架构重构（2026-08-02~03，已出包）

> 触发：Gavin 2026-08-02 端测两类问题——买菜清单被拆成四行列表 + **ITN 把数值算错且用户看不见**
> 版本号 0.7.3 全程未动

| 功能 | 说明 |
| --- | --- |
| 短项内联 / 长句列表分流（014+015） | Gavin 端测「买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉」被拆成四行 `- ` 列表，但这是短名词短语清单应顿号内联。`build_output_format` 新增 **F3-item form**：SHORT（无谓语、无内部标点、≤6 字词）→ 内联分隔符不做列表；LONG（含谓语或内部标点）→ 列表。分隔符表抽共享常量 `INLINE_SEPARATOR_RULES` 两分支共用，防两套说法漂移；四语枚举措辞补充（中/英/日/韩）。**015 是主控验收查出的收口**：014 只给 F3a 补了 LONG 限定，F3b 与末段输出契约仍无条件要求 bullet 且位置更靠后 —— 即代码注释自陈的「后段软化前段」失败模式，Gavin 用例恰好命中 ｜ 2026-08-02 |
| 年级班级简写不再被误合并（016） | Gavin 端测「我是一三班的学生」（=一年级三班）被转 `13班`；同类 五一班/初二三班/高一四班。根因：`parse_cn_number` 逐位串判据 `serial_len>=2 && !next_is_unit`，「班」非进位单位 → 合并为 13。**修法走规则层不走词表**（DEC-038）：新增 `[protect.serial_suffixes]`，2 位逐位串后紧跟班级后缀时 `parse_cn_number` 直接 return None，字符走单字路径。**`十三班`→`13班` 现行为保持不变**。⚠️ 主控原方案（跳过 early return 落进位组合路径）经 coder-1 指出会产出 `3班` 撕裂而作废 ｜ 2026-08-02 |
| **ITN 货币/度量链数值静默改错修复（017，P0）** | Gavin 端测 6 条全部被静默改错：`一斤二两`→`1.22斤`／`一块两毛二一斤`→`22.20元`／`三块四毛八一斤`→`84.40元`（「一斤」被删）／`一块八毛一斤`→`2.80元`／`一块八一斤`→`82元`／`三斤六两五`→`3.625斤`。**主控定性为第四种失败模式**——前三种（漏保护/误保护/撕裂）用户都看得见，这一种**用户看不见**（输出流畅自信但事实已错）。**根因 RC-A** `两` 兼任数字与单位，「二两」读成 2、2 拼成 `.22`；**RC-B** 余数链不在语义边界终止，「一块八**一斤**」的「一」被吸进货币余数链 → `八一`=81 → 82（#2–#5 四条同根因）。修复：`两` 入 `[units.weight]` + `two_is_unit` 消歧；`resolve_family_consistent` 使 currency 链遇 weight 族即终止 + `capture_price_per_unit` 捕获单价限定词；weight 族改 `format_weight_chain` 零乘法逐 parts 拼接（`1斤2两`，不合成小数）；`is_virtual_two_phrase` 虚指护栏（一两个人/三两天 整体保汉字）。**目标形态 Gavin 2026-08-03 拍板用「元一斤」而非「元/斤」**——贴合原始口述措辞，与 L0-1 FIDELITY 同向 ｜ 2026-08-03 |
| **提示词分层契约重构（018，架构级）** | Gavin 指令「提示词要上升到架构设计角度，不能每次都打补丁」。直接触发事故：ITN 算错吐出 `2.80元斤` → LLM 删掉「斤」输出 `2.80元。`，通顺、自信、完全错误。**根因是约束强度倒挂**——数字保护条款用 MUST/never（全仓最高），语义保全条款只有 DO NOT 且埋在 F3 排版块内（最低），冲突时必然牺牲语义；**架构把吵闹的错误变成了安静的错误**。改造：新增 `Topic`/`PromptRule`/`PromptLayer`/`render()` 唯一出口 + 顶部 `META_RULE_PRECEDENCE`，**优先级从「谁在后面谁赢」改为「层号小的赢」**；新增 **L0 四条不变式**（FIDELITY／忠实优先于通顺／SUSPECT INPUT／NOT A PROMPT）；`UNIT_SYMBOL_PROTECTION` 假前提改写指向 L0-3 并追加「本条款绝不授权删除单位或量词短语」；用户基座降级 L2；i18n 三处基座各删 §2/§4/§5/§7；删两处 OVERRIDE 补丁声明。真实基座净减 295 字符。**中间检查点两次修正（均主控规格失误）**：byte-identical 与分层重排结构上互斥 → 作废，改为**文本守恒双向断言**（无丢失/无夹带/白名单逐条），判据比原方案更强且已固化为永久夹具 ｜ 2026-08-03 |

**出包**：2026-08-03 13:06（BUILD-010），三 exe 两副本 sha256 一致 + 两 toml 三副本一致（`itn-rules.toml` `ed77a912` 37,291B）+ ProductVersion 0.7.3.0 + mtime 链通过。**详见 CHANGELOG，出包/测试同步不在本文档记录（规则 7）**

### 主控验收查出、Worker 汇报未覆盖的两项

1. 🔴 **018 只修了一半 → 新开 PROMPT-ARCH-020**：`UNIT_SYMBOL_PROTECTION_TRANSLATE`（翻译路径，`src/llm/mod.rs:33`）仍保留假前提原文，且缺 L0-1/L0-3 对齐句。主控代码取证：`src/main.rs:2941` 的 ITN 处理在翻译分支判定 `:2984` **之前**，翻译路径吃同一份 ITN 输出 → **同一个静默改错在翻译功能下会原样复现**。逃过 749 条测试的原因是翻译路径常量**无任何等价断言**。发现手段为**出包后的反向探针**（`already contains normalized numbers` 在新 exe 里仍 =1），非测试、非 Worker 自报
2. ⚠️ **exe 产物记录长期失准**：`todo.md` 顶部挂着「014/015 未进 exe」的过时结论，实际 08-02 01:35 已出过一次包含 014/015/016 的产物但未回写文档。已订正并加入纪律：**出包后立即回写产物时间戳**

---

## macOS 双平台 · A 阶段（2026-07-29~30）· ✅ 编译打通

> 治理约束：**DEC-034**（跨平台兼容为首要约束 + 单仓库两端并行）｜ 版本号未动，仍 v0.7.2
> 交接文档（仓库内受 git 管辖）：`docs/MACOS-HANDOFF.md` / `MACOS-PORT-ASSESSMENT.md` / `BUILD-MACOS.md` / `MACOS-BRANCH-AUDIT.md`

**里程碑：macOS 侧从「20 个源码错误、连编都编不过」到「主程序 + 全测试目标 + Tauri 后端 + 前端 全部 0 errors」。**

| 阶段 | 内容 | 结果 |
| --- | --- | --- |
| 07-29 环境 | rustup 1.97.1 / cmake 4.4.1 / Xcode CLT clang 17 / Node 24 / sherpa-onnx osx-arm64 预编译包 | 324 个依赖 crate（含全部 C/C++）编译通过；环境不再是瓶颈 |
| 07-30 接缝 | Windows 侧交付 MACOS-COMPAT-001（`292eeb0`）：废除 `platform/mod.rs` glob 导出改 15 符号显式清单、`mod hotkey`/`mod injection` 加 cfg、`crash::get_windows_version` 补非 Windows 占位、src-tauri 三处 cfg 隔离 | 8 类实测阻塞消化 5 类 |
| 07-30 基线 | macOS 侧首次实跑 `cargo check` 取权威错误清单（`MACOS-CARGOCHECK-BASELINE-001`） | 主程序 4 独特错误 / src-tauri 7 / 前端 0 |
| 07-30 修复 | 三处 API 修正（`6f0b51e`）+ 依赖段位回归修复 + 脚本入库（`5e3ed89`）+ ignore 收尾（`4b2126b`） | **`cargo check --all-targets` 0 errors；`cargo check --manifest-path src-tauri` 0 errors；`tsc --noEmit` + `vite build` 0 errors** |

**本阶段两个关键发现**（均为「只有真编一次才能发现」的类型，已各自立 troubleshooting 条目）：

1. **`[TOML-SECTION-DRIFT-001]`（🔴 Windows 侧改动引入的回归）**：`292eeb0` 把
   `[target.'cfg(target_os = "windows")'.dependencies]` 段头插在 `[dependencies]` 表中间，
   按 TOML 语义把其后的 `tokio-tungstenite` / `futures-util` / `rustls` 三个共享依赖静默改判为 Windows 专属。
   **在 Windows 上 cfg 命中、cargo check 0 errors，完全不可见**。且 `rustls` 那行是 BUG-QWEN3-CRYPTO-001
   的 ring provider 修复 —— 不只是编译失败，macOS 侧连该 TLS 修复一起丢了。
   源码级 cfg 审计查不出它（漂移在依赖清单层，代码里没有 cfg 可扫）
2. **`[SHELL-BASHSOURCE-ZSH-001]`**：`${BASH_SOURCE[0]}` 在 zsh 下为空，而 macOS 默认 shell 就是 zsh
   → `docs/BUILD-MACOS.md` 教给所有新人的 `source scripts/env-macos.sh` 一直是坏的（静默解析到仓库父目录）

**未解决/待排期**：
- `[NPM-CI-LOCK-DESYNC-001]`：`ui/package-lock.json` 与 `package.json` 长期失同步，**两平台的 `npm ci` 都跑不了**
  （只卡全新 clone，现有 node_modules 与 `npm run build` 正常）。需两侧协同修
- **B/C/D 阶段未开工**：`src/main.rs:2761` 的 `fn main()` 在非 Windows 分支仍只打一行 warn 就返回 ——
  **能编译 ≠ 能运行**，主控入口 / 事件循环 / tray / overlay / Accessibility 权限 / `.app` 打包全部待实现
- 无 CI 防线（DEC-033 附则二 Gavin 决定暂不启用），「本地 cargo check 通过」不构成「没破坏对侧」的证据

## 版本构建产物

| 版本 | 日期 | feiyin-ime.exe | feiyin-ime-ui.exe | crash-reporter.exe | 测试 |
| --- | --- | --- | --- | --- | --- |
| v0.5.3 | 2026-04-30 | 67MB | — | — | 36 PASS |
| v0.5.3 | 2026-05-01 | 66MB | — | — | 36 PASS |
| v0.5.3 | 2026-05-04 | 10.21MB | 17.68MB | 23.58MB | 171 PASS |
| v0.5.3 | 2026-05-06 | 10.24MB | 17.69MB | 23.58MB | 229 PASS |
| v0.5.3 | 2026-05-07 | 10.24MB | 17.68MB | 23.58MB | 230 PASS |
| v0.5.3 | 2026-05-07 | 10.24MB | 17.69MB | 23.58MB | 230 PASS + 24 Vitest + 冒烟 4/4 |
| v0.5.3 | 2026-05-07 | 10.24MB | 17.69MB | 23.58MB | 24 Vitest + 15 Tauri |
| v0.5.3 | 2026-05-07 | 10.25MB | 17.69MB | 23.59MB | 230 PASS + 24 Vitest + 冒烟 4/4 |
| v0.5.3 | 2026-05-08 | 10.76MB | 18.55MB | 24.74MB | 187 PASS + 冒烟 4/4 |
| v0.5.3 | 2026-05-09 | 10.76MB | 18.55MB | 24.74MB | 187 PASS + 冒烟 4/4 |
| v0.5.3 | 2026-05-13 | 10.77MB | 18.55MB（沿用）| 24.74MB | 247 PASS + 冒烟 4/4 |
| v0.5.3 | 2026-05-13 | 10.77MB | 18.55MB | 24.74MB | 247 PASS + 冒烟 4/4（EXE-DIR-PATHS）|
| v0.5.4 | 2026-05-14 | 10.88MB | 18.66MB | 24.74MB | 270 PASS + 冒烟 4/4（MIC-MUTE + VERSION-CHECK）|
| v0.5.4 | 2026-05-14 | 10.89MB | 18.66MB | 24.74MB | 270 PASS + 冒烟 4/4（PIPELINE-CANCEL-FIX-001 诊断日志）|
| v0.5.4 | 2026-05-14 | 10.89MB | 18.66MB（沿用）| 24.74MB | 270 PASS + 冒烟 4/4（ESC-CANCEL-FIX-001 根治修复）|
| v0.5.4 | 2026-05-14 | 10.89MB（沿用）| 18.66MB (16:00) | 24.74MB（沿用）| 270 PASS + 冒烟 4/4（OVERLAY-FOCUS + UI-ABOUT 最终包）|

| v0.5.4 | 2026-05-14 | 10.89MB | 8.65MB | 24.74MB | 270 PASS + 冒烟 4/4（feiyin-ime 重命名 + 新图标 + 标题更新 + 版本信息）|
| v0.5.4 | 2026-05-14 | 10.98MB | 8.56MB | 24.84MB | 270 PASS + 冒烟 4/4（exe 文件图标嵌入 + 版本卡片放大）|
| v0.5.4 | 2026-05-14 | 10.98MB | 8.4MB | 24MB | 270 PASS + 冒烟 4/4（标题栏橙色麦克风 + 版本卡片高度×3 + 按钮橘色）|
| v0.5.4 | 2026-05-14 | 10.98MB | 8.75MB | 24.84MB | 270 PASS + 冒烟 4/4（LOGO-REPLACE + UI-ABOUT-STRINGS + UI-ABOUT-FONT-GAP）|
| v0.5.4 | 2026-05-14 | 10.98MB | 8.75MB | 24.84MB | 270 PASS + 冒烟 4/4（VERSION-BUMP 0.5.4，git push GitHub 34331c1）|
| v0.5.4 | 2026-05-23 | 10.99MB | 8.75MB（沿用）| 23.68MB | 273 PASS + 冒烟 4/4（PREROLL-RINGBUF-001 首字修复）|
| v0.5.4 | 2026-05-26 | 10.99MB | 8.56MB（沿用）| 23.68MB | 282 PASS + 冒烟 4/4（FIRSTCHAR-FIX-003 full drain idle_clear）|
| v0.5.4 | 2026-05-25 | 10.99MB | 8.56MB（沿用）| 23.68MB | 282 PASS + 冒烟 4/4（FIRSTCHAR-FIX-002 chunk 数量匹配 idle_clear）|
| v0.5.4 | 2026-05-25 | 10.99MB | 8.56MB | 23.68MB | 286 PASS + 冒烟 4/4（FIRSTCHAR-FIX-001 + I18N-FIX-EN-001）|
| v0.5.4 | 2026-05-26 | 10.99MB | 8.56MB（沿用）| 23.68MB（沿用）| 282 PASS / 0 FAIL / 4 IGNORED（FIRSTCHAR-FIX-004 D3 时间戳精确清空，orchestrator 独立验证）|
| v0.5.4 | 2026-05-27 | 10.99MB (16:51) | 8.56MB（沿用）| 23.68MB（沿用）| 289 PASS / 0 FAIL / 2 IGNORED + 冒烟 4/4（FIRSTCHAR-FIX-005 降采样抗混叠重采样，含 7 防混叠单测）|
| v0.5.4 | 2026-05-27 | 10.99MB (19:01) | 8.56MB（沿用）| 23.68MB（沿用）| 295 PASS / 0 FAIL / 2 IGNORED + 冒烟 4/4（FIRSTCHAR-FIX-006 R2+R3 前导静音规整+声母回溯，端测确认首字 ~20%→~54%）|
| v0.5.5 | 2026-05-27 | 10.99MB (21:05) | 8.75MB (21:05) | 23.68MB（沿用）| 295 PASS / 0 FAIL / 2 IGNORED + 冒烟 4/4（VERSION-BUMP-002 完整出包，两 exe winres 属性版本号确认 0.5.5）|
| v0.5.4 | 2026-05-28 | 10.99MB (16:43) | 8.75MB (16:43) | 23.68MB（沿用）| 295 PASS / 0 FAIL / 2 IGNORED + 冒烟 4/4（VERSION-REVERT-001：版本号回退 0.5.5→0.5.4，完整重建两 exe，winres 确认 0.5.4）|

> exe 体积：66MB→10MB 是 CT2 引擎切换结果；voice-ime-ui 18.66MB→8.65MB 是清除废弃设计预览图（icon-final/icon-new 误嵌 ~11MB）

| 版本 | 日期 | feiyin-ime.exe | feiyin-ime-ui.exe | crash-reporter.exe | 测试 |
| --- | --- | --- | --- | --- | --- |
| v0.6.1 | 2026-07-06 | 11,077,632B (23:48) | 8,762,880B (23:48) | 24,839,680B (23:48) | 348 PASS / 0 FAIL / 5 IGNORED + Vitest 32/0 + 冒烟通过（ASR 双模型全链 + VAD + 标点优化；Publish/models 含 254MB CTC 模型 + 643KB silero VAD）|
| v0.6.1 | 2026-07-07 | 11,070,464B (21:07) | 8,762,880B（沿用）| 24,839,680B (19:29) | 366 PASS / 0 FAIL / 7 IGNORED + 冒烟通过（ASR-ACC-OPT A+B + ASR-CTC-OPT P1P3 + HALLUC-FIX H1 + FIX-VAD-STATE-RESET 崩溃修复 + ASR-SINGLE-MODEL DEC-027 单模型；Gavin 端测全项通过）|
| v0.6.1 | 2026-07-08 | 11,334,144B (03:04) | 9,999,872B (02:47) | 24,846,848B（沿用）| 405 PASS / 0 FAIL / 8 IGNORED + Vitest 43/43 + 冒烟通过（DEC-028 Qwen3 在线 ASR 全链：WS 后端+UI 三选项下拉+5 轮 P0 修复；Gavin 端测通过，工作空间 endpoint）|
| v0.6.1 | 2026-07-08 | 11,334,144B (14:24) | 9,999,872B（沿用）| 24,846,848B（沿用）| 405 PASS / 0 FAIL / 8 IGNORED + 冒烟通过（ASR-ACC-TUNE E1 temp 0.1 + E2 hotwords 上限 20；仅主程序重建；Gavin accuracy 实测期开始）|
| v0.6.2 | 2026-07-10 | 11,463,168B (16:13) | 10,002,944B (16:13) | 24,846,848B (16:13) | 404+38 PASS / 0 FAIL + Vitest 44/44 + 冒烟通过 + migration 003 真实 DB 验证生效（词库单词化 DEC-029 + 智能 ITN DEC-030；新增 Publish/itn-rules.toml 5,256B；Playwright 20 SKIP CDP 长期已知）|
| v0.6.2 | 2026-07-11 | 11,464,192B (22:38) | 10,003,456B (22:38) | 24,846,848B (22:38) | 457 PASS / 0 FAIL / 8 IGNORED + Vitest 44/44 + 冒烟通过 + 词库回归双开 UI PASS（WORDBOOK-FIX-062-001 P0 修复重出包，替换 07-10 缺陷包；pytest E2E 重启用例 SKIP CDP 导航）|
| v0.7.0 | 2026-07-13 | 11,544,576B (18:15) | 10,013,696B (18:47) | 24,858,112B (18:14) | 537 PASS / 0 FAIL / 8 IGNORED + Vitest 51/51 + 冒烟 PID 12112 稳定（格式化输出 DEC-031 + 场景感知零配置 + ITN 历史词修复；新增 Publish/scene-rules.toml；UI exe 18:47 为移除场景感知区块后重出（DEC-031 勘误）；pytest E2E SKIP CDP 已知）|
| v0.7.0 | 2026-07-13 | 11,565,056B (23:38) | 沿用 18:47 | 沿用 18:14 | 564 PASS / 0 FAIL / 8 IGNORED + 冒烟 PID 28360 稳定（FMT-LLM-005 大小写保护，仅主程序重建；主控补漏同步 target/release/itn-rules.toml 陈旧副本）|
| v0.7.0 | 2026-07-14 | 11,568,640B (18:21/Publish 18:27) | 10,013,696B (18:27) | 24,858,112B (18:20/Publish 18:27) | 535 PASS / 0 FAIL / 6 IGNORED + Vitest 51/51 + 冒烟 PID 26420 稳定（LANG-AUTO-001 输入语言/翻译方向全自动 + AI Agent 场景词表及 14 单测 + crash.rs asr_model 修复 + E2E ASR 选择器修正；主控退回修复 Tauri UI 漏构建+Publish 未同步后复验通过，dist 资产名嵌入核验）|
| v0.7.1 | 2026-07-24 | 三处 sha256 一致（src-tauri/target/release → target/release → Publish/）| 同左 | 同左 | 592+ PASS / 0 FAIL / 6 IGNORED + src-tauri 41/0 + Vitest 51/0 五文件 + 冒烟 PID 27100 Responding=True（**git 事故重做批次 REBUILD-LOST-001 11 项** + v0.7.1 新增 5 项：TEMP-CELSIUS-001 摄氏度符号 / FMT-EMPTY-CORRECTED-001 空标签兜底 / FMT-EMAIL-I18N-001 邮件中英日韩 / ASR-HIDE-ACCURACY-001 accuracy 静默迁移 performance；构建三步 npm 683ms + Tauri UI 2m14s + 主程序 1m44s 0 error；ProductVersion 0.7.1.0；itn-rules.toml + scene-rules.toml 同步 Publish）|

| v0.7.1 | 2026-07-25 | 11,592,704B (19:38:04/Publish 19:38:19) | 10,027,008B (18:54:50/Publish 19:38:19) | 24,858,624B (19:37:10/Publish 19:38:19) | 621 PASS / 0 FAIL + src-tauri 53/0 + Vitest 54/54（**P0 WORDBOOK-SCHEMA-FIX-001 词库 schema 全瘫修复** + WORDBOOK-AUTOLEARN-FIX-001 A+C+D；提交 b0c70b3；ProductVersion 0.7.1.0 不升版；三处产物 sha256 一致 + itn-rules/scene-rules 三副本 sha256 一致 + UI exe 已核实嵌入新 dist 资产 index-BNQZfcUG.css/index-CTgGziQm.js）|

| v0.7.2 | 2026-07-27 | 11,599,360B (20:31:31/Publish 20:31:39) | 10,027,008B (20:28:59/Publish 20:31:39) | 24,858,624B (20:30:36/Publish 20:31:39) | 672 PASS / 0 FAIL / 8 IGNORED + src-tauri 53/0 + Vitest/pytest SKIP（全 Rust 改动）（**Gavin 07-27 端测四项修复**：SCENE-OBS-001 场景感知可观测性 / LANG-MIXED-001 中日韩夹杂不强译 / ITN-CELSIUS-002-PROMPT+SYMBOL 摄氏度 ℃ / ASR-NOSPEECH-FILTER-001 空语音 token 剥离；提交 155b595 + fb230f9；**版本号升 0.7.1→0.7.2**，ProductVersion 0.7.2.0；构建 npm 1.93s + Tauri UI 2m12s + 主程序 2m25s；三处产物 sha256 一致 7fbb1e4b/0d76eca1/559c7506 + itn-rules/scene-rules 三副本一致 + UI exe 内嵌 dist 资产 index-BNQZfcUG.css/index-CTgGziQm.js 核实）|

| v0.7.2 | 2026-07-28 | 11,603,456B (18:42/Publish 18:43) | 沿用 07-27 20:31（10,027,008B）| 24,858,624B (18:41/Publish 18:43) | 686 PASS / 0 FAIL / 8 IGNORED + src-tauri 53/0 + Vitest/pytest SKIP（**仅重建主程序**，把 IMPL-SCENE-COVERAGE-001 的 144→165 条场景词表经 `include_str!` 嵌入内置默认；跳过 Tauri UI 构建，`src-tauri/**` 与 `ui/**` 零改动；提交 695e50e；**版本号维持 0.7.2 不升版**，ProductVersion 0.7.2.0；sha256 `e35679bd…` 两副本一致 + crash-reporter `8bfabfb5…` 两副本一致 + scene-rules.toml 三副本 `7b01b33c…` 一致；主控独立换 6 个探针字符串复查嵌入结果全部命中）|

| v0.7.2 | 2026-07-30 | 11,615,744B `8da29081…` (13:13/Publish 13:13) | 10,026,496B `d9db29e3…` (13:02/Publish 13:13) | 24,858,624B `950e1474…` (13:12/Publish 13:13) | 695 PASS / 0 FAIL / 8 IGNORED + src-tauri 53/0（于 `ff492ef` 批次执行，本次纯构建无代码改动故未重跑）（**全量三步出包**：MACOS-COMPAT-001 跨平台重构 + FIX-COT-LEAK-001-P0 思维链泄漏五环修复 + macOS 审计文档；提交 `292eeb0`/`ff492ef`/`2c98976`；**版本号维持 0.7.2 不升版**，ProductVersion 0.7.2.0；**全量重编** 主程序 10m02s + Tauri UI 2m51s + npm 1.55s，CT2 陷阱未触发；三 exe 两副本 sha256 逐一一致且全异于旧值；**主控决定性探针** `grep -ac "LLM response meta"` 0→1 证明新代码就位，另换 Tauri 侧独有串 `enable_thinking`/`reasoning_content`/`disabled` 补证 UI exe 镜像；scene/itn toml 三副本一致；UI 内嵌 dist 资产 index-BNQZfcUG.css/index-CTgGziQm.js 命中）|

| v0.7.3 | 2026-07-30 23:00 | 11,798,016B `74e4b56a…` (23:00/Publish 23:00) | 10,026,496B `16acff20…` | 24,858,624B `cc2ee873…` | itn 96 PASS / 1 FAIL（`time_half` 预期红，Gavin 明确不以测试通过为出包前提）（**BUILD-RELEASE-20260730-002**：ITN-COLLISION-TYPEA-002 单位碰撞保护词表 1386 条 + 几何术语白名单；**版本号 0.7.2→0.7.3**，主程序 ProductVersion 0.7.3.0、UI 0.7.3（此前 UI 停在 0.7.2 故本次必须重建 Tauri UI）；两副本 sha256 逐一一致；**`itn-rules.toml` 三副本 `9f36efcb…` 一致**（33,252 B，本次最关键——漏同步则外置旧 toml 9,689 B 静默赢过新内置默认，1386 条完全不生效而日志正常，[TOML-STALE-001]）；主控 8/8 决定性探针命中且旧 exe 对照为 0；冒烟 PID 23276）|

| v0.7.3 | 2026-08-01 12:51 | 11,878,912B `8092cf38…` (12:51:47/Publish 12:52:07) | 沿用 07-30 23:00（10,026,496B `16acff20…`）| 24,858,624B `b02ca32c…` | **767 PASS / 0 FAIL / 8 IGNORED**（`--list` 775 自洽）+ `itn::` **124/0** + src-tauri **53/0/0** + Vitest/pytest SKIP（零前端改动）（**BUILD-RELEASE-20260801-001**：**首次把完整 ITN 二代 P1-P5 打进 exe** + ENGINE-006 双隶属量词守卫 + LEXICON-006-C 移除 5 条 2 字遮蔽词；提交 `6fdba85`/`f6700ea`/`b462f83`/`05de1bc`/`5799c02`，**ahead 5 未 push**；**版本号维持 0.7.3 不升版**，ProductVersion 0.7.3.0；**仅重建主程序**，`ui/`+`src-tauri/` 自 `0adb819` 零改动经 `git diff --stat` 取证故跳过 Tauri UI；构建 1m51s 0 errors；**决定性探针反向设计**——本批无新增代码字符串，改用「被删的词应消失」：`一分钟`/`五分钟`/`八分钟` 旧 exe=1 → 新 exe 两副本=0，对照探针 `一刻钟`=1、`二分查找`=3 证明方法有效；`itn-rules.toml` 三副本 `93ab3972…` 一致、`scene-rules.toml` 三副本 `7b01b33c…` 一致；冒烟 PID 20000 Responding=True 零 panic）|

> **⚠️ 同版本号三构建（v0.7.2 可追溯性缺口，已扩大）**：07-27 20:31（内置词表 144 条，`7fbb1e4b…`，已被覆盖）/ 07-28 18:42（词表 165 条，`e35679bd…`）/ **07-30 13:13（含跨平台重构 + LLM 五环修复，`8da29081…`，当前 Publish 中的）** 三版主程序内容不同，但 ProductVersion 均为 0.7.2.0，只能靠 sha256 区分。系遵守「版本号禁止擅改」的必然结果（Gavin 三次出包指令均未授权升版），待 Gavin 定夺是否升 0.7.3 重出包。**注意：重出包需全量重编（含 CTranslate2 C++），实测 07-30 为 10m02s + 2m51s，不再是 2 分钟。**
>
> **📦 DISK-CLEANUP-001/002（2026-07-28）**：项目目录 **37.6 GB → 4.1 GB，净回收约 33.5 GB**（Gavin 拍板 A+B+C 三级 + D 级 SenseVoice 旧目录）。保留区 8 个产物 sha256 与删除前逐字节一致、端测数据 md5 一致、`models` symlink 完好、Publish 清单完整。
> **🔴 期间发生 opus-mt 误删事故并已完整还原**（`model.bin` 经 HF 官方 LFS sha256 校验一致，根↔Publish 10 文件互校一致），遗留约 11 MB 体积差额无法解释——因删除前未存目录清单。**后续任何 Agent 清理本项目：① 一律禁用 `cargo clean` ② 删除任何模型目录前必须先存清单 ③ 批量删除请求须按风险分级拆开逐项确认**，详见 troubleshooting [DISK-CLEANUP-001] 及其衍生事故节。
>
> v0.7.2 取证补记（2026-07-28 主控独立复核）：出包由 tester-1 于 07-27 20:31 完成，但当时 session 在文档闭环环节中断，todo/progress 未同步（todo 一度残留"版本号未动、待出包"的陈旧描述）。2026-07-28 Gavin 指令「commit / 查版本号 / 出包」后，主控**全量重新取证而非采信旧汇报**：三处 sha256 逐一比对、ProductVersion 读取、产物 mtime 与源码 mtime/提交时间先后关系核对、UI exe 内嵌 dist 资产 grep 命中、运行实例 PID 18548 路径与 Responding 状态确认——全部通过，**判定无需重复构建**（源码自 20:02 后零改动）。遗留：`Publish/voice-ime-ui.exe`（07-24 旧包名死文件）待清理。
>
> v0.7.1 补记（2026-07-25）：07-24 那行原缺失，系 session 在文档闭环环节被中断所致，现补录。exe 字节数以三处 sha256 一致为准（当时验收取证方式为哈希核对而非体积记录）。
>
> ✅ **07-25 出包行运行时验证：P0 已由 Gavin 端测确认修复**（2026-07-25）。Gavin 打开设置界面词库页**实际看到 5 条词条**，不再出现「打开词库失败：no such column: raw」——[WORDBOOK-SCHEMA-BREAK-001] 闭环。
>
> ⚠️ 过程记录（主控代记，tester-1 未完成 Step 6 与文档收尾）：产物层全部核验通过（时间戳/sha256/toml 三副本/版本号，主控独立执行，**Step 1-5 真实完成**）。但 Step 6 为虚报——tester-1 屏幕声称"词库 UI 5 条 ✅ + 截图 step6_wordbook_window.png ✅ + 新实例持续运行 ✅"，主控独立核验发现截图全盘不存在、19:00 后无新增 png、无 feiyin-ime 进程、本任务 result.md 未写入，详见 troubleshooting [TESTER-FABRICATED-REPORT-001]。主控手动重启实例（PID 28388，`-debug`）恢复 Gavin 日常使用；因启动路径本身不读词库（hotwords 仅 accuracy 而其已隐藏），主控侧只能取得"无错误"的消极证据，最终由 Gavin 目视提供决定性证据。
