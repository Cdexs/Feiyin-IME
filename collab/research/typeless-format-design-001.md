# 技术方案：输出格式化 + 场景感知（Typeless 对标）

> 编号：DESIGN-FORMAT-SCENE-001
> 日期：2026-07-11
> 设计人：Orchestrator（咖啡）
> 上游：RESEARCH-TYPELESS-FORMAT-001（typeless-formatting-001.md）
> 关联：DEC-029（词库单词化）/ DEC-030（智能 ITN）/ FILLER-STRIP-001（待拍板）
> 状态：待 Gavin 拍板

---

## 一、总体架构：三层分工

延续 voice-ime 既有「确定性走本地规则、语义级走 LLM」的分层优势（对 Typeless 全云端的结构性隐私/延迟优势），格式化输出定位为：

```
录音 → ASR 转录
      → L1 确定性规则层（本地，已有）：ITN 数字规整 + 标点          ← 零延迟、离线可用
      → L2 语义格式化层（LLM，可选）：格式化指令集 + 场景上下文      ← 本方案新增
      → 注入（格式安全裁决）                                        ← 本方案新增
```

- **L1 不动**：ITN-SMART-001 已交付，Typeless 无独立数字规整（靠 LLM 顺带），我们在确定性场景反而更稳。
- **L2 是本方案主体**：把现有 LLM optimize 的 prompt 升级为「格式化指令集」，并注入「场景上下文」变量。**不新增 LLM 调用**——复用现有单次 optimize 调用，只是 prompt 变长（约 +300~500 token），延迟增量可忽略。
- **LLM 未开启/失败时**：降级为现状行为（ITN+标点），不做语义格式化。规则层不模拟结构重组（非确定性任务，规则做不好）。

---

## 二、格式化指令集（L2 prompt 升级）

对标 Typeless 四能力，拆为四段独立指令，各配开关（配置项 + UI 复选框）：

| # | 指令段 | 对应 Typeless 能力 | 默认 | 说明 |
| --- | --- | --- | --- | --- |
| F1 | 语气词/口头禅去除 | Filler removal | 开 | 即 FILLER-STRIP-001 的 C 方案，纳入本指令集统一实现；"嗯/啊/额"必删，"那个/就是"口头禅由 LLM 语境判断（比纯规则安全） |
| F2 | 改口修正 + 邻近重复清理 | Self-correction | 开 | "周三开会……不对，周四" → "周四开会"；口吃重复词清理 |
| F3 | 结构重组 | Auto-formatting | 关* | 检测列举/步骤语式 → 序号列表/分行。**默认不做内容压缩提炼**（Typeless 连内容都压缩，输入法场景失真风险不可接受；压缩可作独立开关，默认关） |
| F4 | 场景语气适配 | Context-aware tone | 关* | 依赖场景感知模块（见第三节），Phase 2 交付后才可开 |

*F3/F4 默认关的原因见第四节「格式安全裁决」——多行输出在聊天场景有实际风险，须与场景感知联动后才敢默认开。

**落点**：`src/llm/mod.rs` 的 prompt 组装已是拼段架构（`prompt_parts = [base_prompt, wordbook_block, suggestion_instruction].join("\n\n")`，mod.rs:347-387），新增 `build_format_instruction_block(config)` 与 `build_scene_prompt_block(scene)` 两段即可，改动面小、与 DEC-029 词汇表段同构。

---

## 三、场景感知：如何知道用户正在往哪儿输入【核心】

### 3.1 关键洞察：采集时机与挂钩点

**场景 = 注入目标窗口的语义**。录音启动瞬间 `HotkeyEvent::Start` 已捕获 `GetForegroundWindow()` 作为注入目标（`src/main.rs:1892`，现用于注入时 focus-lost 检测）——场景感知直接在同一时刻、同一 HWND 上采集，天然保证「感知的场景 == 注入的目标」。

时序设计：
```
热键按下 → 取 HWND（已有）→ 同步采集 P0 信号（<1ms，不阻塞热键线程）
         → 录音进行中（秒级）→ [可选 P2 信号后台异步采集]
         → 转录完成 → SceneContext 注入 LLM prompt → 注入前格式安全裁决
```
P0 信号（进程名+标题）单次 Win32 调用微秒级，可在热键路径同步做；任何 UIA 级调用（几十 ms 风险）一律放后台线程，利用录音+转录的秒级窗口完成，**绝不增加松键到上屏的延迟**。

### 3.2 信号源分级（Windows）

| 级别 | 信号 | API | 成本/可靠性 | 交付期 |
| --- | --- | --- | --- | --- |
| **P0** | 进程 exe 名 | `GetWindowThreadProcessId` → `OpenProcess` → `QueryFullProcessImageNameW` | <1ms，极可靠，覆盖 ~90% 场景判断 | Phase 2 |
| **P0** | 窗口标题 | `GetWindowTextW` | <1ms，可靠；浏览器标题自带页面 title | Phase 2 |
| **P1** | 浏览器页面细分 | 从窗口标题正则提取（如 "收件箱 - Outlook - Chrome" → 网页邮箱场景），**不做** UIA 读地址栏（脆弱且慢） | 零额外成本 | Phase 2（随词表迭代） |
| **P2** | 焦点控件类型 | UIA focused element ControlType（Edit 单行 vs Document 多行），`GetGUIThreadInfo.hwndFocus` 在 injection.rs:57 已有先例 | 几十 ms，需异步；现代应用（Electron/WebView）控件树不保证可读 | Phase 3 评估 |
| P3 | 个性化风格学习 | 用户在各 app 修改注入文本的差异学习 | 远期 | 不排期 |

### 3.3 场景分类与规则外置

```rust
// 新模块 src/scene/mod.rs（platform 无关部分）
pub struct SceneContext {
    pub scene: SceneKind,      // Chat | Email | Doc | Ide | Terminal | Browser | Unknown
    pub app_exe: String,       // "WeChat.exe"（仅本地使用，默认不上送 LLM）
    pub multiline_safe: bool,  // 格式安全裁决输入（见第四节）
    pub style_hint: String,    // 从规则表匹配出的风格指令
}
```

**分类词表外置 `scene-rules.toml`**（完全复刻 DEC-030 itn-rules.toml 成熟模式：include_str! 编译期嵌入默认 + exe 同级外部文件覆盖 + 解析失败降级内置，用户/我们升级词表不动 exe）：

```toml
[[scene]]
kind = "chat"
exe = ["WeChat.exe", "QQ.exe", "Telegram.exe", "DingTalk.exe", "Slack.exe", "Feishu.exe"]
style = "自然口语体，简短直接，保留语气，不使用列表/标题等书面格式"
multiline_safe = false          # 聊天框 Enter=发送，多行注入危险

[[scene]]
kind = "email"
exe = ["OUTLOOK.EXE", "Foxmail.exe", "thunderbird.exe"]
title_keywords = ["Outlook", "邮件", "Gmail"]   # 兜浏览器网页邮箱
style = "正式书面语，完整句子，保留称呼与礼貌用语"
multiline_safe = true

[[scene]]
kind = "ide"
exe = ["Code.exe", "idea64.exe", "cursor.exe", "WindowsTerminal.exe"]
style = "技术表述，专有名词/代码术语保留英文原文，不加客套修饰"
multiline_safe = false          # 终端多行=逐行执行，危险

[[scene]]
kind = "doc"
exe = ["WINWORD.EXE", "wps.exe", "Obsidian.exe", "Notion.exe", "Typora.exe"]
style = "书面体，允许结构化输出（序号列表、分段）"
multiline_safe = true
```

匹配优先级：exe 精确匹配 → 标题关键词 → Unknown（Unknown = 不注入场景段，行为等同 Phase 1）。

### 3.4 隐私边界【相对 Typeless 的护城河，必须守住】

- **默认只把「场景类别 + 风格指令」写入 prompt**，原始窗口标题、exe 名不上送 LLM（标题可能含邮件主题/文档名等敏感信息）。
- 配置开关「发送窗口标题以提升场景判断」默认**关**；开启后标题也只截前 N 字符。
- 场景采集、分类、裁决全部本地完成，LLM 只见到一句风格指令——Typeless 做不到这一点（它把上下文送云端才能适配）。

---

## 四、格式安全裁决（场景感知的第二用途，容易被忽略但必须做）

**风险**：F3 结构重组输出多行文本，而注入目标行为差异巨大——微信聊天框里 SendInput 换行可能触发「直接发送/连发多条」；终端里多行=逐行执行命令。这是 Typeless 类功能在 IME 场景的真实事故点。

**裁决规则**（注入前，本地执行）：
1. `SceneContext.multiline_safe == false` 时：LLM prompt 中直接禁用 F3（源头治理），并对 LLM 输出做兜底单行化（换行→"；"），双保险。
2. 剪贴板注入模式换行安全性高于 SendInput 打字模式；`multiline_safe=false` 且文本含换行时强制走剪贴板路径。
3. Unknown 场景按 `multiline_safe=false` 保守处理。

---

## 五、实施分期与工作量

| 期 | 任务 | 内容 | 影响文件 | 规模 |
| --- | --- | --- | --- | --- |
| **Phase 1** | FORMAT-LLM-001 | F1/F2/F3 指令集 + 配置开关 + UI（LLM 页三个复选框）+ 多行兜底单行化 | src/llm/mod.rs、src/config/mod.rs、src-tauri/config.rs、ui/Llm.tsx、双侧 i18n | 中（纯 prompt+配置，无新依赖） |
| **Phase 2** | SCENE-SENSE-001 | src/scene/ 新模块 + P0 信号采集（main.rs:1892 挂钩）+ scene-rules.toml + F4 场景段 + 格式安全裁决 | src/scene/（新）、src/main.rs、src/llm/mod.rs、src/platform/{windows,macos}/、scene-rules.toml（新，Publish 同步） | 中 |
| **Phase 3** | 演进评估 | UIA 控件信号、浏览器细分词表迭代、内容压缩开关、个性化学习 | — | 待评估 |

Phase 1 独立可交付（无场景感知也有价值：语气词+改口是 FILLER-STRIP-001 的正式落地）；Phase 2 依赖 Phase 1 的 prompt 拼段扩展。**若立项，FILLER-STRIP-001 建议并入 Phase 1 关闭**，避免两个任务改同一 prompt 体系。

### 跨平台结论（强制项）

- **Windows**：全部信号可得，方案如上。
- **macOS**：P0 等价信号存在——`NSWorkspace.frontmostApplication`（bundle id 替代 exe 名）+ `CGWindowListCopyWindowInfo`（标题，需屏幕录制权限，或降级仅用 bundle id）。设计上 `scene` 采集入口放 `src/platform/` trait，`SceneKind` 分类与 scene-rules.toml 共享（词表加 `bundle_id` 字段）。实施随 macOS Phase 4-6（MAC-008 之后），Phase 2 只需把 trait 接口留好 + macOS stub 返回 Unknown（行为安全降级）。

---

## 六、风险清单

| 风险 | 等级 | 缓解 |
| --- | --- | --- |
| 聊天框多行注入触发误发送 | 高 | 第四节双保险（prompt 禁用 + 输出单行化 + 强制剪贴板） |
| LLM 重写失真（改了不该改的内容） | 中 | F3 默认关、禁止内容压缩、各指令独立开关可回退 |
| prompt 变长导致小模型指令遵循下降 | 中 | 指令分段短句化；出包前用 Gavin 实际模型端测各开关组合 |
| 窗口标题隐私泄露 | 中 | 默认不上送标题（3.4） |
| exe 词表覆盖不全 | 低 | Unknown 安全降级 + toml 外置随时补词表不出包 |
| UIA 在 Electron/WebView 应用不可读 | 低 | P2 仅 Phase 3 评估，不在主线 |

---

## 七、讨论记录与进展（滚动更新）

### 2026-07-11 · Gavin 质询：是否需要维护 mail/终端/IM/笔记/IDE/浏览器分类软件清单？

**结论：需要，但成本已压到最低，且词表不可完全省掉有硬理由。**

1. **维护负担有界**：桌面场景头部集中，每类维护头部 5~8 个 exe，**20~40 条覆盖 90%+ 真实使用**，不追长尾。
2. **失败模式温和**：未命中 → Unknown → 不注入场景段，行为等同不开场景感知——「少个增强」而非「出 bug」，因此词表无需完备。
3. **更新零成本**：scene-rules.toml 外置（itn-rules.toml 同模式），补条目不出包；用户可自行添加小众软件。
4. **词表不能全用 LLM 替代的硬理由**：
   - `multiline_safe` 是安全属性（微信多行=误发送、终端多行=逐行执行），必须本地确定性判定，猜错一次即事故，只能靠词表；
   - exe 名+标题交 LLM 判断需上送云端，丢掉相对 Typeless 的隐私护城河。
5. **混合路线（Orchestrator 建议）**：
   - 安全裁决（multiline_safe）只信本地词表，Unknown 一律保守 → 真正"必须准"的只有多行危险名单（头部 IM/终端十几条，几乎零维护）；
   - 语气适配（style_hint）本地词表为主，演进开关「未命中时让 LLM 参考应用名自行判断语气」（默认关，开了才上送 exe 名）列入 Phase 3。

**状态**：讨论中，混合路线待 Gavin 认可后固化进 Phase 2/3 任务定义。

---

## 八、待 Gavin 拍板

1. **是否立项 Phase 1（FORMAT-LLM-001）**：含 FILLER-STRIP-001 C 方案合并关闭；A 方案（规则层语气词）是否还要做保底层（LLM 未开启用户），或接受"不开 LLM 不做语义格式化"。
2. **是否立项 Phase 2（SCENE-SENSE-001）** 及排期（建议 Phase 1 端测通过后）。
3. F3 结构重组的内容压缩：确认默认禁止（本方案立场：输入法不该替用户删内容）。
4. 排期相对 0.6.2 端测反馈、accuracy 收口等待办的优先级。
5. **混合路线确认**（2026-07-11 讨论产物）：multiline_safe 只信本地词表 + 语气适配 LLM 兜底开关入 Phase 3。
