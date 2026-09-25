# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-25 归档：2026-09-24 共 22 条已移入 `handoffs-archive.md`（本文件曾达 303 行，超 200 行上限）。
> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-25 — coder-1 — TEST-SYNC-411 ✅ 交付（阶段三·非作者护栏 5 条；生产零改动）

- **范围**：只在 `src/main.rs` `#[cfg(test)] mod testsync411_tests`（5 条，`19309` 起）；未碰 `speaker.rs`/`mod.rs`（coder-2 的 412 护栏）、未碰生产区。
- **契约 1~5**：性质（6 分布拼接==整段/边界/多字节）｜常规滑窗组合（真实 `plan_windows`：live `[(0,1),(1,2)]`+pending 2、tail `[(2,3)]`）｜切分+末尾窗 accept（410 基准两部分非空、真实精解 accept）｜切分+末尾窗被拒（只替 pending、前片不动）｜**端到端 BUILD-409 16:44Z 真实三窗无重复**（新 == 三窗精解各一次；旧复现整段重复）。
- **真实数据**：`debug.log:1212+`（整段 86 字流式按 80 截断 = `STREAM80`；ACC0/1/2 = 41/49/28 字；样本 10/10/3.85s）。期望值经沙箱复刻 406（win0 0.882 / 整段 0.412）与 `plan_windows` 验算。
- **白名单**：`rustfmt --config skip_children=true src/main.rs` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87=基线；**未跑 cargo test**。
- **观察（只报告不修）**：相邻窗不共享切片 ⇒ `OrderedReflow` 不去重 ⇒ 窗间同人短语重复仍在（组窗粒度、非 411；取向宁重复不丢字）。
- 红线：未碰生产代码 / 未 commit / 零凭证。

## 2026-09-25 — coder-1 — SPEAKER-MERGE-SHORT-412 ✅ 阶段一交付（待主控验收）

- **改动**（`src/transcription/speaker.rs`；`mod.rs` 仅 +1 源码锚点测试）：新增纯函数 `merge_speech_units` + `MERGE_GAP_SECS=0.5s`（**暂定，待端测校准**）+ `SpeechUnit`（语音时长不含间隔；退化零长区间保留为零长成员保一一对应）；`filter_ranges_by_voiceprint` 改**按单元**判定（单元 ≥2s 拼接全部成员语音算一次声纹、判定作用全成员；<2s `KeepShort`）；注册/漂移按单元 offer（**起点** ≥ `new_slice_from`，跨界单元保守不 offer）。
- **7 条影响面逐条结论+测试**：① `kept` 仍原各段（剪静音喂 `f.kept`，388 不变）② kept/dropped 口径同改前 ③ 解码后重解用原 ranges+整窗音频 ④ 漂移上限仍 0.25（8s/100s 单元同位移）⑤ 跨界单元不 offer ⑥ 无就绪档逐位一致 ⑦ 缺模型合并未执行。
- **测试**：`ts412_*` 9 + `mod.rs` 锚点 1 = **10P/1I**；`#[ignore]` 真模型单元 vs 2.4s 长段 **0.915**。同步更新 408B `ts408b_new_slice_from_contract_anchor` 锚点（**契约不变**）。
- **验证**：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` **1701P/0F/53I**。证据 `collab/evidence/412/`。
- **R1 返修（数据校准，主控退回）**：`MERGE_GAP_SECS` **0.5→0.8s**（BUILD-409 同窗相邻间隔一半以上 0.5~0.8s，原阈值下窗#4 一段拼不上）；规则改 **只为凑够 2s 才拼**（单元 ≥2s 即封口）+ **尾段并入前单元** + **≥2s 单段独立**；安全性（拼错只令混合单元得分居中 ⇒ 不误删）入注释；补 4 条测试（窗#4 真实区间/凑 2s 封口/长段独立/尾段并入）⇒ `ts412` **14P/1I**；7 条影响面复核结论不变。
- 红线：未碰 `main.rs` / 未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-1 — FIX-TAIL-GUARD-410 ✅ 阶段一交付（待主控验收，只改 main.rs）

- **① 比对基准只取前片后缀**：纯函数 `tail_streaming_baseline`（前片流式按样本占比取末尾相应**字符数**、char 切 + pending 流式）替代「整前片 + pending」⇒ 不再误拒正确精解。
- **② 末尾窗被拒/空只兜底 pending**：`TailPending` + `window_tail_pending`；harvest 用 span `(p,p+1)` + pending 文本 ⇒ 前片不动。406 逻辑未改。
- **③ 兜底先打标点**：进程级常驻标点服务线程（首次兜底才 spawn、引擎线程内加载、全进程复用）；500ms 超时、失败/超时原样 + warn；**Gavin 追加**：异常退出 ⇒ 重启重试最多 2 次、全失败 ⇒ 无标点 + 30s 冷却、超时不重启。
- **测试**：`f410_*` **8 条**（真实数据误拒→accept、前片不变+pending、char 切 emoji、关闭/已含标点、异常退出后重启成功、两次失败走无标点、超时不重启、冷却窗口）+ `#[ignore]` 真模型（`今天天气不错我们出去玩吧`→`今天天气不错，我们出去玩吧。`）。
- **验证**：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` **1674P/0F/52I**。证据 `collab/evidence/410/`。
- 红线：只改 `src/main.rs` / 未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — tester-1 — TEST-EXEC-412（411+412 合包）✅ 回归通过 · 🔴 出包暂停

- **交付源码**：HEAD `94ab214`（`35a1e8e` 411+412+阶段三 + `94ab214` 返修），版本 0.9.3。单：`FIX-SPLIT-SLICE-STREAMING-411`（多片切分时流式文本按样本占比分配）+ `SPEAKER-MERGE-SHORT-412`（声纹判定前短段拼 ≥2s 单元）+ 阶段三 15 条。
- **回归**：首轮 bin **1715P/1F/53I**（1 条红 `ts412b_unit_scope_three_members` 系手算常量 2.8 写错、应为 0.8+0.9+0.9=2.6，阶段三测试自身错）→ 返修（期望改由区间数据推导）后复跑 bin **1716P/0F/53I**、总 **1804P/0F/55I**；`src-tauri` **92P/0F/0I**；`fmt --check` EXIT 0；Vitest SKIP。
- **NEW/GONE**（对 BUILD-410 bin 1680P/52I）：NEW **+36P**（`ts411_*` 7 + `ts411guard_*` 5 + `ts412_merge_confined_to_predecode_trim_anchor` + `ts412_*` 14 + `ts412b_*` 10）**+1I**（`ts412_real_merge_short_segments`）；**GONE 0**（`main.rs::slice_streaming_text` 为同名重构的生产 fn，非用例）。
- **构建**：Step1 残 0 → Step2 SKIP（ui/src-tauri 无 diff）→ Step3 主程序 **2m36s**（**CT2 编译计数 0**）。产物仅在 `target/release/`（main 01:54:56 / crash 01:53:17）。
- 🔴 **出包暂停（主控指令：Gavin「别总频繁重复出包」）**：**未执行 Step4 同步 Publish、未冒烟/未强杀输入法**；`Publish/` 仍为 BUILD-410（main `01:05:59`/crash `01:04:01`/ui `13:45:21`/ct2 `efa16d81…`），config `da2be5da…` 未变。待主控统一号令后补 Step4 + 冒烟 + 九项。
- **只读专项**：A CT2 不重编（sha `efa16d81…` 与上包逐位相同）；B 声纹模型 `aa3cfc16…` 仍在包内；D 用户数据只列不删（`voiceprint.bin` 不存在）；C 冒烟 N/A。三特殊点只读核验全 PASS。
- 未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未动 Publish / 零凭证。证据 `collab/evidence/412/`。

## 2026-09-25 — tester-1 — TEST-EXEC + BUILD-410 ✅ 出包（末尾窗 × 406 配合修正；九项 + 专项 A~D + 三特殊点全 PASS）

- **交付源码**：HEAD `1814baf`（`e4cae44` FIX-TAIL-GUARD-410 + `1814baf` TEST-SYNC-410），工作区 clean，版本 0.9.3。单：末尾窗比对基准只取前片对应末尾 + pending、被拒只兜底 pending 前片不动、流式兜底经进程级标点服务线程打标点（懒启动/异常重启≤2/冷却 30s/超时不重启）+ 6 条非作者护栏。
- **回归**：root bin **1680P/0F/52I**、总 **1768P/0F/54I**（EXIT 0）；`src-tauri` **92P/0F/0I**；`cargo fmt --check` **EXIT 0**；Vitest **SKIP**（`ui/` 无 diff）。**NEW/GONE**（对 BUILD-409 bin 1666P/51I）：**+14P**（`f410_*` 8 + `ts410_*` 6）**+1I**（`f410_real_punctuate`）；**GONE 0**⇒ 1680P/52I。预期逐位命中。
- **构建**：Step1 残 0 → Step2 **SKIP**（`ui/`+`src-tauri/` 无 diff）→ Step3 #1 **2m57s**（**CT2 编译计数 0**）/#2 **0.94s** 全增量（0 Compiling）→ Step4 `scripts/init-publish.ps1`（Step5 三 exe 全 OK）+ 声纹模型 + 四张 rules toml。产物 main `a3365d7925e7…`(15,099,904B/01:05:59) / ui `ee7f2571f1d4…`(未变) / crash `b675b6f481a4…`(24,879,104B/01:04:01) / ct2 `efa16d8110bd…`(未重编)；两副本全等。
- **九项逐项 PASS**：①时间戳 01:03–01:06 ②两副本 sha 相等（main/crash 异于上包；ct2/ui 未变属预期）③**0.9.3** ④冒烟 PID **3364 Responding=True**/无新 crash.json/无 panic/`WM_CLOSE` **380ms**/残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/9（+`ctranslate2-sys` 18 基线）⑦正 `[LocalRT-DBG-410]`=4（反探针 N/A：本批无删除字面量）⑧scene/itn 三副本全等 ⑨**VC++ 五运行库 sha 与 Redist `14.44.35112` 全等**。
- **专项 A~D**：A **CT2 不重编**（#1=0、#2=0.94s，sha `efa16d81…` 与上包逐位相同）；B 声纹模型 `aa3cfc16963a…` 28,281,164B 仍在包内；C 冒烟（未录音：DBG-406/407/410 新窗=0，标点服务未 spawn 属正常，无 shadow finalize/speaker warn/panic）；D 用户数据**只列不删**（`voiceprint.bin` 不存在；config.toml 2026-09-20 / wordbook.sqlite 2026-09-10）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…`；③ `Publish/models/` 1.7B 七文件与源全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未删用户数据 / 零凭证。证据 `collab/evidence/410/`。

## 2026-09-25 — tester-1 — TEST-EXEC + BUILD-409 ✅ 出包（405~408B 合包；九项 + 专项 A~E + 三特殊点全 PASS）

- **交付源码**：HEAD `d215a95`（`79240dd` + 返修 `05a6d7d`/`d215a95`），工作区 clean，版本 0.9.3。单：405（移除影子收尾 DEC-086 + 显示缓存 + CT2 不重编 + 脚本 exe 名）/406（精解与同窗流式比对守卫）/407（1900ms 末尾组窗）/408A+B（声纹 CAM++ 接入路B）+ 阶段三护栏。
- **回归**：首轮 bin **1664P/2F/51I**，2 条红均为阶段三测试自身 bug（`ts407_tail_only_when_pending_source_guard` 源码护栏窗口 2600 字符过小；`ts408b_partition_ranges_conservative` 误按 32kHz 算时长），非生产缺陷；主控核实后派作者返修。复跑 bin **1666P/0F/51I**，总 **1754P/0F/53I**（EXIT 0）；`src-tauri` **92P/0F/0I**；`cargo fmt --check` **EXIT 0**；Vitest **SKIP**（`ui/` 无 diff）。
- **NEW/GONE**：对 **408B `73537a2`**（bin 1641P/51I）**+25P/0G** ⇒ 1666P/51I；对**上包 BUILD-399 `49bae07`** NEW **80** 测试、GONE **2**（`endpoint_confirm_text_takes_longest_of_two`/`_len_not_below_main`）；`vad.rs::find_gap_cut` 为同名重构、不计 GONE。
- **构建**：Step1 残 0 → Step2 **SKIP**（`ui/`+`src-tauri/` 无 diff）→ Step3 #1 **6m06s**（CT2 重编一次：405 改了 build.rs，属预期）/#2 **2m18s**（voice-ime 返修后重编；**CT2 编译计数 0、sha 不变**）/#3 **0.90s** 全增量（0 Compiling）→ Step4 `scripts/init-publish.ps1`（Step5 三 exe 全 OK）+ 声纹模型 + 四张 rules toml。产物 main `b37aa194d76f…`(15,031,296B/00:23:06) / ui `ee7f2571f1d4…`(未改) / crash `a555f11c4141…`(24,879,104B/00:21:35) / ct2 `efa16d8110bd…`(27,405,312B)；两副本全等。
- **九项逐项 PASS**：①时间戳 00:12–00:23 ②两副本 sha 相等（main/crash/ct2 异于上包；ui 未改属预期）③**0.9.3** ④冒烟 PID **27040 Responding=True**/无新 crash.json/无 panic/`WM_CLOSE` **340ms**/残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/9（+`ctranslate2-sys` 18 基线）⑦正 `[LocalRT-DBG-408]`=3、反 `shadow finalize`=0 ⑧scene/itn/homophone/wordbook 四表三副本全等 ⑨**VC++ 五运行库 sha 与 Redist `14.44.35112` 全等**。
- **专项 A~E**：A **CT2 第二次起不重编**（#2=0、#3=0.90s，sha `efa16d81…` 稳定）；B 声纹模型进包 `aa3cfc16963a…` 28,281,164B（与源等）；C `init-publish.ps1` Step5 自动复制 `feiyin-ime.exe`/`feiyin-ime-ui.exe`/`crash-reporter.exe`（**无 SKIP、无手工 cp**）；D 冒烟：未录音（N/A 三证 + exe `[LocalRT-DBG-408]`=3），新窗 `shadow finalize`=0、无 speaker warn；E 用户数据**只列不删**（`Publish/voiceprint.bin` 不存在；config.toml 2026-09-20 / wordbook.sqlite 2026-09-10）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…`；③ `Publish/models/` 1.7B 七文件与源全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未删 Publish 用户数据 / 零凭证。证据 `collab/evidence/409/`。

## 2026-09-25 — coder-2 — FIX-SPLIT-SLICE-STREAMING-411 ✅ 交付（多片切分流式按占比分配）

- **缺陷**（BUILD-409 16:44Z）：23.85s 切 3 片 ⇒ `slice_streaming_text` 旧「k==0 整段 / 其余空」使窗#0 基准=整段 86 字（406 retention 0.38 误拒）⇒ 整段兜底回灌 ⇒ 最终文本重复；后片窗基准缺字；末尾窗 pending 兜底为空（410 潜在丢字）。
- **改动（仅 `src/main.rs`）**：`slice_streaming_text(k, seg, &slice_samples)` 按样本占比分配（`round(total_chars×cum/total)` 单一定义 ⇒ 无缝、求和==总字数；char 切）；调用传 `&new_lens`；Debug 埋点 `[LocalRT-DBG-411]`。
- **影响面**：常规窗 concat 基准逐片正确；末尾窗 `tail_streaming_baseline` 前后片非空；🔴 被拒 pending 兜底非空（旧为空⇒丢字）；344-G `hole_fill_decision` 生产无调用点 + `partial_win_committed` 不读流式 ⇒ 不受影响。
- **测试**：新增 `fix411_tests` 7 条（含真实数据、切分+末尾窗被拒兜底非空、源码护栏）；更新 386 用例计数（17⇒9+8）。**验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1697P/52I**）。
- **未验证**：实机端测（多片切分不再误拒/重复、`[LocalRT-DBG-411]` 埋点）交 tester-1/Gavin。
- **红线**：未改 406/407/410/声纹；未加 config/env；未 commit / 未 push / 未 build release / 零凭证。

## 2026-09-25 — coder-2 — TEST-SYNC-412 ✅ 交付（阶段三 · 短段拼接非作者护栏 6 条）

- **被测**：`SPEAKER-MERGE-SHORT-412`（相邻短段按 <0.8s 间隔拼「连续说话单元」再判；作者 coder-1，HEAD `25d097b`）。
- **范围**：`src/transcription/speaker.rs` **仅**追加 `mod testsync412_guard_tests`（+173/0，6 条）；生产代码零改动；未碰 main.rs/mod.rs。
- **10 条**（首轮 6 + 主控裁量 A 后补 4）：阈值精确边界（恰 0.8s 不拼）；随机性质 500 组（成员一一对应 / `speech_samples`==成员和 / 单元内间隔 <0.8s / 退化零长段）；BUILD-409 间隔集不 panic；三成员单元作用域（Drop 全剔 / Keep 原段）；无就绪 KeepNotReady + 缺模型生产早退源码护栏；空成员合成单元边界；**契约2 前向封口**；**③ 收尾例外**（`[2.1s,0.3gap,0.5s]`⇒1；`[2.1s,0.3gap,0.5s,0.3gap,2.5s]`⇒3）；**契约6 注册门**（起点边界 + 跨界不 offer + 源码护栏）；**契约3 窗#4 精确**（恰 2 单元均 ≥2s）。
- **契约2 措辞（主控裁量 A）**：③ 收尾「<2s 尾段并入前一单元」为 412 R1 有意例外（非缺陷）；契约2 限定「前向累积阶段」，据此分别钉住。
- **验证（白名单）**：`rustfmt --config skip_children=true src/transcription/speaker.rs` CLEAN；`cargo check --all-targets` **EXIT 0**、0 error、warnings **92/87** = 基线；sandbox 复刻 `merge_speech_units` 500 组性质 bad=0 + 边界 + BUILD-409 ✅。🔴 **未跑 `cargo test`**（阶段三禁止，首跑由 tester-1）。
- **疑似生产缺陷：未发现**；**未改生产代码 / 未 commit / 未 push / 零凭证**。
- **返修（tester-1 回归红，测试自身错误）**：`ts412b_unit_scope_three_members` 三段 0.8+0.9+0.9=2.6s，两处期望手算成 2.8 ⇒ 红；改为由数据导出 `total=Σ(e-s)/SR` 再比。sandbox 未覆盖之因：复刻只移植 `merge_speech_units`、未移植 `partition_ranges` 秒数累计，且阶段三禁 `cargo test`。复跑：`rustfmt --skip_children` CLEAN；`cargo check --all-targets` 0 error、warnings 92/87=基线。
