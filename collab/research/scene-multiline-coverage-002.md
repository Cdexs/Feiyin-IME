# RESEARCH-SCENE-MULTILINE-002 · 场景感知多行输出覆盖面研究

> 来源：Gavin 2026-08-01 两批需求（文字编辑/笔记/办公类 web 版 + IDE/设计软件）
> 纯研究零代码改动，产出供 Gavin 拍板。
> 基线 `ae8d034`（ahead 9 未 push）。

---

## R1 · 桌面 exe 补充清单

### 现状已覆盖（不必重做）

思源(`siyuan.exe`)/Notion(`Notion.exe`)/Obsidian(`Obsidian.exe`)/记事本(`Notepad.exe`)/WPS(`wps.exe`/`wpsoffice.exe`) 均已在 doc 块。Gavin 点名的 IDE 全部已在 ide_terminal 块。

### 新增候选

| 软件 | exe 名 | 证据 | 建议归属 | 备注 |
| --- | --- | --- | --- | --- |
| Windows 便笺/Sticky Notes | `StickyNotesStub.exe` | ✅本机实测：`C:\Program Files\WindowsApps\Microsoft.MicrosoftStickyNotes_4.0.6104.0_x64__8wekyb3d8bbwe\StickyNotesStub.exe`，AppxManifest `Executable="StickyNotesStub.exe"` | doc | 微软自带，UWP 应用 |
| Google Keep | 无 Windows 桌面版 | 📄官方：Google Keep 官网页 `keep.google.com`，无独立桌面客户端（非 PWA 官方包装）| — | 靠 R2 web 关键词覆盖 |
| memos | 无 Windows 桌面版 | 📄官方：memos 是自托管 web 应用（`github.com/usememos/memos`），Docker 部署，浏览器访问 | — | 靠 R2 web 关键词覆盖 |
| 墨刀 | `MockingBot.exe`? | ⚠️推测：墨刀官网提供 Windows 客户端下载，但未本机安装无法验证进程名。墨刀曾用名 MockingBot | doc ⚠️ | 需 Gavin 或端测确认进程名 |
| Axure RP | `AxureRP.exe` | ⚠️推测：Axure 官网提供 Windows 安装包，按惯例进程名 `AxureRP.exe`（v9）或 `AxureRP10.exe`（v10） | ide_terminal ⚠️ | 需端测确认；Axure 是原型设计工具，文本图层 Enter=换行，但争议评论框 Enter 常需 Ctrl+Enter |
| OpenDesign | 无 Windows 桌面版 | 📄官方：OpenDesign 是在线设计平台（`opendesign.com`），无独立桌面客户端 | — | 靠 R2 web 关键词覆盖 |
| 即时设计 | `JsDesign.exe`? | ⚠️推测：即时设计官网提供 Windows 客户端，进程名未证实 | doc ⚠️ | 需端测确认 |
| MasterGo | 无 Windows 桌面版 | 📄官方：MasterGo 是在线设计平台（`mastergo.com`），无独立桌面客户端 | — | 靠 R2 web 关键词覆盖 |
| Sketch | 无 Windows 版 | 📄官方：Sketch 是 macOS 独占（`sketch.com`，系统要求 macOS） | — | 不加（与 Xcode 同理，macOS 独占） |

### Xcode（任务书问及）

**不加**。Xcode 是 macOS 独占，我方 macOS 侧 `run_pipeline` 仍是 `mod macos_stubs` 空壳。macOS 团队 Phase 4 实现时自行添加。

---

## R2 · ⭐ web 版 `title_keywords` 清单

### 约束遵守

- 关键词只加进 **doc 块**（browser 块的 title_keywords 是 no-op，`src/scene/mod.rs:162-164` 显式跳过）
- 子串匹配 `title_lower.contains(kw)`，大小写不敏感
- 误判成本 >> 漏判成本（误判→多行注入→Enter=发送的输入框拆成多条发送）

### 候选清单（四字段表格）

| # | 产品 | 实际标题样本 | 建议关键词 | 特异性 | 误伤场景 |
| --- | --- | --- | --- | --- | --- |
| 1 | Google Keep | `Google Keep: Online Notes and Digital Notebook Lists | Google Workspace`（📄WebFetch 实测 `keep.google.com`）| `Google Keep` | 高 | 未发现（"Google Keep" 作为完整词组只出现在 Keep 相关页面）|
| 2 | memos | `memos`（📄官方：自托管应用，标题通常就是 `memos` 或 `Memos`）| `memos` | **低** | 🔴 `Memos - Reddit`、`Memos of a...`、`memos blog` 等非目标页面会命中。**建议不收** |
| 3 | 思源笔记 web | `思源笔记 - [文档名]`（⚠️推测：思源 web 版通常以 `思源笔记` 开头，但思源也支持自定义服务器标题）| `思源笔记` | 高 | 未发现（"思源笔记"是产品专有名）|
| 4 | Obsidian Publish | `Site Name - Obsidian Publish`（📄官方：Obsidian Publish 站点标题格式 `[站点名] - Obsidian Publish`）| `Obsidian Publish` | 高 | 未发现（"Obsidian Publish" 是产品专有名）|
| 5 | 金山文档/WPS 云文档 | `文档名 - 金山文档`（⚠️推测：WPS 云文档网页版标题通常含 `金山文档`）| `金山文档` | 高 | 未发现（"金山文档"是产品专有名）|
| 6 | 钉钉文档 | `文档名 - 钉钉文档`（⚠️推测：钉钉文档网页版标题通常含 `钉钉文档`）| `钉钉文档` | 高 | 未发现（"钉钉文档"是产品专有名）|
| 7 | 飞书文档 | 已有 `飞书文档` | — | 高 | — |
| 8 | Craft | `[文档名] — Craft`（📄官方：Craft 桌面/web 版标题格式 `[文档名] — Craft`）| `Craft` | **低** | 🔴 `Craft beer`、`Craft shop`、`Minecraft` 含 `craft` 子串。**建议不收** |
| 9 | Bear | `[笔记名] — Bear`（📄官方：Bear 笔记标题格式含 `Bear`）| `Bear` | **低** | 🔴 `Bear blog`、`Bear market`、`Bears` 等。**建议不收** |
| 10 | Roam Research | `Roam Research - [页面名]`（📄官方：标题含 `Roam Research`）| `Roam Research` | 高 | 未发现（"Roam Research" 是产品专有名）|
| 11 | Confluence | `[页面名] - [空间名] - Confluence`（📄官方：Atlassian Confluence 标题格式含 `Confluence`）| `Confluence` | 高 | 未发现（"Confluence" 是产品专有名，子串 `confluence` 日常极少出现在非产品页面）|
| 12 | Coda | `[文档名] - Coda`（📄官方：Coda 标题格式含 `Coda`）| `Coda` | **低** | 🔴 `Coda` 是音乐术语、意大利语"结尾"，`Coda shop`、`Coda piano` 等。**建议不收** |
| 13 | Anytype web | `Anytype`（⚠️推测：Anytype web 版标题通常含 `Anytype`）| `Anytype` | 中高 | 未发现（"Anytype"是产品专有名，但不常见）|
| 14 | Notion | 已有 `Notion` | — | 高 | — |
| 15 | Google Docs | 已有 `Google Docs` | — | 高 | — |
| 16 | 腾讯文档 | 已有 `腾讯文档` | — | 高 | — |
| 17 | 石墨文档 | 已有 `石墨文档` | — | 高 | — |

### 建议收录汇总

| 建议收录 | 关键词 | 理由 |
| --- | --- | --- |
| ✅ 收录 | `Google Keep` | 特异性高，产品专有名 |
| ✅ 收录 | `思源笔记` | 特异性高，产品专有名 |
| ✅ 收录 | `Obsidian Publish` | 特异性高，产品专有名 |
| ✅ 收录 | `金山文档` | 特异性高，产品专有名 |
| ✅ 收录 | `钉钉文档` | 特异性高，产品专有名 |
| ✅ 收录 | `Roam Research` | 特异性高，产品专有名 |
| ✅ 收录 | `Confluence` | 特异性高，产品专有名 |
| ✅ 收录 | `Anytype` | 特异性中高，产品专有名 |
| ❌ 不收 | `memos` | 特异性低，`Memos - Reddit` 等会误伤 |
| ❌ 不收 | `Craft` | 特异性低，`craft` 子串太通用 |
| ❌ 不收 | `Bear` | 特异性低，`bear` 子串太通用 |
| ❌ 不收 | `Coda` | 特异性低，`coda` 是通用词 |

---

## R3 · ⭐⭐ IDE / 代码编辑器分层裁定

### L1 纯编辑器（无内置终端）

| 软件 | 内置终端？ | 判定 |
| --- | --- | --- |
| Notepad++ | **无内置终端**。NppExec 插件可执行命令但非真正终端（无交互 shell、无 PTY）。⚠️ NppExec 是可选插件，默认安装不含 | 可安全放开多行？**是**——NppExec 插件用户极少，且 NppExec 的"运行命令"与代码编辑区是分离的弹窗，不在主编辑区 Enter=换行 |
| Sublime Text | **无内置终端**。Terminus 插件是第三方包，默认安装不含。⚠️ Terminus 确实提供集成终端，但需 Package Control 手动安装 | 可安全放开多行？**是但需标注**——Terminus 插件用户极少，且终端面板可独立区分（如果未来有运行时判据）。当前放开的风险与 Notepad++ 同级 |

**L1 结论**：Notepad++ 和 Sublime Text 可安全放开 `multiline_safe=true`。风险来自可选终端插件（NppExec/Terminus），但：
1. 默认安装不含这些插件
2. 插件终端是独立面板/弹窗，不在主编辑区
3. 即使误判，用户在终端面板口述的概率极低
4. 放开后的收益（代码编辑器口述换行）远大于误判风险

### L2 全功能 IDE（有内置终端）

| 软件 | 内置终端 | 编辑器与终端区分可行性 |
| --- | --- | --- |
| VS Code (`Code.exe`) | ✅ 集成终端 | ❌ 同进程同窗口标题。窗口标题格式 `filename — FolderName — Visual Studio Code`（编辑器）/ `Terminal — FolderName — Visual Studio Code`（终端聚焦时**可能**有差异，但**不稳定**：用户可改标题、split terminal 无标题变化） |
| Cursor (`cursor.exe`) | ✅ Fork of VS Code | ❌ 同上 |
| Windsurf (`Windsurf.exe`) | ✅ Fork of VS Code | ❌ 同上 |
| JetBrains 全家 | ✅ 集成终端 | ❌ 同进程同窗口标题。标题格式 `filename – ProjectName`（编辑器）/ 无显著差异（终端聚焦时标题不变） |
| Visual Studio (`devenv.exe`) | ✅ 集成终端 | ❌ 同进程同窗口标题 |
| HBuilderX | ✅ 集成终端 | ❌ 同进程同窗口标题 |
| Zed (`Zed.exe`) | ✅ 内置终端 | ❌ 同进程同窗口标题 |

**L2 可行性结论：做不到可靠区分**。

**考察方向与结论**：
1. **窗口标题格式差异**：VS Code 终端聚焦时标题**可能**变为 `Terminal — ...`，但这不稳定（split terminal、自定义标题、全屏模式均破坏此规律）。JetBrains 无标题差异。**不可靠**
2. **UI Automation 焦点控件类型**：理论上可查询 UI Automation 树，检查焦点控件是否为 `TextEditor` vs `Terminal`。但：
   - 需要 Windows API 调用（`uiautomation` crate 或 Win32 `IUIAutomation`），违反 DEC-033（不得产出仅 Windows 可编译新代码）
   - 每次听写前查询 UIA 树有性能开销（UIA 遍历可能 10-50ms）
   - 不同 IDE 的 UIA 树结构不同，需要逐 IDE 适配
   - 失败兜底：UIA 查询失败时保守判 false（维持现状），但这意味着不可靠
   - **不可行**（违反跨平台约束 + 适配成本高 + 不可靠）
3. **子窗口类名**：VS Code 的终端是 WebView 内嵌 xterm.js，无独立 Win32 子窗口。JetBrains 终端是 JCEF 内嵌。**无独立窗口类名可区分**

**L2 结论**：维持 `multiline_safe=false`。无法可靠区分编辑器与终端，放开会导致在终端里口述时多行注入→命令拆断。

### L3 纯终端

WindowsTerminal / cmd / powershell / pwsh / ConEmu / vim / gvim → **绝对 false，不容讨论**。

### Zed 归属

**Zed 归 L2**。Zed 有内置终端（`zed terminal`），编辑器与终端同进程同窗口标题。与 VS Code 同理，无法区分。

---

## R4 · ⭐⭐⭐ 架构分叉：`multiline_safe` 是否需要拆成两个维度

### 问题

`multiline_safe`（单 bool）同时控制：
1. 允许多行注入 / 不做 `flatten_multiline` 单行化（`src/llm/mod.rs:660`）
2. 打开 F3 Smart Lists（`build_format_instruction_block` 真分支 + `build_output_format` 真分支，`src/llm/mod.rs:818/795`）

在代码编辑器里口述时，用户要换行，不要 `• ` 项目符号。

### 方案 A：不拆

| 项 | 内容 |
| --- | --- |
| 改动 | 沿用单 bool。代码编辑器要么全开（含列表）要么全关 |
| 代价 | 纯数据改动 → **免构建**（`scene-rules.toml` 同步三副本 + 重启即可） |
| 后果 | 若 L1 放开（`multiline_safe=true`）：用户在 Notepad++ 口述列表时会被插入 `• ` 项目符号——**不期望但可接受**（代码编辑器里口述列表是低频场景，且 `• ` 在代码里不算"损坏"，只是不自然） |
| 后果 | 若 L1 不放开（`multiline_safe=false`）：用户在 Notepad++ 口述时不能换行——**主要痛点不解决** |

### 方案 B：拆两维

| 项 | 内容 |
| --- | --- |
| 改动 | 新增字段 `list_formatting: bool`（或 `allow_lists`），`multiline_safe` 只管换行 |
| 文件 | `scene-rules.toml` schema + `SceneRule` struct（`src/scene/mod.rs:89`）+ `SceneContext`（`:49`）+ `src/scene/mod.rs` 解析+传递 + `src/llm/mod.rs:660`（flatten 仍用 multiline_safe）+ `build_format_instruction_block` + `build_output_format` 参数化（从 bool 改为两个 bool 或 enum） |
| 代价 | **要出包**（改 Rust）。需回查 DEC-031 单开关原则——但 DEC-031 约束的是**用户可见配置开关**，`multiline_safe`/`list_formatting` 是**内部规则数据**（用户不可见，`scene-rules.toml` 非用户编辑），主控倾向不触碰，**我认同不触碰** |
| 跨平台 | `src/scene/mod.rs` 是平台中立模块（已通过 DEC-033 重构），改动对 macOS 透明。`src/llm/mod.rs` 同理。**不违反 DEC-033** |
| 后果 | 代码编辑器可设 `multiline_safe=true, list_formatting=false`——换行放开但列表关闭 |

### 方案 C：复用 scene `style` 字段用自然语言压制 bullet（零 schema 改动）

| 项 | 内容 |
| --- | --- |
| 改动 | 在 ide_terminal 块的 `style` 字段追加自然语言指令（如 `Do NOT use bullet lists (•) or numbered lists (1. 2. 3.) — output plain text with line breaks only.`），同时把 L1 编辑器的 `multiline_safe` 改为 true |
| 文件 | 只改 `scene-rules.toml`（style 字段文本 + multiline_safe 值） |
| 代价 | 纯数据改动 → **免构建**（同步三副本 + 重启） |
| 机制 | `style` 字段写入 LLM prompt F4 段（`src/scene/mod.rs:295-304`），与 F3 Smart Lists 指令并存。F4 是场景风格指令，F3 是格式指令——LLM 会综合两者。**style 压制 F3 bullet 是"软约束"**，LLM 可能不严格遵守 |
| 后果 | 代码编辑器口述时：换行放开（multiline_safe=true 硬约束）+ F4 style 告诉 LLM 不要列表（软约束）。**比方案 A 好**（至少有压制指令），**比方案 B 弱**（软约束 vs 硬约束） |
| 风险 | LLM 可能在 F3 触发条件满足时（用户口述"第一点...第二点"）忽略 F4 style 仍输出 bullet。但这在代码编辑器里是低频场景 |

### 推荐：方案 C

**理由**：
1. **免构建**——纯数据改动，同步三副本 + 重启即生效，与方案 A 同级
2. **解决主要痛点**——L1 代码编辑器换行放开（`multiline_safe=true` 硬约束）
3. **附带压制 bullet**——F4 style 软约束压制 F3 bullet，虽不如方案 B 硬约束但低频场景可接受
4. **零 Rust 改动**——不触碰 DEC-033 跨平台约束、不需要出包、不需要 TEST-SYNC
5. **可升级**——若 Gavin 端测后发现 LLM 不遵守 style 压制，可升级为方案 B（出包）

**方案 B 的触发条件**：Gavin 端测后发现代码编辑器里口述列表被插入 `• ` 且 F4 style 压制无效 → 升级为方案 B。

---

## R5 · 设计软件裁定

### Figma

**维持 browser 分类**。Gavin 2026-07-28 决策 3 亲自拍板归 browser。理由：
1. Figma 桌面版是 Electron 包装的 WebView，行为与 web 版一致
2. Figma 文本图层 Enter=换行（安全），但评论框 Enter=发送（危险），**同进程两种行为**
3. browser 分类（`multiline_safe=false`）是保守安全的降级——单行注入不会损坏输出

**不建议改判**。若要改判为 doc，需解决评论框 Enter=发送的误判风险，而设计软件无法用窗口标题区分"当前焦点在文本图层还是评论框"。

### 墨刀 / Axure RP / OpenDesign / 即时设计 / MasterGo / Sketch

| 软件 | 桌面版 | 建议 |
| --- | --- | --- |
| 墨刀 | ⚠️有 Windows 客户端，进程名未证实 | 暂不加（需端测确认进程名）。若加入归 browser（与 Figma 同理：文本图层 vs 评论框两种行为） |
| Axure RP | ⚠️有 Windows 安装包，进程名 `AxureRP.exe`/`AxureRP10.exe` 未证实 | 暂不加。Axure 是原型工具，文本输入场景少 |
| OpenDesign | 无桌面版 | 靠 web 关键词（但 `OpenDesign` 特异性中等，建议 R2 评估） |
| 即时设计 | ⚠️有 Windows 客户端，进程名未证实 | 暂不加 |
| MasterGo | 无桌面版 | 靠 web 关键词 |
| Sketch | macOS 独占 | 不加（与 Xcode 同理） |

**设计软件统一裁定**：归 browser（`multiline_safe=false`）。理由：设计软件的文本输入场景复杂（文本图层 vs 评论框 vs 标注框），无法用窗口标题区分，保守降级最安全。

---

## 风险汇总

### 本批若全部落地，最坏误伤场景

| 误伤场景 | 触发条件 | 后果 | 概率 |
| --- | --- | --- | --- |
| R2 web 关键词误伤 | 浏览器标题含 `Google Keep`/`Confluence` 等但不是目标产品页面 | 多行注入到 Enter=发送的输入框→消息拆成多条 | 极低（收录的都是高特异性产品专有名） |
| R1 Sticky Notes 误判 | `StickyNotesStub.exe` 始终是便笺，无歧义 | — | 零 |
| R3 L1 放开（Notepad++/Sublime） | 用户在 NppExec/Terminus 插件终端里口述 | 多行注入到终端→命令拆断 | 极低（插件用户少 + 终端口述场景少） |
| R4 方案 C LLM 不遵守 style | 用户在代码编辑器口述"第一点...第二点" | LLM 输出 `1. \n2. ` 而非纯文本换行 | 中低（F3 触发条件明确时 LLM 倾向遵守 F3） |

### 零风险项

- R3 L2 维持 false：无变化无风险
- R3 L3 维持 false：无变化无风险
- R5 Figma 维持 browser：无变化无风险

---

## 落地批次建议

### 免构建（纯数据改动，可先行）

| 项 | 内容 | 文件 |
| --- | --- | --- |
| R2 web 关键词 | doc 块 `title_keywords` 新增 8 条（Google Keep/思源笔记/Obsidian Publish/金山文档/钉钉文档/Roam Research/Confluence/Anytype） | `scene-rules.toml` |
| R1 Sticky Notes | doc 块 `exe` 新增 `StickyNotesStub.exe` | `scene-rules.toml` |
| R3 L1 放开 | Notepad++/Sublime Text 从 ide_terminal 块移到 doc 块（或新建 editor 块？但 SceneKind 无 editor 类型，移到 doc 块最合适）| `scene-rules.toml` |
| R4 方案 C | L1 编辑器 style 字段追加 `Do NOT use bullet lists or numbered lists — output plain text with line breaks only.` | `scene-rules.toml` |

### 需出包（Rust 改动）

| 项 | 内容 | 触发条件 |
| --- | --- | --- |
| R4 方案 B | 拆 `list_formatting` 维度 | Gavin 端测后发现方案 C 的 style 压制无效 |

### 不落地

| 项 | 理由 |
| --- | --- |
| R2 `memos`/`Craft`/`Bear`/`Coda` | 特异性低，误伤风险大 |
| R3 L2 放开 | 无法可靠区分编辑器与终端 |
| R5 Figma 改判 | Gavin 既有决策 + 评论框风险 |
| R1 墨刀/Axure/即时设计 | 进程名未证实，需端测确认 |

---

## 待 Gavin 拍板项

1. R2 收录 8 条 web 关键词是否全部同意？
2. R3 L1（Notepad++/Sublime Text）是否放开 `multiline_safe=true`？
3. R4 选方案 C（推荐）还是方案 B（硬约束）？
4. R1 墨刀/Axure/即时设计是否派端测确认进程名？
5. R5 设计软件是否统一归 browser？