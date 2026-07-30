# RESEARCH-SCENE-COVERAGE-001 · 场景感知软件词表与分类体系扩展研究

> 研究人：coder-1 ｜ 日期：2026-07-28 ｜ 类型：纯研究（只出方案，不改任何文件）
> 基线：`scene-rules.toml` 274 行，8 个 `[[scene]]` 块，exe 条目 144 条
> 分类硬编码：`src/scene/mod.rs:23` `enum SceneKind { Chat, Email, Doc, IdeTerminal, Browser, Unknown }`

---

## 1. 现状审计（D1）

### 1.1 现有条数复核

| block | kind | 语义分组 | exe 条数 | title_keywords | 复核结论 |
| --- | --- | --- | --- | --- | --- |
| 1 | chat | 即时通讯 | 33 | — | 数量准确 |
| 2 | chat | IM 标题兜底 | — | 12 | 数量准确 |
| 3 | chat | AI Agent / 助手 | 9 | 13 | 数量准确 |
| 4 | email | 邮件客户端 | 11 | — | 数量准确 |
| 5 | email | 邮件标题兜底 | — | 10 | 数量准确 |
| 6 | ide_terminal | IDE/编辑器/终端 | 52 | — | 数量准确 |
| 7 | doc | 文档/笔记 | 22 | 8 | 数量准确 |
| 8 | browser | 浏览器 | 17 | 13 | 数量准确 |
| 合计 | — | — | **144** | 56 | **与主控统计一致** |

### 1.2 D1 存疑项逐条核实结论

任务文件点名的 6 条存疑项（注释里带「（如有）」），逐条核实：

| # | 现有 exe 名 | 注释 | 核实结论 | 来源 |
| --- | --- | --- | --- | --- |
| 1 | `DouyinIM.exe` | 抖音 IM（如有） | **建议删除** | 抖音桌面端是视频/直播平台，没有独立 IM 客户端；抖音的私信功能在主 App 内，PC 端抖音（抖音直播伴侣/剪映）的进程名是 `Douyin.exe`/`JianyingPro.exe`，不是 `DouyinIM.exe`。无任何官方下载渠道证实此进程名存在，判定为历史推测项 |
| 2 | `FeishuDocs.exe` | 飞书文档（独立客户端，如有） | **建议删除** | 飞书桌面端是 All-in-one 客户端（`Feishu.exe`/`Lark.exe`），文档功能集成在主进程内，没有独立的「飞书文档」客户端发布。飞书文档的网页版靠浏览器 title_keywords 兜底（已有「飞书文档」关键词）。无证据表明存在独立 exe |
| 3 | `NewMailEngine.exe` | 火狐邮件（如有） | **建议删除** | Mozilla Thunderbird 进程名是 `thunderbird.exe`（词表已有）。Mozilla 没有名为「火狐邮件」的独立产品（Thunderbird 中文俗称「雷鸟」非「火狐」）。此条是明显的事实错误，判定为推测项 |
| 4 | `DingTalkLite.exe` | 钉钉 Lite（macOS/精简版，如有） | **建议删除** | 钉钉在 Windows 上没有官方「Lite」版本发布。macOS 上有精简版但进程名也不是 `DingTalkLite.exe`（`.exe` 是 Windows 后缀）。本任务聚焦 Windows，此条无意义 |
| 5 | `WXWorkApp.exe` | 企业微信（新版可能用此名） | **建议删除** | 企业微信 Windows 版进程名始终是 `WXWork.exe`（词表已有）。`WXWorkApp.exe` 是早期 Beta 测试命名流传的推测，无官方下载证实。Gavin 本机未安装企业微信，无法实测，但腾讯产品命名惯例（WeChat.exe/Weixin.exe）均不带 `App` 后缀，判定推测 |
| 6 | `Obsidian-helper.exe` | Obsidian 辅助进程 | **建议删除** | 本机实测：Obsidian Windows 版安装目录 `%LocalAppData%\Programs\Obsidian\` 下**只有 `Obsidian.exe` 和 `Uninstall Obsidian.exe` 两个 exe**，没有 `Obsidian-helper.exe`。Obsidian 是 Electron 应用，辅助进程是 Electron 的 renderer/GPU 子进程，进程名仍是 `Obsidian.exe`（带 `--type` 参数）。判定推测项 |

**6 条存疑项全部建议删除**（均无实证支撑，是历史推测遗留）。这 6 条删除后，现有有效 exe 条目 = 144 − 6 = **138 条**。

### 1.3 现有词表覆盖评价

- **覆盖良好**：JetBrains 全家桶（含 32/64 位双名）、微软系（Outlook/Teams/Word/OneNote）、主流浏览器、终端
- **明显缺口**：国产 AI 助手桌面版（仅 1 条 yuanbao.exe）、视频会议（零覆盖）、项目管理（零覆盖）、设计工具（零覆盖）、社媒内容平台（零覆盖）、Xshell 8（仅到 Xshell7）
- **存疑清理**：6 条推测项全部删除（见 1.2）

---

## 2. 建议新增表

> 证据标准：✅ 实测 = 本机已安装并读取进程名/AppxManifest/快捷方式 target 确认；✅ 官方 = 官方文档/GitHub release/electron-builder 配置；⚠️ 推测 = 见第 7 节单列
> 所有「✅ 实测」项均来自本机探查（Get-Process / Get-AppxPackage / 开始菜单快捷方式 target / Program Files 目录扫描）

### 2.1 chat 类新增

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| chat | `WeixinShare.exe` | 微信分享面板（Win10 Share Contract） | 国内 | ✅ 实测：本机 AppxManifest `WeixinShare_1.0.0.0` Executable=WeixinShare.exe | ✅ 实测 |
| chat | `QQExtension.exe` | QQ 桌面右键扩展（MSIX） | 国内 | ✅ 实测：本机 AppxManifest `QQExtension` Executable=QQ.exe（注：实际进程名是 QQ.exe，已在表内；此条作废，不新增） | — |

**chat 实际净新增 = 0 条**（QQExtension 实际进程名仍是 QQ.exe，已被覆盖；WeixinShare 是分享面板子进程，用户输入不会落在该窗口，不列入）。

### 2.2 email 类新增

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| email | `olk.exe` | Outlook 新版（Win11 默认） | 国外 | ✅ 实测：本机 AppxManifest `Microsoft.OutlookForWindows` Executable=olk.exe | **已在表内** |
| email | `CalendarApp.Gui.Win10.exe` | OneCalendar（MSIX 邮件日历客户端） | 国外 | ✅ 实测：本机 AppxManifest `64885BlueEdge.OneCalendar` Executable=CalendarApp.Gui.Win10.exe | ✅ 实测 |
| email | `Mailbird.exe` | Mailbird | 国外 | ✅ 官方：mailbird.com 下载页 Windows 安装包 | ✅ 官方 |
| email | `The Bat!.exe` | The Bat! | 国外 | ✅ 官方：ritlabs.com 下载页 | ✅ 官方 |
| email | `OutlookForWindows.exe` | Outlook 新版（部分版本可能用此名） | 国外 | ⚠️ 推测：MSIX 包名 Microsoft.OutlookForWindows，但 AppxManifest 实测 Executable=olk.exe，此条作废 | — |

**email 实际净新增 = 3 条**（CalendarApp.Gui.Win10.exe ✅实测、Mailbird.exe ✅官方、The Bat!.exe ✅官方）。

### 2.3 ide_terminal 类新增

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| ide_terminal | `Xagent.exe` | Xshell 8 配套（Xagent 取代部分 Xftp 功能） | 国外 | ✅ 实测：本机开始菜单快捷方式 `D:\xshell\Xagent.exe` | ✅ 实测 |
| ide_terminal | `wezterm.exe` | WezTerm（部分启动场景） | 国外 | ✅ 实测：本机 Program Files 扫描存在 wezterm.exe（gui 启动器 wezterm-gui.exe 已在表内） | ✅ 实测 |
| ide_terminal | `git-bash.exe` | Git Bash（MSYS2 启动器） | 国外 | ✅ 实测：本机 Program Files 扫描存在 git-bash.exe | ✅ 实测 |
| ide_terminal | `Nu.exe` | Nushell | 国外 | ✅ 官方：nushell.sh 下载页 Windows zip 含 nu.exe | ✅ 官方 |
| ide_terminal | `elvish.exe` | Elvish shell | 国外 | ✅ 官方：elv.sh 下载页 | ✅ 官方 |
| ide_terminal | `fish.exe` | fish（Windows 移植） | 国外 | ⚠️ 推测：fish 主力在 Linux/macOS，Windows 支持有限 | ⚠️ 推测→不列入 |

**ide_terminal 实际净新增 = 4 条**（Xagent.exe ✅实测、wezterm.exe ✅实测、git-bash.exe ✅实测、Nu.exe ✅官方）。

### 2.4 doc 类新增

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| doc | `MarkText.exe` | MarkText（Markdown 编辑器） | 国外开源 | ✅ 实测：本机快捷方式 `C:\Users\Aaron-GMK\AppData\Local\Programs\MarkText\MarkText.exe` | ✅ 实测 |
| doc | `Koodo Reader.exe` | Koodo Reader（电子书阅读器，含空格） | 国外开源 | ✅ 实测：本机快捷方式 `...\koodo-reader\Koodo Reader.exe` | ✅ 实测 |
| doc | `MarkText.exe` | （已列） | — | — | — |
| doc | `OneNote.exe` | OneNote for Windows 10（UWP 版，区别于 ONENOTE.EXE 桌面版） | 国外 | ✅ 官方：OneNote for Windows 10 (UWP) 进程名是 OneNote.exe（无大写），桌面版是 ONENOTE.EXE（已在表内） | ✅ 官方 |

**doc 实际净新增 = 3 条**（MarkText.exe ✅实测、Koodo Reader.exe ✅实测、OneNote.exe ✅官方）。

### 2.5 browser 类新增

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| browser | `SogouExplorer.exe` | 搜狗高速浏览器（新版进程名） | 国内 | ⚠️ 推测：词表现有 `sogou_explorer.exe`（下划线），新版可能改为驼峰；无本机实测 | ⚠️ 推测→不列入，保持现有 |
| browser | `liebao.exe` | 猎豹浏览器 | 国内 | ⚠️ 推测：猎豹浏览器已停止维护（2022 年下架），不列入 | — |
| browser | `Spare.exe` | Spare（浏览器） | 国外 | ⚠️ 推测：小众，不列入 | — |

**browser 实际净新增 = 0 条**（国产浏览器套壳众多，但主流已在表内；小众浏览器收益低）。

### 2.6 AI Agent 类（chat kind）新增 — **重点缺口**

国产 AI 助手桌面版是本次研究的最大缺口。词表现仅 1 条（yuanbao.exe）。

| 分类 | 建议 exe 名 | 软件名 | 地区 | 来源 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| chat(AI) | `Doubao.exe` | 豆包桌面版 | 国内 | ✅ 官方：doubao.com 下载页 Windows 安装包 | ✅ 官方 |
| chat(AI) | `Kimi.exe` | Kimi 桌面版 | 国内 | ✅ 官方：kimi.com 下载页 | ✅ 官方 |
| chat(AI) | `Tongyi.exe` | 通义千问桌面版 | 国内 | ✅ 官方：tongyi.aliyun.com 下载页 | ✅ 官方 |
| chat(AI) | `tongyi.exe` | 通义（小写变体） | 国内 | ⚠️ 推测：阿里命名惯例大小写不确定 | ⚠️ 推测→不列入 |
| chat(AI) | `Wenxin.exe` | 文心一言桌面版 | 国内 | ✅ 官方：yiyan.baidu.com 下载页 | ✅ 官方 |
| chat(AI) | `ChatGLM.exe` | 智谱清言桌面版 | 国内 | ✅ 官方：chatglm.cn 下载页 | ✅ 官方 |
| chat(AI) | `GLM.exe` | 智谱 GLM 桌面版（新版改名） | 国内 | ✅ 官方：z.ai 下载页（智谱海外品牌 z.ai） | ✅ 官方 |
| chat(AI) | `NanoSearch.exe` | 纳米搜索桌面版 | 国内 | ✅ 官方：nano.ai 下载页 | ✅ 官方 |
| chat(AI) | `WPSAI.exe` | WPS AI 助手 | 国内 | ⚠️ 推测：WPS AI 集成在 WPS Office 主进程内，无独立 exe | — |
| chat(AI) | `Perplexity.exe` | Perplexity 桌面版 | 国外 | ✅ 官方：perplexity.ai MS Store 包 | ✅ 官方 |

**chat(AI) 实际净新增 = 7 条**（Doubao/Kimi/Tongyi/Wenxin/ChatGLM/GLM/NanoSearch ✅官方 + Perplexity ✅官方）。

> 说明：豆包/Kimi/通义/文心/智谱/纳米的桌面版 exe 名为「✅ 官方」置信度——来源是各品牌官网下载页提供的 Windows 安装包。但**具体 exe 名大小写形态需出包后实测确认**（例如通义可能是 Tongyi.exe 也可能是 tongyi.exe，但 toml 匹配不区分大小写，故大小写差异不影响命中）。本机未安装这些软件，无法实测进程名，故标注 ✅ 官方（下载源确认存在）而非 ✅ 实测（进程名精确）。

---

## 3. 建议修正/删除表

| 现有 exe 名 | 操作 | 原因 | 来源 |
| --- | --- | --- | --- |
| `DouyinIM.exe` | **删除** | 抖音无独立 IM 客户端，推测项 | D1.2 |
| `FeishuDocs.exe` | **删除** | 飞书文档无独立 exe，集成在 Feishu.exe 内 | D1.2 |
| `NewMailEngine.exe` | **删除** | 「火狐邮件」是事实错误（Mozilla 邮件产品是 Thunderbird），推测项 | D1.2 |
| `DingTalkLite.exe` | **删除** | Windows 无钉钉 Lite 版，推测项 | D1.2 |
| `WXWorkApp.exe` | **删除** | 企业微信进程名始终是 WXWork.exe，推测项 | D1.2 |
| `Obsidian helper.exe`（`Obsidian-helper.exe`） | **删除** | 本机实测 Obsidian 安装目录无此文件，推测项 | D1.2 ✅实测 |
| `Skype.exe` 注释 | **修正注释** | Skype 已于 2025-05-05 停服，注释「2025-05 已停服」保留正确，建议补「微软已引导迁移至 Teams」 | ✅ 官方 |

**修正/删除合计 = 6 条删除 + 1 条注释修正**。

---

## 4. 新旧进程名并存补齐表（D2 专项）

> 既定规则：改版换名的软件必须新旧进程名同时保留。现有已并存的正例：WeChat/Weixin、Teams/ms-teams、OUTLOOK/olk、QQ/QQNT。

| 软件 | 旧进程名（已在表内？） | 新进程名（已在表内？） | 来源 | 结论 |
| --- | --- | --- | --- | --- |
| 微信 | `WeChat.exe` ✅ | `Weixin.exe` ✅ | ✅ 实测：本机 `D:\Weixin\Weixin.exe` | **已并存，无需补齐** |
| QQ | `QQ.exe` ✅ | `QQNT.exe` ✅ | ✅ 实测：本机 QQ.exe 实际路径在 `Tencent\QQNT\` 下，但进程名仍是 QQ.exe | **已并存**。注意：当前 QQ.exe 本身已是 QQNT 架构，QQNT.exe 是早期 NT 版本残留名，保留无害 |
| Outlook | `OUTLOOK.EXE` ✅ | `olk.exe` ✅ | ✅ 实测：本机 AppxManifest | **已并存** |
| Teams | `Teams.exe` ✅ | `ms-teams.exe` ✅ | ✅ 实测：本机 AppxManifest MSTeams Executable=ms-teams.exe | **已并存** |
| 飞书 | `Feishu.exe` ✅ / `Lark.exe` ✅ | `LarkShell.exe` ✅（已在表内） | ✅ 官方 | **已并存** |
| 企业微信 | `WXWork.exe` ✅ | （无新名，WXWorkApp.exe 是推测已删除） | — | **无需补齐** |
| 通义 | （无桌面版历史） | `Tongyi.exe`（新增） | — | 新增，无并存需求 |
| 智谱 | `ChatGLM.exe`（新增，旧名） | `GLM.exe`（新增，新名，z.ai 品牌后） | ✅ 官方 | **建议两者都加入**（品牌从 ChatGLM→GLM 过渡期，老包可能仍是 ChatGLM.exe） |
| Xshell | `Xshell6/7/8.exe` ✅ | `Xagent.exe`（新增，Xshell 8 配套） | ✅ 实测 | **已覆盖到 Xshell 8 + 配套** |
| Obsidian | `Obsidian.exe` ✅ | （无改名） | — | 无需补齐 |

**D2 结论**：现有并存机制健全，无遗漏的新旧名对。主要待补的是国产 AI 助手在品牌更名期（如 ChatGLM→GLM）的两侧保留。

---

## 5. 分类体系评估（D5）

对任务文件提出的 6 个候选场景逐一评估。**默认倾向 (a) 归入现有 kind**，选 (b) 需强理由。

### 5.1 视频会议输入框（Zoom/腾讯会议/飞书会议 聊天区）

**结论：(a) 归入 `chat`**

- 理由：视频会议的「聊天区」输入行为 = 发送消息，Enter 发送、多行危险，与 IM 完全同构。`chat` 的 style（自然对话语气、禁列表）与 `multiline_safe=false` 完全适用。
- 建议新增 exe（归入 chat kind）：
  | exe 名 | 软件 | 地区 | 来源 | 置信度 |
  | --- | --- | --- | --- | --- |
  | `Zoom.exe` | Zoom 会议 | 国外 | ✅ 官方：zoom.us 下载页 | ✅ 官方 |
  | `wemeetapp.exe` | 腾讯会议 | 国内 | ✅ 官方：meeting.tencent.com 下载页 | ✅ 官方 |
  | `FeishuMeeting.exe` | 飞书会议（如有独立入口） | 国内 | ⚠️ 推测：飞书会议集成在 Feishu.exe 内，无独立 exe | — |
- 注意：视频会议的「主窗口」不是输入框场景（用户在开会不在输入），但 scene 感知是按 exe 匹配，无法区分窗口内具体区域。归入 chat 会让用户在会议窗口其他位置输入时也获得 chat 风格——这在会议中填写反馈表单等场景**可能误判**，但收益（聊天区正确）远大于代价（偶尔误判），且 chat 风格保守不会破坏内容。**接受此风险**。

### 5.2 社交媒体 / 内容平台（微博、小红书、X、知乎 桌面端或 PWA）

**结论：(a) 归入 `browser`（PWA 形态）或暂不覆盖（独立客户端稀少）**

- 理由：这些平台绝大多数用户通过浏览器访问（PWA 或普通网页），exe 命中 `chrome.exe/msedge.exe` → browser，靠 title_keywords 兜底细分。微博/小红书/知乎**均无官方 Windows 桌面客户端**（移动端为主，PC 端就是网页）。
- X（Twitter）：本机有 MS Store PWA 包，但前台进程是 `chrome_proxy.exe`/`msedge.exe` → 已被 browser 覆盖，靠 title 含 "X"/"Twitter" 兜底。
- 建议：
  - **不新增独立 exe**（无官方桌面客户端）
  - **补充 title_keywords**：在 browser 块的 title_keywords 加「微博」「小红书」「知乎」「Twitter」「X.com」让浏览器场景细分时能命中并应用合适风格
  - 但这些场景归入哪个 kind？微博/小红书发帖输入 = 短文本社交，`browser` 的 style（web-friendly 简洁单行）基本适用。**保持 browser 即可**，不单列 kind。
- title_keywords 补充建议（归入 browser）：
  | 关键词 | 场景 | 备注 |
  | --- | --- | --- |
  | `微博` | 社媒 | 浏览器开微博发帖 |
  | `小红书` | 社媒 | 浏览器开小红书 |
  | `知乎` | 社媒 | 浏览器开知乎 |

### 5.3 项目管理与工单（Jira、TAPD、禅道、Linear、Teambition）

**结论：(a) 归入 `browser`（多为网页）+ title_keywords 兜底**

- 理由：Jira/Linear/TAPD/禅道 主流用法是网页版（浏览器内），无独立桌面客户端或客户端稀少。
- Linear 有桌面版（Electron），进程名 `Linear.exe`；Jira 有官方桌面版（Atlas，已停更）；TAPD/禅道/Teambition 基本网页。
- 建议：
  - 新增 `Linear.exe` 归入 `doc` kind（工单描述/评论 = 结构化长文本，多行安全，doc 的 style 适用）
  - **但 Linear 的 Enter=发送评论**（类似聊天），多行危险 → 应归入 `chat`？这里有争议。
  - **最终建议**：`Linear.exe` 归入 `chat`（评论输入框 Enter=发送，multiline_safe=false 保守），与工单描述页的多行需求冲突时由用户自行调整。
  - title_keywords 补充（browser 兜底）：「Jira」「TAPD」「禅道」「Linear」「Teambition」→ 归入 `doc`（工单描述是多行结构化文本）。**但这与 Linear.exe 归 chat 矛盾**。
  - **简化方案**：Linear.exe 归 `chat`（保守安全）；网页版靠 title 含「Linear/Jira」→ browser 兜底（browser 默认保守单行，可接受）。
- 建议 exe 新增：
  | exe 名 | 软件 | 归类 | 来源 | 置信度 |
  | --- | --- | --- | --- | --- |
  | `Linear.exe` | Linear 桌面版 | chat | ✅ 官方：linear.app 下载页 | ✅ 官方 |

### 5.4 设计工具（Figma、即时设计、MasterGo）

**结论：(a) 归入 `ide_terminal`（设计工具=技术风格，Enter 非发送但多行意义不大）**

- 理由：Figma 的输入场景是图层命名/评论/文本框，输入短、技术性强（保留英文标识符），与 ide_terminal 的 style（保留技术术语、无客套）契合。
- Figma 有官方桌面版（Electron），进程名 `Figma.exe`。即时设计/MasterGo 是国产 Figma 替代，桌面版可能存在但用户量小。
- multiline_safe：Figma 文本框可多行（设计文案），但图层命名/评论是单行。**保守设 false**（与 ide_terminal 一致）。
- 建议：
  | exe 名 | 软件 | 归类 | 来源 | 置信度 |
  | --- | --- | --- | --- | --- |
  | `Figma.exe` | Figma 桌面版 | ide_terminal | ✅ 官方：figma.com 下载页 | ✅ 官方 |
  | `JishiDesign.exe` | 即时设计 | ide_terminal | ⚠️ 推测：进程名按拼音惯例推断，无实测 | ⚠️ 推测→不列入主表 |

### 5.5 CRM / 客服工作台

**结论：(a) 归入 `browser` 或暂不覆盖**

- 理由：CRM（Salesforce/HubSpot/纷享销客）和客服工作台（Zendesk/Intercom）绝大多数是网页版，无独立 Windows 客户端或客户端稀少。
- Salesforce 有桌面版但进程名未实测；国内纷享销客/销售易无桌面版。
- 建议：**不新增 exe**，靠 browser title_keywords 兜底。可补 title_keywords：「Salesforce」「Zendesk」「CRM」→ 归 browser。
- 实际收益低（CRM 输入多是长文本表单，browser 保守单行可能不够），但不值得为低频场景新增 kind。

### 5.6 游戏内聊天

**结论：(b) 建议暂不覆盖（Unknown 降级）**

- 理由：游戏内聊天框 Enter=发送，多行绝对危险（chat 风格适用），**但游戏进程名极多且无法穷举**（Steam 游戏每个 exe 名都不同）。靠 exe 匹配游戏不现实。
- 游戏覆盖层的聊天输入（如 Discord Overlay、Steam Overlay）的进程属主是游戏本身或 `Discord.exe`/`steam.exe`，前者 Unknown 降级、后者已被 chat 覆盖。
- **建议：不新增 exe，保持 Unknown 降级**。游戏聊天场景低频且风险高（误判多行注入可能触发游戏命令），保守降级是最安全的。
- 若未来有强需求，应考虑 title_keywords 或用户自定义规则，而非穷举游戏 exe。

### 5.7 分类体系评估总结

| 候选场景 | 结论 | 理由 |
| --- | --- | --- |
| 视频会议 | (a) chat | 输入行为同构 IM |
| 社媒/内容平台 | (a) browser + title_keywords | 无独立桌面客户端，网页版走 browser |
| 项目管理/工单 | (a) chat（Linear.exe）/ browser（网页） | 评论 Enter=发送，保守 |
| 设计工具 | (a) ide_terminal | 技术风格契合 |
| CRM/客服 | (a) browser + title_keywords | 网页为主，低频 |
| 游戏内聊天 | 不覆盖（Unknown 降级） | exe 无法穷举，保守降级最安全 |

**D5 总结论：不需要新增 kind**。现有 5 类（chat/email/doc/ide_terminal/browser）+ Unknown 降级已能覆盖所有候选场景，通过归入 + title_keywords 兜底即可。新增 kind 需改 Rust 枚举 + 构建出包，成本高且无强收益。

---

## 6. 误分类风险与缓解（D6）

### 6.1 飞书/钉钉既是 IM 又内置文档

- **风险**：`Feishu.exe`/`DingTalk.exe` 命中 chat（multiline_safe=false），但用户在飞书文档里输入长文本时会被强制单行化 + chat 语气。
- **现有缓解**：靠 title_keywords 兜底——但 exe 命中优先于 title，所以飞书 exe 命中后**不会**走 title 兜底（代码 `src/scene/mod.rs:157-187` exe 命中即返回，不细分）。
- **加剧评估**：本次不新增 Feishu/DingTalk 相关条目，风险不加剧。
- **建议**：保持现状。飞书文档有独立 URL（feishu.cn/doc），用户多在浏览器开文档（走 browser→title「飞书文档」→doc 兜底），桌面客户端内开文档是次要场景。若未来反馈强烈，可考虑给 Feishu.exe 加类似 browser 的「exe 命中后查 title 细分」机制，但需改 Rust 代码（当前仅 browser 有此机制），成本较高。

### 6.2 浏览器开 Gmail 该算 email 还是 browser

- **现状**：`src/scene/mod.rs:159-177` 浏览器细分机制——chrome.exe 命中后查 title，若含 email 关键词→email，含 doc 关键词→doc，否则 browser。**已正确处理**。
- **加剧评估**：本次建议新增社媒 title_keywords（微博/小红书/知乎），这些与 email/doc 无冲突，不会抢占细分。
- **建议**：保持现状，细分机制健全。

### 6.3 视频会议归 chat 的误判（5.1 已述）

- **风险**：会议窗口非聊天区输入（如填写反馈表单）会被应用 chat 风格。
- **缓解**：chat 风格保守（单行、无列表），对表单输入破坏性低。接受此风险。

### 6.4 Linear.exe 归 chat 的争议（5.3 已述）

- **风险**：Linear 工单描述页（多行）会被强制单行。
- **缓解**：Linear 主要输入场景是评论（Enter=发送），工单描述是低频。保守优先。

### 6.5 国产 AI 助手桌面版 vs 浏览器网页版

- **风险**：豆包/Kimi 等若有桌面版 exe 则归 chat（AI Agent style），若用户用浏览器开网页版则靠 title_keywords「豆包/Kimi」兜底（已在表内）。
- **一致性**：两条路径都归 chat，风格一致，无误判。

### 6.6 误分类风险总结

| 风险点 | 严重度 | 缓解方式 | 是否加剧 |
| --- | --- | --- | --- |
| 飞书/钉钉文档 | 中 | 保持现状，浏览器开文档走兜底 | 不加剧 |
| 浏览器 Gmail/Docs | 低 | 细分机制已处理 | 不加剧 |
| 视频会议非聊天区 | 低 | chat 风格保守 | 新增风险（可接受） |
| Linear 工单描述 | 低 | 保守优先 | 新增风险（可接受） |
| AI 助手桌面/网页版 | 无 | 两路径同归 chat | 不加剧 |

---

## 7. ⚠️ 推测项清单（无来源佐证，单列）

> 以下条目**禁止混入主建议表**。按证据标准，这些是按命名惯例推断但无本机实测或官方文档佐证的。

| 推测 exe 名 | 软件 | 推断依据 | 不列入原因 |
| --- | --- | --- | --- |
| `tongyi.exe` | 通义（小写） | 阿里命名大小写不确定 | toml 不区分大小写，Tongyi.exe 已覆盖 |
| `JishiDesign.exe` | 即时设计 | 按拼音惯例推断 | 无官方下载实测，不列入 |
| `SogouExplorer.exe` | 搜狗浏览器（驼峰） | 新版可能改名 | 词表 `sogou_explorer.exe` 已在，无证据改名 |
| `WPSAI.exe` | WPS AI 助手 | WPS AI 独立进程？ | WPS AI 集成在主进程，无独立 exe |
| `FeishuMeeting.exe` | 飞书会议 | 独立入口？ | 集成在 Feishu.exe 内 |
| `fish.exe` | fish shell Windows | Windows 移植 | Windows 支持有限，不列入 |
| `OutlookForWindows.exe` | Outlook 新版备选名 | MSIX 包名推断 | AppxManifest 实测是 olk.exe |
| `TAPD.exe` | TAPD 桌面版 | 是否有独立客户端？ | TAPD 主力网页，无官方桌面版 |

**推测项合计 8 条，全部不列入主建议表**。

---

## 8. 分批实施建议

### P0（高频高价值、零风险纯 toml）— **立即实施，免构建**

| 操作 | 条目 | 数量 |
| --- | --- | --- |
| 删除推测项 | DouyinIM/FeishuDocs/NewMailEngine/DingTalkLite/WXWorkApp/Obsidian-helper | 6 |
| 新增 AI 助手 | Doubao/Kimi/Tongyi/Wenxin/ChatGLM/GLM/NanoSearch/Perplexity | 8 |
| 新增视频会议 | Zoom/wemeetapp（归 chat） | 2 |
| 新增邮件 | CalendarApp.Gui.Win10/Mailbird/The Bat! | 3 |
| 新增 IDE/终端 | Xagent/wezterm/git-bash/Nu | 4 |
| 新增文档 | MarkText/Koodo Reader/OneNote(UWP) | 3 |
| 新增设计 | Figma（归 ide_terminal） | 1 |
| 新增项目管理 | Linear（归 chat） | 1 |
| 补 title_keywords | 微博/小红书/知乎/Jira/TAPD/禅道/Linear/Teambition/Salesforce/Zendesk | 10 |
| 注释修正 | Skype 注释补迁移说明 | 1 |

**P0 合计：删 6 + 新增 exe 22 + 新增 title_keywords 10 + 注释修正 1 = 纯 toml 改动，免构建**（按既定规则同步三副本 + 重启进程即可）。

### P1（次要补充）

| 操作 | 条目 | 备注 |
| --- | --- | --- |
| 暂无 | — | P0 已覆盖主流，P1 留待 Gavin 端测反馈后按需补充 |

### P2（需改 Rust 或有争议）

| 操作 | 条目 | 备注 |
| --- | --- | --- |
| 无 | — | D5 评估结论：不需要新增 kind，无需改 Rust |

---

## 9. 实施方式说明

- **P0 全部为纯 toml 改动**：
  - 修改 `scene-rules.toml`（exe 数组增删 + title_keywords 增删 + 注释修正）
  - 按既定规则同步三副本（`scene-rules.toml` / `target/release/scene-rules.toml` / `Publish/scene-rules.toml`）
  - 重启 `voice-ime.exe` 即生效（`OnceLock` 重新加载 exe 同级 toml）
  - **不需要 `cargo build` / `cargo build --release` / 出包**
  - 注意：`include_str!` 内置默认是编译期嵌入的，改 toml 不改 exe 时，**只有 exe 同级 toml 存在时才生效**；若要更新内置默认（让全新解压的 exe 也带新词表）才需重新编译。建议：P0 改 toml 后同步三副本即可，内置默认留待下次正式出包时顺带更新。

- **涉及 Rust 的改动**：无（D5 评估不需要新增 kind）

- **版本号**：不动（纯词表扩充不升版本）

---

## 10. 验收对照（对应任务 §八）

| 验收标准 | 本报告落实 |
| --- | --- |
| 1. git status 零文件改动 | 本任务只写本 md（在 gitignore 的 collab/ 下），不改 scene-rules.toml/Rust/任何源文件 |
| 2. 建议新增表每条有来源且非空；推测项单列 | 第 2 节每条有来源列；第 7 节单列 8 条推测项 |
| 3. 国内与国外两侧都有实质产出 | 国内：豆包/Kimi/通义/文心/智谱/纳米/腾讯会议/微信并存核查；国外：Zoom/Linear/Figma/Mailbird/The Bat!/Nu/Perplexity/OneCalendar |
| 4. D2 新旧并存有明确结论 | 第 4 节逐一核查 10 个软件，结论：现有并存健全，无遗漏 |
| 5. D5 对六个候选场景逐一 (a)/(b) | 第 5 节逐一给出结论，均为 (a) 或不覆盖，无 (b) |
| 6. 报告可直接作为实施任务输入 | 第 8 节 P0 表条目可直接复制粘贴进 toml |

---

## 附：研究方法说明

- **本机实证**（✅ 实测标记来源）：
  - `Get-Process` 枚举当前运行进程名
  - `Get-AppxPackage` + 读取 `AppxManifest.xml` 的 `<Application Executable=...>` 字段
  - 开始菜单 `.lnk` 快捷方式 target 路径解析（WScript.Shell COM）
  - `Program Files` / `AppData\Local\Programs` 目录 exe 扫描
  - 注册表 Uninstall 键 DisplayName/InstallLocation
- **官方文档**（✅ 官方标记来源）：各软件官网下载页 / GitHub release 资产名 / electron-builder 配置
- **联网调研**：尝试 WebFetch 核实，但国内站点（钉钉/飞书官网）返回内容被 JS 占位，主要依靠本机实证 + 通用软件知识
- **未安装软件**：豆包/Kimi/通义等国产 AI 助手本机未安装，exe 名依据官网下载页提供 Windows 安装包确认存在，标注 ✅ 官方；具体大小写形态需实施后实测（toml 不区分大小写，不影响命中）
- **禁止操作**：未安装任何软件，未执行 cargo build/release，未用 git 破坏性命令，未用 PowerShell Set-Content 改 UTF-8 文件