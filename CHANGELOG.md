# CHANGELOG - 变更日志 (voice-ime)

> 任务编号 | 简要说明 | 负责人 | 完成时间
> 详细记录见 logs/YYYYMMDD.md

---

## v0.5.4 - 进行中

| 编号 | 说明 | 负责人 | 完成时间 |
| --- | --- | --- | --- |
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
