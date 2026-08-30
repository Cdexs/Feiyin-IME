# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-18 — coder-2 — OVERLAY-054-C/D/E + 054-F ✅ 视觉项 + 编辑态字体加档（src/main.rs 唯一改动，阶段一完成，待主控验收）

- **来源**：Gavin 令「三个问题一起做了再出包」，后追加「编辑态文字再调大一号」。任务书由主控在会话重启后重写，Worker 独占 `src/main.rs`
- **054-C 边框提亮收敛**：新增文件级 `OVERLAY_BORDER_GRAY = COLORREF(0x3A3A3C)`；原 10 处散落局部 `0x060607`（含 `CIRC_BORDER`）全部改引用该常量。覆盖窗口边框、分隔线、编辑态、处理中/预览/错误态。Gavin 拍板「轮廓清楚」
- **054-D 编辑态锯齿根因修复**：新增 `OverlayWindowState.edit_font: Option<HFONT>`；`create_edit_control` 创建 EDIT 后 `SendMessageW(WM_SETFONT)` 发独立 ClearType 字体并 `lParam=1` 立即重绘；`destroy_edit_control` 在 `DestroyWindow(edit_hwnd)` 之后 `DeleteObject(font)`。未复用 `cached_font`（其生命周期被 `take()+DeleteObject` 绑定在隐藏/退出）。圆角硬裁剪本批未动
- **054-E 字号统一**：新增文件级 `OVERLAY_FONT_SIZE: i32 = -13`；`overlay_paint` 缓存字体与 `adjust_overlay_pos_size_for_text` 量宽字体同步替换，防止窗口宽度算错
- **054-F-B EDIT 框几何抽纯函数**：新增 `compute_edit_box_geometry(rect_h, tm_height, fixed_margin, corner_floor) -> (top_offset, height)`，零行为变更；desired <= available 时 height 恰好等于 desired，top_offset 仅由居中值与 `corner_floor` 决定，`fixed_margin` 不再参与 `.max()` 链；`create_edit_control` 内联算式改为调用该纯函数。
- **054-J 修正 `compute_edit_box_geometry` 契约 4**：只改函数体，`mod tests` 不动。旧语义 desired > available 时回退 fixed_height=16，导致 tm=26→27 高度 28→16 断崖；新语义 `height = desired.min(available).max(1)`（尽量给、最多给到 available）。调用侧 `log::error!` 不变。真实运行域 tm≈17~19 新旧逐位一致。
- **054-H 自绘文字区垂直内缩 4px 防裁字**：Gavin 裁决；新增 `OVERLAY_TEXT_DRAW_VERTICAL_INSET = 4` 仅供自绘文字区；`draw_recording_overlay_with_text` 的 `text_top/text_bottom` 改用它（16px → 28px）。视觉零位移：10/10 与 4/4 都关于窗口中心对称，`DT_VCENTER` 参考中心不变。未碰横向 clip region；未加 `DT_NOCLIP`；未加高窗口；`STREAMING_TEXT_*_MARGIN` 仍为 10（EDIT 回退布局与 `text_hit_rect` 用）。全文件 `draw_text` 调用点扫描，<24px 的只有 16x16 `⏎` 箭头与 18x18 X 按钮图标（已报出，未改）。
- **054-G 字号统一为 -14**：删除 `OVERLAY_EDIT_FONT_SIZE`；`OVERLAY_FONT_SIZE` 从 -13 改为 -14（Gavin 拍板「上屏文字提上来对齐编辑态」）；`create_edit_control` 两处字体与 `adjust_overlay_pos_size_for_text` 量宽字体统一引用 `OVERLAY_FONT_SIZE`，删除 054-F 的 status 分支。
- **054-F-B EDIT 框几何抽纯函数**：新增 `compute_edit_box_geometry(rect_h, tm_height, fixed_margin, corner_floor) -> (top_offset, height)`；desired <= available 时 height 恰好等于 desired，top_offset 仅由居中值与 `corner_floor` 决定，`fixed_margin` 不再参与 `.max()` 链；`create_edit_control` 内联算式改为调用该纯函数。
- **字体同源复核**：全文件仅 `create_clear_type_font:325` 一处 `CreateFontW`；上屏缓存字体与 EDIT 控件字体均经此创建，face="Segoe UI" / weight=FW_NORMAL / quality=CLEARTYPE_QUALITY 完全相同。
- **测试区同步**：`:6439/:6454/:6455/:6706` 四处测试字面量从 `0x060607` 改为 `0x3A3A3C`
- **主控追加授权**：`BTN_BORDER` 提到文件级 `OVERLAY_BTN_BORDER = COLORREF(0x707070)`，值不动；`:6708` 亮度判据从失效的 `4x` 改为「逐通道 ≥0x20 + 总亮度方向更亮」，注释说明旧 4x 是近黑时代偶然产物
- **验证**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error（108 既有 warnings，无新增指向本批改动）/ `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error
- **红线合规**：只改 `src/main.rs`；未动版本号 0.8.0 / `bErase=false` / `pos:[0,0]`；未加高窗口 / 未加 DT_NOCLIP；未 commit；未跑 `cargo test` / 未出包（阶段一）
- **遗留**：Gavin 端测目视拍板按钮边框与窗口边框是否仍可区分；TEST-SYNC-054 阶段三将跟 054-J 测试语义更新 + 单调性全区间转绿
- **详情**：logs/20260818.md + CHANGELOG.md + result.md

---

## 2026-08-18 — tester-1 — E2E-HARNESS-050 ✅ 修复 E2E 两道陈年错位 + 全套 harness 修复（9 FAIL→2 FAIL，待主控验收）

- **来源**：Gavin 批评「代码几轮修改仍未拿到主程序流程完全走通的版本」，主控复盘根因是 E2E 门禁**自初始提交起从未绿**。基线 HEAD `0049c33`（⚠️ 工作区实际含 coder-2 **未提交** OVERLAY-051-G WIP：`src/main.rs` +270 / `qwen_inference.rs` +45，全程不触碰）
- **错位一（配置路径）**：harness 写 `%APPDATA%\voice-ime\config.toml`，程序 `src/config/mod.rs:340-346` 只读 `<exe_dir>/config.toml`。三处实例统一改为 `tests/conftest.py` 新增 `voice_ime_config_file()` 单一来源；未加 `VOICE_IME_CONFIG` 环境变量（遵 DEC-031）
- **错位二（overlay 判据）**：`state_detector.py` 导入时正则解析 `src/main.rs:880-884` 三常量动态建表（零硬编码）+ `TOLERANCE_PX=15` 容差带（依据 <3px 抖动 << 80px 状态间隔）；`recording==processing` 240x36 纯尺寸不可分局限已在 docstring 如实声明
- **附带修复**：`test_platform.py` `--lib`→`--bin feiyin-ime`（bin-only crate）+ `_cargo_bin()` 定位 + UTF-8 capture（原 GBK 崩线程）；`test_tauri_v2_commands.py` 白名单补 3 命令（前端 6 个 invoke 已核实全覆盖）
- **hotkey cold 竞态**（[E2E-COLD-START-RACE-001]）：worker `src/main.rs:3969-3974` Start 清空 stop 信号，cold 窗口内二击丢失 / PTT<300ms 防误触 `cancel-stop`；warm 4/4 探针证据 → harness 加 `_prewarm_recording()` 预热往返，不改产品不弱化断言
- **实跑**：Publish/ 真包 `pytest tests/test_cases/ -m "not hardware"` → **2 FAIL / 61 PASS / 32 SKIP / 7 deselected（142.96s）**，基线 9 FAIL 中 **7 条 harness 全过**；残留 2 条 `test_cargo_test_*` = **② coder-2 阻塞**（未提交 WIP `qwen_inference.rs:712` 签名改 vs `:1645` 测试闭包 1 参 → E0593，非 harness）
- **红线合规**：`git diff --stat` 仅 `tests/**` 8 文件（+138/-47，CRLF 归一）；生产零改动；未放宽断言；config.toml 字节级还原（最终 sha=`3186ec8c05cd…`=备份）；guard 改字节级还原防 LF→CRLF；版本号/commit/出包均未动
- **详情**：result.md + logs/20260818.md + CHANGELOG.md + troubleshooting [E2E-COLD-START-RACE-001] 新增

---

## 2026-08-18 — coder-1 — OVERLAY-051-G-FIN ✅ 051-G 收尾：时间戳驱动回放（纠正方向，src/main.rs + qwen_inference.rs +264/-58，待主控验收）

- **来源**：昨天被打断的 051-G 半成品是「固定速率打字机」（每字 clamp 180-350ms、backlog 摊 1000ms、超 20 字 jump），违反 Gavin 三条指示（时间戳驱动/不压缩停顿/words 空退回立即显示）。基线 HEAD 12f0915
- **改动**（仅 src/main.rs + src/transcription/qwen_inference.rs +264/-58）：
  1. 修编译错误（qwen_inference.rs:1653 测试闭包 |_, _|{} 适配 :712 新签名 FnMut(&str, &[WordTiming])）
  2. WordTiming 补 end_time/punctuation 两字段（缺字段降级不整条丢弃，官方 schema 取证 asr-qwen-audio-3.0-integration-001.md:105-107）
  3. **words 累积放 StreamingAsrState**（关键设计，避免原 WIP 的 extend 合并 bug）：增 confirmed_words/current_words 字段 + on_result 扩 words 参数（与文本同构）+ display_words()；回调每次下发与 display_text 完全对齐的全量词表 → overlay 侧 UpdateWordTimings 整体替换
  4. 删 compute_tween_advance + 5 速率常量 + 9 测试，新增 reveal_chars_by_timeline 纯函数（契约：words 空→立即全显/以 words[0].begin_time 取差值不依赖绝对起点/不压缩停顿/单调不回退/.min(total_chars) 宁可多显绝不少显）
  5. RecordingWithText 分支改时间戳驱动（origin 墙钟 + reveal_chars_by_timeline + .max(displayed) 单调 + 降级 words 空立即全显）+ 四个清理点补 word_timings.clear/tween_audio_origin=None 防跨会话复用
- **自证**：① words 累积放 StreamingAsrState 与文本同构（end=true push confirmed 并清空 current；同 id 整体替换 current；新 id 换 id 替换 current）→ display_words 与 display_text 字符级对齐 → UpdateWordTimings 整体替换无合并 bug；② reveal_chars_by_timeline 五契约逐条推演（words 空→total_chars；words[0].begin_time 取差值 1.5s pre-roll 消掉；不压缩停顿无上限下限；.max(displayed) 单调；.min(total_chars) 宁可多显）；③ 降级路径 src/main.rs:1365 word_timings.is_empty()→立即全显；④ 四个清理点补防跨会话复用；⑤ 未动三条历史红线（InvalidateRect bErase=false/interpolate_step/STREAMING_STOPPED）
- **验证**：cargo fmt clean / cargo check --all-targets 0 error（99 既有 warnings 无新增）/ cargo check src-tauri --all-targets 0 error / git diff --stat 仅两文件 +264/-58 / 残留自查全空（grep 不到 compute_tween_advance 及 5 常量）/ 版本号 0.8.0 三处未动
- **红线合规**：未碰 src/audio/**/src/vad.rs/ui/**/src-tauri/**/tests/**（文件域独占）/ 未跑 cargo test/build（阶段一）/ 未 push / 未 commit（主控统一提交）/ 版本号 0.8.0 未动
- **Gavin -debug 实证前暂定参数**：无新常量引入（删掉了 5 个）。时间戳驱动完全依赖服务端 words[].begin_time，无任何人为参数。下一棒 tester-1 出诊断包 → Gavin 跑 -debug 看 words=N 实际值校准：① 词表是否覆盖完整文本；② begin_time 差值是否与墙钟对齐
- **详情**：outbox/coder-1/result.md + logs/20260818.md + CHANGELOG.md（OVERLAY-051-G-FIN 条目）+ docs/MACOS-HANDOFF.md §OVERLAY-051-G-FIN

---

## 2026-08-18 — coder-1 — OVERLAY-054-B-FIX ✅ 录音窗口闪左上角修复（类型层面根治，src/main.rs，待主控验收）

- **来源**：Gavin 2026-08-18 端测第二次报同一现象（上一轮只修本地模型路径，在线路径没修）。录音开始窗口先闪屏幕左上角再跳回正确位置
- **根因**（主控已 Read 取证三步闭环）：show_overlay_streaming_idle 绕过 overlay_geometry 硬写 pos:[0,0]（在线流式第一次 Show）；OVERLAY-046 恢复无条件 SetWindowPos 后 [0,0] 真的生效 → 窗口摆到左上角；随后第一条 StreamingText 走 show_overlay 算出正确位置 → 窗口跳一下。三处 pos:[0,0] 的实际危害：streaming_idle 真错位（Gavin 看到的）/ FocusLost 真错位（注入失败提示框会出现在左上角）/ StreamingEditing 当前无害（adjust_overlay_pos_size_for_text 重算 x/y 忽略传入 pos）但同样要清
- **改动**（仅 src/main.rs，与 051-G-FIN 同文件串行无冲突）：
  1. OverlayRequest.pos 从 [i32;2] 改为 Option<[i32;2]>（None=调用方无位置可给，overlay 线程 Show 时解析）
  2. Show 端入口 unwrap_or_else(|| overlay_geometry(&request.status, hwnd).0) 解析，写回 Some(resolved_pos) —— 在 overlay 线程内算比调用侧算更对（monitor_work_rect 解析的是 overlay 窗口所在显示器，多屏时调用侧 hwnd 可能不同块）
  3. streaming 分支用 final_pos 局部变量跟踪，最终 request.pos = Some(final_pos)
  4. SetWindowPos 读 resolved_pos（已解析）而非 request.pos[0]
  5. 插值路径 req.pos.unwrap_or([0,0]) 兜底（state.request 存的已是 Some）
  6. 9 个构造点：3 处 [0,0] 改 None（streaming_idle/FocusLost/StreamingEditing）；4 处已算好位置改 Some(pos)；1 处 overlay 线程内 .. 模式匹配不改；1 处 show_overlay 改 Some(pos)
- **同批 051-G-FIN 尾部托底修复**：reveal_chars_by_timeline 循环记录 broke_early，若无 break（全部词到期）→ return total_chars（词表覆盖不到的尾部一并放出，契约 5「宁可多显绝不少显」的托底，修前尾部差额卡到松键 flush 才补上，端测表现是最后一两字迟迟不上屏）
- **自证**：① grep "pos:[0,0]" 零命中；② OverlayRequest.pos 已是 Option<[i32;2]>（:182）；③ Show 端 unwrap_or_else 兜底（:1044）；④ 9 构造点逐点对照表（见 result.md）；⑤ 三条红线未动（OVERLAY-046 无条件 SetWindowPos 仅改读 resolved_pos 不改调用 / InvalidateRect bErase=false / 051-G 揭示+interpolate_step+STREAMING_STOPPED）
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri --all-targets 0 error / git diff --stat 仅 src/main.rs + qwen_inference.rs / 版本号 0.8.0 三处未动
- **红线合规**：未跑 cargo test/build（阶段一）/ 未 push / 未 commit（主控统一提交）/ 未碰 src/audio/**/src/vad.rs/ui/**/src-tauri/**/tests/**
- **详情**：outbox/coder-1/result.md + logs/20260818.md + CHANGELOG.md + docs/MACOS-HANDOFF.md §OVERLAY-054-B-FIX

---

## TEST-SYNC-051G/054B · 阶段三 · coder-2 → tester-1 → 全体

- **交接物**：`src/main.rs` 新增 `mod overlay_051g_reveal_tests`（T1-T8，+122）；`src/transcription/qwen_inference.rs` `mod tests` 追加（T9-T16，+210）。工作区已含两文件新增，**未 commit**。
- **运行要求**：tester-1 阶段四 `cargo test` 请以 `export PATH="/c/Users/Aaron-GMK/.cargo/bin:$PATH"` 开头；仅白名单 `cargo fmt` / `cargo check`（DEC-048）。`cargo test` 已在消融推演列表就绪（A 删 `- origin_begin`→T2 红；B 引入 clamp→T3 红；C append 回归→T10/T12 红）。
- **已评估未实施**：OVERLAY-054-B-FIX 的 E2E 定位断言（`state_detector.py` rect + `MonitorFromWindow` 判下半部）需构建产物，留作未来增强单独派发。
- **提出者追加**：OVERLAY-051-G 回放 reveal 的 EST-09 命题源自既有 EST-04/05/06/07（tester-1 已置 Reflect）；OVERLAY-054-B-FIX 命题 EST-11 置为 verify-pass，spec 不新增测试要求。

---

## 2026-08-18 — 主控 — tester-1 额度耗尽，TEST-EXEC-056 中断（进度存档，供重启后接手）

- **中断点**：阶段四 TEST-EXEC-056 **未交付**，`outbox/tester-1/result.md` 为空，
  五文档均无本单条目。从 pane 残留看，它已进入消融环节并做过 `git stash` / `stash pop`。
- **仓库状态（主控已核）**：HEAD `cb0d82b` 未变；`git stash list` **空**（无残留）；
  `git diff --ignore-cr-at-eol` **空**（仅 CRLF 噪声，非真实改动）；
  唯一残留是仓库根目录一个 **0 字节** 的 `gray_sum` 文件（命令重定向手误产物），已删。
- 🔴 **违规记录（重启后要转达新实例）**：任务书明令「还原禁用 `git reset/checkout/stash/clean`」，
  它仍用了 `git stash`。所幸 `stash pop` 已还原、无数据丢失。
  **原因是它把「消融还原禁令」理解成只约束消融那一步** —— 重启后的任务书要写死：
  **整个任务期间对本仓库禁用这四条命令，不分场景**。
- **重启后从零重跑 TEST-EXEC-056**（四步回归 + 消融 A/B/C/D），不继承任何中间结论。

---

## 2026-08-18 — tester-1 — TEST-EXEC-056 ✅ 阶段四全量回归 + 消融实测（重启后从零重跑，③ 真回归 0，过闸）
- **来源**：主控派单（基线 HEAD `1668abe`，工作区 clean）。上一实例额度耗尽中断，本单从零重跑，不继承任何中间结论
- **四步回归全过**：Step1 `cargo test` = **1113/0/11**（基线 1083，+30 全量归因：feiyin +16 = config/mod.rs +14 + transcription/mod.rs +1 + qwen_inference.rs +1；crash-reporter +14 = config 测试模块经 #[path] 编入重复计数；integration 36 不变）→ Step2 Vitest **84/84**（+6 = Voice.test.tsx 26→32）→ Step3 src-tauri **76/0**（+2 = src-tauri/config.rs 5→7）→ Step4 pytest **SKIP**（Publish/ 17:22 早于 HEAD 21:03，BUILD-021 旧包，E2E 留阶段五 Step 5）
- **消融实测**：**A**（model_family_prefix 改回 splitn(2,'-')）→ **4 红**（实例表 + v2 回归护栏 + 2 resolve 链路，本批最重要护栏实证真能抓住）；**B**（is_online_streaming 去 FunAsrRealtime）→ **1 红**（四变体穷举）精确吻合；**C**（隐藏字段 default 改 500）→ **1 红**（默认值护栏）精确吻合；**D**（handleAsrModelChange 改回两次连调）→ **3 红**（FUN-UI-004 + FALLBACK-002 + FUNFALLBACK-002 同因扩散，证明 UI 用例断言真行为非假护栏）
- **还原自证**：四次消融均编辑器还原（禁 git reset/checkout/stash/clean），每次 `git diff -w` 0 字节；终态工作区完全 clean（status/diff -w/diff --ignore-cr-at-eol 全空）；复跑 cargo test 1113/0/11 + Vitest 84/84 + src-tauri 76/0 逐数一致
- **红条三分类**：③ 真回归 **0** / ① 0 / ② 0，过闸无阻塞项
- **红线合规**：版本号 0.8.1 未动 / 未 commit / 未出包（BUILD-022 待主控下令）/ 未 push
- **详情**：outbox/tester-1/result.md + logs/20260818.md + CHANGELOG.md
→27 高度 28→16 断崖；新语义 `height = desired.min(available).max(1)`（尽量给、最多给到 available）。调用侧 `log::error!` 不变。真实运行域 tm≈17~19 新旧逐位一致。

---

## 2026-08-18 — tester-1 — BUILD-022 ✅ v0.8.1 阶段五出包（fun-asr-realtime 进 exe，零生产改动）
- **来源**：主控派单（基线 HEAD `1668abe`，主控裁定跳过回归快门）。Gavin 急着端测 fun-asr 新模型
- **四步构建**：Step1 无残留进程（feiyin-ime-nor.exe 备份保留）→ Step2 npm build 682ms（index-3w1Jngeu.js 新）+ Tauri UI 1m49s（custom-protocol）+ cp 时间戳一致 → Step3 主程序 2m02s（110 warnings 全既有）→ Step4 同步 Publish/ 三 exe + scene/itn 两 toml 三副本（config.toml 运行时数据未覆盖）
- **七项核验全 PASS**：① 六 exe 时间戳本次构建 ② 三 exe 两副本 sha 逐一相等（feiyin `2eac2008…`/ui `d9f13438…`/crash `fb51e32c…`）③ 两 toml 三副本 hash 全等（scene `0a3a0b9a…`/itn `b208271b…`）④ **ProductVersion 0.8.1.0/0.8.1/0.8.1.0 关键核验点通过** ⑤ **正向探针 fun-asr-realtime count=1 新模型真进包**（反向 qwen-audio count=1 两模型并存）⑥ 大小 feiyin 12,180,480B（+13.5KB 合理）/ui 不变/crash 不变 ⑦ 冒烟 PID 15184 Responding=True 无 panic
- **Step5 E2E 门禁 0 FAIL**：65 PASS / 33 SKIP / 7 deselected（174.49s），与 BUILD-021 基线精确一致零真回归；测后清理残留 notepad
- **红线合规**：版本号 0.8.1 未动 / 未 commit / 未 push / Publish 运行时数据未覆盖 / feiyin-ime-nor.exe 保留
- **Gavin 端测**：设置→语音输入→ASR 模型下拉选 fun-asr-realtime（qwen 保留可切回，不需重启）；debug.log 看模型分辨行 + `[Latency]` 分段 + `words=N` 字级；`asr_online_max_sentence_silence` 隐藏字段默认 800 可独立 A/B
- **详情**：outbox/tester-1/result.md + logs/20260818.md + CHANGELOG.md
_geometry` 契约 4**：只改函数体，`mod tests` 不动。旧语义 desired > available 时回退 fixed_height=16，导致 tm=26→27 高度 28→16 断崖；新语义 `height = desired.min(available).max(1)`（尽量给、最多给到 available）。调用侧 `log::error!` 不变。真实运行域 tm≈17~19 新旧逐位一致。

---

## 2026-08-19 — tester-1 — TEST-SYNC-058 ✅ 阶段三测试同步（ASR-058 埋点 + [ASR-WORDS] 时间轴，qwen_inference.rs +165 纯新增）

- **来源**：主控派单（基线 HEAD `710cec9`，工作区 clean）。本批改动：建连从「VAD 命中后」提前到「录音开始即建连」（首字提速 143ms）+ AsrSummary/[ASR-SUMMARY]（15 退出点，outcome 三态，未取到填 -1）+ [ASR-WORDS] 词时间轴（仅 debug）
- **改动**：仅 `src/transcription/qwen_inference.rs` 测试模块 +165 纯新增，6 条用例
- **契约 0（关键判据）**：`asr_058_summary_defaults_are_minus_one_not_zero` —— AsrSummary 默认值必须 -1 不是 0（`assert!(!line.contains("=0"))` 钉死假数据）
- **契约 1** outcome 三态齐全 / **契约 2** 行首 [ASR-SUMMARY] + 12 字段顺序严格递增（Gavin 靠 grep+列切分，顺序变了要红）/ **契约 3** 两族 model 串原样透传不截断 / **契约 4** 行首锚点
- **任务②** `asr_058_asr_words_timeline_roundtrips_to_tuples`：[ASR-WORDS] `text[begin-end]` 空格连接格式能还原成 (begin,end,text) 序列（与生产 :1371 同构，格式被改即数据源丢失）
- **不可测项如实**：🔴 本批最有价值改动（建连提前）无纯函数可测，正确性只能靠端测日志（connect_ms/first_text_ms），未写假护栏
- **消融推演**：A 默认值改 0→用例1 红；B 字段顺序调换→用例3 红；C 去行首锚点→用例5+3 红
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / src-tauri check 0 error / git diff --numstat 165/0 纯增量零删除
- **红线合规**：版本号 0.8.1 未动 / 未 commit / 未跑 test/build / 禁 git reset/checkout/stash/clean 全程未用
- **详情**：outbox/tester-1/result.md + logs/20260818.md + CHANGELOG.md
Binary file (standard input) matches

## 2026-08-30 — coder-1 — VERSION-059 ✅ 版本号升级 v0.8.1 → v0.9.0（零功能改动，待主控验收）

- **来源**：Gavin 2026-08-30 明确指示「升级版本到 v0.9.0」，属「版本号禁止擅改」规则例外放行
- **改动**：3 处手改 + 2 处 cargo 自动写回
  - `Cargo.toml:3` `version = "0.8.1"` → `"0.9.0"`
  - `src-tauri/Cargo.toml:3` `version = "0.8.1"` → `"0.9.0"`
  - `src-tauri/tauri.conf.json:9` `"version": "0.8.1",` → `"0.9.0",`
  - `Cargo.lock:5893` + `src-tauri/Cargo.lock:934` 由 `cargo check` 自动同步
- **方案评估**：同意主控方案。全仓 grep `0.8.1`（排除 target/node_modules/Publish/Backup/.venv）仅命中主控列的 3 处，无第 4 处产品版本号漏列。macOS `scripts/Info.plist:20` 的 `0.7.3` 由 `build-macos.sh:92-147` PlistBuddy 动态覆盖，非手改项（已记 docs/MACOS-HANDOFF.md §VERSION-059）。`ui/package.json:4` 的 `0.1.0` 按红线不动。
- **验证**：cargo fmt clean / cargo check --all-targets 0 error（99 既有 warnings）/ cargo check src-tauri --all-targets 0 error（13/11 既有 warnings）/ git diff -w 五文件仅版本号行变更 / 旧版本号 0.8.1 grep 0 命中 / 新版本号 0.9.0 恰好 5 处
- **红线合规**：只改版本号未碰功能代码 / 未跑 test/build（阶段一）/ 未 commit/push / 禁用 git 破坏性命令全程未用 / 未动 ui/package.json / UTF-8 编写（bash heredoc + Edit 工具）
- **跨平台**：版本号平台中立，macOS 侧无需同步改动，详见 docs/MACOS-HANDOFF.md §VERSION-059
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md（新建 v0.9.0 段落）

## 2026-08-30 — coder-1 — ASR-067/070 取证 ✅ v0.9.0 两条 P0 根因取证（零生产改动，待主控验收）

- **来源**：主控派单（基线 HEAD `9c9ff73`，v0.9.0）。两条 P0：ASR-067 停顿 1 秒后麦克风不接受输入 / ASR-070 松键后尾部文字丢失
- **ASR-067 结论**：主控 800ms 假设大概率不成立。官方文档确认 sentence_silence 断句不断连（新句 sentence_begin 自动开始），客户端 on_result 状态机（qwen_inference.rs:472-492）正确处理新句，主循环 sentence_end=true 后无 break/return。真根因需 Gavin -debug 实测，三方向：A 服务端静默期 / B channel 积压 / C heartbeat 未过滤。🔴 需确认 Gavin 端测用哪个模型（Publish/config.toml 是 performance 本地模型，若本地则 800ms 假设完全不适用）
- **ASR-070 结论**：根因锁定 `final_text()`（qwen_inference.rs:520-522）只返回 confirmed_sentences 不含 current_sentence。松键时若最后一段未收到 sentence_end=true 则尾部丢。候选 c（揭示进度截断）排除——最终提交取 final_text 不取 displayed_chars（reveal 只管动画）。修复点落 qwen_inference.rs，归 coder-1 与 coder-2 零冲突。建议修法：final_text 改返回 display_text
- **文件域**：只读 qwen_inference.rs / main.rs / audio/mod.rs + 官方文档 + research；**未写入 main.rs**（coder-2 独占）
- **红线合规**：零生产改动 / 未写入 main.rs / 未跑 test/build / 未 commit/push / 禁用 git 破坏性命令 / Publish 运行时数据只读 / 版本号 0.9.0 未动 / UTF-8
- **需主控确认**：① Gavin 端测用哪个 ASR 模型 ② ASR-070 修法 A 是否可立项 ③ ASR-067 是否等 -debug 日志再定方案
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md

---

## 2026-08-19 — coder-1 — ASR-058 ✅ 首字提速 143ms + A/B 对比埋点（`qwen_inference.rs`，提交 `710cec9`）

> 🔴 **本条为 2026-08-30 主控补录**：任务完成时 handoffs 未建条目，属 `[DOC-STATE-DRIFT-001]`。
> 内容以提交正文为准，非记忆。

- **来源**：RESEARCH-ASR-057（coder-1 与主控**双路独立研究**，结论收敛后实施）
- **① 热键按下即建连**（收益 108ms，6 次实测 108-117ms 稳定）：原为串行「等 VAD 命中 → 才 DNS/TCP/TLS/WS」，改为录音一开始就建连，建连期间 `chunk_rx` 积攒音频不丢
- **四条硬约束全遵守**：命中前零音频发送（run-task 不含音频可先发）／未说话就松手时连接优雅关闭／2s 安全网保留／🔴 **只在本次录音内提前建连，不跨录音复用**（040-C 雷区：空闲连接被服务端静默杀掉）
- **② task-started 往返 35ms 移出关键路径**：建连提前后在 VAD 命中前即完成，35ms 自然被吸收。coder-1 **如实说明这是「被吸收」而非「并行编码」，未夸大实现**
- **③ `[ASR-SUMMARY]` A/B 对比埋点**（Gavin 要求）：每次录音一行 12 字段汇总
- 🔴 **口径红线写死在注释**：`first_text_ms` 起点是「首个音频字节发出」而非热键按下 —— 否则用户的反应时间会被算到服务端头上，两个模型无法公平比较

## 2026-08-19 — tester-1 — TEST-EXEC-058 ✅ 阶段四过闸（1119/0/11，消融三条全实证，提交 `6009aa3`）

> 🔴 **本条为 2026-08-30 主控补录**，同上。

- **四步回归**：cargo test **1119/0/11**（基线 1113，+6 精确命中 TEST-SYNC-058 新增契约，零残差）／Vitest **84/84** 持平／src-tauri **76/0** 持平／pytest 按书 **SKIP**（`Publish/` 当时为 BUILD-022，早于本批）
- **消融三条全有判别力**：A（`AsrSummary` 默认值 -1 改回 0）→ **1 红**（最关键护栏：填 0 会把「没测到」读成「零延迟」）；B（调换字段顺序）→ **1 红**；C（去行首 `[ASR-SUMMARY]` 锚点）→ **2 红**
- **还原自证**：三次消融均编辑器还原（禁 git reset/checkout/stash/clean），`git diff -w` 0 字节，复跑逐数一致
- **红条三分类**：③ 真回归 **0**，过闸无阻塞项
- 🔴 **如实交代的覆盖缺口（未粉饰，主控认可）**：建连提前、15 个退出点汇总行接线 —— **无纯函数可测，阶段四也测不到**，只能靠 Gavin 端测 `[ASR-SUMMARY]` 日志验证


## 2026-08-30 — coder-1 — ASR-070-FIX ✅ 修复松键后尾部文字丢失（qwen_inference.rs，待主控验收）

- **来源**：主控派单（基线 HEAD `9c9ff73`）。ASR-070 取证已验收，根因 `final_text()` 丢弃 current_sentence 确认
- **改动**：仅 `src/transcription/qwen_inference.rs` +27/-4
  - `final_text()`（:520-528）：`confirmed_sentences.join("")` → `display_text()`（confirmed+current），注释改写说明根因
  - 断言 :1793（:1795-1797）：`"第一句"` → `"第一句第二"`，加注释说明契约变更
  - 新增护栏（:1816-1830）：`asr_070_final_text_includes_current_sentence`，消融改回旧实现必红
- **重复计数核查**：on_result（:472-491）end=true 时 push confirmed 同时 clear current（:480/:482 原子），其余分支覆盖非追加 → 无重复风险
- **5 条断言推演**：逐条手工推演，仅 :1793 需改（confirmed+current 场景），其余 4 条 current 为空旧新一致
- **fallback 保留**：:1404-1413 未动，:1411-1413 改后成死代码但 bail 路径 :1409 仍必要
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri 0 error / git diff -w 仅 qwen_inference.rs +27/-4
- **红线合规**：只改 qwen_inference.rs 未写入 main.rs / 未跑 test/build / 未 commit/push / 禁 git 破坏性命令 / 版本号 0.9.0 未动 / 未碰 target/release/config.toml / UTF-8
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md + docs/MACOS-HANDOFF.md §ASR-070-FIX

## 2026-08-30 — coder-1 — ITN-071/FMT-072 ✅ 三五成群补词+护栏 / 一点半点挂起 / FMT-072取证挂起 / 死代码化简（待主控验收）

- **来源**：主控派单（基线 HEAD `958cadb`）。ITN-071 两条 ITN 错误 + FMT-072 有序列举失败 + 顺带清理 qwen_inference 死代码
- **① 三五成群（已修复）**：不在任何保护集，补进 `itn-rules.toml [protect.idioms]`（:163），三副本同步 `be2ef5c7...`。护栏 2 条，消融删词变红
- **② 一点半点（挂起）**：代码层面 unit_collision_map 一桶 + check_protection 第五步应匹配保护，主控独立复核属实。卡点不在代码而在不知道 Gavin 实际方向，等 Gavin 用例
- **③ FMT-072（取证挂起）**：时间线排查三结论存档，静态分析未找到碰坏有序路径的改动，需 API 验证，主控已向 Gavin 索要用例
- **④ 死代码化简**：qwen_inference.rs:1404-1413 不可达 fallback 化简，保留 bail，+4/-6 行为不变
- **改动**：itn-rules.toml +1、src/itn.rs +17、qwen_inference.rs +4/-6。src/llm/mod.rs 零改动
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri 0 error
- **红线合规**：未写入 main.rs/ui / 未跑 test/build / FMT-072 未自行烧 API / 未 commit/push / 禁 git 破坏性命令 / 版本号 0.9.0 未动 / 未碰 config.toml 等运行时数据 / UTF-8
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md + docs/MACOS-HANDOFF.md
