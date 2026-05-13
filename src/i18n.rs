use crate::config::UiLanguage;

/// All translatable UI strings.
#[allow(dead_code)]
pub struct Strings {
    pub app_title: &'static str,
    pub settings_title: &'static str,
    pub settings_nav_label: &'static str,
    // Tab labels
    pub tab_general: &'static str,
    pub tab_voice: &'static str,
    pub tab_llm: &'static str,
    pub tab_wordbook: &'static str,
    pub tab_about: &'static str,
    // Tray tooltips
    pub tray_idle: &'static str,
    pub tray_recording: &'static str,
    pub tray_processing: &'static str,
    pub tray_error: &'static str,
    // Tray menu items
    pub tray_menu_settings: &'static str,
    pub tray_menu_exit: &'static str,
    // General tab
    pub general_section: &'static str,
    pub auto_start: &'static str,
    pub hotkey_current_label: &'static str,
    pub hotkey_mode_section: &'static str,
    pub hotkey_mode_toggle_short: &'static str,
    pub hotkey_mode_ptt_short: &'static str,
    pub hotkey_presets: &'static str,
    pub ui_language_label: &'static str,
    // Voice tab
    pub input_device_section: &'static str,
    pub microphone_label: &'static str,
    pub microphone_desc: &'static str,
    pub default_device: &'static str,
    pub audio_settings_section: &'static str,
    pub silence_thresh_label: &'static str,
    pub silence_thresh_desc: &'static str,
    pub silence_dur_label: &'static str,
    pub silence_dur_desc: &'static str,
    pub max_dur_label: &'static str,
    pub max_dur_desc: &'static str,
    pub overlay_opacity_label: &'static str,
    pub overlay_opacity_desc: &'static str,
    pub enable_streaming: &'static str,
    pub enable_streaming_desc: &'static str,
    pub transcription_section: &'static str,
    pub transcription_lang_desc: &'static str,
    pub asr_lang_zh: &'static str,
    pub asr_lang_en: &'static str,
    pub chinese_output_desc: &'static str,
    pub injection_section_title: &'static str,
    pub injection_strategy_desc: &'static str,
    pub clipboard_restore_label: &'static str,
    pub clipboard_restore_desc: &'static str,
    // LLM tab
    pub llm_section: &'static str,
    pub llm_service_title: &'static str,
    pub llm_service_desc: &'static str,
    pub llm_enable: &'static str,
    pub llm_api_url: &'static str,
    pub llm_api_url_desc: &'static str,
    pub llm_api_key: &'static str,
    pub llm_api_key_desc: &'static str,
    pub llm_model: &'static str,
    pub llm_model_desc: &'static str,
    pub llm_test_btn: &'static str,
    pub llm_testing: &'static str,
    pub llm_testing_msg: &'static str,
    pub llm_prompt_hint: &'static str,
    pub llm_disabled_msg: &'static str,
    pub system_prompt_dialog_title: &'static str,
    pub system_prompt_dialog_desc: &'static str,
    /// Default system prompt for English UI (LLM optimization)
    pub default_system_prompt_en: &'static str,
    pub dialog_save: &'static str,
    pub dialog_cancel: &'static str,
    // Wordbook tab
    pub wordbook_section: &'static str,
    pub wordbook_count_format: &'static str,
    pub wordbook_desc: &'static str,
    pub wordbook_raw: &'static str,
    pub wordbook_corrected: &'static str,
    pub wordbook_add: &'static str,
    pub wordbook_delete: &'static str,
    // About tab
    pub about_section: &'static str,
    pub about_subtitle: &'static str,
    pub about_version: &'static str,
    pub about_build: &'static str,
    pub about_engine: &'static str,
    pub check_update_btn: &'static str,
    pub not_supported: &'static str,
    // Misc
    pub save_btn: &'static str,
    pub save_ok: &'static str,
    pub ui_language_section: &'static str,
    pub ui_lang_zh: &'static str,
    pub ui_lang_en: &'static str,
    pub overlay_recording: &'static str,
    pub overlay_cancel_hint: &'static str,
    pub overlay_cancel_btn: &'static str,
    pub overlay_transcribing: &'static str,
    pub overlay_optimizing: &'static str,
    pub overlay_injecting: &'static str,
    pub overlay_processing: &'static str,
    pub chinese_script_section: &'static str,
    pub chinese_script_simplified: &'static str,
    pub chinese_script_traditional: &'static str,
    pub preview_copy_btn: &'static str,
    pub preview_copied: &'static str,
    pub preview_title: &'static str,
    pub preview_close: &'static str,
    pub preview_esc_hint: &'static str,
    pub preview_title_bar: &'static str,
    pub auto_saved: &'static str,
    pub save_failed: &'static str,
    pub overlay_error: &'static str,
    pub error_network_timeout: &'static str,
    pub error_api_unavailable: &'static str,
    pub error_model_init: &'static str,
    pub error_microphone: &'static str,
    pub error_mic_muted: &'static str,
    pub error_transcription_empty: &'static str,
}

static ZH: Strings = Strings {
    app_title: "飞音语音输入",
    settings_title: "飞音语音输入 - 设置",
    settings_nav_label: "导航",
    tab_general: "通用",
    tab_voice: "语音输入",
    tab_llm: "优化模型",
    tab_wordbook: "词库",
    tab_about: "关于",
    tray_idle: "飞音语音输入 - 待机",
    tray_recording: "飞音语音输入 - 录音中...",
    tray_processing: "飞音语音输入 - 处理中...",
    tray_error: "飞音语音输入 - 错误",
    tray_menu_settings: "设置 (Settings)",
    tray_menu_exit: "退出 (Exit)",
    general_section: "通用设置",
    auto_start: "开机自动启动",
    hotkey_current_label: "当前快捷键",
    hotkey_mode_section: "触发方式",
    hotkey_mode_toggle_short: "切换模式（按一次开始，再按一次结束）",
    hotkey_mode_ptt_short: "按住说话（按住录音，松开结束）",
    hotkey_presets: "快捷键预设",
    ui_language_label: "界面语言",
    input_device_section: "输入设备",
    microphone_label: "麦克风设备",
    microphone_desc: "选择音频输入设备，留空则使用系统默认。",
    default_device: "默认设备",
    audio_settings_section: "音频设置",
    silence_thresh_label: "静音阈值",
    silence_thresh_desc: "检测环境静音的灵敏度",
    silence_dur_label: "静音时长",
    silence_dur_desc: "结束录音的静音判定（毫秒）",
    max_dur_label: "最长录音",
    max_dur_desc: "单次录音允许的最大秒数",
    overlay_opacity_label: "悬浮窗透明度",
    overlay_opacity_desc: "控制录音悬浮条的透明程度",
    enable_streaming: "开启流式输入",
    enable_streaming_desc: "实时预览转录结果，录音结束后进行二次修正以提高准确率",
    transcription_section: "转录设置",
    transcription_lang_desc:
        "选择语音识别语言。中文使用 Paraformer（高准确率），英文使用 Whisper。",
    asr_lang_zh: "中文",
    asr_lang_en: "英文",
    chinese_output_desc: "统一指定最终输出为简体或繁体中文。",
    injection_section_title: "文本注入",
    injection_strategy_desc: "默认推荐剪贴板粘贴；如有兼容性问题再切换注入方式。",
    clipboard_restore_label: "剪贴板恢复延迟",
    clipboard_restore_desc: "恢复原剪贴板内容的延迟（毫秒）",
    llm_section: "LLM 配置",
    llm_service_title: "文本优化服务",
    llm_service_desc: "配置模型、接口地址和系统提示词，用于转录后的文本优化。",
    llm_enable: "启用 LLM 优化",
    llm_api_url: "接口地址",
    llm_api_url_desc: "LLM 服务地址",
    llm_api_key: "接口密钥",
    llm_api_key_desc: "用于服务鉴权",
    llm_model: "模型名称",
    llm_model_desc: "文本优化所用模型",
    llm_test_btn: "测试连接",
    llm_testing: "测试中...",
    llm_testing_msg: "正在测试连接...",
    llm_prompt_hint: "按 Ctrl+T 编辑系统提示词",
    llm_disabled_msg: "LLM 优化已禁用，语音输入将直接输出转录结果。",
    system_prompt_dialog_title: "系统提示词编辑",
    system_prompt_dialog_desc: "用于约束优化后的语言风格、错别字修正和 Markdown 格式。",
    default_system_prompt_en: r#"You are a professional voice input correction and formatting expert. I will provide you with raw text transcribed from speech. Process according to these rules:

1. **Transcription Error Correction ONLY**: Only fix errors caused by speech recognition mistakes (homophones, misheard words, similar-sounding substitutions that make the text nonsensical). DO NOT change words the user clearly intended, including:
   - English words mixed in Chinese (OK, PPT, API, app, URL, etc.)
   - Technical terms and jargon
   - Internet slang and colloquial expressions
   - Any word that makes sense in context, even if informal

2. **Punctuation**: Add appropriate punctuation if the input lacks it. Rules:
   - End every sentence/statement with a period (.)
   - End every question with a question mark (?)
     - Chinese: detect 吗/呢/吧/什么/谁/哪/怎么
     - English: detect What/Where/When/Who/Why/How/Is/Are/Do/Can/Would/Will at start, or questioning tone
     - Other languages: detect question words or questioning semantics
   - Use commas (,) at pause points, clause separations, and list items
   - Use exclamation marks (!) for emphatic/urgent expressions

3. **Filler Removal**: Remove filler words (um, uh, 嗯, 啊, 那个, 就是说) that add no semantic value. Keep words serving grammatical/semantic purposes.

4. **Markdown Formatting**: Use headings and paragraph breaks where semantically appropriate.

5. **List Formatting**: Convert enumeration (第一点/第二点, firstly/secondly) to Markdown lists.

6. **Wordbook Priority**: Before applying any correction, check the provided <wordbook> mappings. If a phrase matches a wordbook entry, use the mapped replacement EXACTLY. These are user-defined preferences that override default correction logic.

Example: If wordbook contains "PPT -> 演示文稿" and input contains "PPT", output should use "演示文稿" (or keep "PPT" depending on mapping direction).

7. **Wordbook Suggestions**: After the corrected text, if you detect a stable correction pair that should be learned into the wordbook, append exactly one JSON object on a new final line:
{"suggestions":[{"raw":"...","corrected":"..."}]}
Only use this JSON line for suggestions. If there are no suggestions, omit it entirely.

Return ONLY the processed text. No explanations."#,
    dialog_save: "保存",
    dialog_cancel: "取消",
    wordbook_section: "词库管理",
    wordbook_count_format: "词库管理（{} 条）",
    wordbook_desc: "维护常用术语映射，让转录结果更符合你的日常输入习惯。",
    wordbook_raw: "原词",
    wordbook_corrected: "修正词",
    wordbook_add: "添加",
    wordbook_delete: "删除",
    about_section: "关于软件",
    about_subtitle: "智能语音转文字，高效输入工具",
    about_version: "版本",
    about_build: "构建",
    about_engine: "引擎",
    check_update_btn: "检测更新",
    not_supported: "暂不支持",
    save_btn: "💾 保存配置",
    save_ok: "✓ 配置已保存，重启后生效",
    ui_language_section: "界面语言 / UI Language",
    ui_lang_zh: "中文",
    ui_lang_en: "英文",
    overlay_recording: "录音中...",
    overlay_cancel_hint: "(按Esc 取消)",
    overlay_cancel_btn: "取消",
    overlay_transcribing: "转录中...",
    overlay_optimizing: "优化中...",
    overlay_injecting: "注入中...",
    overlay_processing: "识别处理中...",
    chinese_script_section: "中文输出",
    chinese_script_simplified: "简体中文",
    chinese_script_traditional: "繁体中文",
    preview_copy_btn: "复制",
    preview_copied: "已复制",
    preview_title: "语音输入结果",
    preview_close: "关闭",
    preview_esc_hint: "(按Esc 取消)",
    preview_title_bar: "输入文本",
    auto_saved: "已自动保存",
    save_failed: "保存失败：",
    overlay_error: "识别出错",
    error_network_timeout: "网络超时，请重试。",
    error_api_unavailable: "服务不可用，请检查网络或 API 设置。",
    error_model_init: "语音模型初始化失败。",
    error_microphone: "麦克风不可用。",
    error_mic_muted: "麦克风已静音，请取消静音后重试。",
    error_transcription_empty: "识别结果为空。",
};

static ZH_TW: Strings = Strings {
    app_title: "飛音語音輸入",
    settings_title: "飛音語音輸入 - 設定",
    settings_nav_label: "導航",
    tab_general: "通用",
    tab_voice: "語音輸入",
    tab_llm: "優化模型",
    tab_wordbook: "詞庫",
    tab_about: "關於",
    tray_idle: "飛音語音輸入 - 待機",
    tray_recording: "飛音語音輸入 - 錄音中...",
    tray_processing: "飛音語音輸入 - 處理中...",
    tray_error: "飛音語音輸入 - 錯誤",
    tray_menu_settings: "設定 (Settings)",
    tray_menu_exit: "退出 (Exit)",
    general_section: "通用設定",
    auto_start: "開機自動啟動",
    hotkey_current_label: "當前快捷鍵",
    hotkey_mode_section: "觸發方式",
    hotkey_mode_toggle_short: "切換模式（按一次開始，再按一次結束）",
    hotkey_mode_ptt_short: "按住說話（按住錄音，鬆開結束）",
    hotkey_presets: "快捷鍵預設",
    ui_language_label: "介面語言",
    input_device_section: "輸入裝置",
    microphone_label: "麥克風裝置",
    microphone_desc: "選擇音訊輸入裝置，留空則使用系統預設。",
    default_device: "預設裝置",
    audio_settings_section: "音訊設定",
    silence_thresh_label: "靜音閾值",
    silence_thresh_desc: "檢測環境靜音的靈敏度",
    silence_dur_label: "靜音時長",
    silence_dur_desc: "結束錄音的靜音判定（毫秒）",
    max_dur_label: "最長錄音",
    max_dur_desc: "單次錄音允許的最大秒數",
    overlay_opacity_label: "懸浮窗透明度",
    overlay_opacity_desc: "控制錄音懸浮條的透明程度",
    enable_streaming: "開啟串流輸入",
    enable_streaming_desc: "即時預覽轉錄結果，錄音結束後進行二次修正以提高準確率",
    transcription_section: "轉錄設定",
    transcription_lang_desc:
        "選擇語音識別語言。中文使用 Paraformer（高準確率），英文使用 Whisper。",
    asr_lang_zh: "中文",
    asr_lang_en: "英文",
    chinese_output_desc: "統一指定最終輸出為簡體或繁體中文。",
    injection_section_title: "文字注入",
    injection_strategy_desc: "預設推薦剪貼簿貼上；如有相容性問題再切換注入方式。",
    clipboard_restore_label: "剪貼簿恢復延遲",
    clipboard_restore_desc: "恢復原剪貼簿內容的延遲（毫秒）",
    llm_section: "LLM 設定",
    llm_service_title: "文字優化服務",
    llm_service_desc: "設定模型、介面地址和系統提示詞，用於轉錄後的文字優化。",
    llm_enable: "啟用 LLM 優化",
    llm_api_url: "介面地址",
    llm_api_url_desc: "LLM 服務地址",
    llm_api_key: "介面金鑰",
    llm_api_key_desc: "用於服務鑑權",
    llm_model: "模型名稱",
    llm_model_desc: "文字優化所用模型",
    llm_test_btn: "測試連線",
    llm_testing: "測試中...",
    llm_testing_msg: "正在測試連線...",
    llm_prompt_hint: "按 Ctrl+T 編輯系統提示詞",
    llm_disabled_msg: "LLM 優化已禁用，語音輸入將直接輸出轉錄結果。",
    system_prompt_dialog_title: "系統提示詞編輯",
    system_prompt_dialog_desc: "用於約束優化後的語言風格、錯別字修正和 Markdown 格式。",
    default_system_prompt_en: r#"You are a professional voice input correction and formatting expert. I will provide you with raw text transcribed from speech. Process according to these rules:

1. **Transcription Error Correction ONLY**: Only fix errors caused by speech recognition mistakes (homophones, misheard words, similar-sounding substitutions that make the text nonsensical). DO NOT change words the user clearly intended, including:
   - English words mixed in Chinese (OK, PPT, API, app, URL, etc.)
   - Technical terms and jargon
   - Internet slang and colloquial expressions
   - Any word that makes sense in context, even if informal

2. **Punctuation**: Add appropriate punctuation if the input lacks it. Rules:
   - End every sentence/statement with a period (.)
   - End every question with a question mark (?)
     - Chinese: detect 嗎/呢/吧/什麼/誰/哪/怎麼
     - English: detect What/Where/When/Who/Why/How/Is/Are/Do/Can/Would/Will at start, or questioning tone
     - Other languages: detect question words or questioning semantics
   - Use commas (,) at pause points, clause separations, and list items
   - Use exclamation marks (!) for emphatic/urgent expressions

3. **Filler Removal**: Remove filler words (um, uh, 嗯, 啊, 那個, 就是說) that add no semantic value. Keep words serving grammatical/semantic purposes.

4. **Markdown Formatting**: Use headings and paragraph breaks where semantically appropriate.

5. **List Formatting**: Convert enumeration (第一點/第二點, firstly/secondly) to Markdown lists.

6. **Wordbook Priority**: Before applying any correction, check the provided <wordbook> mappings. If a phrase matches a wordbook entry, use the mapped replacement EXACTLY. These are user-defined preferences that override default correction logic.

Example: If wordbook contains "PPT -> 演示文稿" and input contains "PPT", output should use "演示文稿" (or keep "PPT" depending on mapping direction).

7. **Wordbook Suggestions**: After the corrected text, if you detect a stable correction pair that should be learned into the wordbook, append exactly one JSON object on a new final line:
{"suggestions":[{"raw":"...","corrected":"..."}]}
Only use this JSON line for suggestions. If there are no suggestions, omit it entirely.

Return ONLY the processed text. No explanations."#,
    dialog_save: "儲存",
    dialog_cancel: "取消",
    wordbook_section: "詞庫管理",
    wordbook_count_format: "詞庫管理（{} 條）",
    wordbook_desc: "維護常用術語映射，讓轉錄結果更符合你的日常輸入習慣。",
    wordbook_raw: "原詞",
    wordbook_corrected: "修正詞",
    wordbook_add: "新增",
    wordbook_delete: "刪除",
    about_section: "關於軟體",
    about_subtitle: "智慧語音轉文字，高效輸入工具",
    about_version: "版本",
    about_build: "構建",
    about_engine: "引擎",
    check_update_btn: "檢測更新",
    not_supported: "暫不支援",
    save_btn: "💾 儲存設定",
    save_ok: "✓ 設定已儲存，重啟後生效",
    ui_language_section: "介面語言 / UI Language",
    ui_lang_zh: "簡體中文",
    ui_lang_en: "繁體中文",
    overlay_recording: "錄音中...",
    overlay_cancel_hint: "(按Esc 取消)",
    overlay_cancel_btn: "取消",
    overlay_transcribing: "轉錄中...",
    overlay_optimizing: "優化中...",
    overlay_injecting: "注入中...",
    overlay_processing: "識別處理中...",
    chinese_script_section: "中文輸出",
    chinese_script_simplified: "簡體中文",
    chinese_script_traditional: "繁體中文",
    preview_copy_btn: "複製",
    preview_copied: "已複製",
    preview_title: "語音輸入結果",
    preview_close: "關閉",
    preview_esc_hint: "(按Esc 取消)",
    preview_title_bar: "輸入文字",
    auto_saved: "已自動儲存",
    save_failed: "儲存失敗：",
    overlay_error: "識別出錯",
    error_network_timeout: "網路超時，請重試。",
    error_api_unavailable: "服務不可用，請檢查網路或 API 設定。",
    error_model_init: "語音模型初始化失敗。",
    error_microphone: "麥克風不可用。",
    error_mic_muted: "麥克風已靜音，請取消靜音後重試。",
    error_transcription_empty: "識別結果為空。",
};

static EN: Strings = Strings {
    app_title: "Feiyin Voice Input",
    settings_title: "Feiyin Voice Input - Settings",
    settings_nav_label: "Navigation",
    tab_general: "General",
    tab_voice: "Voice",
    tab_llm: "LLM",
    tab_wordbook: "Wordbook",
    tab_about: "About",
    tray_idle: "Feiyin Voice Input - Idle",
    tray_recording: "Feiyin Voice Input - Recording...",
    tray_processing: "Feiyin Voice Input - Processing...",
    tray_error: "Feiyin Voice Input - Error",
    tray_menu_settings: "Settings",
    tray_menu_exit: "Exit",
    general_section: "General Settings",
    auto_start: "Auto-start on boot",
    hotkey_current_label: "Current hotkey",
    hotkey_mode_section: "Trigger mode",
    hotkey_mode_toggle_short: "Toggle (press to start, press again to stop)",
    hotkey_mode_ptt_short: "Push-to-talk (hold to record, release to stop)",
    hotkey_presets: "Hotkey presets",
    ui_language_label: "UI Language",
    input_device_section: "Input Device",
    microphone_label: "Microphone device",
    microphone_desc: "Select audio input device. Leave empty for system default.",
    default_device: "Default",
    audio_settings_section: "Audio Settings",
    silence_thresh_label: "Silence threshold",
    silence_thresh_desc: "Sensitivity for detecting silence",
    silence_dur_label: "Silence duration",
    silence_dur_desc: "Silence timeout to end recording (ms)",
    max_dur_label: "Max duration",
    max_dur_desc: "Maximum recording duration (seconds)",
    overlay_opacity_label: "Overlay opacity",
    overlay_opacity_desc: "Transparency of the recording overlay bar",
    enable_streaming: "Enable streaming input",
    enable_streaming_desc: "Preview transcription in real-time, then refine after recording ends",
    transcription_section: "Transcription Settings",
    transcription_lang_desc: "Select speech recognition language. Chinese uses Paraformer (high accuracy), English uses Whisper.",
    asr_lang_zh: "Chinese",
    asr_lang_en: "English",
    chinese_output_desc: "Specify output as Simplified or Traditional Chinese.",
    injection_section_title: "Text Injection",
    injection_strategy_desc: "Clipboard paste is recommended. Switch injection method if compatibility issues occur.",
    clipboard_restore_label: "Clipboard restore delay",
    clipboard_restore_desc: "Delay before restoring original clipboard (ms)",
    llm_section: "LLM Configuration",
    llm_service_title: "Text Optimization Service",
    llm_service_desc: "Configure model, API URL and system prompt for post-transcription text optimization.",
    llm_enable: "Enable LLM Optimization",
    llm_api_url: "API URL",
    llm_api_url_desc: "LLM service endpoint",
    llm_api_key: "API Key",
    llm_api_key_desc: "For service authentication",
    llm_model: "Model",
    llm_model_desc: "Model for text optimization",
    llm_test_btn: "Test connection",
    llm_testing: "Testing...",
    llm_testing_msg: "Testing connection...",
    llm_prompt_hint: "Press Ctrl+T to edit system prompt",
    llm_disabled_msg: "LLM optimization disabled. Voice input will output transcription directly.",
    system_prompt_dialog_title: "System Prompt Editor",
    system_prompt_dialog_desc: "Define language style, typo correction, and Markdown formatting rules.",
    default_system_prompt_en: r#"You are a professional voice input correction and formatting expert. I will provide you with raw text transcribed from speech. Process according to these rules:

1. **Transcription Error Correction ONLY**: Only fix errors caused by speech recognition mistakes (homophones, misheard words, similar-sounding substitutions that make the text nonsensical). DO NOT change words the user clearly intended, including:
   - English words mixed in Chinese (OK, PPT, API, app, URL, etc.)
   - Technical terms and jargon
   - Internet slang and colloquial expressions
   - Any word that makes sense in context, even if informal

2. **Punctuation**: Add appropriate punctuation if the input lacks it. Rules:
   - End every sentence/statement with a period (.)
   - End every question with a question mark (?)
     - Chinese: detect 吗/呢/吧/什么/谁/哪/怎么
     - English: detect What/Where/When/Who/Why/How/Is/Are/Do/Can/Would/Will at start, or questioning tone
     - Other languages: detect question words or questioning semantics
   - Use commas (,) at pause points, clause separations, and list items
   - Use exclamation marks (!) for emphatic/urgent expressions

3. **Filler Removal**: Remove filler words (um, uh, 嗯, 啊, 那个, 就是说) that add no semantic value. Keep words serving grammatical/semantic purposes.

4. **Markdown Formatting**: Use headings and paragraph breaks where semantically appropriate.

5. **List Formatting**: Convert enumeration (第一点/第二点, firstly/secondly) to Markdown lists.

6. **Wordbook Priority**: Before applying any correction, check the provided <wordbook> mappings. If a phrase matches a wordbook entry, use the mapped replacement EXACTLY. These are user-defined preferences that override default correction logic.

Example: If wordbook contains "PPT -> 演示文稿" and input contains "PPT", output should use "演示文稿" (or keep "PPT" depending on mapping direction).

7. **Wordbook Suggestions**: After the corrected text, if you detect a stable correction pair that should be learned into the wordbook, append exactly one JSON object on a new final line:
{"suggestions":[{"raw":"...","corrected":"..."}]}
Only use this JSON line for suggestions. If there are no suggestions, omit it entirely.

Return ONLY the processed text. No explanations."#,
    dialog_save: "Save",
    dialog_cancel: "Cancel",
    wordbook_section: "Word Library",
    wordbook_count_format: "Word Library ({})",
    wordbook_desc: "Maintain custom term mappings to match your input habits.",
    wordbook_raw: "Original",
    wordbook_corrected: "Replacement",
    wordbook_add: "Add",
    wordbook_delete: "Delete",
    about_section: "About",
    about_subtitle: "Smart voice-to-text, efficient input tool",
    about_version: "Version",
    about_build: "Build",
    about_engine: "Engine",
    check_update_btn: "Check for updates",
    not_supported: "Not available",
    save_btn: "💾 Save",
    save_ok: "✓ Saved. Restart to apply.",
    ui_language_section: "界面语言 / UI Language",
    ui_lang_zh: "中文",
    ui_lang_en: "English",
    overlay_recording: "Recording...",
    overlay_cancel_hint: "(Press Esc to cancel)",
    overlay_cancel_btn: "Cancel",
    overlay_transcribing: "Transcribing...",
    overlay_optimizing: "Optimizing...",
    overlay_injecting: "Injecting...",
    overlay_processing: "Processing...",
    chinese_script_section: "Chinese Output",
    chinese_script_simplified: "Simplified Chinese",
    chinese_script_traditional: "Traditional Chinese",
    preview_copy_btn: "Copy",
    preview_copied: "Copied",
    preview_title: "Voice Input Result",
    preview_close: "Close",
    preview_esc_hint: "(Press Esc to cancel)",
    preview_title_bar: "Input Text",
    auto_saved: "Auto saved",
    save_failed: "Save failed: ",
    overlay_error: "Recognition error",
    error_network_timeout: "Network timeout. Please try again.",
    error_api_unavailable: "Service unavailable. Check network/API settings.",
    error_model_init: "Speech model initialization failed.",
    error_microphone: "Microphone is unavailable.",
    error_mic_muted: "Microphone is muted. Please unmute and try again.",
    error_transcription_empty: "Transcription result is empty.",
};

pub fn get(lang: UiLanguage) -> &'static Strings {
    match lang {
        UiLanguage::Chinese => &ZH,
        UiLanguage::TraditionalChinese => &ZH_TW,
        UiLanguage::English => &EN,
    }
}
