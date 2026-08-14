# RESEARCH-TSF-036 · Windows 组合文本（Composition）可行性调研

> **任务**：RESEARCH-TSF-036（纯调研，零代码改动，禁止 PoC）
> **核心问题**：跨进程插入 TSF 组合文本，飞音是否必须注册成活动的 TIP？
> **红线**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0

---

## 〇、结论先行（三选一）

**建议：丙（判死 TSF 跨进程注入路径，走 overlay 回退方案）。**

**A1 的答案：是，必须。** 跨进程插入 TSF 组合文本，飞音必须注册成活动的 TIP（Text Input Processor），且用户必须把系统输入法切换到飞音。这是 TSF 架构的固有设计——text service 是 COM in-proc server，由 TSF manager 加载进目标进程，**不存在「外部进程通过 TSF 往别的进程插组合文本」的 API 通路**。

走 TIP 路线意味着产品形态从「托盘工具 + 全局热键」变成「真·输入法」，代价极大（见 C 节），且无绕开路径（见 B 节）。

**PoC 建议：不值得做。** 核心问题（A1）已由官方架构文档明确回答，不需要 PoC 验证。若 Gavin 仍想验证，最小 PoC 只需验证一件事：注册一个空壳 TIP 后，能否在非活动状态下获得焦点 context 的 `ITfContext` 指针（预期：不能）。但这在官方文档已有明确答案，PoC 价值极低。

---

## 一、A 节 · 可行性主问题

### A1 · 跨进程插入 TSF 组合文本，是否必须是当前活动的 TIP？

**答案：是，必须。**

**出处**：
- [TSF Architecture](https://learn.microsoft.com/en-us/windows/win32/tsf/architecture)：
  > "A text service is implemented as a COM in-proc server that registers itself with TSF. When registered, the user interacts with the text service using the language bar or keyboard shortcuts."
  > "A text service never interacts directly with an application. All communication passes through the TSF manager."

- [Compositions](https://learn.microsoft.com/en-us/windows/win32/tsf/compositions)：
  > "A text service creates a composition by calling ITfContextComposition::StartComposition."

- [ITfContext](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontext)：
  > "The ITfContext interface is implemented by the TSF manager and used by applications and text services to access an edit context."
  > 示例代码显示获取 context 的方式：`pThreadMgr->GetFocus(&pFocusDoc)` → `pFocusDoc->GetTop(&pContext)`

**论证链**：
1. TSF 的三方架构：Application ↔ TSF Manager ↔ Text Service。Text Service 是 COM in-proc server，**被 TSF Manager 加载进应用进程**。
2. `ITfContext` 由 TSF Manager 实现，text service 通过 `ITfThreadMgr::GetFocus` → `ITfDocumentMgr::GetTop` 获取当前焦点 context。
3. `ITfThreadMgr` 是 per-thread 的 TSF manager 实例，只有被 TSF Manager 加载的 text service 才能从线程内获取到它。
4. 创建 composition 需要 `ITfContextComposition::StartComposition`，需要 `TfEditCookie`（来自 `ITfEditSession::DoEditSession`），而 edit session 必须通过 `ITfContext::RequestEditSession` 请求——**所有操作都在 TSF Manager 的管理域内**。
5. **不存在「从外部进程获取另一进程的 ITfContext」的 API**。TSF 是进程内 COM 架构，不是跨进程 RPC。

**结论**：飞音要从外部进程往 Word/Chrome/VS Code 的光标处插入 TSF 组合文本，**唯一途径是注册成 TIP 并被 TSF Manager 加载进那些进程**。而且用户必须把输入法切换到飞音，因为 TSF Manager 只把活动 TIP 加载进焦点应用。

### A2 · TIP 的加载模型

**答案：in-proc COM DLL，会被加载进每一个目标进程。**

**出处**：[TSF Architecture](https://learn.microsoft.com/en-us/windows/win32/tsf/architecture)：
> "A text service is implemented as a COM in-proc server"

**说明**：
- TIP 是一个 DLL（不是独立 exe），通过 COM `CoCreateInstance` 在目标进程内实例化。
- 当用户在 Word 里把输入法切到飞音，TSF Manager 在 **Word 的进程空间内** 加载飞音 TIP 的 DLL。
- 同理在 Chrome、VS Code 里，各自加载一份飞音 DLL 的实例。
- **这意味着飞音的 TIP 代码会在数十个不同进程的地址空间里运行**，每个进程一份独立实例。

### A3 · TIP 崩溃是否会拖垮宿主应用

**答案：是，TIP 崩溃直接拖垮宿主。无进程级隔离机制。**

**出处**：TSF 架构文档明确 text service 是 in-proc COM server。in-proc COM 的固有特性：DLL 在宿主进程地址空间运行，DLL 的异常/访问违规直接导致宿主进程崩溃。

**隔离机制**：
- Windows 不为 TSF text service 提供进程级隔离（不像 out-of-proc COM server 有 surrogate 进程）。
- 理论上可以用 `IUnknown` 的错误返回码做**逻辑层容错**，但**内存安全类崩溃（panic/segfault）无法捕获**——Rust 的 panic 默认 abort 进程。
- 唯一的缓解：Rust 的 `panic = "abort"` 改为 `panic = "unwind"` + catch_unwind，但这只能捕获 Rust panic，不能捕获底层 C/Win32 API 的访问违规。

**影响评估**：飞音 TIP 的 bug 会直接崩溃 Word/Chrome/VS Code 等宿主应用，用户未保存的工作丢失。这是产品风险的致命项。

### A4 · 32 位 / 64 位

**答案：需要按目标进程的位数提供对应 DLL。**

**出处**：COM in-proc server 的固有限制——64 位进程只能加载 64 位 DLL，32 位进程只能加载 32 位 DLL。

**现状**：
- Windows 10/11 上大多数应用是 64 位，但仍有 32 位应用（旧版 Office、某些老工具）。
- 飞音目前只构建 64 位 exe。
- 走 TIP 路线需要同时提供 32 位和 64 位 TIP DLL，否则 32 位应用里飞音输入法不可用。

### A5 · 注册要求

**答案：需要 CLSID 注册表项 + ITfInputProcessorProfiles 注册 + 代码签名。绿色免安装形态不能保持。**

**出处**：[Text Service Registration](https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration)：
> "In addition to the standard COM in-proc server registry entries, a text service must register itself with the Text Services Framework."
> "Text service providers should also provide digital signatures with their binary executables."

**注册步骤**：
1. COM 注册：CLSID 注册表项（`HKEY_CLASSES_ROOT\CLSID\{clsid}`）+ InprocServer32 指向 DLL 路径
2. TSF 注册：`ITfInputProcessorProfiles::Register(clsid)`
3. 语言配置文件：`ITfInputProcessorProfiles::AddLanguageProfile`（每个支持的语言一个）
4. 分类注册：`ITfCategoryMgr::RegisterCategory`（声明是 TIP / display attribute provider 等）
5. **代码签名**：官方文档明确要求 digital signatures

**安装权限**：
- 写 `HKEY_CLASSES_ROOT\CLSID` 需要管理员权限（或 per-user `HKEY_CURRENT_USER\Software\Classes\CLSID`，但 TSF 是否支持 per-user 注册需实测确认）
- 代码签名需要证书（自签名不被 Windows 信任，用户会看到 SmartScreen 警告）

**对绿色免安装形态的影响**：
- 现在飞音是「解压即用」的绿色 exe，无安装过程
- TIP 注册需要写注册表 + 放 DLL 到固定位置 + 签名
- **绿色形态不能保持**，必须改为安装包（至少需要一次注册过程）
- 可以做「首次运行时自动注册 TIP」，但需要管理员权限弹 UAC

---

## 二、B 节 · 绕开路径

### B1 · IMM32 路径

**答案：同样要求是活动 IME。无法绕开。**

**出处**：[ImmSetCompositionString](https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immsetcompositionstringa)：
> 参数 `HIMC`（input method context）—— 这是当前活动 IME 的上下文句柄，通过 `ImmGetContext(hwnd)` 获取。

**说明**：
- `ImmSetCompositionString` 操作的是**当前活动 IME 的 composition string**，不是凭空创建一个 composition。
- `ImmGetContext(hwnd)` 返回的是**绑定到该窗口的当前 IME 的 HIMC**。如果当前活动 IME 是微软拼音，那 HIMC 是微软拼音的上下文。
- 要让 `ImmSetCompositionString` 产生下划线预输入效果，**当前活动 IME 必须是飞音**（或一个配合的 IME）。
- **结论**：IMM32 路径与 TSF 路径一样，要求飞音是活动 IME，不构成绕开。

**补充**：IMM32 是 TSF 的前身（Windows XP 之前的 IME 架构），现代应用（TSF-aware）不直接用 IMM32。IMM32 对 TSF-aware 应用的行为是兼容层模拟，不一定能产生组合文本视觉效果。

### B2 · UI Automation 的 TextPattern / ValuePattern

**答案：主控预判成立——只能设最终文本，无组合态。**

**出处**：UI Automation 的 [TextPattern](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.textpattern) / [ValuePattern](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.valuepattern) 是辅助功能 API，用于读写控件的文本内容。

**说明**：
- `ValuePattern.SetValue()` 直接设置控件的值——这是**最终文本**，没有「未提交」状态。
- `TextPattern` 用于读取文本范围、插入点等，**不支持设置组合文本**。
- UI Automation 的设计目标是辅助功能（屏幕阅读器等），不是输入法模拟。
- **没有「未提交带下划线」的概念**，也无法产生组合文本视觉效果。

### B3 · 其他官方途径

**答案：不存在。**

查遍 Microsoft Win32 文档，产生「未提交带下划线」跨应用视觉效果的唯一官方途径是 TSF composition（或其前身 IMM32 composition），两者都要求是活动 IME/TIP。

不存在「外部进程通过某种 API 让另一进程显示组合文本」的机制——这是**输入法系统的固有设计**，组合文本是输入法与目标应用之间的协议，只有被系统认可的输入法才能参与。

### B4 · 回退方案体验损失

**回退方案**：overlay 浮层实时显示 + 松开后 LLM 格式化再一次性注入（本项目已有 Win32 原生 overlay，DEC-003）。

**体验对比**：

| 维度 | TSF 组合文本（理想态） | Overlay 回退方案 |
|------|------------------------|------------------|
| 实时反馈位置 | 在光标处（输入框内） | 屏幕底部浮层（不在输入框内） |
| 视觉形态 | 下划线未提交文本 | 浮层显示文字 |
| 用户心智 | 「正在输入，还没确认」 | 「系统在听写，结果在浮层」 |
| 松键后 | 组合文本变正式文本（在光标处） | 一次性注入光标处（与现状一致） |
| LLM 格式化 | 替换组合文本（视觉连续） | 注入新文本（可能有闪烁） |
| 出错时 | cancel composition 即撤销 | 不注入即可（无副作用） |

**核心损失**：组合文本的「在光标处实时显示」体验无法用 overlay 复现。overlay 在屏幕底部，用户眼睛要在「输入框光标」和「屏幕底部浮层」之间切换。但这与**现有飞音的体验一致**（现在就是 overlay + 松键注入），所以回退方案 = 维持现状，无体验退化（相对当前版本）。

---

## 三、C 节 · 交互与产品形态（若走 TIP 路线）

### C1 · 用户怎么正常打字

**答案：TIP 必须做全键盘透传。代价中等，风险高。**

**说明**：
- 用户把输入法切到飞音后，所有键盘事件先经过飞音 TIP。
- 飞音 TIP 必须把非热键的键盘事件**原样传递给应用**，否则用户无法打字。
- TSF 的键盘透传通过 `ITfKeyEventSink` 接口实现——TIP 可以选择「不处理」某个按键，TSF Manager 会把它传给应用。
- **代价**：实现 `ITfKeyEventSink`，正确判断哪些键该吞（热键）、哪些该透传（所有其他键）。
- **风险**：透传逻辑的 bug 会导致用户打字丢键/乱序，这是高频伤害场景。

### C2 · 与用户原有输入法的切换

**答案：无法做到「按热键时临时接管、松开还回去」。**

**说明**：
- TSF 的输入法切换是**系统级状态**，通过 `ITfInputProcessorProfiles::ActivateLanguageProfile` 切换。
- 切换输入法是**有副作用的操作**——系统语言栏变化、其他应用的输入法也跟着变（TSF 是 per-thread 的，但 UI 语言栏是全局的）。
- 「按热键时切到飞音、松开切回去」意味着每次录音都要做两次系统输入法切换——**延迟高（每次切换 ~100-300ms）、用户体验割裂**。
- 更严重：如果用户录音时正在用微软拼音打一半拼音，切到飞音会**打断用户的拼音输入**。

**结论**：TIP 路线的产品形态是「用户主动选择用飞音输入法」，不是「用热键临时借用」。与现有「按住说话」的交互模式根本冲突。

### C3 · 现有全局热键与 TIP 键盘处理冲突

**答案：冲突。两套机制不能共存于同一按键。**

**说明**：
- 现有 `RegisterHotKey`（DEC-004）是系统级全局热键，由 Windows 热键表管理。
- TIP 的 `ITfKeyEventSink` 是输入法级键盘拦截，发生在热键之前。
- 如果同一按键既注册了 `RegisterHotKey` 又被 TIP 拦截，行为未定义（实测可能 TIP 吞掉热键）。
- 走 TIP 路线后，热键需改为由 TIP 的 `ITfKeyEventSink` 处理，放弃 `RegisterHotKey`。

---

## 四、D 节 · 实现代价

### D1 · Rust 生态 TSF 绑定覆盖度

**答案：windows-rs 覆盖了 TSF COM 接口的 IDL 定义，但无高层封装。缺口在 COM 实现侧。**

**说明**：
- `windows` crate（项目用 0.52.0/0.54.0）通过 Windows Metadata 自动生成 Win32 API 绑定，包含 `msctf.h` 的 TSF 接口（`ITfThreadMgr`、`ITfDocumentMgr`、`ITfContext`、`ITfComposition`、`ITfEditSession` 等）。
- 但 TSF text service 需要的不是「调用」这些接口，而是「实现」COM 接口（`ITfTextInputProcessor` / `ITfKeyEventSink` / `ITfEditSession` / `ITfCompositionSink` 等）——这需要 `windows-implement` 或 `windows-interface` crate 做 COM 实现。
- `windows-implement` 0.53.0 支持 `#[implement]` 宏实现 COM 接口，但 TSF 的回调接口较多，需要逐个实现。
- **缺口**：无现成的 TSF TIP 骨架或高层封装 crate。社区无活跃的 TSF Rust 项目（搜索 crates.io 无结果）。

### D2 · 最小可用 TIP 需实现的 COM 接口清单

| 接口 | 职责 | 必须 |
|------|------|------|
| `ITfTextInputProcessor` | TIP 主入口，`Activate`/`Deactivate` | ✅ |
| `ITfTextInputProcessorEx` | 扩展激活接口 | 推荐 |
| `ITfKeyEventSink` | 键盘事件拦截/透传 | ✅ |
| `ITfEditSession` | edit session 回调（读写文本） | ✅ |
| `ITfCompositionSink` | composition 事件回调 | ✅ |
| `ITfDisplayAttributeProvider` | 提供组合文本显示属性（下划线） | ✅ |
| `IEnumTfDisplayAttributeInfo` | 枚举显示属性 | ✅（配合上条） |
| `ITfThreadMgrEventSink` | 线程管理器事件（焦点变化） | 推荐 |
| `ITfCreatePropertyStore` | 属性存储 | 可选 |

**注册侧**：
- `ITfInputProcessorProfiles::Register` + `AddLanguageProfile`
- `ITfCategoryMgr::RegisterCategory`（`GUID_TFCAT_TIP_KEYBOARD`、`GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`）

### D3 · 工程量粗估 + 三大技术风险

**工程量**：~30-40 人天（有 TSF 经验的开发者），~60-80 人天（从零学 TSF）。

拆分：
- TIP 骨架 + COM 注册：5-8 天
- 键盘透传逻辑：3-5 天
- Composition 创建/更新/提交：5-8 天
- Display attribute（下划线）：3-5 天
- Edit session 文本读写：3-5 天
- 热键集成（替代 RegisterHotKey）：3-5 天
- 32/64 位双构建 + 安装包：3-5 天
- 跨应用兼容性测试（Word/Chrome/VS Code/记事本等）：5-10 天

**三大技术风险**：

1. **崩溃拖垮宿主**（致命）：TIP 在数十个进程内运行，任何内存安全 bug 直接崩溃宿主应用。Rust 的安全性不覆盖 FFI 边界——TSF COM 调用的参数错误、null 指针、use-after-free 都可能导致宿主崩溃。测试不可能覆盖所有宿主应用的所有状态。

2. **键盘透传的兼容性地雷**：不同应用对 TSF 键盘事件的处理方式不同（Chrome/Electron 可能不完全遵循 TSF 协议），透传逻辑需要逐应用适配。丢键/乱序是高频用户伤害。

3. **产品形态不可逆**：从「托盘工具」变「输入法」后，用户体验、安装方式、与现有输入法的关系全部改变。如果上线后发现体验不可接受，回退意味着全部 TIP 代码废弃——**沉没成本极高**。

---

## 五、E 节 · 跨平台公用代码设计

### E1 · 平台无关组合文本抽象

```rust
/// 组合文本状态机
///
/// 状态转换：
///   Idle → (begin) → Composing
///   Composing → (update) → Composing
///   Composing → (commit) → Committed → Idle
///   Composing → (cancel) → Idle
///
/// 跨平台语义：
///   Windows: TSF ITfComposition (需 TIP，见 A1)
///   macOS: NSTextInputClient.setMarkedText / insertText (需是活动输入源)
///   Linux: IBus update_preedit_text / commit_text (需是活动 IBus engine)
pub trait CompositionText: Send + Sync {
    /// 开始组合文本会话。在光标处创建未提交文本区域。
    /// 初始文本可为空或含首段流式结果。
    fn begin_composition(&mut self, initial_text: &str) -> Result<()>;

    /// 更新组合文本内容（替换未提交区域的文本，保留下划线标记）。
    /// 流式 ASR 每出新片段调一次。
    fn update_composition(&mut self, text: &str) -> Result<()>;

    /// 提交组合文本——未提交文本变为正式文本，下划线消失。
    /// LLM 格式化完成后调用，提交格式化后的最终文本。
    /// 若 final_text 与当前组合文本不同，先替换再提交。
    fn commit_composition(&mut self, final_text: &str) -> Result<()>;

    /// 取消组合文本——未提交文本消失，不注入任何内容。
    /// 用户取消录音或出错时调用。
    fn cancel_composition(&mut self) -> Result<()>;
}

/// 组合文本会话状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompositionState {
    Idle,       // 无组合文本
    Composing,  // 有未提交的组合文本
    Committed,  // 已提交（瞬态，立即回 Idle）
}
```

### E2 · 三端映射与语义不一致

| 抽象方法 | Windows (TSF) | macOS (NSTextInputClient) | Linux (IBus) |
|----------|---------------|---------------------------|--------------|
| `begin_composition` | `ITfContextComposition::StartComposition` + `ITextStoreACP::InsertTextAtSelection` | `setMarkedText(_:selectedRange:replacementString:)` | `update_preedit_text(text, cursor_pos, visible)` |
| `update_composition` | `ITextStoreACP::SetText` on composition range | `setMarkedText` (再次调用替换) | `update_preedit_text` (再次调用) |
| `commit_composition` | `ITfComposition::EndComposition` + 先 `SetText` 替换为 final | `insertText(_:replacementRange:)` | `commit_text(text)` + `hide_preedit_text` |
| `cancel_composition` | `ITfContextOwnerCompositionServices::TerminateComposition` | `setMarkedText("", ...)` (空串取消) | `hide_preedit_text` |

**语义不一致点**（决定抽象是否漏参数）：

1. **光标位置**：macOS `setMarkedText` 有 `selectedRange` 参数指定组合文本内的光标位置；IBus `update_preedit_text` 有 `cursor_pos`；TSF 通过 `ITfContext::SetSelection` 单独设置。**抽象漏了 `cursor_pos` 参数**——应补：
   ```rust
   fn update_composition(&mut self, text: &str, cursor_pos: Option<usize>) -> Result<()>;
   ```

2. **替换范围**：macOS `insertText` 有 `replacementRange` 可指定替换已有文本的范围；TSF/Linux 无直接对应（TSF 靠 range 操作，IBus 靠 commit_text 替换整段）。**抽象的 `commit_composition` 漏了可选替换范围**——但本项目场景是「替换整个组合文本」，不需要额外参数。

3. **显示属性**：TSF 需要单独实现 `ITfDisplayAttributeProvider` 提供下划线样式；macOS 的 `setMarkedText` 自带下划线；IBus 的 preedit 自带下划线。**这是实现侧差异，不影响 trait 签名**。

**修正后的 trait**：
```rust
pub trait CompositionText: Send + Sync {
    fn begin_composition(&mut self, initial_text: &str) -> Result<()>;
    fn update_composition(&mut self, text: &str, cursor_pos: Option<usize>) -> Result<()>;
    fn commit_composition(&mut self, final_text: &str) -> Result<()>;
    fn cancel_composition(&mut self) -> Result<()>;
}
```

### E3 · 与现有 `src/platform/` 分层对接

**现状**：`src/platform/mod.rs` 有 15 个平台契约符号（见 `docs/MACOS-HANDOFF.md` §1），包括 `inject_text`、`capture_scene_signals`、`capture_focused_text_snapshot` 等。

**扩充建议**：
- 新增 `composition_text` 契约符号（返回 `Box<dyn CompositionText>`）
- Windows 侧实现：`src/platform/windows/composition.rs`（TSF 实现，**若走 TIP 路线才需要**）
- macOS 侧：`src/platform/macos/composition.rs`（`NSTextInputClient` 实现，**需注册为输入源**）
- Linux 侧：`src/platform/linux/composition.rs`（IBus engine 实现）

**但本调研的结论是丙（不走 TIP）**，所以：
- **Windows 侧**：不新增 `composition_text` 契约，继续用 `inject_text`（一次性注入）
- **macOS/Linux 侧**：同理，不走输入法路线
- **跨平台抽象**：`CompositionText` trait 的设计**保留在方案文档中**，待 Gavin 确认要做输入法时再用

**对 `MACOS-HANDOFF.md` 的影响**：本调研结论为丙，不新增平台契约，MACOS-HANDOFF 无需更新。若 Gavin 拍板走 TIP 路线，则需在 §1 契约清单新增 `composition_text` 并在 §2 各节说明 macOS 对应实现。

---

## 六、PoC 建议

**不值得做。**

理由：
1. **A1 已由官方架构文档明确回答**——TSF text service 是 in-proc COM server，不存在跨进程注入通路。这是架构级事实，不需要 PoC 验证。
2. PoC 能验证的唯一新信息是「具体哪些应用拒绝 composition」，但这是兼容性细节，不影响「是否走 TIP 路线」的决策。
3. PoC 需要先注册 TIP（写注册表 + 签名 + 重启系统识别），本身工程量 ~5 天，投入产出比极低。

**若 Gavin 坚持要 PoC**，最小 PoC 应验证：
1. 注册一个空壳 TIP（只实现 `ITfTextInputProcessor::Activate/Deactivate`）
2. 用户切换到该 TIP 后，在记事本中能否通过 `ITfContextComposition::StartComposition` 产生下划线文本
3. 验证 TIP DLL 确实被加载进记事本进程（用 Process Explorer 确认）

这验证的是「TSF composition 在最简应用里能不能工作」，不是「跨进程注入」——后者已被架构文档否定。

---

## 七、出处链接汇总

| 内容 | 链接 |
|------|------|
| TSF 架构（三方架构 + in-proc COM） | https://learn.microsoft.com/en-us/windows/win32/tsf/architecture |
| Compositions（组合文本概念） | https://learn.microsoft.com/en-us/windows/win32/tsf/compositions |
| ITfContext（edit context 接口） | https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontext |
| ITfContextComposition::StartComposition | https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition |
| Text Service Registration（注册要求 + 签名） | https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration |
| Edit Sessions | https://learn.microsoft.com/en-us/windows/win32/tsf/edit-sessions |
| Document Locks | https://learn.microsoft.com/en-us/windows/win32/tsf/document-locks |
| Text Services（text service 编程元素） | https://learn.microsoft.com/en-us/windows/win32/tsf/text-services |
| Applications（TSF-enabled app 编程元素） | https://learn.microsoft.com/en-us/windows/win32/tsf/applications |
| ImmSetCompositionString（IMM32 路径） | https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immsetcompositionstringa |
| Input Method Manager Reference | https://learn.microsoft.com/en-us/windows/win32/intl/input-method-manager-reference |
| NSTextInputClient（macOS 对应物） | https://developer.apple.com/documentation/appkit/nstextinputclient |

---

## 八、对 RESEARCH-ASR-035 的关系说明

本任务不涉及 035 的流式 ASR 模块设计。035 原方案「录完整段再发（非真流式）」的前提已被 Gavin 推翻，需按流式重做——那是另一个任务。本调研的结论（不走 TSF，走 overlay 回退）意味着：
- 流式 ASR 的中间结果**显示在 overlay 浮层**（屏幕底部），不在光标处
- 松键后整段送 LLM 格式化，再一次性注入光标处（与现状一致）
- **035 的流式 ASR 模块设计不受影响**，只是流式结果的展示位置在 overlay 而非组合文本

---

> **本文件为零代码改动研究产物**。`git diff --ignore-cr-at-eol --numstat -- src/` == 0。