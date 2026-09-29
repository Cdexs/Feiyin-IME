
### 2026-09-25 · TEST-SYNC-430-434 交付（阶段三·非作者护栏，只写测试）

- `testsync434_tests`（local_stream.rs，10 用例）+ `testsync430_tests`（main.rs，7 用例），独立推导、与作者用例不同输入/无重名。生产零改动。fmt EXIT0 / check 0 error、warnings 91/87=基线（未跑 test）。

### 2026-09-25 · SEAM-PINYIN-ALIGN-435 · R1 交付（切点/接续点同一回溯路径，恢复零重复）

- 首轮放宽「允许 ≤1 字边界重复」被退回。根因：loose k 与真实对应错位 1 字（22:36 真正起点是前一窗「有」↔后一窗「由」）。修复：`semiglobal_dp` 一次回溯返回 `(cont,best,first)`；取 B 用加权路径求前一窗切点（切在「有」之前），取 A 用字形接续（加权会把末字对到近音字、留下真同字 ⇒ 重复）。
- `fix433_case5` 恢复精确全文断言；`ts431g` 320 组恢复 `got == P+S`。5 例最终全文见 result（R1 列全精确）。fmt/check 全绿，warnings 91/87=基线，全量 test 0 failed（bin 1872P/0F/65I）。
- ✅ R1 追加：真实 R 重算 5 接缝（`fix435_real_stream_arbitration`）——前 4 例与朗读原文一致；22:36 判 A 而 ref 为 B（预览同错，GIGO 上限，如实报告）。bin 1873P/0F/65I。

### 2026-09-25 · SEAM-PINYIN-ALIGN-435 交付（重叠区按读音加权对齐，mod.rs + pinyin 依赖）

- 新增依赖 pinyin v0.11.0（MIT，多音字）；`char_sub_cost`（同音 0.2 / 近音 0.6 / 其余 1.0；近音含 n/l、zh/z、ch/c、sh/s、an/ang、en/eng、in/ing 归一）+ 加权编辑距离；`align_try_k_ratio`/`semiglobal_align`/`estimate_overlap` 改用加权，插删仍 1.0。
- 433 裁判保持逐字、dedupe 不动、431 语义不变；阈值未改。命中变化：22:36 forced → **Loose**（k=12, cont=14）。
- `fix435_tests` 6/6 + 既有 416/431/433 同步（变化逐条见 result）；fmt EXIT0 / check 0 error、warnings 91/87=基线 / 全量 test 0 failed（bin 1871P/0F/65I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · SEAM-ARBITER-STREAMING-433 交付（预览原文裁判接缝 + forced/估算层，主改 mod.rs）

- 重叠区取舍升级为**预览模型原始文本裁判**（A=前一窗/B=后一窗，比 `LCS/max`，更像 R 者胜；平局/R 空⇒A、维持 431）。新层 `AlignLayer::Forced`（有先验 e≥8、半全局有对应且 `best/e≤0.90`）不再 concat；长度比超 [0.70,1.43] 取**较长版**；无先验也**估重叠**取较长版；完全对不上才 concat+warn `[DBG-433] concat fallback`。
- R 必须为流式原文：`window_streaming_texts[seq]` → `push_window_streaming` → `last_stream` → `streaming_overlap_region`；日志 `[DBG-416] seam` 加 `arb/sim_a/sim_b/R`，新增 `[DBG-433] win`（acc/stream 不截断）。
- `fix433_tests` 14/14（含 5 接缝裁判推演表：前三取 A、四/五取 B 且五无整段重复）；416/431 用例同步（errors=3 现全无重复）。fmt EXIT0 / check 0 error、warnings 91/87=基线 / 全量 test 0 failed（bin 1863P/0F/64I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · VOICEPRINT-JUDGE-1P5S-432 交付（判定门槛 1.5s / 注册 offer 2.0s，只改 speaker.rs）

- 拆常数：`MIN_JUDGE_SECS=1.5`（判定 + 412 合并目标 + 语种门）、新增 `MIN_OFFER_SECS=2.0`（注册/漂移 offer）；`DROP_THR=0.45` 不动。offer 两条件抽纯函数 `unit_offer_eligible`，调用处改调之。
- `fix432_tests` 5/5 + 5 处 412/408 用例同步（`ts412_merge_only_to_reach_judge_secs`、window#4 改判 3 单元 2 可判）；fmt EXIT0 / check 0 error、warnings bin 91=基线 / 全量 test 0 failed（bin 1844P/0F/64I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · SEAM-KEEP-PREV-TEXT-431 · R1 交付（主控裁决 B：半全局对齐定接续点，只改 mod.rs）

- 重叠区文字以前一窗为准、只采纳后一窗边界标点；接续点改 `semiglobal_continuation`（前一窗重叠区有效字完整对齐、后一窗 k+4 末端自由，回溯取最后一个有对应的字之后；平局取较大 j 偏替换）。
- 五例全绿（插入不/插入去/替换世时/纯标点/删除 1 字）；`fix431_*` 5/5；fmt/check/全量 test 0 failed（bin 1830P/0F/62I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · LOCALRT-PREVIEW-HIDE-NONUSER-429 交付（声纹全剔段立即从预览移除，只改 main.rs）

- `PreviewReflow` 加 `non_user_hide`；harvest full_drop 窗发回灌 = 已定稿权威文本 + 该段之后的流式尾巴（剔除本段流式，`[DBG-429]`）；`render_authoritative_reflow` hide 分支用 `reflow_preview`（acc 空也成立）。本人逐位不变、编辑态照拦、最终文本不受影响。
- `fix429_tests` 2/2；fmt/check/全量 test 0 failed（bin 1817P/0F/61I）。harvest 平台中立、渲染 cfg(windows)（MACOS-HANDOFF 已记）。Gavin 目视：放他人音频该段精解完成后预览立即消失。未出包。

### 2026-09-25 · LOCALRT-ALWAYS-CONTEXT-427 交付（所有常规窗带前一片后缀，只改 main.rs）

- `short_context_span` 判据改为「紧邻前一片仍在缓冲内即取后缀」（`must_start-1 >= buf_base`），不再受 `gs`/10s 组窗上限影响；首片 / 前片出缓冲无前文。其余沿 413 路径。
- 回放（175022 #1）：改前「母亲。」→ 改后「总会出现，比如说某人在某一事，可能母亲。」；+4s 音频 +866ms。3 条 427 单测；fmt/check/全量 test 0 failed（bin 1806P/0F/60I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · FIX-ACC-EMPTY-RETRY-426 交付（精解报错/空纳入重试 + 重试换条件，只改 mod.rs）

- 首解 Err（`No transcription result`）改按空进 `apply_acc_disposition`（触发一次重解，`[DBG-426]`）；重解走 `decode_accuracy_allow_empty_lang` + `lang_to_sherpa` 指定本窗语种（L 未知不指定）；重解仍 1 次；碎片窗留 424。首解逐位不变、421 full_drop 不变。
- `fix426_tests` 5/5；390 cap 护栏改写（不放松）；425 harness 重放 #10 带 `language=Chinese` 出正确句。fmt/check/全量 test 0 failed（bin 1806P/0F/57I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · LOCALRT-STOP-TAIL-UNIFY-423 交付（松键收尾统一走末尾组窗，只改 main.rs）

- 新增 `emit_tail_window!` 宏统一组装末尾窗（有前片 ⇒ 前片后缀 + pending；首片 ⇒ `tail_window_alone` 单独解），长静默 407 与松键收尾共用；`plan_windows` 删 `is_tail`/规则 4、删 `TAIL_MERGE_MAX_SECS`；松键 `pending_slice.take()` 防重复。
- 旧护栏 `ts413_rule4_*`→`ts423_*`、testsync386/407/411/413 改写；fmt EXIT0 / check 0 error、warnings 91/87≤基线 / 全量 test 0 failed（bin 1801P/0F/55I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · UI-FOCUSLOST-WINDOW-430 交付（回显窗样式：可滚动 + 宽 ×1.7 + 按钮固定底栏，未出包）

- 只改 `src/main.rs` 回显窗区：正文纵向滚动（`WM_MOUSEWHEEL` + 裁剪/偏移/滚动条滑块）、宽 ×1.7 夹紧工作区 + 高 ≤60%工作区（`preview_size`）、正文区与底栏按钮不重叠 + 分隔线。
- macOS `overlay.rs` 同有「窄 + 单行无滚动」⇒ 需 Mac 端同改+验证（HANDOFF 已记）。文案未新增。
- `preview430_tests` 3P；fmt EXIT0 / check 0 error、warnings 91/87≤基线。⚠️ 全量 test 4 红在 mod.rs（coder-1 在途 435），非本单。未出包。

### 2026-09-25 · TAIL-CUT-REAL-PAUSE-434 交付（前片后缀切点只落真停顿，阶段一，未出包）

- `vad.rs` 新增 `real_pause_cut_candidates`（连续 ≥120ms 低能量段中点；旧内核未动）；`local_stream.rs` 新增 `TailCutKind`+`find_tail_cut_ex`（首轮最早真停顿→前扩 2×back→旧 WeakGap/NoGap），`find_tail_cut` 兼容返回。
- 实测：21:16 窗#1 旧 6.090s（静音 20ms=「取」字内）⇒ 新 6.520s（静音 400ms）⇒ 避开「获取」字内。
- `tail_cut_434_tests` 5P；全量 test 0 failed（bin 1863P/64I），381 gap_cut 9P / 407 1P 全绿；fmt EXIT0 / check 0 error、warnings 91/87≤基线。未出包。

### 2026-09-25 · TEST-SYNC-431 交付（重叠区保前一窗文字的非作者护栏 7 条，生产零改动）

- 只加 `src/transcription/mod.rs::testsync431_tests`（7 条）：性质 320 组（插入/替换⇒逐字 P+S）、插入首/中/末、删除首/中/近末、替换末字平局偏替换、标点三态、混合/emoji 不 panic、源码锚点（splice 生产区 1 定义+1 调用）。
- 期望值经 sandbox 逐行复刻算法验算（插入/替换性质 0/320 bad）。
- 白名单：fmt EXIT 0、check 0 error / warnings 91/87 ≤ 基线、新模块零 warning；未跑 `cargo test`。
- 🔎 观察（只报告）：删除使重叠短于 k 时可能吃掉后一窗 S 首字（含删除随机 400 组 13 组；定点删除用例不受影响）。未出包。

### 2026-09-25 · DIAG-SHORT-VOICEPRINT-432 交付（1~2s 短句声纹区分，只读诊断）

- `speaker.rs::diag432_tests`（`#[ignore]` 只读）：参考 `voiceprint.bin`；本人/他人切窗 → CAM++ 余弦。
- 语音段**完全分离**：本人 min **0.418**（1~2s 桶 0.631~0.741）、他人 max **0.249**；`DROP_THR=0.45` 套 1~2s ⇒ 他人 15/15 删、本人 0/75 误删。
- 建议（不实施）：`MIN_JUDGE_SECS` 2.0→1.0s、0.45（<1s 仍不判）；🔴 先端测（他人样本单一）。产物 `collab/evidence/432/`。未改生产。
- **R1（多说话人 AISHELL-1 6 人）**：1.0~1.5s **交叠**（S0003 0.349 vs 0.408）⇒ 不做；**1.5~1.8s 可分**（本人 worst-min 0.542 > 他人 worst-max 0.511）。Pooled FRR/剔 0.40=0.56%/97.1%、0.45=1.11%/99.7%。**推荐 `MIN_JUDGE_SECS=1.5s`+`DROP_THR=0.45`（6/6 误删 0%）**；1.60s 旁语句定位后 score 0.086~0.157 ⇒ 必删。临时语料 497MB 已删。

### 2026-09-25 · TEST-SYNC-423-427-429 交付（阶段三·非作者护栏 8 条，生产零改动）

- 只加 `src/main.rs::testsync423_427_429_tests`（8 条）：423 末尾窗统一宏（单宏两调用）/ `tail_window_alone` 边界与决策 / `plan_windows` 去 is_tail·规则4 + `take()`；427 `short_context_span` 独立下标 + `context_tail_pending` 锚点；429 `reflow_preview` 三态 + `compose` 短路反证 + hide 源码锚点。
- 白名单：fmt EXIT 0、check 0 error / warnings 91/87 ≤ 基线、新模块零 warning；未跑 `cargo test`。未出包。

### 2026-09-25 · TEST-SYNC-426 交付（空/Err 重试的非作者护栏 9 条，生产零改动）

- 只加 `src/transcription/mod.rs::testsync426_tests`（9 条）：首解成功不重解 / 空·Err 重解恰 1 次（Ok 采用、Err·空 invalid 返空）/ `lang_to_sherpa` 四语+未知 / 421 全剔早退序 / Echo·Tag·Collapse 触发重解 + 带语种锚点。
- 🔴 报告：`lang_to_sherpa` 对 `ZH`/`zh-CN`/`en-US`/` ch` 均 None（静默不指定），生产现为小写短码不触发，只报告不改。
- 白名单：fmt EXIT 0、check 0 error / warnings 91/87 ≤ 基线；未跑 `cargo test`。未出包。

### 2026-09-25 · DIAG-FRAG-AND-PREVIEW-428 交付（碎片窗 + 浮层预览他人语音，只读诊断）

- **A**：`trim_to_speech` 对 #5 的 0.35s 段间停顿**原样保留**；两变体（只剪首尾 / thr .3/.5/.8）与现状**逐字节相同**（#5 恒 5.040s）⇒ 改阈值零效果，建议不动 trim；#5 属 VAD 切碎+模型碎窗不稳，走 426。
- **B**：流式链路无声纹、`on_segment` 有精确字符区间 ⇒ **建议 B1**（离线全剔⇒清预览段，零实时开销）；不推荐 B2（实时 CAM++）。B1 只动 main.rs。
- 产物 `collab/evidence/428/report.md` + `diag428_tests`（2P/0F）。未改生产。未出包。

### 2026-09-25 · POC-QWEN3-PREFIX-424 交付（已识别文本作输出前缀续写，纯 PoC/研究，生产零改动）

- 补丁 sherpa Qwen3 impl（per-stream `prefix`，置于 `language X<asr_text>` 后）+ 独立 C++ runner；四组对比（现状/甲后缀比例前缀/乙前片全文前缀/丙全部历史前缀）。
- **结论：不建议上生产**——乙/丙 让模型把前缀当正文续写/复读 ⇒ 大段重复+丢窗（CER 0.67~11×）；甲与现状相当、略优、无回显，增益在噪声内。与 DEC-083 同源。
- 专项①声纹未实测（缺 07:03:50 wav）/③13:17:53 wav 缺；②换语言部分。口径：runner 组窗为近似（非生产复刻）。
- 产物 `collab/evidence/424/report.md`+patch+tsv+log；编译目录 2.6 GB 保留。未改生产。未出包。

### 2026-09-25 · TEST-SYNC-421 交付（声纹兜底的非作者护栏 10 条，生产零改动）

- 只加 `src/main.rs::testsync421_tests`（10 条）：本人段窗首/尾/中全留、极短 KeepShort 段不吞、他人全删、全剔空且不兜底、dropped=0 逐位不变、模型空仍兜底、先剥后打、源码锚点。
- 独立字表/场景（不复用作者 fix421_tests）；期望值经 sandbox 复刻 `kept_streaming_text` 逐条复算一致。
- 白名单：fmt EXIT 0、check 0 error / warnings 91/87 ≤ 基线；未跑 `cargo test`。未发现生产缺陷。未出包。

### 2026-09-25 · FIX-VOICEPRINT-FALLBACK-421 · R1 交付（估算路径防吞本人字）

- 估算路径改为把流式字**只铺在语音时间轴**（kept∪dropped 裁剪到本段）并按「**落在 dropped 才删**」，其余一律保留；时间戳路径同改；`VoiceprintFilter`/`AccDropStats` 加 `dropped_ranges`。
- `fix421_tests` 11P+1I（新增本人 kept/他人 dropped/中间静音用例）；fmt/check/全量 test 0 failed（bin 1789P/0F/55I）。未出包。

### 2026-09-25 · FIX-VOICEPRINT-FALLBACK-421 交付（声纹剔除后兜底只取保留部分 + 全剔不兜底 + 补标点）

- 全剔（dropped>0 ∧ kept≤0）⇒ 该窗空、不走流式兜底（`window_final_text`）；模型空(未剔)仍兜底。部分剔除 ⇒ 406 基准与兜底只取保留区间流式（`kept_streaming_text`：时间戳/估算两路；`AccDropStats.kept_ranges`）；删 408B `acc_vs_streaming_after_drop` scale，回统一门槛；410 pending 同筛。`punctuate_via_service` 先剥零星标点再整段送。
- `fix421_tests` 10P+1I（含日志数值回放：窗 #0 空、窗 #2 不含他人语音）；fmt EXIT0 / check 0 error、warnings 91/87=基线 / 全量 test 0 failed（bin 1788P/0F/55I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · FIX-REFLOW-RAW-BASE-420 交付（回灌底稿改原始流式文本，只改 main.rs，未出包）

- 根因（BUILD-419 05:18）：`render_authoritative_reflow` 底稿用 `last_streaming_text` 镜像（= 合成文本），而 `committed_len` 是原始流式坐标 ⇒ 截尾错位多挂旧字（镜像131/acc126/committed124 ⇒ 133，多 7 旧字）。
- 改动：新增 `last_raw_streaming_text`（仅 `StreamingText` 收包时写合成前 `text`，带 gen，写于 043 门闩前）；`render_authoritative_reflow` 底稿改取它并按 gen 过滤（不符/无 ⇒ 预览=acc 全文）；形参经 `process_controller_events`/`try_resolve_reflow` 透传。镜像写入/用途与三个合成函数不动。
- 坐标审计：`compose_with_acc_for_gen` 生产 2 处传原始 `text` 坐标正确；`last_streaming_text` 生产读取仅 7663（自学习基准，非坐标）；无同类错位。
- 测试 `fix420_*` 4P；fmt EXIT 0 / check 0 error、warnings 91/87 ≤ 基线 / 全量 test 0 failed（bin 1779P/54I）。🔴 须 Gavin 目视末句预览与上屏一致。未出包。

### 2026-09-25 · FILLER-LONG-REPEAT-419 交付（后处理节点规则 C：整段重复只留第一份）

- `strip_fillers_conservative` A/B 后加规则 C：紧挨着、一字不差重复的整段只留第一份；忽略标点/空白比较；中/日/韩 ≥6 字、英文 ≥3 词；排除周期性单元与纯数字；差一字不动；迭代到不动点。不区分管线，平台中立（MACOS-HANDOFF 已记）。
- `rule_c_*` 10 条；fmt EXIT0 / check 0 error、warnings 91/87≤基线 / 全量 test 0 failed。未出包。

### 2026-09-25 · FIX-ALIGN-GATE-416 · R1 交付（宽松层改绝对编辑距离排序，去 xfail ignore）

- 中段插入 1 字原丢 1 字（宽松层按最低编辑率，分母偏置）⇒ 改按**编辑距离绝对值最小**、打平取较小 k；编辑率仅作门槛。coder-2 xfail 去 `#[ignore]` 转正；严格层 ③ 自查无同类偏置。
- repro416 9/9、testsync416 10/10；fmt EXIT0 / check 0 error、warnings 91/87≤基线 / 全量 test 0 failed（bin 1775P/0F/54I）。未出包。

### 2026-09-25 · TEST-SYNC-416 交付（接缝对齐分层的非作者护栏 10 条，生产零改动）

- 只加 `src/transcription/mod.rs::testsync416_tests`（10 条）：长句不重复 / 各层不丢字（错字+插入+删除）/ 同编辑率取小 k / e<8 / 接缝去重不误伤 / 全不同拼接 / 旧整片形状 / 源码锚点 + 阈值常量；自建字表与场景，不复用作者夹具。
- 🔴 **FINDING**：重叠**中段插入 1 字** ⇒ 宽松层取 k=13（2/13<2/12）⇒ 丢 1 字；`#[ignore]` xfail 挂起，主控裁决「必须修，派 coder-1」，R1 规则=宽松层改「编辑距离绝对值最小、打平取较小 k」；注释写明 R1 后去 ignore。
- 验证：fmt EXIT 0 / check 0 error、warnings 91/87 ≤ 基线 / **未跑 cargo test**（阶段三禁）；期望值经 sandbox 复刻纯逻辑逐条复算一致。未出包。

### 2026-09-25 · REPRO-413-ALIGN-GATE-416 + FIX-ALIGN-GATE-416 交付（复现 + 修复，只改 mod.rs，未出包）

- 413 后常规窗重叠占比随新句变长而变小 ⇒ 原比例长度门挡真实重叠 ⇒ 拼接 ⇒ 后缀重复（复现：后缀 8/12/16 在 S2 ≥19/29/41 起）。修复分层：严格（有先验且 e≥8 时门只要求 ≥8）→ 宽松（[0.5e,1.5e] 最低编辑率 ≤0.35）→ 接缝去重（≥4 有效字）→ 拼接（最后手段）；e<8 不进放宽路径防切多丢字；④/无先验不变。
- 快照翻转（0/1/2 错字全 `=`、0 错字精确）；真实 debug.log 15 窗回放 修前 dup=2/丢=0 ⇒ 修后 dup=0/丢=0；反例 3 条。repro416 9 条；fmt EXIT0 / check 0 error、warnings 91/87≤基线 / 全量 test 0 failed（bin 1755P/0F/54I）。平台中立，MACOS-HANDOFF 已记。未出包。

### 2026-09-25 · LOCALRT-STREAM-THREADS-417 + DEBUG-SESSION-WAV-418 交付（阶段一，未出包）

- **417**：`local_stream_num_threads()` `min(8)`→**`min(4)`**（取不到回落 4），只改预览侧；doc 写 Gavin 09-25 指示 + F-F-01 理由；测试更名 `..._caps_at_4`、封顶 4、新增「≥8 核与精解侧刻意不同口径」断言。平台中立（MACOS-HANDOFF）。
- **418**：`audio/mod.rs` 新增 `SessionDump`——`-debug` 累积送本地实时管线的 16k 单声道音频，`Drop` 后台线程写 `debug-audio/session-<ts>.wav` + `[DBG-418]`；`record_streaming` 内 `emit` 包裹全部 `on_chunk` 出口；`enforce_dump_limit_where`+`is_session_dump_file` ⇒ session 上限 10 与 preroll 20 分开计数。Warn 零动作 / 录音线程零阻塞 / 只读旁路。
- 测试 `diag418_*` 2P；fmt EXIT 0 / check 0 error、warnings 92/87=基线 / 全量 test 排除 coder-1 在途 RED `repro416_tests` 后 0 failed（bin 1746P/54I）。未出包。

### 2026-09-25 · OVERLAY-MEASURE-CACHE-415 交付（浮层量宽结果缓存，阶段一，未出包）

- 根因（F-A-01）：浮层每帧重绘都重量字宽（GDI `GetTextExtentPoint32W`+`encode_wide`；DWrite `CreateTextLayout`+`GetMetrics`+`encode_utf16`），文字只在流式/补间时变。
- 改动（只 `src/main.rs` 浮层区）：`MeasureKey{text,font_size,dpi}` + GDI `LastMeasure<i32>::get_or_insert_with`（命中不编码/不量宽）+ DWrite `DwriteMeasure`（缓存编码结果供 `DrawText` 复用）；两套无锁 thread_local，GDI/DWrite 分缓存（281 禁互串）；`measure_text_width` 加 `font_size`；`streaming_text` 先查缓存。
- 不变：GDI 兜底、`log_draw_geo_277`、`streaming_scroll_offset` 口径。平台：Windows 专属，macOS 已评估无影响（MACOS-HANDOFF）。
- 测试 `omc415_*` 4P/0F；fmt EXIT 0 / check 0 error、warnings 92/87=基线 / 全量 test 0 failed（bin 1744P/54I）。🔴 浮层视觉须 Gavin 目视（滚动贴右/随文字变宽/改字号/多屏 DPI）。未出包。

### 2026-09-25 · TEST-SYNC-413 交付（常规窗「少带前文」的非作者护栏 10 条，生产零改动）

- 只加 `src/main.rs::testsync413_tests`（10 条）：常态/首片 must_start+前文片、规则 3 pending 集成（`plan_windows` 4×4s）、规则 4 收尾整片、两处调用点源码锚点、区间裁剪平移+concat 窗内坐标、字速快慢冷启动回溯、多字节按 char 比例基准、兜底只覆 must、BUILD-399 数值（5.83/6.85 vs 旧 9.33）。
- 期望值经 sandbox 复刻 `plan_windows`/`group_window_start_secs`/`tail_backtrack_secs`/`take_context_suffix`/`trim_and_shift_ranges`/`shift_and_concat_ranges` 全部一致。
- 白名单：fmt EXIT 0、check 0 error / warnings 92/87=基线、`main.rs` 零新 warning；未跑 `cargo test`。未发现生产缺陷。未出包。

### 2026-09-25 · LOCALRT-SHORT-CONTEXT-413 交付（常规窗「少带前文」，阶段一，未出包）

- 常规窗音频：整段前文片 → 紧邻前一片后缀（`tail_backtrack_secs` ≥2s/≥12 字 + `find_tail_cut`）+ `[must_start..ge)` 完整解。`must_start` 纯函数决定（常态=窗末片；规则 3 并入 pending 的首窗=pending）；对齐 span `(must_start-1, ge)`；VAD 区间 `trim_and_shift_ranges` 裁剪平移（不新增每窗 VAD）。
- 407/413 共用 `take_context_suffix`（非复制）；407 输出逐位不变；规则 4 收尾短尾窗仍整片重解（源码护栏锁）。
- 验证：fmt EXIT 0 / check 0 error、warnings 92/87=基线 / 全量 test 0 failed（新增 9 条）。BUILD-399 09:48Z 末片 2.85s：旧送解 9.33s ⇒ 新约 5.83~6.85s。仅改 `main.rs`；MACOS-HANDOFF 已记。未出包。
- **R1 返修（主控退回）**：带前文常规窗 406 兜底收窄为只兜底 must 片（`context_tail_pending` → span `(must_start,ge)`；无前文仍 None 不变），防前片后缀被流式顶掉（410 item2）；删重复注释。测试 +2 = 11/11；fmt EXIT0 / check 0 error、warnings 92/87 / 全量 test 0 failed（bin 1730P/0F/54I）。仍未出包。

### 2026-09-25 · TEST-SYNC-411 交付（多片切分流式分配的非作者护栏 5 条，生产零改动）

- 只加 `src/main.rs::testsync411_tests`（5 条）：性质（6 分布/边界/多字节）、真实 `plan_windows` 组窗组合、切分+末尾窗 accept、切分+末尾窗被拒（只替 pending、前片不动）、端到端 BUILD-409 16:44Z 真实三窗无重复。
- 期望值经沙箱复刻 406（win0 accept 0.882 / 整段 reject 0.412，与生产一致）+ `plan_windows` 验算。
- 白名单：rustfmt EXIT 0、check 0 error/warnings 92/87=基线；未跑 cargo test。观察：窗间同人短语重复仍在（非 411，组窗粒度；宁重复不丢字）。未出包。

### 2026-09-25 · SPEAKER-MERGE-SHORT-412 交付（相邻短段拼「连续说话单元」再判声纹，未出包）

- 纯函数 `merge_speech_units`（间隔 <0.5s 暂定合并）+ `SpeechUnit`（只算语音、不含间隔；退化零长区间保留为零长成员保一一对应）；`filter_ranges_by_voiceprint` 按单元判定（≥2s 拼接成员语音算一次声纹、判定作用全成员；<2s KeepShort）；注册/漂移按单元 offer（起点 ≥ `new_slice_from`，跨界单元不 offer）。
- 7 条影响面逐条结论+测试：kept 仍原各段 / 秒数口径同改前 / 解码后重解用原 ranges / 漂移上限仍 0.25 / 跨界单元不 offer / 无就绪档逐位一致 / 缺模型合并未执行。
- 测试 `ts412_*` 13 + mod.rs 锚点 1 = **14P/1I**（真模型单元 vs 2.4s 长段 0.915）；全量 `cargo test` **1701P/0F/53I**。仅改 `speaker.rs`+`mod.rs`（后者仅加测试）。
- **R1 返修（数据校准）**：`MERGE_GAP_SECS` 0.5→**0.8s**（实测间隔一半以上 0.5~0.8s，原阈值下窗#4 一段拼不上）；规则改为 **只为凑够 2s 才拼**（单元 ≥2s 即封口）+ **尾段并入前单元** + **≥2s 单段独立**；补窗#4 真实区间/长段独立/尾段并入/凑 2s 封口 4 条测试。

### 2026-09-25 · FIX-TAIL-GUARD-410 交付（末尾窗 × 406 配合三错 + 标点服务重试/冷却，只改 main.rs）

- ① 比对基准按样本占比只取前片流式末尾相应字符数（char 切）+ pending（`tail_streaming_baseline`）⇒ 不再误拒正确精解。
- ② 末尾窗被拒/空只兜底 pending（span `(p,p+1)`）⇒ 前片已回灌结果不动。
- ③ 兜底先打标点：进程级常驻标点服务线程（首次兜底才启动、引擎线程内加载）；异常退出重启最多 2 次、全失败走无标点 + 30s 冷却、超时不重启。
- 测试 `f410_*` 8 条 + `#[ignore]` 真模型；全量 `cargo test` 1674P/0F/52I。未出包。

### 2026-09-24 · TEST-SYNC-406-408B 交付（阶段三·非作者护栏 16 条，生产零改动）

- 只加 `#[cfg(test)]`：`mod.rs::testsync406_408b_tests` 10 + `speaker.rs::testsync408b_guard_tests` 6。
- 406：门限常量/边界、归一化、同音错字/幻觉、精解更短、after_drop 逐位一致+下限、未闭合标签；408B：多档最高分、保留门、partition、新语种闸、new_slice_from、v1→v2 迁移/重建。
- 契约 7、9(new_slice_from) 内联需模型 ⇒ 源码锚点护栏。白名单：rustfmt CLEAN、check 0 error/warnings 92/87=基线；未跑 cargo test。未发现缺陷。未出包。

### 2026-09-24 · SPEAKER-VERIFY-408B 交付（声纹接入路B + 按语种分档存档）

- 流程定稿：解码前对所有已就绪语种档取最高分 <0.45 ⇒ 剔除；解码一次取 L（前缀/字符集）；L=ja/未知/档未就绪 ⇒ 原 ranges 重解保护；注册漂移按 L 归档；分档存档（v2，v1 迁移不丢弃）。
- 改动 `transcription/speaker.rs`（重写）+ `transcription/mod.rs` + `main.rs` + `audio/mod.rs` + `poc_slice_cut_381.rs`；删 `src/bin/poc_speaker_408.rs`。
- 真模型 `#[ignore]`：self 0.966 / 他人 ≤0.093；window self 0.992 / 他人 0.076。
- 验证：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1641P/51I**）。未出包。

### 2026-09-24 · LOCALRT-TAIL-WINDOW-407 交付（1900ms 长静默自动末尾组窗）

- `local_stream.rs`：长静默（≥1900ms）只发一次 `on_long_silence(pcm_pos)`，恢复说话复位；`LONG_SILENCE_TAIL_MS`+纯函数。
- `main.rs`：`AccInput::LongSilence`；有 pending ⇒ 末尾窗（前片回溯 2s 找字缝 + pending，无字缝回落 2s）→ 立即投解码回灌，无需停止键；新增 `dispatch_tail_window!`+`tail_window_span`。
- `vad.rs`（主控许可）：`find_gap_cut` 提权+拆内核，新增 `find_gap_cut_gap_only`（判据只一处）。
- 测试 `ts407_*`/`tw407_*`/真模型；全量 `cargo test` 1601P/0F/49I。未出包。

### 2026-09-24 · FIX-ACC-MISMATCH-GUARD-406 交付（精解不得覆盖正确预览：同窗流式比对 + 未闭合标签）

- 新增 `transcription::acc_vs_streaming`（LCS **子序列** 保留率 + 长度比；阈值 0.5/0.8；流式 <6 字符不判）+ `normalize_for_mismatch` + `lcs_subseq_len`；接入 `main.rs` `harvest_acc_window!`（精解非空且不通过 ⇒ 用同窗流式替换 + `[LocalRT-DBG-406]` warn，仅本地实时滑窗路B）。
- `strip_angle_tags` 新增**未闭合标签**剥离（`<`+≥3 连续 ASCII 字母、名字 run 内无 `>`）；`a<b`/`3<5`/`<3块钱` 不误剥。
- **校准**：三例 reject（0.43/0.43、0.04/0.32、0.00/0.48）；正常纠错 accept（0.83/1.00、0.96/1.04、全角/大小写 1.00/1.00）；`reflow applied` 24 条长度比代理 21/24≥0.8；🔴 历史 acc 串不落盘 ⇒ 字符串 retention 不可还原。
- 验证：`rustfmt --skip_children`（两文件）CLEAN；全仓 `cargo fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1597P/48I**）。未改动 local_stream/patches/scripts；未出包。

### 2026-09-24 · SPEAKER-VERIFY-408A 交付（声纹模块，独立，未接入管线）

- 新增 `src/transcription/speaker.rs`（平台中立）：加载/embed、自动注册（≥12s/≥3 段 + 离群剔除）、漂移 EMA、保守判定（<2s / ja / 未就绪 全保留；score<0.45 才剔除）。
- 模型 `models/speaker-campplus-zh-en/…advanced.onnx`（sha256 `aa3cfc…`，gitignore）。
- 不改 `mod.rs`：临时 `src/bin/poc_speaker_408.rs`（`#[path]` 宿主）；模块 **10P/1I**、真模型本人 0.966 vs 他人 ≤0.093；`check --all-targets` 0 error、warnings 92/87=基线；全量 test 0 failed。未出包。

### 2026-09-24 · LOCALRT-PERF-405 交付（本地实时流式性能五项）

- **影子收尾移除**（DEC-086）：删影子重解分支/状态/常量/`endpoint_confirm_text`/DBG 日志/测试，显示改主解，新增护栏；未动切句/342/派发/sentence_id。
- **F-C-01** `DisplayCache` 增量缓存（+等价性测试）；**F-A-02** 只为日志的计时/计数收进 Debug 守卫。
- **CT2 每次重编根因**：build.rs `rerun-if-changed` 指向不存在路径 ⇒ `MissingFile`；改为仅存在路径登记，第二次 build **0.89s 不编 CT2**。
- **ps1 Step5** 产物名改 `feiyin-ime.exe` 等 + 缺件 exit1。仅动 3 文件（+主控许可删 `qwen_inference.confirmed_text` 死方法）。未出包。

### 2026-09-24 · POC-SPEAKER-VERIFY-404B 交付（声纹四语复测 + 跨语言注册 PoC）

- 四语数据：zh=AISHELL-1 10 人 / en=LibriSpeech 12 人 / ko=Zeroth 12 人，各 ≥10 条 ≥2s；**ja=缺口**（无公开带标注自然人语料）。
- **推荐修正为 `CAM++ zh_en`**（中 0%、英 2-5s 0.10%、韩 1.33%）；备选 ERes2NetV2/ERes2Net en-vox；CAM++ en-vox 异常差不推荐；≥2s 才可靠、韩语最难。
- 跨语言注册无「同一人多语」数据 ⇒ 缺口 + 补录清单。仅改 `#[cfg(test)]` PoC + 文档；scratch ~4.3GB 已清理。未出包。

### 2026-09-24 · POC-SPEAKER-VERIFY-404 交付（声纹判别模型选型 PoC）

- 6 个 sherpa-onnx speaker-embedding 模型对比；**跨说话人轮流注册**（8 人，每人轮流当使用人，满足「不是只为我定制」）。
- **推荐 CAM++ zh-cn**（27MB）：≥2s 池化 EER **0.26%（最优）**、最快（3s ~40ms/单线程）；ERes2NetV2 为 1-2s 备选；ResNet293 不推荐。
- **<1s 不可用**（EER 9.5~41%）；**高置信剔除阈值 0.55~0.65 → 背景剔除 ~96~99%**、使用者误拒≈0；两人叠加 score 0.82→0.42（不剔除）。
- 接入设计：只做高置信剔除、延迟到最终注入前（不撤回预览）、最短音频 ≥2s。仅新增 `#[cfg(test)]` PoC + 文档，未改生产。未出包。

### 2026-09-24 · RELEASE-ISS-FIX-401 交付（安装脚本三处小修：BOM / MinVersion / 卸载杀进程）

- 两份 `.iss` → UTF-8 with BOM（`efbbbf`；加 BOM 前后去 BOM 内容 sha 相同 ⇒ 中文逐字未变）。
- `MinVersion` 6.1→10.0（DEC-000）；`[UninstallRun]` 增杀 `feiyin-ime-ui.exe` / `crash-reporter.exe`（各独立 RunOnceId、runhidden）。
- 全文无写死 `voice-ime.exe` 残留；两份 sha 全等。未构建/未出包。

### 2026-09-24 · RELEASE-ISS-FROM-PUBLISH-400 交付（安装脚本改从 Publish 取 + 过期内容修正，不带模型）

- `installer/voice-ime.iss`（+`Publish/voice-ime.iss` 逐字副本）`[Files]` 全部改从 `..\Publish\`（17 Publish + 2 assets 白名单，无通配）：三 exe、sherpa×2、onnxruntime×2、`ctranslate2.dll`、五运行库、四规则表；删弃用 paraformer。
- 排除 `models\`（Gavin 定不带）+ 用户数据 + 开发脚本 + `cudnn64_9.dll`/`libiomp5md.dll`（dumpbin 证无导入）；`MyAppVersion`→0.9.3、`MyAppExeName`→`feiyin-ime.exe`、`MyAppId` 未动。两份 sha 全等。
- 实证 Source 实存 **19/19 OK**；规则表 exe 同级读取行号已给。
- 🔴 **核查**：程序无自动模型下载器，缺口单列（仅 Accuracy 有 UI 引导）。未构建/未出包。

### 2026-09-25 · TEST-EXEC + BUILD-410 出包（末尾窗 × 406 配合修正）

- 回归：root bin **1680P/0F/52I**、总 1768P/0F/54I；`src-tauri` 92P/0F；`fmt` EXIT 0；Vitest SKIP。NEW/GONE（对 409）+14P/+1I（`f410_*` 8 + `ts410_*` 6 + ignored `f410_real_punctuate`），GONE 0。
- 构建：Step2 SKIP；Step3 #1 2m57s（CT2 编译 0）/#2 0.94s；Step4 `init-publish.ps1` Step5 三 exe 全 OK + 声纹模型 + 四 toml。产物 main `a3365d79…` / ui `ee7f2571…` / crash `b675b6f4…` / ct2 `efa16d81…`（未重编）。
- 九项 + 专项 A~D + 三特殊点全 PASS；冒烟 `WM_CLOSE` **380ms** 退出。
- 🔴 出包强杀输入法，已提醒 Gavin 重启 + 带 `-debug` 端测。

### 2026-09-25 · TEST-EXEC + BUILD-409 出包（405~408B 合包）

- 回归：首轮 bin 1664P/2F/51I（两条红系阶段三测试自身错，作者返修）→ 复跑 bin **1666P/0F/51I**、总 1754P/0F/53I；`src-tauri` 92P/0F；`fmt` EXIT 0；Vitest SKIP。NEW/GONE：对 408B +25/0；对 399 NEW 80/GONE 2。
- 构建：Step3 #1 6m06s（CT2 重编 1 次）/#2 2m18s（CT2 0）/#3 0.90s 全增量；Step4 `init-publish.ps1` Step5 三 exe 全 OK + 声纹模型 + 四 toml。产物 main `b37aa194…` / ui `ee7f2571…` / crash `a555f11c…` / ct2 `efa16d81…`；两副本全等。
- 九项 + 专项 A~E + 三特殊点全 PASS；冒烟 `WM_CLOSE` **340ms** 退出。
- 🔴 出包强杀输入法，已提醒 Gavin 重启 + 带 `-debug` 端测。

### 2026-09-24 · BUILD-399 出包（撤回 398 词库前缀 · 直接出包，不跑回归）

- 源码 `49bae07`（`src/` 与 `678c7a0` 逐字节一致）；Gavin 明示不跑 `cargo test`/Vitest。
- Step1 残 0 → Step2 SKIP（ui/src-tauri 无 diff）→ Step3 main 6m14s → Step4 `init-publish.ps1` + 手工 cp。产物 main `0dba0c97…` / ui `ee7f2571…`(未改) / crash `a3ee1930…` / ct2 `5655295e…`；两副本全等。
- 九项 + 专项 + 三特殊点全 PASS；冒烟 `-debug` 新窗 `hotwords inject`=0、`WM_CLOSE` **328ms** 退出。
- ⚠️ CT2 被重编（`rerun-if-changed=CTranslate2`）sha 变、同大小、导入表不变、冒烟 `(threads=8)` 证正常；`init-publish.ps1` Step5 exe 名过期已手工补齐。
- 🔴 出包强杀输入法，已提醒 Gavin 重启 + 带 `-debug` 端测。

### 2026-09-24 · TEST-EXEC + BUILD-398 出包（397 + 398 合包；回归与构建并行）

- 回归：root `cargo test --no-fail-fast` **1685P/0F/46I**（EXIT 0）/ `src-tauri` **92P/0F/0I** / `fmt --check` EXIT 0 / Vitest SKIP（`ui/` 无 diff）。
- NEW/GONE（基线 BUILD-395 `1a8f00d`）：+8P −5P +1I（`fix396_*` 相对 395 净零）。
- 出包八项 + 专项 A~D + 三特殊点全 PASS；产物 main `15c3ec9f…` / ui `ee7f2571…` / crash `c6a38d5b…`；CT2 DLL **27.4MB**（旧 8.1MB）已同步 `Publish/`，导入仅多 `VCOMP140.DLL`；冒烟 `-debug` 实证 `NLLB … (threads=8)`，`WM_CLOSE` **335ms** 退出。
- 🔴 出包强杀输入法进程，已提醒 Gavin 重启 + 带 `-debug` 端测。

### 2026-09-24 · LOCALRT-TERMS-PREFIX-398 交付（本地实时路B 恢复词库注入，改用 `Technical terms: a, b, c.` 固定前缀）

- **推翻 DEC-083、立 DEC-084**：删除 `load_hotwords_for_accuracy` 的 `AsrModel::LocalRealtime` 早退（与 Accuracy 同走 `uses_accuracy_engine()` 门）；`build_ctx_system` 输出由裸词表 `a,b,c` 改为 `Technical terms: a, b, c.`（前缀 + `, ` 连接 + 英文句点；按 `,` 切分逐条 trim、丢空段；空词表 None）。
- **回显防护**：抽取 `find_best_terms_run`（374/398 共用，`strip_terms_echo` 逻辑不变），新增 `strip_technical_terms_echo`（大小写不敏感识别前缀并整段剥离 + 误伤护栏「前缀后须有 ≥1 真词条」），走 `apply_acc_disposition` 的 Echo ⇒ 不带注入重解同一路径。
- **依据**：外部 184 次实测（TypeWhisper#321，Qwen3-ASR）唯一 0 泄漏格式（WER 33.8%→18.2%）；Gavin 2026-09-24 拍板照搬，明示豁免 DEC-059 A/B。🔴 **中文场景外部未测 ⇒ 靠端测验证**。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1597P/44I**）。未出包 / 未改版本 / 零凭证。

### 2026-09-24 · RELEASE-VCRT-APPLOCAL-399 交付（VC++ 运行库 app-local + 安装脚本补 ctranslate2.dll）

- 最小运行库清单（dumpbin 全量复核）：`msvcp140` / `msvcp140_1` / `vcruntime140` / `vcruntime140_1` / `vcomp140`；`api-ms-win-crt-*`（UCRT）不带。
- `scripts/init-publish.ps1` 新增 Step 2（vswhere 动态定位 VS Redist、缺件报错、拷 `Publish/`+`target/release/`、`-RuntimeOnly` 单跑、UTF-8 BOM）。
- 两份 `voice-ime.iss` `[Files]` 补 `ctranslate2.dll` + 五个运行库（sha 全等）；`build-test-guide.md` 出包核验八项→九项；DEC-085 立档。
- 实证：`-RuntimeOnly` 实跑，三处 sha256 逐一 MATCH。纯 Windows 打包，macOS 无影响。未出包。

### 2026-09-24 · TRANS-CT2-DNNL-THREADS-397 交付（本地翻译提速：CT2 换 oneDNN + 线程数按核数传入）

- Windows 目标依赖给 `ctranslate2-sys` 追加 `dnnl` + `openmp-runtime-comp`（通用依赖与 macOS 不变；`cargo tree --target aarch64-apple-darwin` 证无 dnnl/openmp）；`patches/ctranslate2-sys/build.rs` 两处补丁：① `CMAKE_PREFIX_PATH` 让 CT2 找到自编 oneDNN（否则 `WITH_DNNL` 必 FATAL_ERROR）② DLL 同步拷 `<target>/deps/`（修 `[CT2-DLL-SHADOW-397]`）。
- 新增 `nllb_num_threads()`（`available_parallelism().min(8)`、回落 4，与识别侧同口径），`NllbModel::new` 加载时传 `num_threads_per_replica`（原 0⇒4）；生产不读 env/config。
- **实测提速 10.8~15.6×**（新 DLL 8 线程 vs 旧 DLL 基线，S1~S4）；`CT2_VERBOSE=1` 证 `GEMM_S8 backend: DNNL` / `int8_float32` / `ISA AVX2`；导入表只多 `VCOMP140.DLL`。
- 回归：`cargo fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 **1685P/0F/46I**。阶段一交付，未出包。

### 2026-09-23 · VAD-V6-AND-TIMELINE-REUSE-393 交付（silero VAD v6.2.3 + 本地实时 60s + 实时时间线复用剪静音）

- **C**：`models/silero-vad/silero_vad.onnx` 同路径替换为 silero **v6.2.3**（2,327,524B，sha256 `1a153a22…`）；v4 备份 `collab/evidence/vad-v4-backup/`（运行时不引用）；**无 v4 回退机制**。v6 下 2 条正弦冒充语音夹具改 `full.wav` 真人声 + 新增纯正弦负例 ⇒ `--ignored vad` 11/11。
- **B**：`LOCALRT_VAD_MAX_SPEECH_SECS=60.0` 仅本地实时两构造使用，离线/在线仍 20s。
- **A**：实时 VAD 时间线随派发下发，剪静音优先复用（不再每窗重跑 VAD）；空区间 + 流式非空 ⇒ 整窗解码不吞字；无时间线 ⇒ 回退 391 自跑 VAD（仍 v6）。
- **验证**：`cargo fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 97/88=基线；新增单测 11 条 + 真模型 E2E（60.15s→53.79s）；全量 `cargo test --bin feiyin-ime` **1580P/0F/38I**。未出包。

### 2026-09-22 · UNWIRE-STRIP-NODE-365（365+366 收口）交付

- 350 剥离节点从路B 主路径摘除、降级分支仍挂（条件挂载 `b_strip_enabled`）；代码/7 单测/护栏保留可回挂；sync352-4 锚点同步、语义不变。
- 366 三件全不做（🥇 满核实测负收益 16 vs 8 慢 1.73×/1.79×；② 取消路A 丢 fallback；③ 无中断 API）；366 PoC 保留为证据，`default_acc_num_threads` 注释写明「8 是实测最优」。
- 验证：release EXIT 0；全量 cargo test 0 failed（1363P）；warnings 98/89；numstat==-w。未出包。

### 2026-09-22 · DUAL-PATH-REFINE-364 交付（录音上限 180s + 预算精确计算）

- 录音上限 `MAX_RECORD_SECONDS` 300→**180**（含断言/文案/注释连带）。
- 路B 预算改**精确计算**：音频 token 移植 C++ `FeatToAudioTokensLen`（20s→260 吻合实测、180s→2340）；注入段（清理指令+跨录音上下文+词库）tokenizer 实数×2.0；`SAFETY_MARGIN` 512→64。判据 `audio+inject+256+64 ≤ 4096` ⇒ 阈值 ≈`(3776−inject)/13`s，**180s 不降级**；闸门保留。
- 验证：`cargo build --release` EXIT 0；全量 `cargo test --no-fail-fast` 0 failed（1363P）；warnings 98/89 基线；numstat==-w。**未出包**（DEC-079）。

### 2026-09-22 · DUAL-PATH-ACC-363 交付（acc 双路：路A 切片刷预览 + 路B 累积全量出终文）

- Gavin 拍板：acc 后台分两路。路A 保持现状（1200ms 静默→最新片→回灌预览，零改动）；路B 松手后跑一次**累积全量解**，全文整体替换切片拼装作最终输出（无拼装 ⇒ 接缝重复消失 + 自我纠错前文）。
- 全量音频取 ASR 线程返回的 `local_pcm` ⇒ 主路径零新增开销；不中途预解（361 证无增量复用）。路B 走 `transcribe_acc_ctx`，注入跨录音上下文 + 词库。
- 预算闸门 `path_b_budget_ok`（音+词库+生成+提示+上下文+余量 ≤ 4096）；不够则路B 降级退回拼装并打日志。
- 验证：`cargo build --release` EXIT 0；全量 `cargo test --no-fail-fast` 0 failed（1363P）；warnings 98/89 基线；numstat==-w；闸门单测 4 条。**未出包**（DEC-079）。

### 2026-09-22 · POC-TIMESTAMP-DECODE-CURVE-361 交付（RP-1 时间戳不填值；decode 线性、无增量复用）

- 纯 PoC 零生产代码。
- **RP-1 失败**：Qwen3 1.7B `timestamps`/`durations` 均 `Some(len=0)`（tokens 有 61）⇒ 不填值 ⇒ RP-2 路断。
- **RP-3①**：decode 开销随长度近似线性（1s 691ms → 20s 6633ms → 56s 22885ms，斜率≈0.41 s/s）⇒ 每次从头解、无增量复用 ⇒ 累积重识别须窗口封顶。
- **前文改写**：同前缀重复解稳定；增长时前文被改（二比零→二比一）⇒ 豆包式回改文字层存在。
- 结果入 `collab/research/qwen3-1.7b-capability-roadmap.md`；PoC `d7650b3`（纯测试）。未改生产代码/未改版本/未 push。

### 2026-09-22 · MIGRATE-1.13.8-1.7B-359 交付（升库+换 1.7B+剥前缀，一批到位）

- Gavin 拍板不分两批：升 sherpa 1.13.8 + 换 Qwen3 1.7B + 应用层剥前缀（方案 B，语言无关）；回滚整批一起回。
- 升库 `sherpa-onnx 1.13.8`（lock 已更）；换官方 `shared-MD-Release`（**ORT 1.28.2**）到 `vendor/`（旧 1.12.38 目录保留＝回滚备份）。
- 换模型目录：唯一来源常量 `QWEN3_MODEL_SUBDIR` 接入 5 处（生产路径无 0.6B 残留）。
- 新增语言无关前缀剥离，并入 `strip_asr_special_tokens`；对 0.6B/无前缀 no-op；单测 4 条。
- 验证：`cargo build --release` EXIT 0；**全量 `cargo test --no-fail-fast` 0 failed**（1359P）；warnings 98<99；numstat==-w。**未出包**（DEC-079）。⚠️ 首轮一次 `itn356_*` FAIL 系 coder-2 356 在飞中间态，非本批。

### 2026-09-22 · UPGRADE-SHERPA-1.13.8-358 交付（接口 diff：接口零风险、行为有真风险）

- 第一步只做接口 diff，不真升级，零文件改动。
- **接口兼容**：1.13.8 相对 1.12.38，`offline_asr.rs` 仅 +8 行（新 `unsafe impl Send/Sync`）、`lib.rs` 仅 doc、其余仅 +Send/Sync；**10 字段与签名零变化 ⇒ 预期零改动编译通过**；无 Send 冲突；features 不变。
- **行为风险**：🔴 **#3873 改 centered-STFT 特征 ⇒ 输出可能变（含 0.6B）⇒ 必须全量回归 + 端测**；#3907 仅覆盖整段全静音；#3912 竞态现状不触发；onnxruntime 1.24.4→1.28.2。
- **dll**：选 `shared-MD-Release` 档、换 `SHERPA_ONNX_LIB_DIR` 即可。
- 产出 `collab/research/sherpa-1.13.8-upgrade-358.md`。未真升级/未改 Cargo.toml·lock/未编译/未 push。

### 2026-09-22 · RESEARCH-QWEN3-CALL-OPTIMIZE-357 交付（提示词能传但模型不照做）

- 前提：换 1.7B 已拍板；本单只摸调用接口。零生产代码。
- 🔴 **`hotwords` 通道 = system prompt 通道**（C++ 把 hotwords 包进 `<|im_start|>system…`）⇒ 提示词能传、且生产一直在传（`decode_accuracy_once` 的 `set_option("hotwords", s)`）。**A/B = B（能）。**
- 🔴 但**模型不照做**：ITN 指令零效果、去口水词几乎不变（英文指令→空输出）、格式指令反噬 ⇒ **与 DEC-070 同型**，ITN/文本清理继续自研。
- 版本侧：1.12.38 与 1.13.8 的 Qwen3 config 字段相同（10 个、无 prompt/itn），上游 C++ 亦无独立 prompt 字段 ⇒ 升级拿不到。
- PoC `fab61cc`（纯测试）；产出 `collab/research/qwen3-call-optimize-357.md`。未改生产代码/未改版本/未 push。

### 2026-09-22 · RESEARCH-QWEN3-1.7B-CAPABILITY-355 交付（能力增量：无 B 级增量）

- Gavin 问「换 1.7B 有哪些 0.6B 没有的收益」+「调用上可调优点」。官方卡：0.6B/1.7B **同一段功能描述** ⇒ 功能集相同，1.7B 唯一官方差异=精度（已给定，不再测）。
- 实测（只读生产链）：ITN/文本清理**无增量**；专名 `·` 1.7B **更差**；语种识别两者都有但 1.7B 泄漏 `X<asr_text>` 前缀且**判错**（韩语→`汉语`）。
- 调用层：7 个写死字段对 1.7B 语义相同、无需改；前缀**无法配置关闭**（Qwen3 config 仅 10 字段）；**未发现只对 1.7B 划算的调用方式**（30s 大分片两者都可行）。
- 附带：生产恒发注入是**承重墙**；1.7B 对注入更敏感。🔴 崩溃未定性（4 次 1 成 3 崩，崩点不定，非系统 OOM）。
- PoC `5651ccc`（纯测试）；产出 `collab/research/qwen3-1.7b-capability-355.md`。未改生产代码/未改版本/未 push。

### 2026-09-22 · POC-QWEN3-1.7B-RETEST-353 交付（生产口径重测：无统计显著差异）

- 判据变更（Gavin）：内存/速度不再是障碍 ⇒ **唯一判据=生产口径精度**；351 在 ITN 前测 CER 的口径缺陷已修正。
- 只读复现生产链（`itn::normalize_numbers → normalize_text_for_language → normalize_unit_symbols_only`）后：
  0.6B **0.0267**(6/225)、1.7B **0.0356**(8/225，剥前缀)；差距 2/225 且全部集中在一个人名的中点号 `·`；含泄漏前缀时 1.7B=0.1467。
- 🔴 「二比一→2比1」ITN **不修**（已查实，只查不改）；ITN 反引入 2 处过度转换（只报不改）；控制前缀修复 ~5–10 行、须端测（只查不改）。
- **结论：n=1 无统计显著差异，需扩充语料才能定论**；「为精度换 1.7B」在当前证据下不成立。
- PoC `4112354`（+73/-0，纯测试）。未改生产代码/未改版本/未 push。

### 2026-09-22 · POC-QWEN3-1.7B-351 交付（1.7B 实测：**不换**）

- 目标：填 347 留的两个空白（本机 CPU RTF / 1.7B 中文 CER）。实测（16 核 / acc_threads=8 / full.wav 56.15s / 与 313 同切片）：
  0.6B `rtf=0.218 cer=0.0356 peak_priv=2020MB`；1.7B `rtf=0.324 cer=0.1556(含前缀)`，剥前缀后 **cer=0.0444**。
- 三条判据全未过：CER 未优反劣、RTF 1.36–1.49×、**分片态**峰值 **4.97GB 越界**（> DEC-076 否决的 4.3GB）。
- 🔴 发现 1.7B 输出带 `language chinese<asr_text>` 控制前缀、生产解析剥不掉 ⇒ **1.7B 非 drop-in，347「零改动只换目录」被推翻**。
- 方法教训入库：权重比外推解码耗时高估 1.8–2.0×（2.70× → 实测 1.36–1.49×），选型须实测。
- 前置重构 `e829c67`（零行为变更提取）；PoC `f335bd8`。产出 `collab/research/qwen3-asr-1.7b-poc-351.md`。

### 2026-09-22 · PUNCT-FINAL-REDO-350 交付（标点剥离独立节点，只挂本地 realtime）

- Gavin 要求：本地 realtime 最终 acc 文本先剥光已有标点、再整段重打，避免分片接缝处标点乱打、破坏语义连续性；**其余管线不能动**（在线 ASR 标点可能更准）。
- 落地为**独立节点** `strip_punctuation_node`（照 `apply_filler_strip` 形态），只挂本地 realtime 自有编排块；剥光后 `native_punctuated` 恒 false ⇒ 下游既有 `apply_local_punctuation` 门自动放行整段重打。
- 共享代码零 diff（`apply_local_punctuation` 与 HEAD md5 相同、`run_pipeline_core` 签名未动）⇒ 其余两档结构上不可能受影响（DEC-066）。验证全绿（fmt / check 0 error / warnings 99/90 / numstat==-w / 全量 cargo test 0 failed）。
- 版本仍 0.9.3；未出包、未 commit。等 Gavin 端测本地 realtime 实机效果。

### 2026-09-21 · BUILD-321 端测复盘与处置

- 六条端测问题全部定性：2 条已修（⑤护栏、⑥无需动作）、1 条待重建即消（②tokenizer）、2 条已派单（①ITN → coder-2、pre-roll → coder-1）、2 条排队待派（③尾字、④自学习，均撞 `main.rs`）。
- 调优复盘：后端并行派发是本版最大收益（后台解码 20.0s，用户实际等待中位 0.32s）；endpoint 门限有效；pre-roll 证实全程空转已派单处置；shadow 依 Gavin 端测观察保留不动。
- 尾字问题的两条既有修法（291/307）**双双证伪**，方向改为用 accuracy 分片结果回灌预览。
��证结果）
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

| **翻译路径事实保全补齐（020）** | 018 只修了主路径 `UNIT_SYMBOL_PROTECTION`，翻译路径常量 `UNIT_SYMBOL_PROTECTION_TRANSLATE`（`src/llm/mod.rs:33`）原样保留假前提「输入已含规范化数字」，且缺 L0-1/L0-3 对齐句。主控代码取证：`src/main.rs:2941` 的 ITN 处理在翻译分支判定 `:2984` **之前**，`optimize_and_translate` 与 `try_nllb_translate` 吃的都是同一份 ITN 输出 → **`一块八一斤` 那个静默改错在翻译功能下会原样复现**，且翻译路径连兜底句都没有。修复：常量自带完整 SUSPECT 语义（**不悬空引用 L0** —— 翻译路径独立拼装 system_content 未注入 L0 四条，主控裁定直接引用 L0 是有害方案而非仅超范围）+ 追加「保留不自洽原样以便用户看见并修正」+「本条款绝不授权删除单位或量词短语」；保留翻译路径特有的 `In the <corrected> line` 限定（刻意设计差异不得抹平）｜ 2026-08-03 |
| **F3 判据从字面重复改语义并列（021）** | Gavin 端测「建议从以下方面入手：**比如**…**再比如**…**还有就是**…」场景信号全部正常却未走无序列表。根因：DECISION RULE 要求 `the SAME marker appearing in 2 OR MORE` —— **三个标记各不相同，没有任何一个出现 ≥2 次**，规则把最常见的口语枚举形态排除，同时让 30+ 条无序词表整体失效。**选错了判定维度**：用「标记字面是否重复」代替「是否存在语义并列项」。修复：判据改为 `TWO OR MORE spans stand in a PARALLEL relation`（同一句法角色、同一语义功能、共同构成集合）+ 新增 **F3-semantic fallback 兜底授权**（DEC-039：清单 ILLUSTRATIVE 非 EXHAUSTIVE／判据语义非词汇／标记不同或不在清单或无标记均可判枚举／反向护栏并列不成立不得列表）+ `scene-rules.toml` 三处 F4 补无序族与 ILLUSTRATIVE 措辞 + 2 条负向 few-shot ｜ 2026-08-03 |
| **四语枚举标记清单恢复并扩充（023）** | **Gavin 推翻主控在 021 中的精简决策**：「提示词就是要列举常用示例，**越充分越好**，不用太在意大小……这样才能让大模型学习到我们的诉求，它才能去做推导」。主控自认三处误判：①误读 012 教训（那次结论是「结构问题不是措辞问题」，不是「文本多有害」）②结构问题 018 分层已解决，长度早已不是主要矛盾 ③低估示例作用（模型认得词 ≠ 知道我们希望它在这个词出现时做什么）。落地：恢复 `9eb80b7` 完整清单（脚本验证 **132 标记短语 0 遗漏**）+ 四语主动扩充（中 `其次是/接下来/像是/好比` 等／英 `to start with/among them` 等／日 `はじめに/例を挙げると` 等／韩 `첫 번째로/가령` 等／结构性句式 4 条新增）+ per-language contrast 四语恢复并改用**标记不同**形态演示语义并列 + F3c few-shot 同步 + 修正韩语错别字 `쓰하는`→`쓰는`。**T4 长度预算测试定位变更**：从「控制膨胀」改为「探测异常暴涨」，上界 16000→40000，仅拦截数量级错误（DEC-039 修正）｜ 2026-08-03 |

| **货币链撕裂 + 尾零 + 单段保原单位（026 / 026-B，P0）** | 017 修好了「一块八一斤」这类，但**货币链遇未知后继词仍撕裂**：`try_parse_unit_chain` 的隐式末级尾数被 `after_is_boundary` 与数值合成耦合，后继未知时链只剩 1 段 → 返回 None → 逐字路径把「五块一斤」劈成 `5块` + `一斤`；且 `format_currency_chain` 用 `{:.2}` 不去尾零，与乙型 `format_implicit_decimal` 行为不一致（`1.80元` vs `1.8`）。修复：①去掉 `after_is_boundary`，数值合成与后继识别正交 ②隐式尾数单位从硬编码 `"分"` 改为按前级层级动态决定（块/元→毛，毛/角→分）③多段链去尾随零 ④**允许单段 currency 链**（原 `parts.len()>=2`）。**026-B 是 Gavin 方案 C**：单段链一律保留原单位不归一到元（`五块一斤`→`5块一斤`／`八角`→`8角`／`二十五块`→`25块`），多段才归一（`五块一`→`5.1元`）——**判定依据是段数不是 `per_unit`**，理由是单段说明用户明确用了某个货币单位，系统不替他改写表达 ｜ 2026-08-03 完成 / 2026-08-04 收口 |

**出包**：2026-08-03 13:06（BUILD-010，含 017+018）／17:07（BUILD-011，含 021+020）／17:42（BUILD-012，含 023）／**2026-08-04 00:48（BUILD-013，含 026+026-B，当前 Publish 产物）**。三 exe 两副本 sha256 一致 + 两 toml 三副本一致（`scene-rules.toml` `7C1F0620`／`itn-rules.toml` `ED77A912`）+ ProductVersion 0.7.3.0 + mtime 链通过。**详见 CHANGELOG，出包/测试同步不在本文档记录（规则 7）**

---

## 🔄 v0.7.3 增补三 · 标点子系统治理 + 大数守卫（2026-08-08，**代码闭环，回归与出包未做**）

> 版本号 0.7.3 全程未动。四个 commit：`94bfb0b` / `5fc390d` / `201bb4f` / `d2ee6b3`，**本地 ahead 未 push**。
> ✅ **TEST-EXEC-030 已完成**（2026-08-09 20:5x，tester-1 执行 / 主控独立复算验收）：A0–A7 **零 FAIL**
> —— `itn::` 225 ｜ `punctuation::` 43 ｜ `transcription::` 105+4ign ｜ `llm::` 140 ｜ 主 crate 全量 958+8ign
> ｜ src-tauri 53 ｜ `--list` 自洽 966==966。四族零回归（017 重量／026 货币／027 大额 DEC-042／031 万一守卫）全绿。
> 主控**未采信汇总表格**，用源码 `#[test]` 计数逐项复算，六个数字全部吻合（详见 `logs/20260809.md`）。
> ✅ **BUILD-015 已出包**（2026-08-09 21:13，主控七项独立复算全过）：`Publish/` 三 exe
> `feiyin-ime.exe` `831c254d…` / `feiyin-ime-ui.exe` `14411dee…` / `crash-reporter.exe` `9fa58f9b…`，
> 两副本 sha256 全等；两 toml 三副本一致（`scene-rules.toml` `0a3a0b9a…` / `itn-rules.toml` `b208271b…`）；
> ProductVersion 0.7.3.0/0.7.3。**030/031 首次进 exe。⏭ 待 Gavin 端测。**
> 🔴 出包时拦截到 `scene-rules.toml` 两副本停留 08-03 旧版（41714B vs 45591B，差 3877B），
> 系 macOS 端 `f96c817` 经 merge `7e76465` 带入而本端未同步 —— `[TOML-STALE-001]` 第二次发作、
> 全新来路。已收敛并把 toml 同步补进 `build-test-guide.md` Step 4。

| 功能 | 说明 |
| --- | --- |
| **标点开关全源治理（030-A/A-2/B/B-2/C/D/E）** | 起因 Gavin 提「开自动标点时字/词数 ≤5 不加末尾标点」，主控给补丁式方案被打回：「每一个功能都要从架构程度全局层面来设计方案」。盘点发现**标点有 6 个产出源、开关只完全控制 1 个**。架构定为 **L1 源头控制为主、L2 后处理补位**（Gavin：能在源头关的就别产出后再剥）。L1：LLM optimize/translate 补 `NO_PUNCT` 明确禁止句（原为「什么都不说」）、`step1_correct` 硬编码条件化、列表分隔符切 `INLINE_SEPARATOR_RULES_NO_PUNCT`（空格连接禁 `、；,;`）；L2：`main.rs` 收口块**删除全部来源判据**（`llm_handled`/`native_punctuated`/`translate`），6 源一视同仁、将来新增第 7 源自动受控。已核实 Qwen3 在线 ASR／本地 native／NLLB 三条源头物理不可控（阿里云 `session.update` 无标点参数），故 L2 不可省 ｜ 2026-08-08 |
| **短句 ≤5 不加末尾标点** | `count_units`（中日逐字／英韩按空格词／混合相加，Gavin 口径）+ `strip_trailing_punctuation`（末尾直接删除不留空格，区别于全文剥离的换空格）。阈值 `SHORT_TEXT_UNIT_THRESHOLD=5`，仅开关开启时生效 ｜ 2026-08-08 |
| **成对符号不再被剥（030-A-2）** | `strip_trailing_punctuation` 原用全量 `PUNCT_CHARS`，会把 `（笑）`剥成`（笑`、`他说“好”`剥成`他说“好`，留下孤儿左半。拆出 `TRAILING_PUNCT_CHARS` 只含句末终结标点 ｜ 2026-08-08 |
| **Qwen3 `native_punctuated` 由假设改为实测（DEC-047）** | 原硬编码 `true`。官方 API 确无标点参数（源头关不掉成立），但「模型必定输出标点」是假设——假设为假时标点引擎被跳过，**用户开着开关却拿不到标点**。改用 `has_effective_punctuation`：对 `PUNCT_CHARS` 全集合统一「词内嵌豁免」，`3.14`／`don't`／`3:30`／`example.com` 不计为标点 ｜ 2026-08-08 |
| **「万一」→`0.1万` 修复（031）** | Gavin 端测。`itn.rs` 万/亿分支缺「大单位前必须有数字」守卫，`digit=0` 照常结算产出零系数锚点 `('万',0)`，末尾单字再命中隐式千位。百/千分支不设锚点故未暴露——是 027-E 锚点机制把洞暴露出来。修法走机制层不走词表（DEC-038），`亿万`/`百万`/`千万`/`万万没想到` 等同族一并覆盖 ｜ 2026-08-08 |

**累积**：`src/punctuation/mod.rs` +436 ｜ `src/llm/mod.rs` +341 ｜ `src/main.rs` +33 ｜ `src/transcription/mod.rs` +95 ｜ `src/itn.rs` +58

---

### BUILD-013 的两处方法论（2026-08-04）

1. **崩溃遗留的反向判定**：Gavin 问「上次会话是否有测试或出包任务未完成」，主控用 `[SESSION-CRASH-RECOVERY-001]` 三件套取证——sha256 两副本一致（构建发生过）**但 mtime 链不通过**（产物 `17:42` ＜ `src/itn.rs` `23:47`，差 6 小时）→ 判定 026 代码已落地、**测试与出包均未做**，与 08-03 那次「做了没记」相反。此前 todo/handoffs 均无 026 条目，只有 CHANGELOG/decisions/logs 有 —— **单看文档会误判为无遗留**。
2. **纯逻辑改动的出包验收不能套字符串探针**：026 只改两个函数、**零新增运行时字符串**，正向探针必然搜不到。改用 **mtime 链（决定性）+ sha256 必须异于上一版（证明真的重编译）** 为主判据，旧批次探针仅用于证明本次构建没冲掉 023 成果。

### 主控验收查出、Worker 汇报未覆盖的两项

1. ✅ **018 只修了一半 → 新开 PROMPT-ARCH-020（已闭环）**：缺陷由**出包后的反向探针**查出（`already contains normalized numbers` 在 BUILD-010 的新 exe 里仍 =1），**非测试、非 Worker 自报** —— 749 条测试全绿也没抓到，因为翻译路径常量**无任何等价断言**。修复随 021 合批落地，TEST-SYNC-022 补 5 条对称断言，BUILD-011/012 反向探针 =0。**方法论固化**：反向探针（「本该消失的字符串是否真的消失」）对「修改不彻底」类缺陷比正向探针更有判别力，已列为出包验收固定项
2. ⚠️ **exe 产物记录长期失准**：`todo.md` 顶部挂着「014/015 未进 exe」的过时结论，实际 08-02 01:35 已出过一次包含 014/015/016 的产物但未回写文档。已订正并加入纪律：**出包后立即回写产物时间戳**

### ✅ LLM 连接池僵尸连接修复（LLM-CONN-POOL-028，2026-08-08，未出包）

> 触发：Gavin 端测发现 LLM 优化间歇性 0ms 失败（请求未上网络即挂）。非 ITN，平台中立批次。

**根因**：`reqwest::Client::builder()` 只设 `connect_timeout`，吃全局默认 `pool_idle_timeout=90s`；DeepSeek 服务端 keep-alive 约 60s 关连接 → 60-90s 窗口内池中残留「服务端已关、客户端以为活着」的死连接，复用即失败。实测失败时间点 62.5/67.9/72.3/72.8s（<60s 与 >98s 成功）与该窗口假说吻合——**这是「默认值比服务端行为慢」类缺陷，单测天然测不出，只能靠真实网络规模暴露**。

**修复（`src/llm/mod.rs` + `src-tauri/src/llm.rs` 镜像）**：
1. `POOL_IDLE_TIMEOUT=30s`（新增具名常量，必须 < 服务端 ~60s keep-alive 留足余量）+ builder `.pool_idle_timeout()`——空闲超过 30s 的连接由池主动关闭，不再进复用窗口
2. 重试判据放宽 `e.is_connect() || e.is_timeout()` → `+ e.is_request()`——`is_request()` = reqwest `Kind::Request` 桶（client.rs 构造点 150/156/3071），覆盖**连接复用失败**这一类曾经漏判的错误；body→`Kind::Body`、decode→`Kind::Decode`、builder→`Kind::Builder`、status→`Kind::Status` 各自独立桶，主控担忧的「覆盖过宽」经 reqwest-0.12.28 error.rs 源码核证不成立
3. 新增 `fmt_error_chain`（逐层 `Error::source()` 展开），三处错误日志改用完整链路——解决 reqwest `Display` 只输出最外层错误、底层 hyper 连接关闭原因被吞导致的排障盲区

**决策**：采纳主控 `is_request()` 方案，不选 source-chain 备选（hyper-util legacy `ChannelClosed/SendRequest` 包装致链接判定在不同 hyper 版本下不稳定，error.rs Kind 桶在 reqwest 层稳定）。

**验证**：cargo fmt clean / cargo check 0err（13.5s，pre-existing warnings）/ src-tauri check 0err（33.56s）/ `llm::` 131/0 / src-tauri 53/0。**仅改两文件**，现有值 CONNECT_TIMEOUT/ATTEMPT_TIMEOUTS/MAX_ATTEMPTS 未动；未出包。

---

## 🔄 v0.8.0 · 在线 ASR 引擎更替 + 流式上屏（2026-08-14 起，**进行中**）

> **版本号 0.7.3 → 0.8.0**（Gavin 2026-08-14 明确指示「升级版本号」+「版本号就按照你的建议来」）。
> 定为 minor bump 的理由：替换核心在线 ASR 引擎（协议族更换）+ 流式管线 + VAD 计费门控
> + 词库热词注入 + overlay 流式预览与编辑态 —— 引擎级更替，非 patch 级修复。
> 三处已改：`Cargo.toml:3` / `src-tauri/Cargo.toml:3` / `src-tauri/tauri.conf.json:9`；
> `ui/package.json`（0.1.0）与产品版本号独立，未动。

### 一、设计阶段 ✅ 已闭环（四份文档，全部主控验收通过且互相对齐）

| 任务 | 文档 | 核心结论 |
| --- | --- | --- |
| RESEARCH-ASR-035 | `research/asr-qwen-audio-3.0-integration-001.md`（720 行） | 协议/能力/商务三层 14 问全答；**§4 整合方案的「录完再发」前提已被 Gavin 推翻**，由 038 取代，事实部分仍有效 |
| RESEARCH-TSF-036 | `research/tsf-composition-feasibility-001.md`（25039 B） | TSF 组合文本**判死** → **DEC-050** |
| DESIGN-OVERLAY-037 | `research/overlay-streaming-preview-design-001.md`（28446 B） | 复用 Win32 GDI overlay（DEC-003）；双阶段窗口样式；Win32 `EDIT` 子类化去边框（中文 IME 支持为决定性因素），规格 `(42,10)-(191,26)` |
| RESEARCH-ASR-038 | `research/asr-streaming-pipeline-design-001.md`（533 行 / 30388 B） | 真流式管线 + VAD 三层门控（只做①入口）+ 热词注入链路 |

**主控 037/038 交叉复核**：三个高风险接口点（推送频率／窗口抖动防护／中间结果覆盖／取消信号）**双向对齐**。
唯一轻微措辞不一致已记录：节流由消费端 16ms timer + 100ms 尺寸节流两道保证，**推送端不设限**。

**关键拍板**（详见 `logs/20260814.md` 第四节汇总表）：上屏走 overlay 否决 TIP（DEC-050）｜
PTT 录音中点击即进编辑态（不等松键）｜流式文本白色不加下划线｜`language_hints` 固定 `[zh,en,ja,ko]`｜
热词 user=5／system=4 且 **`wordbook_candidates` 禁止注入**｜ITN 保护词表不注入 ASR｜
`speech_detected` RMS 与 Silero VAD 并存不替换｜本地降噪暂不做｜`context` v1 不上。

### 二、实施阶段 🔄 进行中

| 批次 | 内容 | 文件域 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| **TRANS-HOTKEY-039** | 翻译热键全链失效修复（VK_TO_LABEL Shift 标签修正 + 轮询跟随录音生命周期 + 硬上限兜底 + flag 日志 + 6 处终止路径通知） | `ui/src/pages/HotkeySettings.tsx`、`src/platform/windows/hotkey.rs`、跨 `src/main.rs` 6 处调用 | coder-2 | ✅ 已验证 |
| **TRANS-HOTKEY-039-D** | 抽判据纯函数补真回归护栏 + 清理死常量 | `src/platform/windows/hotkey.rs` | coder-1 | ✅ 已验证（should_stop_translate_poll_on_keyup + hotkey_mode_to_u32 纯函数 + 删 TRANSLATE_WINDOW_MS + 4 真护栏测试） |
| **ASR-038-A** | `qwen_inference.rs` 新建（流式协议 + 二进制帧 + 热词 + 四语） | `src/transcription/` 新文件 | coder-1 | ✅ **主控独立复现验收通过** |
| **ASR-038-B** | VAD 入口门控 + 管线改造（边录边发 + 增量接收） | `src/audio/`、`src/main.rs`、`src/transcription/`、`src/ui/overlay.rs`（仅数据字段） | coder-1 | 🟡 **真流式核心已实施**（C-1 VadSegmenter滚动方法 + C-2 record_streaming + C-3 transcribe_streaming_realtime + C-4 worker接线 + 040-A埋点）。编译已通 / 949+32测试全绿。待主控终验（重点验FIRSTCHAR等价+record零改动+040-A四段覆盖） |
| ASR-038-C | overlay 流式显示 + 编辑态 + EDIT 控件 | `src/main.rs`、`src/ui/overlay.rs`（绘制与交互） | coder-2 | 🔜 等 B（**同动 `src/main.rs`，零并行空间**） |
| TEST-SYNC-038 | 测试同步（阶段三） | 各 `mod tests` | tester-1 | ✅ **已验收**（返工一轮）：10 用例 + 039-D 纯函数护栏 |
| TEST-SYNC-038-B | main.rs 规格表 + 038-C 覆盖点 + 041/041-B 用例补全 | 3 测试文件 | tester-1 | ✅ **已复算通过**（主控）：新增 6 用例，生产零改动，038-C 判不可纯单测（建议抽纯函数） |
| TEST-EXEC-038 | 全量回归（阶段四） | — | tester-1 | ✅ **已复算通过**（主控）：A0-A4 全 PASS，1014/55/54，红条三分类 ③0/①0/②0，待主控提交 |
| BUILD-016 | v0.8.0 首包（阶段五） | — | tester-1 | 🔜 |
| **ASR-041** | 在线 ASR 模型换代 UI 选项 + 存量配置静默迁移 | `ui/src/pages/Voice.tsx`、`ui/src/i18n/*.ts`、`ui/src/pages/Voice.test.tsx`、`src/config/mod.rs` | coder-1 | ✅ 已验证（UI 下拉指向 qwen_audio_online + 三份 i18n 去 Qwen3 字样 + 存量 qwen3_online 静默迁移 + 17 处测试同步） |
| **ASR-041-B** | 清除旧在线 ASR 代码路径 + 字段改名通用名 | `src/transcription/**`、`src/config/mod.rs`、`src/main.rs`、`ui/src/**`、`src-tauri/src/config.rs`、`src-tauri/src/main.rs` | coder-1 | ✅ 已验证（删 Qwen3Online 枚举+qwen3_online.rs(686行)+qwen3_asr 配置+热重载+match 臂+测试；f32_to_pcm16_le 搬家；三字段改通用名+alias；镜像补字段；存量迁移保留） |
| **ASR-042** | 在线流式 ASR 采样率修复（StreamingResampler） | `src/audio/mod.rs` | coder-1 | ✅ 已验证（48kHz→16kHz 流式重采样；数学等价于 resample_anti_alias；pre-roll→post-hotkey→主循环喂同一实例+finish()；+6 测试含改坏会红自证；record()/resample_anti_alias/vad.rs 零改动） |
| **ASR-045** | 流式识别结果被管线丢弃修复（P0） | `src/main.rs` | coder-1 | ✅ 已验收已提交 `54cde62` |
| **TEST-SYNC-045** | 阶段三测试同步：ASR-045 护栏缺口 | `src/main.rs` mod tests | tester-1 | ✅ 已验收已提交 `fd994a5` |
| **TEST-EXEC-042/045** | 阶段四全量回归 + 消融自证 | — | tester-1 | ✅ 已验收已提交 `2a173f0` |
| **BUILD-017** | v0.8.0 第二包出包（ASR-042 + ASR-045 进 exe） | — | tester-1 | ✅ 已验收（产物 08-16 23:25，七项核验主控独立复算通过） |
| **OVERLAY-043** | 录音悬浮层五项显示与流畅度修复 | `src/main.rs` | coder-2 | ✅ 已验收已提交 `a588509` |
| **OVERLAY-043-B** | 抽 `interpolate_step` + `should_ignore_streaming_text` 纯函数补护栏 | `src/main.rs` | coder-2 | ✅ 已验收已提交 `5940e73` |
| **TEST-SYNC-043** | 阶段三测试同步：两纯函数护栏 + 不可测项如实说明 | `src/main.rs` mod tests | tester-1 | ✅ 已验收已提交 `497131f` |
| **TEST-EXEC-043** | 阶段四全量回归 + 消融实测（OVERLAY-043 批） | — | tester-1 | ✅ 已验收已提交 `b499cc3` |
| **BUILD-018** | 阶段五出包：OVERLAY-043 全批进 exe | — | tester-1 | ✅ 已验收（产物 08-17 13:57-14:00，七项核验主控独立复算通过） |
| **OVERLAY-046** | 修复录音 overlay 窗口从未被定位/定尺寸（P0 阻塞日常使用，OVERLAY-043 真回归） | `src/main.rs` | coder-2 | ✅ 已验收已提交（BUILD-019 内） |
| **HOTKEY-047** | 设置 UI 热键录制重做（P0）：焦点竞态 + 左修饰键可单设 + 任意组合键 + AltGr 合成 Ctrl 过滤 | `ui/src/pages/HotkeySettings.tsx` + 三份 `ui/src/i18n/*.ts` | coder-1 | ✅ 已验收已提交（BUILD-019 内，TEST-EXEC-046/047 全绿） |
| **BUILD-019** | 阶段五出包：OVERLAY-046 + HOTKEY-047 + HOTKEY-048 进 exe + 首次真跑 E2E 门禁 | — | tester-1 | ✅ 已验收（产物 08-17 15:45-16:18，七项核验全 PASS；E2E 首跑 10 FAIL+4 error 全 harness/环境缺陷无真回归，见 troubleshooting [E2E-CONFIG-PATH-STALE-001]） |
| **HOTKEY-047** | 设置 UI 热键录制重做（P0）：焦点竞态 + 左修饰键可单设 + 任意组合键 + AltGr 合成 Ctrl 过滤 | `ui/src/pages/HotkeySettings.tsx` + 三份 `ui/src/i18n/*.ts` | coder-1 | ✅ 已验收已提交（BUILD-019 内，TEST-EXEC-046/047 全绿） |
| **BUILD-019** | 阶段五出包：OVERLAY-046 + HOTKEY-047 + HOTKEY-048 进 exe + 首次真跑 E2E 门禁 | — | tester-1 | ✅ 已验收（产物 08-17 15:45-16:18，七项核验全 PASS；E2E 首跑 10 FAIL+4 error 全 harness/环境缺陷无真回归，见 troubleshooting [E2E-CONFIG-PATH-STALE-001]） |
| **BUILD-020** | 阶段五出包：v0.8.0 第五包（HOTKEY-049 + OVERLAY-051 七项 + OVERLAY-051-A/H + WORDBOOK-053 A+B + 测试文档 6 提交进 exe） | — | tester-1 | ✅ 已出已验收自有核验（产物 08-17 19:34-19:37，七项核验全 PASS：sha `5e2bc716…`/`225aaeb8…`/`1af60bf0…` 全异于 BUILD-019、新 JS `index-B5q249eW.js`、ProductVersion 0.8.0 未变、冒烟零 panic；E2E 9FAIL/54PASS/32SKIP BLOCKED 无新增失败类型；端测清单 14 项列于 result.md；push 待主控） |
| **HOTKEY-049** | 翻译热键与录音热键重复检测 + 拦截（按键集合交集判据，语音侧+翻译侧双向拦截，新弹窗无放行出口） | `ui/src/pages/HotkeySettings.tsx` + 三份 `ui/src/i18n/*.ts` | coder-1 | 🟡 **阶段一完成待验收** |
| **OVERLAY-051-A/H** | EDIT 子类化转发修复（CallWindowProcW 替代 DefWindowProcW）+ 编辑态横向滚动（去 ES_MULTILINE 改单行） | `src/main.rs` | coder-1 | 🟡 **阶段一完成待验收** |
| **ASR-055** | 配置 UI 测试按钮 100% 失败修复（Inference 协议重写，model 在 payload，三类错误信息） | `src-tauri/src/qwen3.rs` `src-tauri/src/main.rs` | coder-1 | 🟡 **阶段一完成待验收** |
| **WORDBOOK-053-C** | 词库候选词有效性校验（is_valid_candidate 7 条规则，log::debug! 拒绝） | `src/wordbook/mod.rs` | coder-1 | 🟡 **阶段一完成待验收** |
| **WORDBOOK-053-D** | 脏数据排查（只读报告，12 条脏候选已报告，未删除） | — | coder-1 | ✅ **报告完成** |
| **OVERLAY-051-G** | 抖动缓冲（打字机效果）：服务端到达节奏与屏幕显示节奏解耦，16ms 循环推进游标，速率自适应（积压摊 1000ms 每字 180-350ms），四个时机排空，三个边界处理（撤回/新句/积压） | `src/main.rs` | coder-1 | 🟡 **阶段一完成待验收** |

**038-A 验收取证**（主控独立复现，未采信报告）：`cargo fmt --check` clean ｜ `cargo check --all-targets` 0 error（当时）
｜源码 `#[test]` 计数 **45** 与报告一致 ｜文件域零越界。四处硬红线全部落实：`"model"` 在 payload 内 ｜
`AUDIO_CHUNK_BYTES = 3200` ｜ `ASR_LANGUAGE_HINTS = &["zh","en","ja","ko"]` ｜ `vocab_weight` user=>5／system=>4
**且自加 `_ => 3` 保守兜底**（主控未要求，加得对）。

### 三、🔴 当前断点（2026-08-15 主控 `cargo check` 实跑取证）

```
src\transcription\mod.rs:301:52: error[E0425]:
  cannot find function `load_wordbook_vocabulary` in module `crate::transcription`
error: could not compile `voice-ime` (bin "feiyin-ime") due to 1 previous error; 10 warnings
error: could not compile `voice-ime` (bin "feiyin-ime" test) due to 1 previous error; 14 warnings
```

**唯一 1 个 error**，14 个 warning 全为既有 unused variable（`itn.rs:2013`／`punctuation/mod.rs:284`／
`main.rs:4305,4559` 等），非本批引入、非阻塞。

**根因**：`mod tests` 在 `src/transcription/mod.rs:838` 开、`:1570` 闭（直到文件末尾），
`pub fn load_wordbook_vocabulary()`（`:1314–1354` 含文档注释）**被插在 `mod tests` 内部** ——
虽写在第 0 列看着像顶层，词法上仍属测试模块，正式构建不可见。
**修法**：移到 `:837` 的 `#[cfg(test)]` 之前。一处改动。

> ⚠️ **教训（已记）**：上次会话主控靠**缩进目测**判断该函数在顶层 → 判断错误。
> **模块归属以编译器 `help` 输出为准，不要靠缩进目测。**

### 四、038-B 一条硬性验收项主控已提前代验通过

要求「贴出调用链证明词库数据源为 `wordbook` 表」，主控已自行追到 SQL 层：

```
load_wordbook_vocabulary()
  → Wordbook::list_all()      (src/wordbook/mod.rs:41)
  → db::load_word_entries()   (src/wordbook/db.rs:63)
  → SELECT id, word, source, created_at FROM wordbook ORDER BY id DESC
```

**只查 `wordbook` 表，完全不碰 `wordbook_candidates`（163 条未确认候选）。** ✅ 红线守住。

**剩余待 Worker 自证**：VAD 门控最坏情况不吞字 —— pre-roll 须自证「热键按下瞬间即开口」不丢音频；
**VAD 模型缺失时降级为总是建连**（宁可多花钱不可吞字）。

### 五、提交与遗留

| commit | 内容 |
| --- | --- |
| `4ba4933` | DEC-050 流式上屏走 overlay，否决 TSF 组合文本 TIP 路线 |
| `01e9c2f` | 版本号 0.7.3 → 0.8.0 + 新 ASR 架构三份设计收敛 |
| `003ba65` | wip(asr)：ASR-038-A 验收通过 + 038-B 进行中快照（🔴 **当前不可编译**） |
| `2447dbb` | 补提交 `Cargo.lock`（版本号联动）+ CHANGELOG 增补 ← **当前 HEAD** |

**本地 ahead 2 未 push**（push 需 Gavin 明确指示）。工作区干净。

**待端测顺手确认的低成本项**：计费口径（`usage.duration` 是「上传音频时长」还是「墙钟会话时长」）
文档未覆盖，038 已按保守假设（墙钟）设计。`-debug` 跑一次录音对比即可，无需单独开单。

**跨端**：`docs/MACOS-HANDOFF.md` §ASR-038-A 已追加，平台中立模块 macOS 无需同步改动。

---

## v0.8.1 · 在线 ASR 双模型并存与首字提速（2026-08-18 ~ 08-19，补录）

> 🔴 **本段为 2026-08-30 主控补录**。原因：v0.8.1 的两项功能级改动完成时
> 只更新了 CHANGELOG 与 logs，progress.md 从未建段，属 `[DOC-STATE-DRIFT-001]` 复现。
> 补录内容全部以 git 提交正文为准，非记忆。

| 功能 | 说明 | 日期 |
| --- | --- | --- |
| ASR-056 在线 ASR 双模型并存 | 集成 `fun-asr-realtime`，与既有 `qwen-audio` 族**并存可切换**（设置→语音输入→ASR 模型下拉，不需重启）。新增隐藏字段 `asr_online_max_sentence_silence`（默认 800ms）供独立 A/B。提交 `bde893e` | 2026-08-18 |
| ASR-058 首字提速 143ms | ① **建连从「VAD 命中后」提前到「录音开始即建连」**（实测收益 108ms，6 次 108-117ms 稳定）：建连期间 `chunk_rx` 积攒音频不丢，VAD 命中时连接通常已就绪；四条硬约束全遵守——命中前零音频发送／未说话松手时连接优雅关闭／2s 安全网保留／🔴 只在本次录音内提前建连不跨录音复用（040-C 雷区：空闲连接被服务端静默杀掉）。② **task-started 往返 35ms 移出关键路径**（建连提前后自然被吸收，如实说明是「被吸收」非「并行编码」）。③ 新增 `[ASR-SUMMARY]` A/B 对比埋点：每次录音一行 12 字段汇总（model/outcome/vad_hit_ms/connect_ms/task_started_ms/first_audio_byte_ms/first_partial_ms/first_text_ms/final_ms/words_total/chars_total），🔴 口径红线写死在注释——`first_text_ms` 起点是「首个音频字节发出」而非热键按下，否则用户反应时间会被算到服务端头上，两个模型无法公平比较。方案来自 RESEARCH-ASR-057（coder-1 与主控双路独立研究，结论收敛后实施）。提交 `710cec9` | 2026-08-19 |

**v0.8.1 遗留**：ASR-058 最有价值的两项改动（建连提前、15 个退出点汇总行接线）
**无纯函数可测，阶段四也测不到**，只能靠端测 `[ASR-SUMMARY]` 日志验证。
截至 2026-08-30 Gavin 尚未就该埋点回报数据。

---

## v0.9.0 · 版本号先行升级（2026-08-30，里程碑起点，功能待入）

> **版本号 0.8.1 → 0.9.0**（Gavin 2026-08-30 明确指示「升级版本到 v0.9.0」）。
> 定为 minor bump 的理由：Gavin 已提交 13 项端测问题（overlay 视觉 6 项 + ASR 2 项 P0 + 热键 1 项 + ITN/格式化 2 项 + 交互优化 2 项）构成独立里程碑批次，版本号先行升级，后续本批所有改动归入 v0.9.0。
> 三处已改：`Cargo.toml:3` / `src-tauri/Cargo.toml:3` / `src-tauri/tauri.conf.json:9`；
> `Cargo.lock:5893` / `src-tauri/Cargo.lock:934` 由 cargo 自动同步；
> `ui/package.json`（0.1.0）与产品版本号独立，未动。

| 功能 | 说明 |
| --- | --- |
| VERSION-059 版本号升级 | 仅改版本号零功能改动。3 手改 + 2 lock 自动写回。全仓 grep 确认无第 4 处产品版本号漏列；macOS `Info.plist` 占位由 `build-macos.sh` PlistBuddy 动态覆盖非手改项。验收 6 条全过：fmt clean / 主 crate check 0 error / src-tauri check 0 error / diff 仅 5 文件 / 旧版本号 0.8.1 清零 / 新版本号 0.9.0 恰好 5 处 ｜ 2026-08-30 |
| REFACTOR-088/089 内联公式抽纯函数 | coder-2。**起因是 tester-1 拒写假护栏**：它在 TEST-SYNC-087 交付前自审发现两条同构模拟用例判别力≈0（改生产照样绿），并自己想清楚了原因——075 那批同构有效是因为判据是生产真函数，而这两条的消融对象是**内联分支**，模拟救不了。由此得出通用判据：**看消融对象是不是可被用例真实调用的东西**。主控据此派两个抽取单。**088** 抽 `streaming_scroll_offset(text_width, visible_w)`：滚动公式原内联两份（GDI `:2576` 整数版、D2D `:3564` 浮点版），**重复实现迟早分叉，而 GDI 是兜底路径平时不可见、分叉要等回落那天才炸**（同 SECRET-082 模式表只留一份的道理）。f32/i32 统一走 i32，等价性四环链条（`w` 由整数像素宽转来是整数值 f32 → 两 margin 是 i32 常量 → 差值 `w-91.0` 仍整数 → 屏宽远小于 f32 精确整数上限 2^24 故减法无舍入 → 截断永不发生）与**断裂条件**（margin 改非整数或高 DPI 让窗口宽度带小数则真截断，届时两调用点须一起换 f32 口径）均写入 doc-comment。**089** 抽 `advance_width(current, target)`：含吸附一起抽（coder-2 论证两轴独立、吸附条件只读本轴，故「step w→step h→snap w→snap h」与「(step+snap w)→(step+snap h)」逐位同值；主控独立复算 d==0 / |d|=1 / |d|=2 三边界确认），`interpolate_step` 本体零触碰（TEST-SYNC-043 护栏）。两单均为行为保持的机械抽取，零功能零视觉改动，主控独立 `cargo check` 0 error 验证。**价值**：Gavin 痛点最大的变宽插值（Bug 3）与滚动公式，从此有**真会红**的护栏钉住——本项目反复吃过「先前修复被后来重构静默回退」的亏（OVERLAY-046 即 043 重构中丢失无条件 `SetWindowPos`，直到端测报「录音窗口完全不出现」才发现）｜ 2026-09-04 |
| D2D-P1 流式两态迁移 Direct2D | coder-2。DEC-055 分批灰度第二批（红线 4：禁止八态一次全改）。**共用层三层化**：`with_d2d` 帧封装集中 `BindDC`/`BeginDraw`/`EndDraw`，并补上 P0 缺失的 `D2DERR_RECREATE_TARGET`（0x8899000C）处理——设备丢失（休眠唤醒/驱动更新/RDP 切换）时丢弃缓存资源、本帧回落 GDI、下帧重建；P0 的处理中态**机械包装进同一路径**，故该修复同时覆盖已出包的处理中态（主控独立脚本比对旧 `draw_with` 与新 `draw_processing_primitives` 函数体：归一化后旧 130 行/新 128 行，仅差 `let w`/`let h` 两行——移入包装层当参数，**绘制内容零差异**，Gavin 已确认满意的处理中态视觉不变）。**两态迁移**：`RecordingStreamingIdle` + `RecordingWithText`，拆五原语（chrome / mic 原生 AA 替换 4x HALFTONE 超采样 / stop / placeholder / streaming_text 含 `PushAxisAlignedClip` 滚动裁剪），P2/P3 可直接复用。GDI 兜底全程保留，D2D 任何失败路径都能出帧，overlay 永不空白。**三处 Worker 自主决策经主控核准**：① 流式 text_format 用 Segoe UI + normal，与 GDI `create_clear_type_font` 同 face——**顺带查出 P0 注释错误**（其声称 YaHei UI「must match the GDI path's font face」，实际 GDI 用的是 Segoe UI、weight 也非 SemiBold；P0 未暴露是因处理中态纯中文触发 fallback 恰好掩盖），已改为事实但不动 P0 字体本体（Gavin 已目视确认满意，改则需重新端测）② **全程 GDI 单一度量源**，D2D 侧不引入 DirectWrite 度量，彻底避开双度量漂移导致的窗口宽度与文字宽度失配 ③ 护栏建议含「`show_placeholder=false` 不走 D2D，防 P2 前误迁录音波形态」。**主控验收打回一处**：D2D 路径漏画 GDI 版的右分隔线（文字区与停止键之间，`rect.right-36`、高 20、2px），形成「D2D 常态缺线、回落 GDI 才有线」的回归；已补齐，几何逐项照抄未重新设计。**一处行为变更留端测判断**：超 65% 屏宽时 GDI 的 `DT_END_ELLIPSIS` 省略号在 D2D 无对应 option，主控裁定不补——流式预览语义是「最新文字贴右边缘」而省略号恰画在右端会盖住最新文字，去掉可能更对，但属推断故请 Gavin 判断。验证：fmt clean / check --all-targets 0 error / diff 仅 `src/main.rs`（+512/-33）/ 测试零触碰 / 版本号未动 / OVERLAY-086 四处成果未被回退（`applied_size = state.current_size` 与 `tween_timeline_origin` 均在）。🔴 **视觉效果静态论证不能结案（DEC-055 红线 5），待 Gavin 端测目视确认** ｜ 2026-09-04 |
| OVERLAY-086 端侧三缺陷修复 | coder-2。**Bug1 处理中窗口圆角多余灰线**：真因是 region 传 `None` 走 `CreateRectRgn`（直角，未做圆角）+ D2D `FillRectangle` 直角填充与 radius 16 圆角描边失配，四角留背景楔形（另一会话分析报告归因「GDI RoundRect 1px 笔」不成立，处理中态已迁 D2D）。修法：D2D 背景改 `FillRoundedRectangle` 与描边共用 `corner_radius` 绑定 + 辉光同裁 + GDI 兜底 `FillRgn` + region `Some(16)`。楔形消除，灰线保留（Gavin 选方案 A）。**Bug2 流式文字消失只剩空窗口**：真因是服务端 VAD 预热期成批发空文本包（Gavin log 实测 21 包/170ms，开嗓必现），`qwen_inference.rs` 的 `is_empty` 判断只在 `task-finished` 终局分支、realtime 路径零过滤，空包被包成 `Show(RecordingWithText{text:""})` 顶掉 `RecordingStreamingIdle` 占位窗，且空串先污染 WORDBOOK-053-B 镜像。三层修：① 源头空闸门（一处 `if !display.is_empty()` 同时切断转发与镜像污染）② 渲染层空文本回落 placeholder（不变量：空流式文本永不产出空窗口）③ 中段「冻 1.9s + 爆发追涨」——真因是时间映射两端锚点不一致（墙钟零点锚定一次，时间轴零点每次从当前词表现算），服务端合并词使 `words[0].begin_time` 变小时揭示边界整体后退；新增 `tween_timeline_origin` 双端锚定，`reveal_chars_by_timeline` 加 `Option<i64>` 第 4 参数（`None` 保持旧行为，既有 11 处护栏零改动）。**Bug3 宽度扩展抖动/左移**：真因不是水平居中锚点（Gavin 明确要两边同时扩展，居中是对的），而是变宽从不插值——`if dx > 0 { snap }` + `:1334` 流式 Show 二次 snap，每包瞬跳。修法：变宽改插值、删两处 snap、Show 流式 `SetWindowPos` 用 in-flight `current_size`。**主控打回一次**：`:1249` 居中仍按 `desired_size` 算而尺寸已用 `current_size`，位置与尺寸不同源；另有一句「reveal follows interpolated width」是代码中不存在的承诺。两处已整改，`applied_size` 与 `:1366`、R1 三处同源，注释改为事实。验证：fmt clean / check --all-targets 0 error / diff 仅 2 生产文件 / 测试零触碰 / 版本号未动。**待 Gavin 端测目视确认** ｜ 2026-09-04 |
| OVERLAY-068 位置跳动+闪烁修复 | coder-2 阶段一：流式文字 x 坐标跟随当前渲染宽度而非目标宽度；streaming Show 更新时同步 current_size=target_size，保证每帧只有一处 SetWindowPos。未碰绘制、尺寸、EDIT 控件。验证：fmt clean / check 0 error / tauri check 0 error。待 tester-1 运行时验证。｜ 2026-08-30 |
| OVERLAY-061 左上角闪屏排查 | coder-2 阶段一：交付可见路径×是否已定位对照表；创建时坐标兜底到 (-32000,-32000)。根因待 REPRO-061 逐帧截图实证后再定。｜ 2026-08-30 |
| OVERLAY-064 边框消失根因 | coder-2 阶段一：根因报告，三假设核验。本轮不改绘制（DEC-055 D2D 迁移负责）。｜ 2026-08-30 |
| ASR-070-FIX 松键尾部文字丢失修复 | P0 数据丢失。根因 `final_text()`（qwen_inference.rs:520）只返回 confirmed_sentences 丢弃 current_sentence，依赖未验证假设「finish-task 后服务端发最后 sentence_end=true 清空 current」——Gavin 端测实证不成立。修法 A：final_text 改返回 display_text()（confirmed+current）。改动仅 qwen_inference.rs +27/-4：实现+注释改写、1 条断言更新（逐条推演 5 条仅 :1793 需改）、新增护栏用例（消融改回旧实现必红）。重复计数核查：on_result end=true 时 push confirmed 同时 clear current（原子），无重复风险。fallback 保留。验证 fmt clean/check 0 error ｜ 2026-08-30 |
| HOTKEY-060 热键设置重复拦截修复 | 代码已在 9506eac+e7a6a29 内（主控 `git add -A` 误扫入 ITN 提交，未走验收流程）。finalizedRef 两侧拆分（voice/translation 各自独立）+ reset 拆分 + 冲突检测收敛为共用 helper `applyHotkeyIfNoDupConflict` + 删除死常量 TRANSLATION_SINGLE_KEYS（死代码等价）。主控补审：设计合规/tsc 0 error。2026-09-03 coder-2 补账（零代码）｜ 2026-08-30 实现 / 09-03 补账 |
| OVERLAY-075 跨 session 流式渗漏隔离 | 布尔门闩表达不了会话身份：A finalize 拖尾期间开 B → 门闩重置 → A 迟到包与 B 的 Recording 交替 Show（拉锯）+ A 旧文字渲染进 B 窗口。修法：会话代际——StreamingText 加 u64 代际字段 + STREAMING_GENERATION AtomicU64（Start bump → ASR 闭包捕获 → 每包加戳 → 消费闸门不匹配丢弃，先于词库镜像）；STREAMING_STOPPED 未动（两闸正交：跨 session 身份 × 本 session 松手后）。顺带修复 macOS 三处 1 字段签名存量破损（051-G 遗留，macOS 此前必编不过）。fmt clean / check 0 error / diff 仅 main.rs +44/-5｜ 2026-09-03 |
| ITN-071 三五成群补词+死代码化简 | 三五成群不在任何保护集，补进 itn-rules.toml [protect.idioms]，三副本同步+护栏2条（消融删词变红）。一点半点挂起（代码层面应保护，等 Gavin 实际输入输出定方向）。qwen_inference.rs:1404-1413 死代码化简（ASR-070 后不可达 fallback 删掉保留 bail）。FMT-072 取证挂起（时间线排查三结论存档，等 Gavin 用例+API 验证）。改动：itn-rules.toml +1、src/itn.rs +17、qwen_inference.rs +4/-6 ｜ 2026-08-30 |
| ITN-071-B 时间语境绕过成语保护修复 | Gavin 实测：一点半点→1点半点、一点点→1点点。产出源全表：主循环10条路径均受 check_protection 前置门控。根因：一点点不在保护集→is_date_suffix("点")误转；一点半点防御性挪到 idioms 最高优先级。修法：一点半点 unit_collisions→idioms、一点点新增到 function_words。一点半绝不加保护表。回归4条全过。护栏4条。改动 itn-rules.toml+3/-2、src/itn.rs+40 ｜ 2026-08-30 |
| ASR-074 音频上行背压丢帧根治 | P0。循环转速 65.2Hz<100Hz 丢 351 chunks=3.51s、finalize 拖尾 4.05s。修法三段：2-A 每轮排空 chunk_rx 最多 16 chunks 合并一次 send；2-B 松手前 drain warm.rx，**时间上限 500ms 非数量上限**（on_chunk 是阻塞 send，无上限则 ASR 故障传导到录音线程=松手卡死）；2-C 三处 try_send 静默丢帧改计数+warn。永久埋点 [ASR-LOOP]/[ASR-BACKLOG]/[ASR-DROP] + read/send 分别 p95。顺带修三个埋点缺陷。改动 audio/mod.rs + qwen_inference.rs ｜ 2026-09-03 |
| ASR-074-GUARD chunk_tx 超时防卡死 | 主控验收 ASR-074 时查出的跨文件域残留风险：drain 的 500ms deadline 在循环头判断，单次 on_chunk→chunk_tx.send() 阻塞即回不去。修法 send_timeout(200ms)（健康消费 20 倍余量），Timeout→[ASR-DROP] warn + 计数，Disconnected 静默。仅 src/main.rs，容量 256 未动 ｜ 2026-09-03 |
| D2D-073-P0 处理中态迁移 Direct2D | DEC-055 九阶段的 P0（依赖+骨架+处理中态）。D2D1_PIXEL_FORMAT 由「本地 repr(C) + mem::transmute」改为直构 crate 类型，删本地结构与两个 i32 常量。过程实证：误用 Dxgi 域常量被 E0308 当场拦截——直构优于 transmute 的活例。软件光栅不改（GPU 加速留后续，动 WM_PAINT 碰 043 红线）｜ 2026-09-03 |
| HOTKEY-078 AltGr 尾随 keyup 串键修复 | tester-1 阶段四报 S12 红并倾向「仅显示问题」，主控逐行 Read 判定为**真生产缺陷且测试断言正确**。根因：AltGr 抬起合成两个 keyup（AltRight→ControlLeft），而抑制旗 `altGrSynthCtrlActiveRef` 在 AltRight keyUp 就清零，第二个 ControlLeft 到达时抑制已失效 → 二次调用 `checkAndApplyVoiceHotkey(0xA2)`；又因该函数是 async、`finalizedRef` 只在 await 后置位，同步到达的第二个 keyup 畅通。available=true 路径被 finalizedRef 掩盖，available=false 路径的 else 分支不置反而清零 finalizedRef → 弹窗把用户按的 Right Alt 说成 Left Ctrl。修法：删掉提前清旗 3 行，旗的唯一清零点回归 `resetVoiceRecordingState()`（会话级生命周期）。diff 精确 -3 代码 +5 注释，测试文件零触碰。主控独立验：`npx tsc --noEmit` 0 error / diff 逐行核对 / 五文档 grep 属实。Part B 取证：**翻译侧同缺陷类未修**（单键+keyDown 即 finalize+全同步，AltGr 首事件 ControlLeft 直接被录成 Left Ctrl，无用例覆盖故未红）→ 已开 HOTKEY-079。同步闸门加固经 coder-2 反论后否决（入口置 finalized 会让 :218-228 catch 回退路径永远写不进热键）｜ 2026-09-03 |
| HOTKEY-079 翻译侧 AltGr 串键修复 | HOTKEY-078 Part B 取证立项（禁止补丁式设计：AltGr 有两个产出源，语音侧修完必须盘翻译侧）。翻译侧只挂 onKeyDown、全组件无 keyUp handler，拿到第一个键就同步 finalize → AltGr 的第一个合成事件 ControlLeft 直接被录成 Left Ctrl。**修法的硬约束**：`getModifierState('AltGraph')` 在合成 ControlLeft 的 keyDown 上仍为 false，那一刻信息量不足以判别，故裁决必须后推，无第二条路。Plan A（只延后 ControlLeft，其余键零改动）—— 新 `translationPendingCtrlRef` 翻译侧独占（HOTKEY-060 红线：两侧状态不复用）+ keyDown 两分支（ControlLeft→挂起不 finalize；AltRight 且 pending→录 0xA5）+ 新 keyUp handler（pending 仍 true 的 ControlLeft keyUp→录 0xA2）+ reset 清 pending 为唯一清零点（HOTKEY-078 同型生命周期陷阱预防）。diff +60/-0 单文件，测试零触碰。主控独立验：`npx tsc --noEmit` 0 error / diff 逐行 / 五文档 grep 属实。🔴 **主控验收更正了 coder-2 行为差异表两行**：Ctrl+F5 与 Ctrl+Right Ctrl 修前都录 Left Ctrl（非「录第二个键、不变」）——修前第一个键 finalize 时 setRecording(false) 会卸载聆听态 div 的监听器，第二个键根本到不了 handler。故实际行为变化三处（ControlLeft 时机／Ctrl+非Ctrl键产出／Ctrl+AltGr 产出），**主控裁定全部接受不返工**（单键语义下「第一个键定局」才反直觉）。另补真实代价：按 Ctrl 后抬起前被夺焦 → onBlur 清 pending → 本次零录入，判定可接受。T5 定案：HOTKEY-078 上单「旗残留误抑制」经逐路径推演**不存在可执行键序**，从「已备案」降级为理论窗口，不写用例 ｜ 2026-09-03 |

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

| v0.8.0 | 2026-08-16 00:48 | 12,106,752B `86176283…` (00:47/Publish 00:48) | 10,026,496B `db095564…`（00:45/00:48） | 24,859,648B `a9c30e4e…`（00:46/00:48） | **1014 PASS / 0 FAIL / 11 IGNORED**（`--list` 1025 自洽）+ src-tauri **55/0/0** + Vitest **54/0/0**（**BUILD-016 · v0.8.0 首包**，2026-08-16 Gavin 下达出包指令；**新 ASR 引擎首次进 exe**；基线 `be76fc1`；Step1 清进程 → Step2 npm build（新 `index-DkzLqu_f.js`）+ Tauri UI release（custom-protocol）→ Step3 主程序 2m40s → Step4 同步 Publish/（三 exe + 两 toml，config.toml 等运行时数据未覆盖）；**七项核验全 PASS** + **🔴 双探针**：正向 `qwen-audio-3.0-asr-flash-streaming`/`api-ws/v1/inference` 各 1 命中、反向 `api-ws/v1/realtime`/`qwen3-asr-flash-realtime` 0 命中（新代码进包 + 旧引擎清干净）；ProductVersion 0.8.0.0/0.8.0/0.8.0.0；版本号未动；冒烟 PID 30276 Responding=True 测后清理）|
| v0.8.0 | 2026-08-17 16:18 | 12,115,456B `e0785397…` (15:48/Publish 16:18) | 10,026,496B `1a2b40c8…`（16:18/16:18） | 24,859,648B `7b499bbf…`（15:47/16:18） | **BUILD-019 · v0.8.0 第四包**（OVERLAY-046 + HOTKEY-047 + HOTKEY-048 进 exe；基线 `b5a96a9`+048 未提交文案）；两轮构建（首轮 048 前全量，二轮 048 后仅 Step2/4，**Step3 跳过**——`git diff -w` Rust 零改动，主程序沿用 15:48 构建）；七项核验全 PASS（sha 两副本逐一相等 + toml 三副本 `0a3a0b9a…`/`b208271b…` 未变 + ProductVersion 0.8.0 + index 探针 `index-CZoCPT7t.js` ≠ BUILD-018 `index-DkzLqu_f.js` + 大小与 BUILD-018 完全一致）；**Step 5 E2E 首次真跑**：50 PASS / 32 SKIP / 10 FAIL / 4 error —— 6 hotkey FAIL 系 harness 缺陷（[E2E-CONFIG-PATH-STALE-001] 决定性实验证产品正常），其余 FAIL/error 均预存 harness/环境缺陷，`tests/` 零 diff 证非本批引入，**无真回归** |
| v0.8.0 | 2026-08-17 19:37 | 12,138,496B `5e2bc716…` (19:37/Publish 19:37) | 10,026,496B `225aaeb8…`（19:34/19:37） | 24,859,648B `1af60bf0…`（19:36/19:37） | **BUILD-020 · v0.8.0 第五包**（HOTKEY-049 + OVERLAY-051 七项 + OVERLAY-051-A/H + WORDBOOK-053 A+B + 测试文档 6 提交进 exe；基线 `16ff1d1`；全量四步：Step1 清进程无 Gavin 自启实例 → Step2 npm build 新 `index-B5q249eW.js` + Tauri UI 1m39s → Step3 主程序 2m08s（Rust 实质改动必跑）→ Step4 同步 Publish 三 exe+两 toml）；七项核验全 PASS（sha 两副本逐一相等且**三条全异于 BUILD-019** + toml 三副本 `0a3a0b9a…`/`b208271b…` 与 BUILD-019 相同=未改 + ProductVersion 0.8.0.0/0.8.0/0.8.0.0 未变 + i18n `hotkey_dup` 5 key 在新 JS 内全命中（exe grep 0 为 Tauri 压缩已知行为）+ 后端 `Streaming resampler active`=1 + feiyin +23KB 合理 / ui 同大小但 sha 变（BUILD-019 教训：大小不作唯一判据））；**Step 5 E2E**：9 FAIL / 54 PASS / 32 SKIP 记 **BLOCKED**，失败项与 TEST-EXEC-049/051/053 逐一相同无新增类型（[E2E-CONFIG-PATH-STALE-001]）；**Gavin 端测清单 14 项列于 result.md 待端测**；push 由主控执行 |

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
| ASR-074-GUARD chunk_tx 超时防卡死 | on_chunk→chunk_tx.send() 无超时阻塞：ASR 线程停死→队满→录音线程不返回→松手卡死（主控验收 ASR-074 发现的跨文件域残留风险）。send→send_timeout(200ms)+Timeout warn 计数（[ASR-DROP]，Step 2-C 可见性原则）+Disconnected 静默。仅 main.rs 单点。fmt clean/check 0 error｜ 2026-09-03 |
| v0.9.0 | 2026-09-04 14:23 | 12,217,856B `967539a6…` (14:23/Publish 14:23) | 10,053,632B `f1b47399…`（14:21 src-tauri / 14:23 Publish） | 24,878,088B `12111035…`（14:23/Publish 14:23） | **BUILD-085 · v0.9.0 阶段五出包**（BUILD-022 后 17 天累积改动进 exe：ASR-074+GUARD 音频背压根治 / OVERLAY-075 代际隔离 / D2D-073-P0 处理中态 / OVERLAY-068+ASR-070+ITN-071(+B) / HOTKEY-078+079 AltGr 前端修复 / VERSION-059；基线 `d368c6c`；全量四步：npm 1.46s 新 `index-CJ1JUYoT.js` + Tauri UI 2m03s custom-protocol + 主程序 2m42s + Step4 同步 Publish 三 exe+两 toml；**七项核验全 PASS**：时间戳 09-04 / sha256 三对相等 / toml 三副本 `0a3a0b9a…`/`311cbb96…` 一致 / **ProductVersion 0.9.0.0/0.9.0/0.9.0.0**（VERSION-059 进包） / 探针 ASR-DROP×4+ASR-LOOP×1 / 大小同量级（main +29KB D2D+埋点+闸门预期内）/ 冒烟 PID 25288 Responding 无 panic；运行时数据零覆盖（config.toml sha `3186ec8c…` 零变化）；**Step 5 E2E 门禁**：65 PASS / 0 FAIL / 33 SKIP（175s）与 BUILD-022 65/0 逐位一致，config.toml 字节级备份比对无污染；version_check.json mtime 变化已定性=程序自写缓存非覆盖）|
| v0.9.0 | 2026-09-05 00:37 | 12,228,608B `e6f55e0a…` (00:35/00:37) | 10,053,632B `7651fd4e…`（00:35 src-tauri / 00:37 Publish） | 24,878,088B `0131ff55…`（00:37/Publish 00:37） | **BUILD-093 · v0.9.0 二包**（BUILD-085 后五批进 exe：OVERLAY-086 端侧三缺陷 / D2D-P1 流式两态迁 Direct2D / REFACTOR-088+089 抽纯函数 / TEST-SYNC-087 护栏；基线 `7163856`；全量四步 npm 654ms（index-CJ1JUYoT.js 与 085 同名=前端零改动）+ Tauri UI 1m43s + 主程序 2m12s + Step4 同步；**七项核验全 PASS**：时间戳 09-05 00:35-00:37 / sha256 三对相等且全异于 BUILD-085 / toml 三副本 `0a3a0b9a…`/`311cbb96…` 一致 / ProductVersion 0.9.0.0 / 探针 **D2D-P1×2 + D2DERR_RECREATE_TARGET×1**（本批新符号）/ 大小同量级（main +6.8KB D2D 增量）/ 冒烟 PID 21680 Responding 无 panic 已清理；运行时数据零覆盖（config.toml sha `3186ec8c…` 零变化）；**Step 5 E2E**：64 PASS / **1 FAIL** / 33 SKIP——FAIL=test_hotkey_toggle_stop（**间歇性**第二次 F9 后 5s 未离 recording，全量 FAIL/单跑 PASS-FAIL-PASS 交替），主控裁定不阻塞出包列为 Gavin 端测重点（toggle 连按两次），config.toml 字节级备份比对无污染）|
| D2D-HANG-095 overlay 线程退出 COM Release 加载器锁死锁根因修复 | REPRO-094 实证 thread_local 析构器在 DLL_THREAD_DETACH（加载器锁下）做最后一次 COM Release 自持锁死锁→shutdown_and_join 永不返回（端侧托盘退出无响应）。修复=mod d2d 新增 release_resources()（take() 就地 drop 保线程体内析构）+ spawn_overlay_thread 闭包尾部调用（覆盖全部 ? 早返回路径）。产出源盘点：仅 overlay 线程生产写入+两条 #[ignore] 测试（移交 tester-1 阶段三）；fmt/check 0 error；src/main.rs +23/-0；v0.9.0 未动未 commit｜ 2026-09-05 |
| D2D-HANG-095-B 释放改 Drop 守卫堵 panic 展开路径 | 095 尾部直调在 panic 展开时被跳过（:1475 unwrap/:3115 expect/Mutex 中毒为真实 panic 点，Cargo 无 panic=abort）→ thread_local 析构器照旧在 DLL_THREAD_DETACH 释放 COM 复发死锁；改 D2dReleaseGuard（Drop impl）守卫声明于 run_overlay_thread 之前；双重借用推理复核成立（同帧局部逆序析构先归还借用）用 borrow_mut；release_resources 本体零改动；fmt/check 0 error｜ 2026-09-05 |
| v0.9.0 | 2026-09-05 13:36 | 12,229,632B `39cca97c…`（13:36/Publish 13:36） | 10,053,632B `5d292c16…` | 24,887,808B `ea83504d…` | **BUILD-098 · v0.9.0 三包**（BUILD-093 后一批：D2D-HANG-095 + 095-B 生产修复 + TEST-SYNC-096 测试；基线 `4970309`；七项核验全过：时间戳 09-05 13:36 / sha256 三对相等且 main 全异于 093 `e6f55e0a…` / toml 三副本 `0a3a0b9a…` 一致 / ProductVersion 0.9.0.0 / **判别探针 `D2D-HANG-095` 计数 1（BUILD-093 旧包实测 0）= 新代码进包硬证据**，对照探针 `D2D-P1` 仍为 2 / 大小同量级（main +1,024B）/ 冒烟无 panic；运行时数据零覆盖 config.toml sha `3186ec8c…`；**第 8 项托盘退出端到端 5/5 干净退出**（每次先触发录音确保 D2D 槽非空，`recording_started=true` ×5，debug.log 4 次 `released in-thread` 佐证）—— 修复前同手法 5 次 4 挂，**D2D-HANG-095 端到端成立**；**Step 5 E2E 61P/5F/33S/6deselected**：5F 全部经主控取证为**非本批引入**，详见 todo.md 与 E2E-GATE-099）|
| OVERLAY-101/102 Bug B 修复+宽度上限 50% | Bug B=!do_it 节流帧 x 沿用 overlay_geometry 默认位（240 居中）+ SetWindowPos 应用 current→到上限后右窜且无帧纠正（句界双 Show 同 ms 触发，主控裁定 coder-2 机制分析正确）；修=新增 centered_x 纯函数统一 5 处（Show 两分支一律 current 现算，插值循环+geometry/adjust 纯重构）；Bug A coder-2 假说被驳回（display 输出误当 raw 二次同错）转待证据；OVERLAY-102 0.65→0.50 分步改；fmt/check 0 error｜ 2026-09-05 |
| SECRET-105 密钥闸门 diff 标记误报+漏报双修 | scan_diff 入口剥行首 +/- 标记（修 +@pytest 装饰器类误报，行号 1:1 不变）+ 文件头排除改白名单 (b/|/dev/null|")（修 ++ 开头内容行整行丢弃的漏报，修前复现/修后必拦）+ hook_diff -c 钉死 a/b 前缀防用户 diff 配置漂移；真阳性7+真阴性7 两钩子全矩阵实测过、f38bc06 事故场景无 SKIP 可 commit；只动 scripts/git-hooks/ 3 文件｜ 2026-09-05 |
| HOTKEY-115 Toggle 停不住双缺陷修复 | 缺陷1=钩子 KEYUP PTT_ACTIVE.store 移进 if 内+DOWN 按模式分支；缺陷2=RegisterHotKey Toggle 加 TOGGLE_ACTIVE 翻转（另开状态量，不复用被 poll 线程自旋消费的 PTT_ACTIVE）；B3 单一收口=notify_translate_poll_stop 内复位（覆盖 Done/Cancelled/FocusLost/Error/FormatFailed/EditRequested/CancelStop/ESC 全路径）+ mic-muted 出口 main.rs:5101 补 1 行（主控扩单③）；B2/B4=install/uninstall/sync_binding 三处归零；互斥三道代码证据；fmt/check 过 warning 持平（stash 对照 111/102）；MACOS-HANDOFF 写明 macOS 缺陷②同源｜ 2026-09-06 |
| v0.9.0 | 2026-09-06 14:56 | 12,262,400B `7c73a681…`（14:56/Publish 14:56） | 10,053,632B `280b0650…`（11:39，**沿用 BUILD-118 未重建**） | 24,887,808B `a6da3cfa…`（14:55，随主程序同批重建） | **BUILD-129 · v0.9.0 出包（首个含 BUG-119「请说话哦..」的包）**（基线 `6d21b3c`；Step 2 跳过前提已复核 ui//src-tauri/ 自 BUILD-118 零 diff；只跑 Step1→Step3（2m13s）→Step4；**七项核验全 PASS**：时间戳 14:56 本次构建 / sha256 两副本三对相等且主程序异于 BUILD-118 `f5414693…`（UI 相同=未重建）/ toml 三副本 `0a3a0b9a…`/`311cbb96…` 一致 / ProductVersion 0.9.0.0 / **判别探针 6 条全命中 + 反向对照 BUILD-118 旧包 0 命中**（请说话哦.. / 請說話喔.. / Please say something / NoSpeechError / NoSpeech / no speech detected）= 新代码进包硬证据 / 大小同量级 main +7,168B（BUG-119 增量）/ 冒烟 Responding=True 无 panic 已清理；运行时数据零覆盖 config.toml sha `3186ec8c…` 零变化，version_check.json 由冒烟启动程序自写（非 Step4 覆盖）；crash-reporter sha 变化=同批重建的正常现象（源码零改动+无时间戳嵌入+大小不变+todo 先例），详见 result.md）|
| v0.9.0 | 2026-09-06 17:55 | 12,263,424B `ac70a990…`（17:55/Publish 17:55） | 10,053,632B `280b0650…`（11:39，**沿用 BUILD-129 未重建**） | 24,887,808B `a8ac8a5e…`（17:54，随主程序同批重建） | **BUILD-135 · v0.9.0 出包（FLICKER-130 + OVERLAY-121 per-pixel alpha 进包）**（基线 `2909397`；ui//src-tauri/ 自 BUILD-129 零 diff → Step2 跳过；只跑 Step1→Step3（1m48s）→Step4；**七项核验全 PASS**：时间戳 17:55 本次构建 / sha 两副本三对相等且主程序异于 BUILD-129 `7c73a681…`（UI 相同=未重建）/ toml 三副本 `0a3a0b9a…`/`311cbb96…` 一致 / ProductVersion 0.9.0.0 / **判别探针 `UpdateLayeredWindow` 新=1 BUILD-129 旧=0 = ULW per-pixel alpha 进包硬证据**（apply_alpha_fixup 等被 release 符号化）/ 大小同量级 main +1,024B / 冒烟 Responding=True 无 panic 已清理；运行时数据零覆盖 config.toml sha `3186ec8c…` 零变化，version_check.json 由冒烟启动程序自写；阶段四 root 1105P/0F/9I + 消融 G1-G7 七条全 RED 还原；出包后 E2E/src-tauri/vitest 按主控指令挪下一批（Gavin 等包 3 小时），详见 result.md）|
| INVESTIGATE-142 编辑态移光标仍闪二次取证（只查不修） | 编辑稳态应用层零重绘触发器静态闭合（图层切换+迟到包双证伪）；根因未定，头号候选=EDIT 子控件重绘+DWM SLWA 重合成（可见性需实测 P1-P5+240fps 录屏）；新发现父窗全窗 BitBlt 无 WS_CLIPCHILDREN 结构性放大器；FLICKER-130 定性多源叠加非判错；产出 drafts/overlay-142-editing-flicker.md，src 零改动无里程碑变化（纯调查）｜ 2026-09-07 |
| OVERLAY-141-IMPL 圆角灰边根治（SDF 解析 alpha + 半径单一来源） | 根因=D2D 经 BindDC alpha 未存活，旧提亮规则把 AA 半透明边缘压成全不透明（1px 描边变 2-3px 硬灰带）。修=①外框半径单一来源 OVERLAY_FRAME_RADIUS_LG/SM + overlay_frame_radius 映射，16 处外框站点全收敛（D2D 8 + GDI 调用 5 + GDI 本地 const 4 + 死代码独立 hunk；draw_submit_button:2977 主控裁定内部元素不动）；②apply_alpha_fixup 重写为每像素圆角矩形 SDF 覆盖率写 alpha（cov==0 全零、否则反预乘还原后按 cov·op 预乘回写），顺带消除纯黑像素透明洞缺陷；③WM_PAINT 锁内与 opacity 同处取半径传入，G3 结构保持。验证 fmt/check 0 error/warnings 111/102 持平/cargo test 1104P/1F（G6=预期红交 TEST-SYNC-143）；SLWA 零代码改动；macOS cfg(windows) 内不适用。只动 src/main.rs｜ 2026-09-07 |
| TEST-SYNC-143 阶段三 OVERLAY-141 护栏换血+半径单一来源 | G6 旧三分支命题退役→SDF alpha 四不变量（含🔴反向钉死旧提亮规则=圆角灰边机器判据）+G8 掩码半径映射单一来源+G9 三类外框绘制调用实参单一来源（rustfmt 折行逐调用收集+总数13+豁免透传）；测试区 +146/-15 生产区 0 字节；fmt/check 0 error warnings 111/102 持平；沙箱预演 3 PASS+消融 10 变异全红+豁免核对；未跑 cargo test（白名单）｜ 2026-09-07 |
| OVERLAY-147-DIAG 端测两问诊断（只查不修） | Part A：SDF 数学实证正确（探针 vs 精确面积覆盖 mean|Δ|≤0.0002，最差 0.041）；H1a 圆心错位推翻（深度偏差 r 无关 ~0.2px，两组逐值相同）；H2 持续重绘被 RecordingStreamingIdle（r16+静态+坏）静态推翻；剩余候选={半径值未测路径,底色}，机制未定，E1/E2 半径翻转+E3 角区像素 dump 待派（E3 顺带验证「BindDC alpha 未存活」前提）。Part B：EDIT 静态证死不可能存活到录音帧 ⇒ 光标=caret 泄漏主假设（零 DestroyCaret，探针 GetGUIThreadInfo().hCaret）；新钉死 HotkeyEvent::Stop 缺 OVERLAY_EDITING 守卫→编辑中停止 Processing 浮层永久卡屏；FLICKER-130 耦合证伪。产出 docs/OVERLAY-147-DIAG.md｜ 2026-09-07 |
| OVERLAY-149 caret 泄漏修复+热键停止臂守卫+SDF 判别探针 | F1=destroy_edit_control 销毁前焦点移出+HideCaret+DestroyCaret（待端测判定）+GetGUIThreadInfo 运行时证据；F2=Stop 臂无条件清 OVERLAY_EDITING（RestoreAndHide 备选推演否决，事件序列含 Case-A/B/C）；E3=fixup 前/后四角 8×8+左缘列 BGRA hex（节流每状态前 3 帧）+E4 DPI（GetDeviceCaps 替代已报备）；fmt/check 0+warnings 111/102+cargo test 1107P/0F 零预期红｜ 2026-09-07 |
| TEST-SYNC-150 阶段三 G10/G11（OVERLAY-149 双修复护栏） | G10 caret 清理顺序（SetFocus/HideCaret/DestroyCaret 三步必须早于 DestroyWindow，销毁后清理=空操作）；G11 Stop 臂清 OVERLAY_EDITING（结构锚 raw 计数=1 唯一，macOS 臂 platform:: 前缀不可命中；sanity=if is_recording.load 结构特征）。沙箱预演消融 4 条全红（含主控追加的「删代码留注释→红」注释喂绿验证、挪序→顺序红）；diff +82/-0 单 hunk 测试区 :12102；fmt/check 0 error/warnings 111/102 持平；macOS 定稿 F2 不可达无需守卫。未跑 test/build｜ 2026-09-07 |
| OVERLAY-153 圆角二次修法：覆盖率乘数化 | 推论验证成立（141 的 a=255·k 覆写把内部区描边 AA 强制拉满，弧段 2-3px 实心灰团=包边灰线伪影本体）。修=apply_alpha_fixup 三分支：cov≤0 归零 / a>0 等比 ×(cov·op) 保原 alpha（反预乘删除，恢复 121 语义）/ a==0 GDI 提亮（未存活世界与 141 逐位同）；两世界安全论证复核成立（nuance：存活世界轮廓坡 cov² 略陡如实报告）。G6 needle 全绿零改动仅注释更新（流程变更授权）。fmt/check 0 error/111-102 持平/test 三跑 1109P/0F｜ 2026-09-07 |
| OVERLAY-155 圆角三次修法 GDI/D2D 结构化+半径对齐 | 唯一肇事点=draw_recording_overlay 波形分支（:3010 无条件 GDI chrome 先画，每帧叠画无 AA 台阶+错位灰双线，SDF cov>0 清不掉）；chrome 移进 D2D 失败分支（Info 同构）+LG 16→10 对齐 Info 几何（可逆）；另七处排查本来就是对；fmt/check 0+warnings 111/102+cargo test 1109P/0F=基线零护栏红；fixup 零字节未动，E3/E4 探针照留｜ 2026-09-07 |
| EDIT-FLICKER-157 编辑态右侧文字闪烁：EDIT 开双缓冲 | 机制复核成立（ES_AUTOHSCROLL 右滚条带擦-画直上屏，左侧不滚不闪）。修=CreateWindowExW 扩展样式加 WS_EX_COMPOSITED（一行+import，+8/-3 双 hunk）；SLWA 子控件兼容性未运行时实测，回退=子类拦 WM_ERASEBKGND 自绘（已注释留痕）。fmt/check 0 error/111-102 持平/test 1109P/0F｜ 2026-09-07 |
| TRAY-ICON-158 托盘菜单项加图标（阶段一） | 新增 src/ui/menu_icons.rs 纯 Rust SSAA 4×4 光栅（齿轮外径0.42S/8齿20°/孔0.15S + 电源环0.30S/线宽0.11S/顶70°缺口，品牌橙#FF6B35，明确不用SDF）；Windows 裸 Win32 菜单走 SetMenuItemInfoW+MIIM_BITMAP+32bpp top-down 预乘 BGRA DIB（SM_CXSMICON/CYSMICON min clamp(16,64)，不用 GetDpiForWindow 避 HiDPI feature 红线；失败静默降级；DeleteObject 在 DestroyMenu 后现建现删）；macOS NSMenuItem setImage 18pt 不设 template（只写不构建，MACOS-HANDOFF 留痕）；12 张预览 PNG 交付 outbox/icons 待主控目视；fmt 0/check 0/warnings 111/102 持平/test 1110P/0F（1109+1 dump）；Cargo.toml 零字节，未 commit 未出包｜ 2026-09-07 |
| TRAY-ICON-158-FIX 齿轮几何返工 | 主控验收打回三缺陷全修（照处方只动 gear_covered+常量）：实心盘 hole0.13S..root0.32S 整片实心 + 6 条梯形齿（齿根半角15°/齿顶半角10° 线性收窄）+ 外径0.44S；电源图标验收通过零改动；12 张 PNG 重 dump（16px 齿轮已清晰可辨）；fmt 0/check 0/warnings 111/102 持平/test 1110P/0F；main.rs/tray.rs 零新增改动，Cargo.toml 零字节｜ 2026-09-07 |
| MIC-PULSE-160 流式窗麦克风声波弧动效 | 左右各一组对称弧（R5.5/8.0，右±40°/左140..220°，同相位同亮度），墙钟 1000ms tri 波（内弧 0.0/外弧滞后 0.30/宽 0.55），电平 gain ≥0.35 满亮、静音零回归（绘制+重绘节奏双层逐位相同）；三产出源全改（D2D clip+DrawEllipse / GDI×2 折线+BG 插值）+ 共享纯函数 main.rs:2882/2898；RecordingWithText/StreamingIdle 重绘加 mic_has_audio or 条件（StreamingEditing 不加/MENU_VISIBLE 原位）；8 帧预览 PNG 交付（临时测试已删）；fmt 0/check 0/111-102 持平/1110P/0F；macOS 不适用记 MACOS-HANDOFF｜ 2026-09-07 |

| v0.9.0 | 2026-09-07 19:40 | 12,290,560B `85c1cf26…`（19:40/Publish 19:40） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `81ce829c…`（19:39，随主程序同批重建） | **BUILD-165 · v0.9.0 出包（FIX-162 + FIX-164 进包，Gavin 端测打回四条针对性修复）**（基线 `e012108`+未 commit FIX-162/164；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m00s → Step4 cp -p 同步 Publish/；**六项核验全 PASS**：时间戳 19:40 本次构建 / sha 两副本相等且主程序异于 BUILD-159 `b3043119…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 29352 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 持平；**回归并行** cargo test 全量 1110P/0F/9I + hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 未复现（归档偶发）；Vitest/E2E Skip；出包语义=修 Gavin 端测第 4/3/2 条，🔴 第 1 条托盘图标未修仅加 GetLastError 日志待 debug.log）|
| FIX-162+164 端测打回四条修复 | Q4 WS_EX_COMPOSITED 回退（编辑态大黑屏，62000bc 逆操作）+ Q3 切模型频谱窗（D1 空闲预热 + D2 1500ms 有界等待，DEC-062 部分修订 DEC-025）+ Q2 声波弧（满亮阈值 0.35→0.10 + 可见地板 0.35）+ Q1 托盘图标🔴未修（仅 GetLastError 日志待 debug.log）；cargo test 1110P/0F；2026-09-07 BUILD-165 进包 ｜ 2026-09-07 |
| v0.9.0 | 2026-09-07 20:33 | 12,291,072B `20b371d4…`（20:33/Publish 20:33） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `637fb484…`（20:31，随主程序同批重建） | **BUILD-168 · v0.9.0 出包（DIAG-166 托盘图标根因修复，四条全覆盖首包）**（基线 `2dc9474` clean；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m08s → Step4 cp -p 同步 Publish/；**七项核验全 PASS**：时间戳 20:33 / sha 两副本相等且主程序异于 BUILD-165 `85c1cf26…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 26760 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 / 🔴 判别探针字节级 PASS（新文案含 expected= 进包、旧文案 rgba_len=…) 形态 0 命中，Rust format! 抹占位符故 regex 探测字面量段）；**回归并行** cargo test 全量 1110P/0F/9I + hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 本轮未出现（连续三轮未复现，维持归档偶发）；Vitest/E2E Skip；出包语义=托盘图标（第 1 条）✅ 已修，四条全覆盖首包；顺带修正 logs BUILD-165 条目乱码）|
| DIAG-166 托盘菜单图标根因修复 | 根因=`create_menu_item_bitmap` 长度守卫 `n.checked_mul(4)` 缺 `size²` 因子（rgba.len()=4·size²，比较值只有 4·size）⇒ 对任何实际尺寸恒 None ⇒ 图标自 TRAY-ICON-158 起从未被创建；工装四实验钉死（DC 对照排除 CreateDIBSection、A 层读回验证 SetMenuItemInfoW 成功、MNS_CHECKORBMP 对照组）；修=守卫改 `size.checked_mul(size).and_then(checked_mul(4))` 含溢出链+expected 字段；cargo test 1110P/0F×3；2026-09-07 BUILD-168 进包 ｜ 2026-09-07 |
| v0.9.0 | 2026-09-07 21:20 | 12,291,584B `3a55ad20…`（21:20/Publish 21:20） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `c31eca9a…`（21:19，随主程序同批重建） | **BUILD-171 · v0.9.0 出包（FLICKER-170 编辑态移光标闪烁）**（基线 `8efbf89` clean；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m06s → Step4 cp -p 同步 Publish/；**七项核验全 PASS**：时间戳 21:20 / sha 两副本相等且主程序异于 BUILD-168 `20b371d4…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 25444 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 / 🔴 判别探针三证 PASS（纯 Win32 改动无新增字符串→源码 grep+时间戳+sha；WS_CLIPCHILDREN grep=4 含 2 注释代码级=2，WM_PRINTCLIENT grep=3 含 1 注释代码级=2）；**回归并行** cargo test 全量 1110P/0F/9I + hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 本轮未出现（连续四轮未复现，维持归档偶发）；Vitest/E2E Skip；出包语义=编辑态移光标闪烁（B' WM_PAINT 单次合成）+ 进出编辑态整块被盖（A WS_CLIPCHILDREN）；🔴 已知取舍如实写入（WM_PRINTCLIENT 下选区反白不渲染，Gavin 已接受））|
| FLICKER-170 编辑态移光标闪烁修复 | B'=EDIT 子类 WM_PAINT 单次合成（BeginPaint→兼容 DC→WM_PRINTCLIENT(PRF_ERASEBKGND|PRF_CLIENT)→HideCaret→BitBlt 仅 rcPaint→ShowCaret；资源失败兜底默认绘制绝不黑块），修现象本体；A=父窗 WS_POPUP→WS_POPUP|WS_CLIPCHILDREN，修 INVESTIGATE-142 放大器（进出编辑态/文字更新整块被盖）；协商 2 轮（主控 B 拦 WM_ERASEBKGND 被否决，ghosting 风险）；工装三坑（caret 成对清理、🔴 WM_PRINTCLIENT 下选区反白不渲染 Gavin 已接受、rcPaint 只提交更新区）；cargo test 1110P/0F；diff src/ 80+/9-；2026-09-07 BUILD-171 进包 ｜ 2026-09-07 |
| v0.9.0 | 2026-09-07 22:07 | 12,291,584B `f698a431…`（22:07/Publish 22:07） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `d2ab645f…`（22:06，随主程序同批重建） | **BUILD-173 · v0.9.0 出包（FIX-172 声波弧动效修复 + 编辑态最右侧闪烁本体）**（基线 `b0b5c66` clean；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m07s → Step4 cp -p 同步 Publish/；**七项核验全 PASS**：时间戳 22:07 / sha 两副本相等且主程序异于 BUILD-171 `3a55ad20…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 15508 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 / 🔴 判别探针三证 PASS（纯 Win32/数值改动无新增字符串→源码 grep+时间戳+sha；WM_SETREDRAW grep=4 含 2 注释代码级=2，amplitude grep=4 含 1 注释代码级=3）；**回归并行** cargo test 全量 1110P/0F/9I + hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 本轮未出现（连续五轮未复现，维持归档偶发）；Vitest/E2E Skip；出包语义=声波弧「一起亮一起灭」（A 地板截断改抬高振幅 FIX-164 回归修复）+ 编辑态最右侧闪烁（B 滚动位块 WM_SETREDRAW 包裹）；🔴 已知取舍如实写入（WM_PRINTCLIENT 下选区反白不渲染，Gavin 已接受））|
| FIX-172 声波弧回归修复+最右侧闪烁根治 | A=mic_pulse_alphas 地板从 .max() 截断改抬高振幅（amplitude=FLOOR+(1-FLOOR)*gain，谷底回 0/内先外后相位/静音早退契约原样），修 Gavin「一起亮一起灭」（FIX-164 的 .max() 把 tri*gain 压平在 0.35 下）；B=滚动位块 WM_SETREDRAW 包裹（工装 E1 实证滚动 1894px 绕过 WM_PAINT 直打屏幕、E2 实证包裹后单次 WM_PAINT 全覆盖；消息尾包裹 WM_KEYDOWN/WM_CHAR/EM_SETSEL 等，WM_MOUSEMOVE 不包，连击失效区自动合并）；cargo test 1110P/0F；diff src/ 46+/3-；2026-09-07 BUILD-173 进包 ｜ 2026-09-07 |
| v0.9.0 | 2026-09-07 23:06 | 12,295,168B `9d60f458…`（23:06/Publish 23:06） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `1f2ee0bf…`（23:05，随主程序同批重建） | **BUILD-177 · v0.9.0 出包（ESC-174 + EDITICON-175/176）**（基线 `0839593` clean；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m02s → Step4 cp -p 同步 Publish/；**七项核验全 PASS**：时间戳 23:06 / sha 两副本相等且主程序异于 BUILD-173 `f698a431…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 29192 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 持平（跨两 Worker+menu_icons.rs 新增）/ 🔴 判别探针三证 PASS（`edit_icon_rgba` 全仓 7 处但定义在 menu_icons.rs:28 非 main.rs——实际绘制引用 bgra 变体 GDI :3780+D2D :4836 三变体全存在；`VK_ESCAPE`=5 恰等；时间戳+sha）；**回归并行** cargo test 全量 1112P/0F/9I（1110+2 新增）+ hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 本轮首跑出现 23F 复跑未复现（单独跑 51P/0F 确认，归因多二进制并行争用 %APPDATA%，前五轮未复现本轮首现⇒计数重新累计）；Vitest/E2E Skip；出包语义=ESC-174 编辑态 ESC 关窗+作废录入 + EDITICON-175/176 铅笔图标+分割线；🔴 已知取舍如实写入（WM_PRINTCLIENT 下选区反白不渲染，Gavin 已接受））|
| ESC-174 编辑态 ESC 取消录入 | CancelRequested 链路自带 OVERLAY_EDITING 收口（比 F2 模式完整），ESC 直接复用该臂无需新路径；语义=关窗（Hide→F1 caret 清理+EDIT 销毁）+作废录入（cancel_signal→ASR 关 WS 丢文本，注入通道 SubmitRequested 不经 cancel）；实施=edit_subclass WM_KEYDOWN 补 VK_ESCAPE 分支（照 Enter 同款父通道取 data，位于 FIX-172-B 包裹块之前）；边界四答全过（非编辑态不经过子类/父窗保留/ESC 与 RETURN 无交叉/包裹前 return）；cargo test 1111P/0F ｜ 2026-09-07 |
| EDITICON-175/176 编辑态铅笔图标+分割线 | 175=edit_icon_rgba 光栅化函数（SSAA 4x4 品牌橙 #FF6B35，对角 45° 尖朝左下铅笔，定稿尖点 ±0.40S/笔杆 0.16S/缺口 1.8px，预览 PNG 交付目视）；176=集成 D2D 主路径（DrawBitmap NEAREST 吃 edit_icon_premultiplied_bgra）+GDI 兜底（CreateDIBSection+AlphaBlend 吃 edit_icon_bgra），像素来源单一化吃 175 已验收像素；图标 18px@(6,9)+分割线 x=30/2px/20px/0x3A3A3C；失败语义 best-effort 跳过不破 G1；闪烁零回归（全在现有 WM_PAINT 单次合成帧内）；cargo test 1112P/0F ｜ 2026-09-07 |
| v0.9.0 | 2026-09-08 00:50 | 12,297,216B `6624cdd1…`（00:50/Publish 00:50） | 10,053,632B `280b0650…`（01:15，**沿用 BUILD-151 未重建**） | 24,887,808B `9cdbd978…`（00:48，随主程序同批重建） | **BUILD-186 · v0.9.0 出包（ESC-178 + EDITFONT-183 + EDITICON-185 定稿）**（基线 `fd527e0` clean；Step2 UI 免重建（git-log 法 f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序 2m10s → Step4 cp -p 同步 Publish/；**七项核验全 PASS**：时间戳 09-08 00:50 / sha 两副本相等且主程序异于 BUILD-177 `9d60f458…`（UI 相同=未重建）/ ProductVersion 0.9.0.0 / 冒烟 PID 25412 Responding=True 无 panic 已清理 / config.toml sha `3186ec8c…` 零变化 / warnings 111/102 持平 / 🔴 **二进制字面量判别探针正反对照 PASS**（`ESC-178:` debug 日志可构造：PRE186 反向 0 命中 / 新包正向 1 命中含 7 个 `ESC-178:` 前缀片段；`OVERLAY_EDIT_FONT_SIZE` 源码 grep=6 ≥5 作字号旁证）；**回归并行** cargo test 全量 1112P/0F/9I + hotkey 51P + ui_guard 2P，🔴 crash_reporter config 批量 FAIL 本轮未出现（已立跟踪项）、asr_074/asr_056 异域偶发也未出现；Vitest/E2E Skip；出包语义=ESC-178 轮询旁路（原消息路由机制未定待 debug 日志）+ EDITFONT-183 编辑框 14→16px + EDITICON-185 图标定稿候选 B；🔴 已知取舍三条如实写入（选区反白不渲染/他窗 ESC 也取消/文字 14→16 跳变，Gavin 均已知情））|
| v0.9.1 | 2026-09-17 15:43 | 12,348,928B `cd2ad3e4…`（15:43/Publish 15:43） | 10,046,976B `33ab3c67…`（15:38 src-tauri → cp → Publish 15:43，**本批 UI 重建**） | 24,878,080B `ce3fd534…`（15:42，随主程序同批重建） | **BUILD-228 · v0.9.1 出包（214 + 215 + 216 + 218 + 220 + 221 + 222 + 225 + 226 + 217 十单进包）**（基线 `65d8303`；Step1 清进程残留 0 → Step2 `npm build` 677ms（`index-DmsT9phP.js` / `index-y2eVZ7cR.css`）+ Tauri `1m53s`（custom-protocol，🔴 `src-tauri/Cargo.lock` 自动 `0.9.0→0.9.1`）+ cp → Step3 主程序 `2m29s` → Step4 同步 `Publish/` 三 exe + `scene/itn-rules.toml` 三副本；**🔴 八项核验逐项全 PASS（`build-test-guide.md §一·五` 首次实战）**：①时间戳=本次构建 ②sha256 两副本相等且**三产物全异于上一包**（`ecfe325a…`/`6b12ddbb…`/`80adbc88…`）③ProductVersion **`0.9.1.0` / `0.9.1` / `0.9.1.0`**（本轮唯一按「必须变红再变绿」例外处理的核验值）④冒烟 PID 24360 `Responding=True`、`crash.json` 不存在；`-debug` 运行 811 行启动链路完整（starting→Online ASR→hotkey hook→tray→config watcher），panic/ERROR **0**，测后残留 **0** ⑤`Publish/config.toml` sha `8b453259…` 与 `wordbook.sqlite` 前后**零变化** ⑥warnings `feiyin-ime` **111**=基线 / `crash-reporter` 9 / `feiyin-ime-ui` 17 ⑦判别探针正反全过（后端 `[AUTOLEARN]`=1、`strip_trailing`=1，反向 `voice_strip_trailing_punct`=0；UI **文件名探针**：新 `index-DmsT9phP.js`=1 / 旧 `index-SzdQI43J.js`=0、新 CSS=1 / 旧=0 —— 🔴 前端字面量 `asr-key-input` 在 exe 内 0 命中属 **Tauri 资源压缩**已知行为（`[BUILD-016]` 既定结论），已用「源码 5 处 + dist JS/CSS 命中 + exe 0」三层证据定性而非跳过本项）⑧`scene-rules.toml` `8ea93bb1…`×3 / `itn-rules.toml` `311cbb96…`×3 **三副本全等**，且**三者大小完全相同**（51011 / 37875）= `[TOML-ALL-NUL-001]`「只比大小检不出」的现场写照；回归依据：`TEST-EXEC-224` cargo test `1262P/0F/15I`（10 target 逐个 0F）+ `TEST-EXEC-227` vitest `100P/11S` + browser `5P`；🔴 端测 4 项（overlay 编辑态剥尾标点 / 真实页 Key placeholder 斜体淡灰+focus 消失 / 繁中 ITN `三點半→3:30` 等三例 / 「API 配置（建议…）」新文案布局）交 Gavin 目视）|
| ESC-178 编辑态 ESC 完全不响应：日志链+轮询旁路 | H1-H5 五假设判定（H2/H3/H4/H5 全证伪，H1 系统路由层未定）；交付一=永久 debug 日志链三处（子类入口/分支执行/消费端收口，一次 ESC 三段定位）；交付二=轮询旁路（StreamingEditing 重绘臂加 FocusLost 同款 GetAsyncKeyState(VK_ESCAPE) 0x0001 转变位检查→发 CancelRequested 走既有收口臂→state.request=None 防重发；零重绘约束达成，检查在 dirty 判定前）；已知取舍=编辑态轮询读物理键态⇒其他窗口按 ESC 也取消（Gavin 已知情）；cargo test 1112P/0F ｜ 2026-09-07 |
| EDITFONT-183 编辑框字号 14→16px | 新增 OVERLAY_EDIT_FONT_SIZE=-16，仅作用于 EDIT 控件链路三处（字体创建/tmHeight 测量/守卫日志），自绘文字各态保持 -14 逐位不变；测量一致性=adjust_overlay_pos_size_for_text 按状态选字号（StreamingEditing -16 / RecordingWithText -14）防自动宽度低估；六问全过（36px 装得下 tmHeight≈21、EDIT 盒 19→23px 在 4..32 内、图标盒/分割线 x 坐标不受影响、护栏保持绿、FIX-172-B 不回归）；cargo test 1112P/0F ｜ 2026-09-07 |
| EDITICON-179~185 编辑图标全链（纸笔→微调→字体路线→撤销→三候选→定稿） | 179=换「笔在纸上书写」（Gavin 端测原话「图标不合适」）；180=笔尖楔形+文本线 2→1；182=字体路线（Segoe MDL2 E70F，环境无关性取舍被 Gavin 作废）；184=撤销字体回手绘+三候选 A/B/C+v3 对照；185=定稿候选 B（铅笔+两条短文本线，Gavin 亲自选定），逐位复刻纪律（右上角有墨+其余三角透明断言）；cargo test 1112P/0F 每批持平 ｜ 2026-09-07 |
- 2026-09-07 DIAG-166（coder-1，交付待验收）：Q1 根因定案=长度守卫缺 size² 因子致图标从未挂载（离线工装四实验实证：A 层存储正常/MNS_CHECKORBMP 无关/CreateDIBSection hdc=None 合法）；守卫修复 +9/-3；1110P/0F×3、warnings 111/102 持平；工装已删证据存档 docs/DIAG-166-TRAY-ICON.md。
- 2026-09-07 FLICKER-170（coder-1，交付待验收）：A+B' 双修落地（父窗 WS_CLIPCHILDREN + EDIT WM_PAINT 单次合成）；选区不渲染风险 Gavin 拍板接受并实证（0px diff）；1110P/0F、warnings 111/102 持平。
- 2026-09-07 FIX-172（coder-1，交付待验收）：声波弧 .max() 回归修复（抬高振幅方案）+ 编辑态最右侧闪烁根治（工装实证滚动绕过 WM_PAINT → SETREDRAW 包裹单次合成）；1110P/0F、warnings 111/102 持平。
- 2026-09-07 ESC-174（coder-1，交付待验收）：编辑态 ESC=取消编辑+作废录入（复用 CancelRequested 收口臂，压制链双顺序安全）；30+/0-；1111P/0F。
- 2026-09-07 ESC-178（coder-1，交付待验收）：ESC 不响应=机制未定（H1 路由层）+ 永久日志链三处 + FocusLost 同款轮询旁路（零重绘约束达成）；1112P/0F、warnings 111/102。
- 2026-09-07 EDITFONT-183（coder-1，交付待验收）：编辑框字号 14→16px（独立常量+测渲一致+36px 装得下论证）；1112P/0F、warnings 111/102。
- 2026-09-17 V091-ITN-FIX-YIKE-216（coder-2，交付待验收）：指示代词 +「一」虚指护栏，根治 `那一刻`→`那1刻`。根因=`decide_conversion:2275` 的 `is_date_suffix` 不分「数量词一 / 指示代词构词成分一」。新增 `is_demonstrative_yi` 三判据（当前字「一」+ 前字指示代词（含 那么/这么，裸「么」不算）+ **后字必须命中时间后缀表**），闸门插在 `check_protection` 后、**甲型前**（`那一年半` 走甲型抢跑，只改 decide_conversion 管不到）。两次纠错：位置必须早于甲型；判据不能用「后字非数字」排除法（否则 `这一块八` 的「一」被拦 → 乙/丙型链跳过 → 退化成 `这一块8`）。白名单天然蕴含「一后不接数字/进位字」：那100年 / 这10年 / 那1000米 / 这1万 照常转。零词表打补丁（DEC-038）；刻意边界 这一点五 / 这一点半 保持汉字（三条理由入注释）。生产 +37 / 测试 +132；itn-rules.toml 零改动；🔴 断言为静态 trace 推得（阶段一禁 cargo test），待阶段四实跑。rustfmt --check 单文件零 diff（未跑全量 fmt）/ cargo check + --all-targets 0 error / warnings 111/102 持平 ｜ 2026-09-17 |
- 2026-09-17 WORDBOOK-AUTOLEARN-OBS-218（coder-2，交付待验收）：自动学习两条日志提级 `warn` + `[AUTOLEARN]` 前缀（`wordbook/mod.rs:161/170`，+11/-4，逻辑零改动）。四道闸门里唯一该修者（release 被 `main.rs:8357` 的 `LevelFilter::Warn` 卡死）。不改全局日志级别（Gavin 压日志 IO 要求）；否决独立 `target` 方案（env_logger filter 配置在 main.rs，且 release 默认态下无收益）。cargo check 0 error / warnings 111/102 持平 ｜ 2026-09-17 |

- 2026-09-17 V091-PUNCT-TAIL-214 + V091-ITN-SKIP-ONLINE-215（coder-1，交付待验收）：**句尾不显示标点符号开关**（默认关）——复用 `apply_l2_postprocess`，加第三参 `strip_trailing_always`（全剥优先分支），配置 + Tauri 镜像各增 `strip_trailing`，产出源 5 条全覆盖（#4/#5 overlay 编辑态提交单独剥离），UI + 三语言 i18n；**在线流式 ASR 跳过主通道 ITN**——门控改 `initial_text.is_some()`（否决赛道有竞态的 config 方案），补丁通道保留。验证 `cargo check --all-targets` 0 error、warnings 111/102 持平、`npm run build` 通过；🔴 未跑 cargo test / 未出包。🔴 对 src-tauri 跑 fmt 触发 FMT-COLLATERAL-001（已逐文件 `git show HEAD:` 恢复，主控认可处置并纠正流程）｜ 2026-09-17 |
- 2026-09-17 ITN-HANT-POC-221A（coder-1，交付待验收）：**影子串前提取证（只读，生产零改动）**。Q1 源码取证确认 zhconv 0.4.1 的 `Variant::ZhHans` = 只字形不地区词（`variant.rs:36-66`/`converters.rs:28`/`variant.rs:130`）；产品现状用的 `ZhCN/ZhTW` 是地区变体，221 须替换。Q2 实测（`cargo test --bin feiyin-ime poc_221a`，5 PASS/0 FAIL）：指定 20 样本 20/20 等长、逐字 1:1 PASS、**全局单字扫描 28,096 字非 1:1 的 0 个**、词组 60 条 0 不等长；反面对照 `zhconv("後面",ZhTW)=="後麵"`。Q3 长度守卫够用但不最优，建议逐字构造影子串。Q4 改造面约 33 组函数/40+ 表查找点 + 全部 consumed/索引点 + `format_*` 输出须取原串。产物为 `#[cfg(test)] mod poc_221a_hant_shadow`（一次性，待清理）｜ 2026-09-17 |
- 2026-09-17 V091-ITN-YIKE-HANT-220（coder-1，交付待验收）：**繁中异形字补丁** —— `is_demonstrative_yi` 条件②两个集合补 `這` / `麼`（那/每/哪/某 繁简同形不重复），修「繁中用户一半修好一半没修」（`那一年` 修了、`這一年` 没修）。改动 = 条件②两处字面量 + doc 同步；条件①/③与主循环零触碰，不扩语义范围。另按主控裁定在 doc「已知边界」补写「时间量词刻族」（`这一点一刻`→`这一点1刻`）刻意选择 + 三条理由。🔴 改生产代码 → 本单不写断言（繁体护栏由 coder-2 交叉另派）。验证：`rustfmt --check src/itn.rs` exit 0（未跑全量 fmt）/ `cargo check --all-targets` 0 error、warnings 111/102 持平；未跑 cargo test。边界外风险上报：后缀表「號」仍为简体，本单红线不碰条件③故未动 ｜ 2026-09-17 |
- 2026-09-17 UI-ASRKEY-225 + UI-LLMHINT-226（coder-2，交付待验收）：**纯前端**。225 = 在线 ASR Key 标签改「API Key（阿里云百炼Key）」+ 淡灰**斜体**占位提示「输入你在阿里云百炼平台的API Key...」，**获得焦点即隐藏**（`.asr-key-input:focus::placeholder{color:transparent}`）、清空失焦自动回来；226 = 「API 配置」标签补「（建议使用deepseek-flash模型，参数格式参考提供商文档）」。实现 = 三份 locale（1 新增 key + 2 改值）+ `Voice.tsx` placeholder 改引用 i18n key（移除硬编码 `sk-...`）+ `styles.css` 两条类名限定 `::placeholder` 规则（不污染其它输入框）；`Llm.tsx` 零改动；🔴 **未用 focus/blur 事件改 value**。**Gavin 一票否决判据三条实证**（临时工装渲染真实 VoicePage，结果落 evidence.json、工装已删）：① 提示显示时 state 字段 `""`、全部 updateConfig 调用序列不含提示文案；② `voice_qwen3_empty_key_hint` 已渲染且测试连接按钮 disabled（无法绕过空 Key 分支，后端另有 `transcription/mod.rs:154` bail 兜底）；③ 保存 payload 该字段 `""`(string)、不含提示文案，全仓 grep 该文案 0 命中。**焦点/占位实测**（Chrome 152+CDP+真实构建 CSS，6 态截图 + 像素统计，产物存 outbox）：文字区占位灰像素 65→聚焦 **0**（完全不可见，非背景 40 含光标 16）→失焦 65（**与未聚焦逐像素零差异/md5 相同 ⇒ 稳定复现**）；DOM rect 三态恒 320×38、全视口差异仅限输入框自身 ⇒ 无布局位移；`.input`/`.llm-input` 对照 24/24、51/51 未污染。存量 `qwen_audio_online` 共用同一文案（同一字段+同一 handler+同走 is_online_streaming+同 dashscope/百炼体系，且已从下拉不可达）。验证 `npm run build` 通过；后端零改动 ｜ 2026-09-17 |

---

## 🚀 v0.9.0 发布（2026-09-10）

| 项 | 内容 |
| --- | --- |
| tag | `v0.9.0`（annotated，自 `v0.5.4` 以来第一个 tag，跨 341 次提交 / 2026-05-28~09-10 / 214 文件 +98983 −6357） |
| release note | `docs/RELEASE-NOTES-v0.9.0.md`，覆盖 v0.6 ~ v0.9，按 Gavin 要求走精简版 |
| GitHub Release | https://github.com/Cdexs/Feiyin-IME/releases/tag/v0.9.0 （未挂二进制产物） |
| 里程碑口径 | v0.9.0 = 悬浮窗重制 + 实时上屏体验 + 丢字类问题集中收口 + 热键重做 + 格式化输出治理 |

**遗留**：README 正文功能表仍停留在 v0.6 时代；Release 未挂安装包。


## v0.9.2 · 2026-09-20 · 在线 ASR 两轴参数化（治碎句与满屏句号）

| 项 | 内容 |
| --- | --- |
| 内容 | ASR-SEG-229 在线 ASR 两轴：`asr_online_max_sentence_silence` 默认 800→2000、`semantic_punctuation_enabled` 编译期常量→隐藏 config 字段（默认 false）；VER-BUMP-230 版本 0.9.1→0.9.2 |
| 测试基建 | FIX-TESTENV-231：`TestEnv` 每实例唯一目录，修 `config::tests` 并行竞态（旧实现单目录 `voice-ime-test-{pid}` + Drop `remove_dir_all`） |
| 回归 | root 1266P/0F/15I + src-tauri 77P/0F；`config::tests` 并行连跑 5 次全绿 |
| 出包 | `BUILD-232` 八项核验逐项 PASS，ProductVersion 0.9.2；产物 `feiyin-ime` 12.35MB / UI 10.05MB / crash-reporter 24.88MB |
| 遗留 | 语义断句模式下服务端中间结果频率未知（可能致 overlay 预览变顿）；`[ASR-DROP]` 逐帧 WARN（`src/audio/mod.rs:492-495`）待下批修 |


## v0.9.2（二包 · BUILD-244）· 2026-09-20 · 本地流式实时模型批次（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | LOCAL-RT-UI-240 / ENGINE-239-A/B 本地流式实时管线（第四条 ASR 档位）+ LOCAL-RT-READY-246 双模型就位检测 + WORDBOOK-MINLEN-242 候选词下限；**版本号维持 0.9.2**（Gavin 明确指示不升） |
| 回归 | TEST-EXEC-251 五项全绿：root 1267P/0F/15I + src-tauri 78P/0F + Vitest 100P/0F/11S + browser(Chromium) 5P/0F |
| 出包 | `BUILD-244` 八项核验逐项 PASS（③按「sha 异于上包」替代判据）；产物 `feiyin-ime` 12.43MB `e5807ccd…` / `feiyin-ime-ui` 10.05MB `99a15b02…` / `crash-reporter` 24.88MB `964a7163…`，时间戳 20:48–20:50 |
| 端测 | 待 Gavin：`collab/e2e-checklist-local-realtime.md` 十项；双模型已就位 `Publish/models/`；🔴 第 10 项「现有三档零回归」为红线 |
| 遗留 | `[ASR-DROP]` 逐帧 WARN 未修（非本批）；语义断句中间结果频率待端测 |


## v0.9.2（三包 · BUILD-258）· 2026-09-20 · 四项端测修复合并（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | FIX-252 标点重复 + FIX-252 长音频无输出 + FIX-255 流式右侧留白 + 256 本地流式预览补标点；**版本号维持 0.9.2**（Gavin 指示） |
| 回归 | TEST-EXEC-257 root 1267P/0F/15I + src-tauri 78P/0F；两条 D2D 护栏专项绿（`streaming_scroll_offset_contract` / 护栏 9） |
| 出包 | `BUILD-258` 八项核验逐项 PASS（③按「sha 异于上包」替代判据）；产物 `feiyin-ime` 12.43MB `b8a3fa98…` / `feiyin-ime-ui` 10.05MB `8ab68022…` / `crash-reporter` 24.88MB `240fb14a…`，时间戳 21:48–21:51 |
| 端测 | 待 Gavin 复验四条（标点不重复 / 长音频出字 / 流式无留白(在线档同生效) / 本地流式预览有标点）；`Publish/models/` 未动 |
| 遗留 | `[ASR-DROP]` 逐帧 WARN 未修（非本批） |


## v0.9.2（四包 · BUILD-267）· 2026-09-20 · 热词频率 + KV1024 进包（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 262 模型 512→1024 两副本 + `system_prompt` 置空；260 `max_new_tokens` 0→256；263 `hit_count`/`last_used_at` + 频率排序 + `record_hits` 后台线程；265 热词 20→120 条 + 600 字符预算；266 设置界面词库按频率排序（后端透传） |
| 出包 | `BUILD-267` 八项核验逐项 PASS + 三条额外全 PASS；产物 `feiyin-ime` 12.44MB `db7c7878…` / `feiyin-ime-ui` 10.06MB `01090dc2…` / `crash-reporter` 24.88MB `85e7c5cf…`，时间戳 23:15–23:17 |
| 模型 | `Publish/models/.../llm.int8.onnx` = 600,025,528（1024，`c326cdeb…`）；两处 `.512.bak` 保留未删 |
| 迁移 | 新库 / 旧库 / 状态 C 迁移冒烟全通，无 SQL 报错 |
| 遗留 | `[ASR-DROP]` 逐帧 WARN 未修（非本批） |


## v0.9.2（五包 · BUILD-274）· 2026-09-21 · accuracy 多线程 + 流式预览标点定时 + 首字埋点（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 268 热词 token 预算(356)；**271 accuracy `num_threads` 0→min(逻辑核,8) + provider cpu**；269/269-B 流式预览标点（静默 800ms 或满 4s 触发、全量重打）；272 首字延迟埋点；273 预算基准校正（仅验证） |
| 回归 | TEST-EXEC-274 root 1279P/0F/15I + src-tauri 85P/0F + Vitest 100P/0F/11S；271 对照**内容逐字不变**、耗时 1.83–2.06× |
| 出包 | `BUILD-274` 八项核验逐项 PASS；产物 `feiyin-ime` 14.35MB `40ccd5ea…` / `feiyin-ime-ui` 10.06MB `c788acf0…` / `crash-reporter` 24.88MB `21cb7d8a…`，时间戳 00:04–00:07；主程序 +1.9MB = `tokenizers` crate |
| 端测 | 待 Gavin：② 句中停 1s 不切句 / ③ 预览标点不叠加（需真人发声）；**272 首字延迟实测待 Gavin 配合录音** |
| 遗留 | `[ASR-DROP]` 逐帧 WARN 未修（非本批） |


## v0.9.2（六包 · BUILD-287）· 2026-09-21 · 右留白根治 + 收尾预览 + 预卷裁剪 + 影子解码（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 281 overlay 滚动改 DirectWrite 实渲宽（治右留白）；282 `PipelineEvent::StreamingFinalPreview` 收尾预览；283 `trim_pre_roll_residual`（本地档 true）；284 影子解码静默 400ms；286 埋点降 `debug!` + `log_enabled!` 守卫 |
| 回归 | TEST-EXEC-287 root 1283P/0F/15I + src-tauri 85P/0F + Vitest 100P/0F/11S；warnings 110/101/17 |
| 出包 | `BUILD-287` 八项核验逐项 PASS；产物 `feiyin-ime` 14.38MB `e16e3738…` / `feiyin-ime-ui` 10.06MB `ee6d6e73…` / `crash-reporter` 24.88MB `a4b238aa…`，时间戳 01:14–01:17 |
| 探针 | `[LocalRT-DBG-276/277/278/283/284]`=1/1/2/1/3；284 `LOCAL_RT_SHADOW_MS`=1 |
| 自验 | 日志开关两头：无 -debug 0 条 / 有 -debug 有输出 |
| 端测 | 待 Gavin 录音验四条修复效果 |


## v0.9.2（七包 · BUILD-290）· 2026-09-21 · 尾字修复(289) + 音频日志风暴治理(288)（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 289 endpoint 定稿改用「当前句最完整结果」（治尾字被缺字结果盖回）；288 音频回调日志风暴治理（实时线程只原子自增、消费端节流上报、汇总改「THIS recording」） |
| 回归 | TEST-EXEC-290 root 1284P/0F/15I + src-tauri 85P/0F + Vitest 100P/0F/11S；warnings 110/101/17 |
| 出包 | `BUILD-290` 八项核验逐项 PASS；产物 `feiyin-ime` 14.38MB `835d402b…` / `feiyin-ime-ui` 10.06MB `0454b288…` / `crash-reporter` 24.88MB `ae914a08…`，时间戳 01:45–01:47 |
| 探针 | `[LocalRT-DBG-289]`=1；`chunks dropped during THIS recording`=2（288 功能性字面量） |
| 自验 | 🔴 288 空闲 150s `[ASR-DROP]`=0；日志开关两头验通过（无 -debug 无文件 / 有 -debug 有数据） |
| 端测 | 待 Gavin 录音验尾字与四条修复效果 |


## v0.9.3（批次二 · BUILD-296）· 2026-09-21 · 尾字 flush 修复(291) + pre-roll 音频取证(292) + 首字锚定裁剪/窗口1000ms(293-B)（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 291 endpoint 切句前先 `input_finished()` flush 再确认 + 下一句 `create_stream()` 取代 `reset()`（治中间句结构性丢尾字）；292 debug-only pre-roll WAV dump（手写零依赖 PCM16 writer，`debug-audio/`）；293-B pre-roll 容量 600→1000ms + `select_pre_roll_for_asr` 锚定语音起点统一裁剪（A/B/C+兜底），在线三档仍 600ms |
| 回归 | TEST-EXEC-296 root **1298P/0F/15I**（+17 新测 −3 改名）+ src-tauri **85P/0F/0I**；Vitest SKIP（`ui/` 零 diff）；warnings 110/9/17 = 基线 |
| 出包 | `BUILD-296` 八项核验逐项 PASS；产物 `feiyin-ime` 14.42MB `117d0411…` / `feiyin-ime-ui` 10.06MB `66b006e3…` / `crash-reporter` 24.88MB `f013357a…`，时间戳 11:03–11:05 |
| 探针 | `[LocalRT-DBG-291]`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2；`283`=0 属预期（293-B 改名） |
| 关键发现 | 首跑 `guard291_g3` 假红 → 定位为 294 护栏区域定界缺陷（非生产缺陷），coder-1 `FIX-GUARD-297` 修复；295 I5 三种消融全 RED |
| 端测 | 待 Gavin 真人录音 5 项（291 中间句尾字；293 三场景首字；本地流式+翻译）；带 `-debug` 落 `debug-audio/` WAV |


## v0.9.3（批次二 · BUILD-302）· 2026-09-21 · 本地流式 accuracy 并行转写(298) + 护栏扫描边界收口(301/301-B)（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 298 本地流式说话中按 **800ms 静音切片**并行丢给 accuracy、说完只等尾片；新增显式 `pretranscribed` 参数与 3 个 env 开关（`LOCAL_RT_ACC_PARALLEL`=1 / `_SILENCE_MS`=800 / `_MIN_SEG_MS`=3000）；301/301-B 统一「生产区扫描」为剔除全部 test-gated 项（修 5 条假红，含跨文件漏网的第 5 处 `punctuation/mod.rs`） |
| 回归 | TEST-EXEC + BUILD-302 root **1304P/0F/15I**（上轮 5 条假红逐条 5/5 恢复）+ src-tauri **85P/0F/0I**；Vitest SKIP（`ui/` 零 diff）；warnings 110/9/17 = 基线 |
| 出包 | `BUILD-302` 八项核验逐项 PASS；产物 `feiyin-ime` 14.48MB `5a318429…`（较 296 +58,368B）/ `feiyin-ime-ui` 10.06MB `1cb71954…` / `crash-reporter` 24.88MB `4844bd21…`，时间戳 12:01–12:03 |
| 探针 | `[LocalRT-DBG-298]`=4 / `291`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2 |
| 关键发现 | 301 首跑只修 1/5（漏 `punctuation/mod.rs` 自带 `prod_lines`）→ 301-B 收口；「扫到第一个 X 就停」的边界一律先问「谁能在 X 前插一个 X」 |
| 端测 | 待 Gavin 真人录音四类（291 尾字／293-B 三场景首字／298 长语音看 `tail_wait`／本地流式+翻译）；应急 `LOCAL_RT_ACC_PARALLEL=0` |


## v0.9.3（批次二 · BUILD-306）· 2026-09-21 · 口水词过滤接线(303+305) + 并行最小片长 3s→5s（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 303 本地免费口水词过滤（中英日韩保守版，纯函数）+ 305 **接线**：`main.rs:9499 apply_filler_strip(final_text, !llm_handled)`（标点节点后、inject 前；仅 LLM 未接手时生效，不触 DEC-041）；`ACC_MIN_SEGMENT_MS_DEFAULT` 3s→**5s** |
| 回归 | TEST-EXEC + BUILD-306 root **1342P/0F/15I**（+3 = `filler_strip_303_tests`）+ src-tauri **85P/0F/0I**；Vitest SKIP（`ui/` 零 diff）；扫描 main.rs 生产区护栏逐条点名全绿；warnings 110/9/17 = 基线 |
| 出包 | `BUILD-306` 八项核验逐项 PASS；产物 `feiyin-ime` 14.49MB `bc043e25…`（较 302 +13,824B）/ `feiyin-ime-ui` 10.06MB `467232cd…` / `crash-reporter` 24.88MB `2d05df22…`，时间戳 13:02–13:04 |
| 探针 | `[LocalRT-DBG-298]`=4 / `291`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2；🆕 `然后`=6、`えー`=1 证 303/305 进包 |
| 端测 | 待 Gavin 真人录音六类（新增第 5 类切片粒度 A/B/C、第 6 类口水词过滤） |


## v0.9.3（批次三 · BUILD-321）· 2026-09-21 · Qwen3-ASR 迁移 + 上下文注入 + 同音纠错 + 去全部 env（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 314+315+320 accuracy 档 FunASR Nano → **Qwen3-ASR 0.6B**（DEC-076，单引擎无回滚开关）；320 上下文注入（三段时间线 per-stream，上限 500 字，LCS 回显护栏）+ 词条 120→**200**；317+320 流式/accuracy 线程统一 `min(cores,8)` 兜底 4；318 同音纠错节点 `apply_homophone_fix`（129 条规则外置 toml）；303+305 口水词过滤；307 尾字整句重解码；308 pre-roll 残留剔除；**全部 env 覆盖删除** |
| 回归 | TEST-EXEC + BUILD-321 root **1367P/0F/16I**（与 coder-1 独立一致；+18 运行/+1 ignored）+ src-tauri **85P/0F/0I**；护栏逐条点名 38/38；Vitest SKIP（`ui/` 零 diff）；warnings 110/9/17 = 基线 |
| 出包 | `BUILD-321` 八项核验逐项 PASS；产物 `feiyin-ime` 14.61MB `ea426a0c…`（较 306 +120,832B）/ `feiyin-ime-ui` 10.06MB `8a0a891f…` / `crash-reporter` 24.88MB `c9d7f17c…`，时间戳 15:28–15:29；**Qwen3 模型（954MB）已入 `Publish/models/`**（内容级 sha 核验） |
| 探针 | `[LocalRT-DBG-320]`=1 / `[MIGRATE-QWEN3-320]`=1 / `307`=2 / `293`=1 / `298`=4；同音 `满头大汉`=1 |
| 遗留上报 | ⚠️ `mod.rs:1019` hotwords tokenizer 仍引用 nano 目录（只计 token 非 ASR）；建议改指 Qwen3 自带 tokenizer |
| 端测 | 待 Gavin 四组（浮层预览 / 最终文字含韩文 / tail_wait+redecode / 本地流式+翻译）；🔴 本包无任何 env 可调 |


## v0.9.3（批次四 · BUILD-328）· 2026-09-21 · 端测反馈修复四单 + nano 解依赖（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 323「十分」作程度副词误转成「10分」修复（`[protect.degree_adverbs]`+右邻消歧）；324 回显护栏比对面收窄（修 43% 误触）+ 输出清理指令恒发；322+325 pre-roll 根因定案（非 bug）+ accuracy 分片结果**回灌预览**（尾字）；327 自学习候选抽取二次收窄（治「指导灵」不入库）；77313e5 **nano 最后一处依赖解除**（词条 tokenizer 改指 Qwen3）；322 诊断精度 dBFS + nz_ratio |
| 回归 | TEST-EXEC + BUILD-328 root **1385P/0F/17I**（+18 运行/+1 ignored）+ src-tauri **91P/0F/0I**（+6 镜像）；Vitest SKIP（`ui/` 零 diff）；warnings 110/9/17 = 基线 |
| 出包 | `BUILD-328` 八项核验逐项 PASS；产物 `feiyin-ime` 14.66MB `e39ac3a0…`（较 321 +50,176B）/ `feiyin-ime-ui` 10.06MB `8294a624…` / `crash-reporter` 24.88MB `4fcc90ee…`，时间戳 18:22–18:24；**四张规则表三副本全等**（含新拷的 homophone/wordbook） |
| 探针 | `[LocalRT-DBG-325]`=1 / `[AUTOLEARN]`=1 / `degree_adverbs`=2 / `nz_ratio`=2 |
| 端测 | 待 Gavin 六条（十分/十分钟；尾字回灌；🔴 预览窗编辑；自学习；先说半句 pre-roll；redecode 频次 <43%） |


## v0.9.3（批次五 · BUILD-333）· 2026-09-21 · 回灌持续生效 + 编辑快照基准 + 摘路径B死代码（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 329 回灌被下一个流式包冲掉（流式回调约 6 倍频繁）⇒ 权威前缀**持续生效** + 自学习基准对齐；330 删本地流式档指向已废弃 nano 的「缺失 972MB」UI 假提示块；331 编辑学习基准改取「用户开始编辑时所见」快照 + 词条上限 30→12；332 摘除「注入后观察目标窗口」路径B + 三处死代码（对齐 DEC-058） |
| 回归 | TEST-EXEC + BUILD-333 root **1398P/0F/17I**（全量，NEW 13）+ src-tauri **92P/0F/0I**（NEW 1）+ Vitest **100P/11S/0F**（ui 有改动，实跑）；warnings **99/90/17 = 新基线** |
| 出包 | `BUILD-333` 八项核验逐项 PASS；产物 `feiyin-ime` 14.66MB `703788ed…`（+1,536B）/ `feiyin-ime-ui` 10.05MB `80d79937…`（**−10,240B** = 删提示块）/ `crash-reporter` 24.88MB `c71d46c6…`，时间戳 19:19–19:21；四张规则表三副本全等 |
| 探针 | 正向 `[LocalRT-DBG-325] streaming render`=1 / `[AUTOLEARN]`=1 / `degree_adverbs`=2 / `nz_ratio`=2；🔴 **反向 5 符号全 0**（332 摘除彻底） |
| 端测 | 待 Gavin 七条（尾字 325 不归零／🔴 预览编辑不被冲／🔴 自学习=编辑改对后提交、同词连续两次 + 反向不编辑无 `[AUTOLEARN]`／十分·十分钟／pre-roll onset／redecode<43%／🔴 配置界面无模型提示块） |


## v0.9.3（批次六 · BUILD-338）· 2026-09-21 · 重复标点修复 + 尾字接缝自适应定界（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 334 最终输出重复标点 `。。`/`，。`——一片解码失败即误判「文本无标点」致 CT-Transformer 对已打标点全文再打一遍，改为实测 `has_effective_punctuation`；336 流式 paraformer 不提供 token 时间戳（78 条探针全 ts=0，已定性，仅加诊断探针）；337 尾字接缝自适应定界（`committed_len` 记在派发那刻、流式未吐完 ⇒ 按 a 文本稳定 / b 有声恢复 / c 硬上限 三者最先冻结边界，`boundary=a|b|c`） |
| 回归 | TEST-EXEC + BUILD-338 root **1407P/0F/22I**（全量，NEW 14 = 运行 +9/ignored +5）+ src-tauri **92P/0F/0I**（不变）+ Vitest **100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **99/90/17** |
| 出包 | `BUILD-338` 八项核验逐项 PASS；产物 `feiyin-ime` 14.68MB `91710a71…`（+14,336B）/ `feiyin-ime-ui` 10.05MB `f729ec28…` / `crash-reporter` 24.88MB `268ad4c7…`，时间戳 22:26–22:28；四张规则表三副本全等 |
| 探针 | 自检基准 `feiyin`=19；正向 `LocalRT-DBG-337`=4 / `336`=1 / `325`=1 / `AUTOLEARN`=4 / `degree_adverbs`=2 / `nz_ratio`=2；🔴 反向 5 符号全 0 |
| 端测 | 待 Gavin 五条（🔴 337 尾字接缝 + **b 占比=收益折损须报**／预览编辑闩锁／重复标点消失／自学习两次+反向／335 电平闸旁证） |


## v0.9.3（批次七 · BUILD-341）· 2026-09-21 · 尾字直接修（补静音喂满末尾一块）（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 340 `feed_tail_silence()`：在 shadow 与松手收尾 flush 的 `input_finished()` **之前**补静音（`SHADOW_TAIL_PAD_MS=500` / `FLUSH_TAIL_PAD_MS=2000`），把末尾不完整的一块喂满，逼流式模型吐出压着的尾字。机制：流式 paraformer 需约一个整块（500ms）未来音频才能定当前段字，说完没有未来音频 ⇒ 末字永不吐出；Gavin 观察到豆包同现象 ⇒ 结构性特征，本包主动越过 |
| 回归 | TEST-EXEC + BUILD-341 root **1409P/0F/22I**（全量，NEW 2 = `tailpad340_*`）+ src-tauri **92P/0F/0I** + Vitest **100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **99/90/17** |
| 出包 | `BUILD-341` 八项核验逐项 PASS；产物 `feiyin-ime` 14.68MB `68e4528b…`（大小同 338 但 sha 异）/ `feiyin-ime-ui` 10.05MB `a39b9474…` / `crash-reporter` 24.88MB `1942611d…`，时间戳 22:57–22:59；四张规则表三副本全等 |
| 探针 | 自检基准 `feiyin`=19；正向 `LocalRT-DBG-337`=4 / `336`=1 / `325`=1 / `AUTOLEARN`=4 / `degree_adverbs`=2 / `nz_ratio`=2；反向 5 符号全 0；🆕 340 无字面量 ⇒ N/A（降级「源码引用 5 处 + 时间戳/sha + `tailpad340_*` 2/2」） |
| 端测 | 待 Gavin 六条（🔴 停顿处尾字硬判据 `[LocalRT-DBG-289] used=shadow`/`shadow_len>main_len` 分布／337 b 占比／预览编辑闩锁／重复标点／自学习两次+反向／首字「你/按」旁证） |


## v0.9.3（批次八 · BUILD-342）· 2026-09-22 · 派发条件改回 OR（根因）+ 松手非取消 + 假 endpoint 护栏 + 接缝 padding（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 342（`fbec727`）：**D 根因** accuracy 派发条件由「静默 且 满5s」改回设计原意「静默 **或** 满5s」；**A** 松手不再当取消（原 `skipped-cancel` 丢弃算好的 accuracy 文本）；**F1+F3** 静音流上的假 endpoint 与幻字护栏；**padding** 分片前后各扩 200ms；**B** 仅结论未改码 |
| 回归 | TEST-EXEC + BUILD-342 root **1416P/0F/22I**（全量，NEW 8/GONE 1）+ src-tauri **92P/0F/0I** + Vitest **100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **99/90/17** |
| 出包 | `BUILD-342` 八项核验逐项 PASS；产物 `feiyin-ime` 14.68MB `8758ca66…`（+4,608B）/ `feiyin-ime-ui` 10.05MB `94f2a97a…` / `crash-reporter` 24.88MB `a4e2672d…`，时间戳 00:16–00:18；四张规则表三副本全等 |
| 探针 | 自检 `feiyin`=19；正向 `[LocalRT-DBG-342]`=3 / `337`=4 / `336`=1 / `325`=1 / `AUTOLEARN`=4 / `degree_adverbs`=2 / `nz_ratio`=2；反向 5 符号全 0 |
| 🔴 待裁发现 | `target/release/crash.json`（**00:00:34，早于本 build**）byte-index 非字符边界 panic（`inside '斯'`）；`fbec727` 未显式修；候选 `local_stream.rs:315 raw_full[cache.raw_len..]`；已备份、未改代码 |
| 量化 | 冒烟 idle CPU 0.04%、WS 1791MB；历史改前 47 次派发**最小 silence=800ms**（无一 <800）；`join/total_decode/tail_wait` 本机无数据 |
| 端测 | 待 Gavin 五条（🔴 D：连说 15s+ 出 silence<800ms 派发／🔴 尾字 reflow applied 非 skipped-cancel／句尾幻字／接缝重复／其余照旧） |


## v0.9.3（批次九 · BUILD-345）· 2026-09-22 · P0 崩溃修复 + DEC-077 回滚 + 失败片不留洞（版本号不升）

| 项 | 内容 |
| --- | --- |
| 内容 | 344（`8282203`）：**P0** 中文按字节切片崩溃（`byte index ... inside '斯'`）修复；**DEC-077 回滚**移除 340 shadow 补静音(500ms) + 340 收尾补静音(2000ms) + 307 每次断句整句重解码（三者实测零收益）；**G** 失败片改用该片**流式文本填补**（不再留永久洞致整场回灌全废）+ `d87b8b4` 注释同步。**替换作废的 8758ca66** |
| 回归 | TEST-EXEC + BUILD-345 root **1419P/0F/22I**（全量，NEW 6/GONE 3）+ src-tauri **92P/0F/0I** + Vitest **100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **99/90/17（未下降，如实报）** |
| 出包 | `BUILD-345` 八项核验逐项 PASS；产物 `feiyin-ime` 14.68MB `ce10c4b7…`（−7,680B vs 作废 342）/ `feiyin-ime-ui` 10.05MB `1e3df6d4…` / `crash-reporter` 24.88MB `01378435…`，时间戳 00:58–00:59；四张规则表三副本全等 |
| P0 验证 | 单测 `charboundary344_mid_char_raw_len_does_not_panic` 通过 + 删旧 crash.json 后冒烟**未新增**；原现场（长口述）交 Gavin |
| 探针 | 自检 `feiyin`=19；正向 `337`=4/`336`=1/`325`=1/`AUTOLEARN`=4/`degree_adverbs`=2/`nz_ratio`=2；🆕 **反向 345 `feed_tail_silence`/`SHADOW_TAIL_PAD_MS`/`FLUSH_TAIL_PAD_MS` 全 0** ⇒ 三机制彻底移除 |
| 端测 | 待 Gavin 五条（🔴 长句说到底 `skipped-hole` 基本消失／不得新增 crash.json／停手尾字 1~3s 补上／D 复核／其余照旧） |


## v0.9.3（批次十 · BUILD-346）· 2026-09-22 · 预览/最终标点语义修复 + 派发只判静默（版本 0.9.3）

| 项 | 内容 |
| --- | --- |
| 内容 | 346（`701c4d8`）切片派发只判静默 **1200ms** + 删长度支（DEC-077）/ 349（`e2f259f`）预览标点只在静默 1200ms 打、删 4s 定时与 shadow 强制 / 350（`c9b59b3`）标点剥离独立节点、**只挂本地 realtime**（DEC-066）/ e829c67 零行为提取 `create_qwen3_recognizer_at` / 352（`1269ddc`）6 条独立护栏 / 351（`f335bd8`）1.7B PoC `#[ignore]` |
| 回归 | TEST-EXEC + BUILD-346 root **1435P/0F/23I**（全量，NEW 15+1I / GONE 0；相对 BUILD-345 GONE=2）+ src-tauri **92P/0F/0I** + Vitest **7 files/100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **99/90/17** |
| 出包 | `BUILD-346` 八项核验逐项 PASS；产物 `feiyin-ime` 14.68MB `98df432cccca…`（11:37:09）/ `feiyin-ime-ui` 10.05MB `ea68c4023e52…`（11:33:49）/ `crash-reporter` 24.88MB `887957b195f7…`（11:35:11）；两副本全等、均异于 BUILD-345；四张规则表三副本全等 |
| 探针 | 正：`[LocalRT-DBG-298]`=4 / `[LocalRT-DBG-325]`=2 / `AUTOLEARN`=11 / `Punctuation strip node`=1；🆕 **反：`PUNCT_REFRESH_INTERVAL`=0 / `ACC_MIN_SEGMENT_MS_DEFAULT`=0 / `min_seg_ms`=0** ⇒ 349 定时 + 346 长度支彻底移除 |
| 端测 | 待 Gavin 六条（🔴 核心=预览标点不打句中／⚠️已知代价：长不停顿说话预览无标点（1200ms 才打）请表态／350 最终标点无 `。。`／在线 realtime 与本地 performance 两档须无变化／346 `silence=` 恒 ≥1200ms／不得新增 crash.json） |
| 🔴 边界 | 353(`4112354`,+73)/`mod.rs`(+10) 均晚于本包提交、纯 `#[cfg(test)]` 不进 release，未纳入本次回归（超本批范围） |


## v0.9.3（批次十一 · BUILD-347）· 2026-09-22 · 滑动窗口重构 + 1.7B + 标点语义（版本 0.9.3）

| 项 | 内容 |
| --- | --- |
| 内容 | 354/356（ITN「度」义项消歧 + 固定语保护）/ 359（sherpa 1.13.8 + ORT **1.28.2** + accuracy 换 Qwen3 **1.7B** + 应用层剥语种前缀）/ 363/364（acc 双路 + 录音上限 **300→180s** + 预算精确计算）/ DEC-080-365（摘除「剥光标点+二次打点」节点）/ 367（**滑动窗口四阶段**：路A 废弃、松手后不再全量解码、窗口间并发池 + 有序定稿）/ `ae166e6` fmt 修复 |
| 回归 | TEST-EXEC + BUILD-347 root **1464P/0F/30I**（全量，11 二进制；`feiyin-ime` bin **1376P/28I** = 基线逐位吻合；NEW 30+7I / GONE 1）+ src-tauri **92P/0F/0I** + Vitest **7 files/100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）EXIT 0；warnings **98/88/17**（基线 98/89/17，test −1 如实报） |
| 出包 | `BUILD-347` 八项核验逐项 PASS；产物 `feiyin-ime` 14.74MB `edf7d088b74e…`（19:28:12）/ `feiyin-ime-ui` 10.05MB `af045bc855c7…`（19:25:18）/ `crash-reporter` 24.88MB `6d71c620bbe9…`（19:26:21）；两副本全等、均异于 BUILD-346 |
| 三特殊点 | ① **dll 三副本**：`sherpa-onnx-lib`/`Publish`/`target-release` 四张 dll sha256 全等，`onnxruntime.dll`=`422d776a…` 且 **ProductVersion 1.28.2**；旧 1.12.38 目录保留 ② **itn-rules.toml 三副本**：出包前 root `60b227de` vs 另两处 `ab950ba4` 分叉 → 已同步全等 ③ **Publish/models 补 1.7B**：出包前缺失 → 补拷并与源 sha256 全等；0.6B 保留 |
| 探针 | 正 `SLIDING-WINDOW-367`=3 / `[LocalRT-DBG-298]`=3 / `AUTOLEARN`=11；🆕 反 `sub-seg failed`=0（路A 每片解码，随 367 废弃）/ `PUNCT_REFRESH_INTERVAL`=0 / `ACC_MIN_SEGMENT_MS_DEFAULT`=0 / `min_seg_ms`=0 |
| 端测 | 待 Gavin 九条（🔴 松手等待时间（核心收益）／预览修正质量／🔴 吃字重字（最大风险）／接缝重复／标点语义／1.7B 转写质量／「梅开二度·一度」不再误转／长录音 180s + 不新增 crash.json／重启输入法） |
| 过程 | 首轮 `cargo fmt --check` EXIT 1（`mod.rs:4210`）→ 上报主控 `ae166e6` 修复 → `ae166e6` 重跑全量绑定交付 |

### 2026-09-22 · FIX-ORDERED-REFLOW-DROP-368（P0 吃字）交付

- OrderedReflow 对齐失败时 last_window_text 被覆盖 ⇒ 吃字；修为失败即彻底保守（committed/last 都不动、不回灌、next 仍推进）+ 连续失败≥3 兜底并入（宁可重复不丢字）。
- 仅改 OrderedReflow；新增 4 单测；全量 cargo test 0 failed（1379P）/ fmt --check EXIT 0 / release EXIT 0 / warnings 98-89 基线。BUILD-347 须重出包。


## BUILD-348（重出包 · 修 P0 吃字）· 2026-09-22 · BUILD-347 作废

| 项 | 内容 |
| --- | --- |
| 内容 | `FIX-ORDERED-REFLOW-DROP-368`（HEAD `4201d39`）：`OrderedReflow::push` 对齐失败时**不再**无条件替换 `last_window_text`；新增 `REFLOW_FALLBACK_FAILS=3`，连续失败达阈值触发兜底（旧文本整体并入 committed，不去重）。仅改 `src/transcription/mod.rs`（+133/−18） |
| 回归 | root `cargo test --no-fail-fast` **1467P/0F/30I**（EXIT 0；`feiyin-ime` bin **1379P/28I** = 基线逐位吻合；NEW 3 / GONE 0）+ src-tauri **92P/0F/0I** + Vitest **7 files/100P/11S/0F**；`cargo fmt --check` EXIT 0；warnings **98/88/17**（test −1 如实报） |
| 出包 | `BUILD-348` 八项核验逐项 PASS；产物 `feiyin-ime` 14.74MB `f9ba2822c731…`（20:02:04）/ `feiyin-ime-ui` 10.05MB `7d72bced2fc6…`（19:59:10）/ `crash-reporter` 24.88MB `61e3ab00869b…`（20:00:19）；两副本全等、均异于作废的 BUILD-347 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本 `60b227de…` 全等；③ Publish/models 1.7B 与源逐一 sha256 全等（仅核验，未重拷） |
| 探针 | 正 `SLIDING-WINDOW-367`=4 / `[LocalRT-DBG-298]`=3 / `AUTOLEARN`=11；反 `sub-seg failed` / `PUNCT_REFRESH_INTERVAL` / `ACC_MIN_SEGMENT_MS_DEFAULT` / `min_seg_ms` 全 0 |
| 端测 | 🔴 **置顶：吃字/重复字重点复测**（>4 句让滑窗多次，看有无整段消失）+ 原九条 |
### 2026-09-22 · FIX-WINDOW-DISJOINT-369（🔴 P0 整窗丢字）交付

- **根因**：12s 封顶 vs 长单片（片2 9.34s）⇒ 窗口与上一窗**零重叠** ⇒ 对齐必败；而 368 的「失败 ⇒ 整窗跳过」
  使必败场景变成**整窗丢失**（Gavin BUILD-348：6 片 31s ⇒ 最终 50 字）。
- **修法**：不再靠文本猜重叠 —— `main.rs` 记 `total_slices`/`window_spans`，把窗口切片区间
  `[total_slices - recent_slices.len() + start, total_slices)` 传给 `OrderedReflow::push(seq, ws, we, text)`：
  零重叠 ⇒ **直接拼接**（绝不跳过）；有重叠 ⇒ `align_overlap` 去重；有重叠但对齐失败 ⇒ **退回拼接**（宁可重复不丢字）。
- **清理**：删除被新规则完全取代的 `fail_streak` + `REFLOW_FALLBACK_FAILS`。
- **单测**：+4（含真实日志时长序列复刻，断言 6 片全在）/ −1（368 阈值兜底）/ 改 6 ⇒ bin 1379→1382P。
- **反证**：临时回归 368「跳过整窗」⇒ 真实序列测试 FAILED（片5 整窗丢失）⇒ 证明该测试真能抓到本 P0。

| 项 | 内容 |
| --- | --- |
| 改动文件 | `src/transcription/mod.rs`（OrderedReflow + 单测）、`src/main.rs`（切片区间追踪） |
| 未动 | `align_overlap` / `group_window_start_secs` / `ALIGN_MIN_OVERLAP_RATIO` / `itn.rs` / 版本 0.9.3 |
| 验证 | `cargo fmt --check` **EXIT 0**；全量 `cargo test --no-fail-fast` **0 failed**（bin **1382P/28I**）；`cargo build --release` **EXIT 0**；warnings **98/88** ≤ 基线 98/89 |
| numstat | main `32/2`（== -w）；mod `212/82`（vs -w `205/75`，7 行 whitespace-only 落改动区块内） |
| 结论 | 🔴 **BUILD-348 含此 P0，须重出包**；未 push / 零凭证 |
### 2026-09-22 · FIX-REMOVE-HARDSPLIT-370 + 录音上限 300s 交付

- **根因**：滑窗派发路径仍有 20s 硬切（`SEGMENT_MAX_SECS`，native 时代遗留）⇒ 违背「只按静默停顿切片」，
  并加重 369 的零重叠丢字。
- **修法**：`build_padded_segments_capped(.., max_seg_secs)` 显式上限 + 旧函数变薄包装（其它路径**逐位不变**，有单测证明）；
  滑窗两处调用点走 `build_dispatch_segment`，上限 **13s**（极端长句兜底，超出继续切、不丢弃）；护栏（越界过滤/clamp + 200ms padding）保留；
  派发点保留 KV 撞顶 warn。
- **同批**：`MAX_RECORD_SECONDS` 180 → **300**（滑窗后与 KV 解耦：只解最后一窗 ≤13s；音频缓冲 18MB）。
- **单测**：+8 / 改名 2 ⇒ bin 1382→1390P。

| 项 | 内容 |
| --- | --- |
| 改动文件 | `vad.rs`、`local_stream.rs`、`main.rs`、`config/mod.rs`、`platform/windows/hotkey.rs`、`transcription/mod.rs`、`qwen_inference.rs`、`src-tauri/src/config.rs` |
| 未动 | `align_overlap` / `group_window_start_secs` / 369 OrderedReflow / `itn.rs` / 版本 0.9.3 |
| 验证 | `cargo fmt --check` EXIT 0；全量 `cargo test --no-fail-fast` **0 failed**（bin **1390P/28I**）；`cargo build --release` EXIT 0；`cargo check --manifest-path src-tauri/Cargo.toml` EXIT 0；warnings 98/88（≤ 基线） |
| 13s 依据 | **对齐滑窗封顶 12s 的实测体验**（Gavin 端测：12s 窗解码+回灌「没有明显卡顿」）；13s≈5.3s vs 12s 的 4.9s（+0.4s）；被否 16s/30s/90s |
| KV/内存 | 均非瓶颈（13s≈169 token）；先前按 90s 的外推与 E2E 建议**作废** |
| 结论 | 未 push / 零凭证；🔴 369+370 同在未提交工作区，由主控一并提交 |


## BUILD-349（重出包 · 修滑窗丢字 + 13s 安全阀 + 300s 录音）· 2026-09-22 · BUILD-348 作废

| 项 | 内容 |
| --- | --- |
| 内容 | 369（`24c452a`）零重叠⇒**直接拼接**、有重叠对齐失败⇒退回拼接（**无分支丢整窗**）；370（`919a59e`）去 20s 硬切、单片安全阀 **13s**、`MAX_RECORD_SECONDS` 180→**300**。涉及 `transcription/mod.rs`、`local_stream.rs`、`vad.rs`、`main.rs`、`config`（主+Tauri）。**未动** dll/itn/模型/版本 |
| 回归 | root `cargo test --no-fail-fast` **1478P/0F/30I**（EXIT 0；`feiyin-ime` bin **1390P/28I** = 基线逐位吻合；NEW 11 / GONE 0）+ src-tauri **92P/0F/0I** + Vitest **7 files/100P/11S/0F**；`cargo fmt --check` EXIT 0；warnings **98/88/17** = 基线 **98/88** |
| 出包 | `BUILD-349` 八项核验逐项 PASS；产物 `feiyin-ime` 14.74MB `a645825ffec7…`（21:36:09）/ `feiyin-ime-ui` 10.05MB `b78dc71d48f5…`（21:33:16）/ `crash-reporter` 24.88MB `f7c199e4547a…`（21:34:21）；两副本全等、均异于作废的 BUILD-348 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本 `60b227de…` 全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（仅核验未重拷） |
| 探针 | 正 `SLIDING-WINDOW-367`=4 / `[LocalRT-DBG-298]`=3 / `AUTOLEARN`=11；反 `sub-seg failed` / `PUNCT_REFRESH_INTERVAL` / `ACC_MIN_SEGMENT_MS_DEFAULT` / `min_seg_ms` 全 0 |
| 端测 | 🔴 ①吃字复测（>4 句，含一句 8~10s 长句造零重叠）②连录两次互不污染（Gavin 点名）+ 其余九条（松手等待/前文改对/接缝重复/标点语义/1.7B 质量/梅开二度·一度/长录音 2~3 分钟/不新增 crash.json/重启输入法） |

## 2026-09-22 · FIX-PREFIX-AND-EAT-371（P0 两缺陷）已交付验收

- **A 语种前缀漏进正文**：恢复 355 定的「无条件截断到第一个 `<asr_text>`（含）」，删 359 私加的两闸；
  唯一护栏 `QWEN3_PREFIX_MAX_BYTES=64`；新增 `[LocalRT-DBG-371]` 打印被剥原文。
- **B 重复语句吃掉中间文本**：`align_overlap_with_prior` + `AlignPrior` 四层判据
  （①硬约束精确 → ②软范围**不对称**容差 DOWN 0.50 / UP 0.25 → ③质量门未改 → ④兜底升序小 k）；
  `OrderedReflow::push_window` 接线，`main.rs` 加 `window_samples`（per-recording）。
- 单测 +8（含两条反证：旧算法内联、去层①）；368/369 既有单测全绿；fmt/check 主控复跑干净。
- 主控 22 条清单逐条 Read 代码验收（`collab/acceptance-371.md`）：首轮 20 过 → B3 容差改判不对称、C3 补跨端文档 → 复验全过。
- **下一步**：阶段三 TEST-SYNC → 阶段四 TEST-EXEC → **出包须先问 Gavin**（DEC-079）。BUILD-349 已作废。
- **待观察**：`FORCED-ALIGN-372`（小模型精确对齐设想，未立项，等端测结果决定）。


## BUILD-373（阶段五 · 出包）· 2026-09-22 · 替换作废的 BUILD-349

| 项 | 内容 |
| --- | --- |
| 内容 | `7398459` FIX-PREFIX-AND-EAT-371（P0×2：语种前缀漏出 + 重复句吃字）+ `77185aa` TEST-SYNC-371（阶段三 23 条）+ `dff4fad` TEST-EXEC-371（阶段四全绿）。版本 0.9.3 |
| 回归 | 阶段四已全绿（引用）：root **1511P/0F/30I**（bin 1423P/28I）+ src-tauri **92P** + Vitest **100P/11S/0F**；fmt EXIT 0；warnings **98/88/17** = 基线 |
| 出包 | `BUILD-373` 八项核验逐项 PASS；产物 `feiyin-ime` 14.74MB `3453c6006186…`（23:28:17）/ `feiyin-ime-ui` 10.05MB `dd7b6e4866d7…`（23:25:12）/ `crash-reporter` 24.88MB `29a36303bd4f…`（23:26:19）；两副本全等、均异于作废的 BUILD-349 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 源码级正 6 符号均 ≥1；二进制级正 `SLIDING-WINDOW-367`=4 / `[LocalRT-DBG-298]`=3；🔴 反 `is_qwen3_language_label` / `PUNCT_REFRESH_INTERVAL` / `ACC_MIN_SEGMENT_MS_DEFAULT` / `min_seg_ms` / `sub-seg failed` 全 0 |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启后端测**；重点：吃字/重复字复测（>4 句含 8~10s 长句）+ 连录两次互不污染 + 长录音 300s |

## 2026-09-23 · 374/375 兜底网交付 + 376 PoC 证伪 language 方向

- 374/375 三条修复已验收提交（词条回显剥离 / 预览 stale 修复 / 解码坍塌重解）。
- 🔴 **POC-376 证伪主控的 language 推理**：设 `language` 不消回显，只换回显内容。
  官方资料证实 `language=None` 是主用法、前缀是官方输出格式、官方接口无上下文参数、
  sherpa hotwords 期望纯词表而我们塞了指令句+散文。
- 🔴 **新发现的空白**：换 Qwen3 后「词库偏置」是否仍有效**从未验证**（机制已从解码打分偏置
  变为塞 prompt 文本）。PoC 无法回答（音频里没有词表专名的发音点）。
- **待 Gavin 定**：是否先做词库有效性验证（需他录含词库专名的语音）。
  无效 ⇒ 直接不注入，回显从源头消失并省算力/KV。


## BUILD-379（阶段四全绿 → 出包）· 2026-09-23 · 替换作废的 BUILD-373

| 项 | 内容 |
| --- | --- |
| 内容 | `4f73117` FIX-TERMS-ECHO-374 + FIX-PREVIEW-STALE-AND-COLLAPSE-375 + POC-376；`243dcc4` FIX-INJECT-TO-SPEC-377（注入按 sherpa 规格砍成纯逗号词表）；+ 阶段三 18 条护栏。版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1546P/0F/31I**（EXIT 0；`feiyin-ime` bin **1458P/29I**；NEW 净 +35P/+1I = 374/375 +20 + 377 净 -3P/+1I + 阶段三 +18）+ src-tauri **92P/0F/0I** + Vitest **7 files/100P/11S/0F**；fmt EXIT 0；warnings **98/88/17** |
| 出包 | `BUILD-379` 八项核验逐项 PASS；产物 `feiyin-ime` 14.77MB `7e14a0fee986…`（01:13:03）/ `feiyin-ime-ui` 10.05MB `e4f1c53298f8…`（01:10:03）/ `crash-reporter` 24.88MB `926ed04bd72f…`（01:11:08）；两副本全等、均异于作废的 BUILD-373 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 源码级正 8 符号均 ≥1；二进制正 `SLIDING-WINDOW-367`=4 / `[LocalRT-DBG-298]`=3；🔴 反 `CLEANUP_INSTR_EN`/`CTX_INSTR_EN`/`merge_ctx_timeline`/`CTX_DEFAULT_CHARS`/`ctx_prev1`/`ctx_prev2`/`is_qwen3_language_label` 二进制全 0（源码命中全为注释/PoC/测试护栏） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启后端测**；重点：词条回显不漏进正文 / 预览能刷进 / 出字延迟 / 吃字重复 / 长录音 300s / 不新增 crash.json |

## FIX-PREVIEW-HARVEST-380 · 预览结果到即收 + 松键时延埋点（阶段一·只改代码）· 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | `src/main.rs` + `src/platform/windows/hotkey.rs`（+ `platform/mod.rs`/`windows/mod.rs` 两处导出）。**A**：路 B 滑窗线程 `for … in acc_rx` → `crossbeam_channel::select!`（模块级 `drive_acc_windows`），结果**到即收**回灌预览；两份收取合一 `harvest_acc_window!` 宏。**B**：钩子目标键 DOWN/UP tick 埋点 + `[LocalRT-DBG-380]` `hook_to_controller_ms`（非钩子路径 `n/a`）/ `stop_to_inject_ms` |
| 回归 | root `cargo test --no-fail-fast` **1547P/0F/31I**（EXIT 0；`feiyin-ime` bin **1459P/29I**）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**；warnings **98/88** = 基线 |
| NEW | +1：`preview_harvest_380_tests::drive_acc_windows_harvests_result_while_acc_open`（通道先后制造「`acc_rx` 未关、结果已到」，非 sleep；退回旧语义必超时） |
| 端测 | 🔴 待 tester-1 阶段四全量回归 + 出包，再交 Gavin 端测：现象①预览停顿即刷；现象②看 `hook_to_controller_ms`/`stop_to_inject_ms` 定位 4s 来源 |

## FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382 · 逐片组窗 + 松键立即处理态 + 回灌提速（阶段一·只改代码）· 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/main.rs`。**问题1**：Slice 改逐片组窗（N 片⇒N 窗，修「13s 那片从不进窗」P0 吃字），新增纯函数 `plan_windows` + 覆盖不变量单测。**问题2**：`StreamingFinalPreview`+`Processing` 提到 `acc_join()` 之前发、删后置预览发送，埋点 `stop_to_processing_ms`；`ACC_REFLOW_SUPPRESS` 防闪回（controller 处理 Processing 时置位）。**3A**：`ReflowFastState` 立即渲染（不等边界配对）+ `PreviewReflow.committed_len` 带派发fallback。**3B**：解码共享队列。**3C**：`decode_done_at` 字段 + reflow/queue 埋点 |
| 回归 | root `cargo test --no-fail-fast` **1563P/0F/33I**（EXIT 0；`feiyin-ime` bin **1475P/31I**）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**；warnings **98/88** = 基线 |
| NEW | +12：`plan_windows_382_tests` 4（覆盖不变量/单片回归/任意序列）、`reflow_fast_382_tests` 5（边界先/后到、b 类、两 seg 交错、Processing 后抑制）、`problem2_order_382_tests` 1（源码顺序：预览<处理态<acc_join，join 后不再发预览）、`shared_queue_382_tests` 2（空闲 worker 取下一任务 + 生产单一共享队列结构护栏）。GONE 0 |
| 端测 | 🔴 待 tester-1 阶段四回归 + 出包 + Gavin 端测：①一口气长句不再吃前文；②松键即显「识别处理中」；③`[LocalRT-DBG-382]` 看回灌解码→重画/排队时延 |

## BUILD-380（阶段四全绿 → 出包）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | `FIX-PREVIEW-HARVEST-380`（HEAD `cc83917`）：路 B 滑窗 `select!` 结果到即收 + `replace_all` 刷新预览；`[LocalRT-DBG-380]` `hook_to_controller_ms`/`stop_to_inject_ms`。版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1547P/0F/31I**（EXIT 0；`feiyin-ime` bin **1459P/29I** = 基线 1546P/31I 净 **+1P**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **98/88/17** = 基线 |
| NEW | +1：`preview_harvest_380_tests::drive_acc_windows_harvests_result_while_acc_open`（GONE 无） |
| 出包 | `BUILD-380` 八项逐项 PASS；产物 `feiyin-ime` 14.78MB `266cd61bc23e…`（12:35:59）/ `feiyin-ime-ui` 10.05MB `21bf38fd7f3a…`（12:32:54）/ `crash-reporter` 24.88MB `6011c8cb2e68…`（12:34:17）；两副本全等、均异于 BUILD-379 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 正：`[LocalRT-DBG-380]`=3 / `hook_to_controller_ms`=2 / `stop_to_inject_ms`=1；反：无（本单未删字面量） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启后端测**；重点：现象①停顿即刷预览；现象②`hook_to_controller_ms`/`stop_to_inject_ms` 定位出字延迟 |

## FIX-SLICE-CUT-AT-GAP-381（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 滑窗切片「固定 13s 硬切」→「10s 起找字缝切」（20ms 帧 RMS ≤ 0.3×本片前 10s 中位数且局部极小；[10,12]s 最低能量兜底；剩余 <11s 不切；严格相接）；`WINDOW_MAX_SECS` 12→10，删 `SLIDING_SLICE_MAX_SECS`；20s/VAD 路径逐位不变。纯平台中立，macOS 同吃 |
| 文件 | `vad.rs` / `local_stream.rs` / `transcription/mod.rs` / 新 `poc_slice_cut_381.rs`（🔴 未碰 `main.rs`） |
| §4 实测 | 字缝 CER **0.0356** vs 固定 10s 硬切均值 **0.0656**（3/4 组更优；off=2.7 硬切 0.1111 + 边界幻觉插入）；🔴 off=0 反例 +0.0044=1 字待裁量 |
| §5 实测 | 生产静默 1200ms 切片（2 片）→ 字缝 7 片 → cap12/cap10 窗口相同；ΔCER **0.0000** ≤0.01 ⇒ 照 10s 交付 |
| 验证 | `cargo fmt --check` EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 `cargo test --no-fail-fast` 0 failed（1475P/31I）｜ numstat==-w |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包（Gavin 已授权「直接走测试出包」）；跨文件 13s 过期注释待路由 |

## TEST-SYNC-381 · 非作者护栏：`plan_gap_cuts` 性质/退化/字缝优先 + 20s 路径与 naive_chunk 逐位快照 · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/transcription/vad.rs` 的 `#[cfg(test)]` 区（+407/−0，**生产代码零改动**）。给 coder-2 的 `FIX-SLICE-CUT-AT-GAP-381`（HEAD `1af7212`）按设计契约补 11 条独立用例 |
| 覆盖 | ① 性质：100 段伪随机（0.5~60s，正弦/噪声/静音，幅度 0.001~1.0）四性质 ② 退化：全零/NaN·±inf/end 越界/长度 10s·11s·11s+1·12s ③ 字缝优先（10.5s 浅 vs 11.5s 更深 ⇒ 精确切 10.5s）④ 20s 路径 + `naive_chunk` **逐位快照**（基线 `1af7212^`）⑤ 非帧对齐 start／空·倒置区间／滑窗 10s 合并阈值 |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **88** = 基线；numstat == -w（407/0）。🔴 **未跑 `cargo test`**（白名单；首跑在阶段四） |
| 结论 | **未发现生产缺陷**（NaN/全零无 panic、每轮切点严格推进无死循环）；无停手项 |

## LOCALRT-VAD-SILENCE-384（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 本地 realtime 静默判定改 **silero VAD 判「有没有人声」**（方案 A），音量阈值兜底。`vad.rs` 新增 `try_new_for_local_silence` / `feed_is_speech`（整块喂入 + 每 chunk 一次 `detected()` + 排空已完成段）+ 3 常量；`local_stream.rs` 新增进程级 VAD 缓存（只加载一次、跨录音复用、失败记住、每次 reset）+ 唯一判定 `chunk_has_speech` + 1200ms 计时补偿 + Debug 埋点 |
| 隔离 | 只改本地 realtime；其它 VAD 用途 / `src/audio` / 在线流式 / `src/main.rs` 均未动 |
| 验证 | `cargo fmt --check` EXIT 0 ｜ `cargo check --all-targets` 0 error、warnings 88=基线 ｜ 全量 `cargo test --no-fail-fast` 0 failed（bin 1500P/32I）｜ numstat==-w ｜ ignore 真模型 300s 队列不增长 PASS（≈0.66ms/chunk） |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯 `silence detector=vad` / `vad cost` / 1200ms 一致性 |

## LOCALRT-NEARFIELD-GATE-385（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 本地 realtime 新增**近场音量门**：VAD 判人声后再要求「chunk 音量 ≥ 录音人音量估计 × 0.25」⇒ 挡掉背景人声（更小）。`local_stream.rs` 新增 `NearFieldLevel`（30s 窗口/80 分位/1s 热身）+ 门纯函数 + `ChunkJudgment`；补偿只认 VAD 自身翻转；Debug 埋点 |
| 隔离 | **只改 `local_stream.rs`**；`vad.rs`/`main.rs`/`audio`/在线/离线管线未动 |
| 关键决策 | 先判门、后更新（防背景人声自我放行）；已知局限：背景人一样近一样大分不开 |
| 验证 | fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test 0 failed（bin 1504P/32I，新增 4 条）｜ numstat==-w |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯 `nearfield gate`/`summary` |

### LOCALRT-NEARFIELD-GATE-385 · 第 1 轮验收退回修复（2026-09-23，coder-2）

- 主控退回：窗口按「已入样本累计时长」滑动 ⇒ 录音人中途降音量后永不过门 ⇒ level 永久锁死、剩余录音全判静默。
- 修复：`NearFieldLevel` 改**按会话音频时间过期**（样本记 `now_ms`，判门前 `prune(now−30s)`）；有效样本 <1s ⇒ 未就绪（门不生效、重新热身）；最坏锁定 ≤30s。补回归单测。
- 验证：fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test 0 failed（bin 1505P/32I，385 共 5 条）｜ numstat==-w（390/18）。

## TEST-SYNC-384-385 · 非作者护栏：兜底/近场门/锁死恢复/背景人声/补偿只认 VAD 翻转 · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/transcription/local_stream.rs` 的 `#[cfg(test)]` 区（+238/−0，生产零改动）。给 coder-2 的 384/385 按契约补 5 条独立用例（`ts384385_`） |
| 覆盖 | ① 随机 500 组兜底恒等 `rms>thr` + 门免疫 ② 门两侧 0.26/0.24 ③ 锁死恢复（1.0→0.2：28.5s 挡、≤31.5s 重开、1.2s 重学）④ 背景 3s 无补记 @1200ms 达派发 ⑤ 补偿只认 VAD 翻转（门挡不补/翻转覆盖 300ms/VAD 关不补） |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** ≤ 基线 98/88；numstat == -w（238/0）。🔴 **未跑 `cargo test`**（白名单；首跑阶段四） |
| 结论 | **未发现生产缺陷**；无停手项 |

## BUILD-385（阶段四全绿 → 出包 · 381/382/384/385 合包）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 381（滑窗字缝切 + `WINDOW_MAX_SECS` 12→10）/ 382（逐片组窗 + 早显处理态 + 回灌提速）/ 384（静默判定改 silero VAD + 音量兜底）/ 385（近场音量门）。HEAD `0c443a5`，版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1598P/0F/34I**（EXIT 0；`feiyin-ime` bin **1510P/32I** = 基线 1584P/33I 净 **+14P/+1I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **97/88/17** = 基线 |
| NEW | 15（14P+1I）：384 **5**（含 ignored `localrt_vad_feed_drains_queue_bounded`）+ 385 **5**（`nearfield385_*`）+ TEST-SYNC-384-385 **5**（`ts384385_*`）；GONE **0** |
| 出包 | `BUILD-385` 八项逐项 PASS；产物 `feiyin-ime` 14.82MB `dc88612f4aaf…`（14:55:58）/ `feiyin-ime-ui` 10.05MB `d51a599cafc4…`（14:52:53）/ `crash-reporter` 24.88MB `ea5beaf8c0f1…`（14:54:05）；两副本全等、均异于 BUILD-380 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 正：`[LocalRT-DBG-382]`=3 / `stop_to_processing_ms`=1 / `reflow suppressed after processing`=1 / `[LocalRT-DBG-384]`=4 / `[LocalRT-DBG-385]`=2；反：无（本批未删字面量） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启后端测**；重点：① 384 静默判定改 VAD（停顿切片更贴语义）；② 385 近场门挡背景人声（录音人优先，背景人声不触发）；③ 382 松键立即「识别处理中」不闪回 + 回灌立即刷新；④ 吃字/重复字；⑤ 长录音 300s；⑥ 不新增 crash.json |

## FIX-TAIL-WINDOW-AND-FALLBACK-386 · 组窗末片延后/收尾短尾合并 + 回灌合成 + 失败窗流式兜底（阶段一·只改 `main.rs`）· 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/main.rs`。**A**：`plan_windows` 重写为 `WindowPlan{windows,pending}`（单片立刻 / ≥2 片末片延后 / 下次派发强制含 pending 可超 10s / 松键收尾 <3s 合并前片重解、否则单独）；滑窗线程接计划 + `dispatch_window!` 宏。**B**：回灌渲染改 `compose_reflow_preview`（= `compose_with_acc_for_gen`），删 `reflow_preview_367`。**C**：失败/空窗用流式文本兜底 |
| 回归 | root `cargo test --no-fail-fast` **1608P/0F/34I**（EXIT 0）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**；warnings **97/88** = 基线 |
| NEW | `plan_windows_386_tests` 8（含覆盖不变量随机性质）+ `fix386_tests` 2；更新 `testsync382_tests` 3 条与 `testsync371` 锚点（不放宽） |
| 端测 | 🔴 待 tester-1 阶段四回归 + 出包 + Gavin 端测：①一口气 >10s 不再念「维生素b12」/丢前文；②结尾不再缺字/冒 `<location>`；③预览中途不再闪回更短文本；④松键短尾与前一并重解 |

## FIX-ACC-OUTPUT-GUARD-AND-GATE-SMOOTH-387（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | D 解码输出守卫（`transcription/mod.rs`）：回显命中不再保留残余、直接不带注入重解；残余全词表视同回显；标签 `<[A-Za-z_/][^<>]{0,30}>` 剥后空/只剩标点 ⇒ 重解（有正文只剥标签）；空 ⇒ 重解；重解无效 ⇒ 空 + `[LocalRT-DBG-387]` warn。E 近场门改 300ms 平滑音量（O(1) 滑动窗口 RMS），修 BUILD-385 误挡 ~70%；兜底逐位同 384 |
| 文件 | `src/transcription/mod.rs` / `src/transcription/local_stream.rs`（🔴 未改 main.rs） |
| 单测 | 新增 6 条（`fix387_*` 5 + `ts387_smoothed_volume_*`），384/385 兜底适配签名 |
| 验证 | `cargo fmt --check` EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test 0 failed（bin 1520P/32I）｜ numstat==-w |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯 `[DBG-387]` / `nearfield summary` gated 比例 |

## TEST-SYNC-387 · 非作者护栏：标签边界/回显残余/重解次数/平滑音量/近场门抗误挡 · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/transcription/mod.rs` + `local_stream.rs` 的 `#[cfg(test)]` 区（130/0 + 115/0，**生产零改动**）。给 coder-2 的 387 按契约补 5 条独立用例（`ts387_`） |
| 覆盖 | ① `strip_angle_tags` 标签边界（剥/不剥各 4+ 组 + 30/31/未闭合长度边界）② 末条残余⇒重解、句中单条⇒不触发 ③ 随机 300 组重解≤1 ④ `EnergySmoother` 平滑波动<逐块 + 30 万块稳定 ⑤ 平滑门误挡<5% vs 原始门>20% + 背景全挡 |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat == -w。🔴 **未跑 `cargo test`**（白名单；首跑阶段四） |
| 结论 | **未发现新生产缺陷**；未改版本/未 push/零凭证 |

## BUILD-387（阶段四全绿 → 出包 · 386/387 合包）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 386（中途末片延后组窗 / 松键短尾合并重解 / 预览回灌保留流式尾巴 / 失败窗流式兜底）/ 387（念词表·`<标签>`·空输出判无效后不带注入重解 + 近场门 300ms 平滑音量）。HEAD `3646b4d`，版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1620P/0F/34I**（EXIT 0；`feiyin-ime` bin **1532P/32I** = 基线 1598P/34I 净 **+22P/+0I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **97/88/17** = 基线 |
| NEW/GONE | NEW **29**（`plan_windows_386_tests` 8 / `fix386_tests` 2 / `slice_streaming_386_review_tests` 1 / `testsync386_tests` 5 / `fix374_terms_echo_tests` 1 / `fix387_output_guard_tests` 5 / `guard387_review_tests` 1 / `local_stream::tests` 3 / `testsync387_tests` 3）；GONE **7**（旧 `reflow_preview_367_tests` 2 + 旧 `plan_windows_382_tests` 4 + `ladder_uses_remaining_and_never_redecodes`）。⚠️ 任务书预估 28/2，实测 29/7（模块重写，净 +22P 逐位吻合） |
| 出包 | `BUILD-387` 八项逐项 PASS；产物 `feiyin-ime` 14.84MB `a592182ca12f…`（17:10:09）/ `feiyin-ime-ui` 10.05MB `1a8650c06fcb…`（17:07:14）/ `crash-reporter` 24.88MB `99096bbcc139…`（17:08:21）；两副本全等、均异于 BUILD-385 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 正：`[LocalRT-DBG-386]`=1 / `[LocalRT-DBG-387]`=2 / `[LocalRT-DBG-385]`=2 / `[LocalRT-DBG-382]`=3；反：无（本批未删字面量） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启后端测**；重点：① 386 中途末片延后组窗 + 松键短尾合并重解（防吃字/尾巴丢）；② 386 预览不再闪回更短（保留流式尾巴）；③ 387 念词表/标签/空输出判无效后重解（防回显/坍塌）；④ 387 近场门 300ms 平滑音量（防误挡）；⑤ 吃字/重复字；⑥ 长录音 300s；⑦ 不新增 crash.json |

## FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388 · 解码前剪静音 + 重解产出率 + 冷启动下限（阶段一）· 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | `transcription/mod.rs`(377/24) + `vad.rs`(70/0，只新增)。**A**：VAD 取语音区间 + `trim_to_speech` 剪静音（首尾 ≤200ms、段间 >400ms 压到 400ms），整窗无语音早退空解码，时长用剪后；**D1**：`has_content` 取代 `is_only_punct` + 重解后所有 kind 统一查产出率；**D2**：`output_rate_ok` 冷启动下限（audio≥3s 且 <1 字/s ⇒ 坍塌） |
| 回归 | root `cargo test --no-fail-fast` **1621P/0F/35I**（EXIT 0）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**；warnings **97/88** = 基线 |
| 契约变更 | D1/D2 更新 6 条既有用例期望（不变量未动）：`ladder_redecodes_on_echo_even_with_residual`、`ladder_redecodes_once_when_strip_is_empty`、`guard_echo_redecodes_and_uses_recovered`、`guard_empty_redecodes`、`rate_ok_cold_start_and_tiny_expectation_always_ok`、`rate375_cold_start_three_paths_ok`（详见 CHANGELOG/result.md） |
| 端测 | 🔴 待 tester-1 阶段四回归 + 出包 + Gavin 端测：松键后处理时延是否回落、是否仍念词表/丢结尾；`[LocalRT-DBG-388] trim` / `[LocalRT-DBG-387] guard` |

## FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | C 近场门改整句段判（`SegmentPeakLevel` 段峰值中位数 + `SegmentGate`：未就绪整句确认 / 段峰值 ≥ level×0.3 即整句确认 / 只学已确认段）+ C2 跨录音沿用（进程内存、同设备 10min、连续 2 拒丢弃 seed、录音结束写回、新增 `vad_device` 入参）+ D3 预览不回退（`PreviewReflow.boundary_usable` + `partial_win_committed` 折算 + 部分窗不重渲） |
| 文件 | `src/transcription/local_stream.rs` / `src/main.rs`（🔴 未改 `mod.rs`/`vad.rs`） |
| 单测 | 新增 14 条（段门 5 / C2 4 / ts389 2 / D3 3）+ 384 兜底与计时适配 |
| 验证 | fmt EXIT 0 ｜ check 0 error、warnings 97/88=基线 ｜ 全量 test 0 failed（bin 1540P/33I）｜ numstat main 180/28、ls 495/685 |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯 `[DBG-389] nearfield summary` rejected 比例、`carry level`、预览不缩短 |

## TEST-SYNC-389 · 非作者护栏：整句段门/背景挡住/防锁死/seed 流程/部分窗折算性质 · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `local_stream.rs`(134/0) + `main.rs`(47/0) 的 `#[cfg(test)]` 区（**生产零改动**）。给 coder-2 的 389 按契约补 5 条独立用例（`ts389n_`） |
| 覆盖 | ① 整句不切（首个高值后到段末全有声）② 背景 0.2 全静默 + 峰值不进样本 ③ 防锁死（35s/0.25 ⇒ 过期后整句确认并重学 ≈0.25）④ seed 流程（0.5 确认 / 两句 0.1 ⇒ `seed_dropped` / `seed_usable` 边界）⑤ `partial_win_committed` 随机 500 组界+单调 |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat == -w。🔴 **未跑 `cargo test`**（白名单；首跑阶段四） |
| 结论 | **未发现生产缺陷**；未改版本/未 push/零凭证 |

## TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390 · 窗口解码改串行 + 按语音时长限制生成长度 · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | `transcription/mod.rs`(138/10) + `main.rs`(2/2 注释)。`WINDOW_DECODE_CONCURRENCY` 2→1（实测：单独 274 vs 并发 476 ms/音频秒）；新增 `max_new_tokens_for` + `decode_accuracy_allow_empty` 的 `max_new_tokens` 参数，`transcribe_acc_ctx` 首解/重解按剪静音后时长带 cap |
| 回归 | root `cargo test --no-fail-fast` **1642P/0F/35I**（EXIT 0）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**；warnings **97/88** = 基线 |
| NEW | `fix390_tests` 3 条（并发度=1 / `max_new_tokens_for` 取值·界·非有限·负·单调·极大值 / 源码护栏：首解重解都带 cap、`decode_accuracy_once` 传 `None`） |
| 端测 | 🔴 待 tester-1 阶段四回归 + 388/389/390 合包 + Gavin 端测：解码耗时/预览刷新是否改善、是否仍念词表 |

## BUILD-390（阶段四全绿 → 出包 · 388/389/390 合包）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 388（解码前 VAD 剪静音 / 重解统一查有内容+产出率 / 冷启动坍塌下限 / 首解空进 Empty）/ 389（近场门按 VAD 整句判定 / 跨录音沿用录音人音量 / 部分窗预览不回退 / C2 写回规则）/ 390（窗口解码并发 2→1 / 按剪后语音时长限制 `max_new_tokens`）。HEAD `02c57c8`，版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1645P/0F/35I**（EXIT 0；`feiyin-ime` bin **1557P/33I** = 基线 1639P/35I 净 **+6P/+0I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **97/88/17** = 基线 |
| NEW/GONE | NEW **6**（`fix390_tests` 3 + `testsync390_tests` 3）；GONE **0**；+6P 与 1639→1645 逐位吻合（与任务书「fix390 3 + testsync390 3」一致） |
| 出包 | `BUILD-390` 八项逐项 PASS；产物 `feiyin-ime` 14.87MB `52bd009437a1…`（19:38:48）/ `feiyin-ime-ui` 10.05MB `3a531013225f…`（19:35:55）/ `crash-reporter` 24.88MB `d62b82cc3bf0…`（19:37:02）；两副本全等、均异于 BUILD-387 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 正：`[LocalRT-DBG-388]`=2 / `[LocalRT-DBG-389]`=4 / `max_new_tokens=`=1；反：`nearfield gate: vad=on`=**0**（385 逐块门已删） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启 + 带 `-debug` 端测**；重点：① 390 解码并发 2→1 + 按语音时长限 `max_new_tokens`（解码耗时/是否截断快语速）；② 388 剪静音（无语音不解码）+ 重解质量把关；③ 389 近场门整句判定 + 跨录音沿用音量 + 预览不回退；④ 是否仍念词表；⑤ 吃字/重复字；⑥ 长录音 300s；⑦ 不新增 crash.json |

## FIX-VAD-FEED-BY-WINDOW-391 · VAD 改逐 512 块喂入（修「说话到一半卡住」P0）· 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/transcription/vad.rs`(200/13)。新增 `feed_in_vad_windows`（逐 512 块喂）；`speech_ranges`/`feed_is_speech` 改走它；注释写 sherpa 依据 + 纠正 384「整块喂入」错误指示。未改常量/`segment()`/pub 签名 |
| 真模型实跑 | 4 段 6s 语音 + 前后各 3s 静音 ⇒ 剪后 **6.44/6.31/6.38/5.78s**；旧整块写法 **0.36s**（复现根因）；`feed_is_speech` 块大小 160/512/1600/16000 差异 ≤ 喂入块 |
| 回归 | root `cargo test --no-fail-fast` **1646P/0F/37I**（EXIT 0）；`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error；warnings `vad.rs` 0 新增（bin 98 的 +1 属 coder-2 在飞的 392） |
| NEW | `vad391_feed_in_vad_windows_counts_ceil`（纯） + `vad391_speech_ranges_keeps_speech_cuts_silence`(#[ignore]) + `vad391_feed_is_speech_block_size_invariant`(#[ignore]) |
| 端测 | 🔴 待 tester-1 阶段四回归 + 392 合包 + Gavin 端测：说话不再中途卡住/只出前半段 |

## FIX-GATE-TIMING-ONLY-392（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 近场门拆分为「门只管时序、内容去留只看 VAD」——修 BUILD-390「说一半卡住只出前半段」（根因：门结果同时驱动时序与内容去留 + 偶数上中位锁死 level）。另：`estimate` 偶数取下中位、首段不学习、埋点 `learned=`/`vad_only_speech_chunks`；VAD 不可用兜底逐位同 384 |
| 文件 | `src/transcription/local_stream.rs`（未改 `vad.rs`/`main.rs`/`mod.rs`、未改 pub 签名） |
| 单测 | 新增 `gate392_*` 4 条；改写 8 条（392 契约变更）；既有 342/346/349/337/384/389/ts389 全通过 |
| 验证 | fmt EXIT 0 ｜ check 0 error、warnings 97/88=基线 ｜ 全量 test 0 failed（bin 1562P/35I）｜ numstat==-w（262/54） |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯「不再中途卡住 / 输出完整 / learned= / vad_only_speech_chunks」 |

## TEST-SYNC-392 · 非作者护栏：门只管时序、内容只看 VAD · 2026-09-23 · coder-1

| 项 | 内容 |
| --- | --- |
| 内容 | 仅 `src/transcription/local_stream.rs` 的 `#[cfg(test)]` 区（189/1，**生产零改动**）。给 coder-2 的 392 按契约补 4 条独立用例（`ts392n_`） |
| 覆盖 | ① 门误判整句不丢内容且静默满 1200ms 恰派发一次 ② 背景 10s + 录音人 2s + 停顿 1.5s ⇒ 恰 2 次派发 ③ 首段不学 + 下中位数（0.02 / 0.1）④ VAD 不可用 ⇒ 两标志 == rms>thr 且随机 300 组标志更新逐位一致 |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat == -w。🔴 **未跑 `cargo test`**（白名单；首跑阶段四） |
| 结论 | **未发现生产缺陷**；未改版本/未 push/零凭证 |

## BUILD-392（阶段四全绿 → 出包 · 388/389/390/391/392 合包）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 388（VAD 剪静音 / 重解把关 / 冷启动下限）/ 389（整句近场门 + 跨录音沿用音量 + 部分窗预览不回退）/ 390（解码串行 + `max_new_tokens` 限流）/ 391（VAD 按 512 逐块喂入）/ 392（门只管时序、内容看 VAD；下中位估计、首段不学）。HEAD `ad08251`，版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1657P/0F/38I**（EXIT 0；`feiyin-ime` bin **1569P/36I** = 基线 1645P/35I 净 **+12P/+3I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **97/88/17** = 基线；🔴 **`guard346_acc_counter_wiring` 由上轮 FAILED 转 `ok`**（`ad08251` 修复） |
| NEW/GONE | NEW **15**（12P+3I）= `gate392_*` 5 + `ts392n_*` 4 + `ts391_*` 3（2 ignored）+ `vad391_*` 3（3 ignored）；GONE **0**（8 条 `seg389_*` 属期望值改写，同名保留） |
| 出包 | `BUILD-392` 八项逐项 PASS；产物 `feiyin-ime` 14.87MB `4746f7bff754…`（21:04:27）/ `feiyin-ime-ui` 10.05MB `b8b8e6444663…`（21:01:30）/ `crash-reporter` 24.88MB `efdd988056f9…`（21:02:37）；两副本全等、均异于 BUILD-390 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 正：`[LocalRT-DBG-388]`=2 / `vad_only_speech_chunks`=1 / `learned=`=1；反：`nearfield gate: vad=on`=**0**（385 逐块门已删） |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启 + 带 `-debug` 端测**；重点：① 391 VAD 逐块喂入（修剪静音吞字是否修复）；② 392 门只管时序/内容看 VAD（不再卡住吞字）+ 下中位估计 + 首段不学；③ 388 剪静音 + 重解把关；④ 390 解码耗时/是否截断快语速；⑤ 是否仍念词表；⑥ 吃字/重复字；⑦ 长录音 300s；⑧ 不新增 crash.json |

## TRANS-NLLB-AND-SENTENCE-BATCH-394（阶段一·交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 离线翻译 opus-mt → **NLLB-200-distilled-600M（CT2 int8）**：官方调用规格（src/tgt lang + target_prefix + 去首 token）；`split_sentences` 分句/子句；逐句批量；解码参数 beam4/lenpen1.0/norepeat3/rep1.1、删 min_decoding 强制、max=源tok×2+16≤256；`looks_truncated` 漏译守卫 + 重译一次；数字/专名日志；`Arc<NllbModel>` 双向共享 |
| 文件 | `src/translation/mod.rs` + 模型 `models/nllb-200-distilled-600M-ct2-int8/`（未改 main.rs/transcription/vad/local_stream；pub 签名不变） |
| 模型 | `mijuanlo/nllb-200-distilled-600M-ct2-int8`（594MB int8；sha256 见 result.md） |
| 验证 | fmt EXIT 0 ｜ check 0 error、warnings 92/87 ｜ 全量 test 0 failed（bin 1572P/40I）｜ **真模型实跑 1P/0F/84.27s** |
| 下一步 | 阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包；端测盯长文本完整性/数字保留 |

## TEST-SYNC-393（阶段三 · 非作者护栏）· 2026-09-23 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 给 `VAD-V6-AND-TIMELINE-REUSE-393` 补 10 条独立用例，**只改 `#[cfg(test)]` 区、生产零改动**；未碰 coder-2 在飞的 `translation/mod.rs` |
| 覆盖 | ①段跨两片各截断 + 首片起点 `pad_before+(ts−ps)` ②首尾相接不产空区间 ③中途 `vad_speech`/尾片 `false` 两调用点源码护栏 ④`plan_timeline_trim(Apply)`+`trim_to_speech` 越界 clamp 不 panic ⑤`WholeWindow` 返回 `samples.to_vec()` 源码护栏 ⑥`lens` 缺长度 0 偏移 + 三片累计偏移 ⑦三数组 `remove(0)` 相邻源码护栏 ⑧路B `speech_ranges: None` 源码护栏 ⑨相邻段不 panic + 两函数音频逐位相等 + `pad_before==0` |
| 验证 | 我的 4 文件 `rustfmt --config skip_children=true --check` **4/4 CLEAN**；`cargo check --all-targets` **EXIT 0**、warnings **92/87 ≤ 97/88**；numstat==-w。🔴 **未跑 `cargo test`**（白名单；首跑阶段四）。⚠️ 全仓 `cargo fmt --check` EXIT 1 系 coder-2 在飞 `translation/mod.rs:1639`，非本单文件 |
| 结论 | 9 条要求全部落地为 10 条用例；**未发现生产缺陷**；未改版本/未 commit/未 push/零凭证 |

## TRANS-394-REWORK（阶段一返工 · 交付）· 2026-09-23 · coder-2

| 项 | 内容 |
| --- | --- |
| 退回项 | R1 🔴 `translator_destroy` 死锁（运行期换向/关翻译 + 退出期都会触发）；R2 ❌ 误删 `derive_target_japanese_kanji…`；R3 ⚠️ `no`/`am`/`pm` 缩写误伤句末 |
| R1-a 取证 | 真模型普通线程 drop + 进程 CPU 采样：**A** `load→translate→drop` 🔴 30s 不返回、CPU 16.48s→22.11s **停涨**（死锁非自旋）、`timeout 75` 退出码 **124**；**B** `load→不翻译→drop` ✅ 1.28~1.64s、exit 0（3 次）⇒ 死锁**只在推理过后**发生 |
| R1-b 修法 | 模型改**进程级单例**：`thread_local! static NLLB_MODEL` + `shared_model()`（复用或 `Box::leak`）+ `TranslationEngine{ model: &'static NllbModel }`；`Drop for Ct2Translator` 保留不触发。代价：≈600MB 常驻到退出。选 `thread_local` 免 `unsafe impl Send/Sync`（生产单 worker 串行） |
| R1-c 测试 | 两次 `new` `ptr::eq` 同指针 / 第二次 **0.005ms** / 干净 drop；源码护栏 `NllbModel::new` 生产区恰 1 次 + `Box::leak` + `thread_local!` |
| R2 / R3 | 从 `HEAD` 恢复日文用例；`EN_ABBREVIATIONS` 21→19（删 `am`/`pm`）、`no` 仅后接数字算缩写（`No. 5`）；补 2 条单测 |
| 文件 | `src/translation/mod.rs` + `collab/troubleshooting.md`（未改 main.rs/transcription/vad/local_stream；pub 签名不变） |
| 验证 | fmt EXIT 0 ｜ check 0 error、warnings **92/87** ≤ 97/88 ｜ translation 26P/0F ｜ `--ignored …translation::tests::trans394` **4P/0F/95.79s，进程 exit 0** ｜ 全量 bin **1585P/1F**（唯一失败 = TEST-SYNC-393 期望值错，主控 `c0baf8c` 已修，与本单无关） |
| 下一步 | 主控验收；阶段三 TEST-SYNC（非作者）→ 阶段四 TEST-EXEC → 出包 |

## TEST-SYNC-394（阶段三 · 非作者护栏）· 2026-09-24 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 给 `TRANS-NLLB-AND-SENTENCE-BATCH-394`（+返工）补 8 条独立用例，**只改 `src/translation/mod.rs` 的 `#[cfg(test)]` 区、生产零改动**；未碰 coder-2 在改的 `ui/src/i18n/*` |
| 覆盖 | ①不丢字不变式（4 组手算句数 6/8/1/2）②中文单 `…` vs `……` + `Node.js 3.14` 不因 `.` 断 ③英文 `no.` 数字后置条件（2/1/1 句 + `ends_with_abbreviation` 直测）④`split_and_merge` 全短合成 1 句==原文 ⑤`finalize_sentence` 三态 calls 1/1/0 ⑥`strip_target_prefix` 非目标不丢首词 ⑦`join_parts` 空串无多余空格 + 英→中直连 ⑧源码护栏（无 `Arc<NllbModel>`/`mem::forget`、`&'static` 字段、`new()?` 先于写缓存） |
| 验证 | 本文件 `rustfmt --config skip_children=true --check` **CLEAN**；**全仓** `cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **EXIT 0**、warnings **92/87 ≤ 97/88**；numstat==-w（232/0）。🔴 **未跑 `cargo test`**（白名单；首跑阶段四） |
| 结论 | 8 条要求全部落地；**未发现生产缺陷**；未改版本/未 commit/未 push/零凭证 |

## BUILD-395（阶段四全绿 → 出包 · 393/394/395 合包）· 2026-09-24 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | 393（VAD v6.2 全管线 + 本地 realtime 时间线复用剪静音）/ 394（NLLB-200-distilled-600M int8 + 逐句批量 + 漏译重译 + 模型进程级常驻永不析构）/ 395（翻译热键页文案三 locale）。HEAD `1a8f00d`（=`240042b`+docs），版本 0.9.3 |
| 回归 | root `cargo test --no-fail-fast` **1682P/0F/45I**（EXIT 0；`feiyin-ime` bin **1594P/43I** = 基线 1657P/38I 净 **+25P/+7I**）；`src-tauri` **92P/0F/0I**；**Vitest 7 files/100P/11S/0F**；`cargo fmt --check` **EXIT 0**；warnings **92/87/17/9** ≤ 基线 |
| NEW/GONE | NEW **57**（50P+7I：393/394/TS-393·394 用例 + 7 ignored 实跑）；GONE **25**（opus-mt 专有：`segment_text_*` 7 + 旧解码参数 7 + metaspace 2 + 双目录清单 6 + 旧分段 3）；🔴 `derive_target_*` 五条仍在且全绿（DEC-082） |
| 出包 | `BUILD-395` 十项逐项 PASS；产物 `feiyin-ime` 14.92MB `42e16d64889b…`（00:16:10）/ `feiyin-ime-ui` 10.05MB `61cc642c0881…`（00:13:21）/ `crash-reporter` 24.88MB `3b54e58f9d2a…`（00:14:26）；两副本全等、均异于 BUILD-392 |
| 模型同步 | `Publish/models/silero-vad` v4→v6.2 覆盖（`1a153a22…`）；新建 `Publish/models/nllb-200-distilled-600M-ct2-int8/` 四文件（sha 与源逐一相等）；`target/release/models` 只核联接与 sha（未复制）；`opus-mt-*` 未删 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ Publish/models 1.7B 七文件与源逐一 sha256 全等（0.6B 保留） |
| 探针 | 二进制正：`[LocalRT-DBG-388] trim:`=2 / `source=`=2 / `timeline fallback`=1 / `NLLB model loaded once`=1；反：`opus-mt-zh-en`=**0**；UI：`说中文译为英文` 在 ui/dist =1 |
| 冒烟 | 正常退出（对 controller 窗口投 `WM_CLOSE`，非强杀）⇒ 进程 **1s 内消失**、无 crash.json ⇒ **394 退出不卡死实证通过** |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启 + 带 `-debug` 端测**；重点：① 393 VAD v6.2 全管线 + 剪静音时间线（吞字是否绝迹）；② 394 离线翻译换 NLLB（质量/漏译重译/长文本完整性）；③ 退出不卡死、关翻译不卡死；④ 翻译热键页文案三语言；⑤ 数字/专名保留；⑥ 长录音 300s；⑦ 不新增 crash.json |

## BUILD-396（小改动短路径 · 直接出包）· 2026-09-24 · tester-1

| 项 | 内容 |
| --- | --- |
| 内容 | `LOCALRT-NO-HOTWORDS-396`（DEC-083）：本地实时 B 路径 1.7B 滑窗精解不再注入词库（`load_hotwords_for_accuracy` 对 LocalRealtime 早退 None）。HEAD `5218c92`（=`660378a`+docs），版本 0.9.3。**Gavin 指示不跑全量回归** |
| 简单验证 | `cargo fmt --check` **EXIT 0**；`cargo test --bin feiyin-ime -- fix396` **1P/0F** |
| 出包 | `BUILD-396` 八项逐项 PASS；产物 `feiyin-ime` 14.92MB `1fa081c4bde4…`（00:49:59）/ `feiyin-ime-ui` 10.05MB `34bb0e0c7b9c…`（00:46:59）/ `crash-reporter` 24.88MB `5579119aa19d…`（00:48:08）；两副本全等、均异于 BUILD-395 |
| 模型 | 无变化，只核：`target/release/models` 仍 Junction；`Publish/models` silero `1a153a22…` + NLLB 四文件 sha 与 BUILD-395 一致；`opus-mt-*` 未删 |
| 三特殊点 | ① dll 四张三副本全等 + onnxruntime 1.28.2；② itn-rules 三副本全等；③ 1.7B 七文件与源全等（0.6B 保留） |
| 探针 | ⑦ **不可构造**（纯逻辑改动、无新增/删字面量）⇒ 三证：源码引用（`fix396` 护栏）+ 时间戳 + sha 异于上包 |
| 冒烟 | 正常退出（`WM_CLOSE`，非强杀）⇒ **1s 内消失**、无 crash.json |
| 端测 | 🔴 **Step1 强杀输入法 ⇒ 请重启 + 带 `-debug` 端测**；重点：本地实时 1.7B B 路径**不注词库后识别是否正确**（BUILD-395 端测 18 窗 5 异常 = 28% 是否消失）、窗空/坍塌/念词表/截尾/跳词条是否绝迹；Accuracy/在线档词库注入仍生效 |

## FIX-NOSPEECH-WINDOW-414（阶段一 · 交付）· 2026-09-25 · coder-2

| 项 | 内容 |
| --- | --- |
| 内容 | 时间线判本窗无语音但流式非空 ⇒ **自跑 VAD 复核**（`TimelineTrim::Revad`），不再整窗 11.25s 送解；复核无语音 ⇒ 空结果交流式兜底（不进模型）；VAD 不可用 ⇒ 原样整窗。`Revad`/`None` 共用新抽 `self_vad_ranges` + 纯函数 `plan_self_vad_trim`（非复制） |
| 文件 | `src/transcription/mod.rs`（只此一件）；另 `docs/MACOS-HANDOFF.md` 记跨平台 |
| 不变项 | `Apply`/`Empty` 逐位不变；`None` 输出逐位一致；388 早退/390 cap/406/408B 声纹/412 合并 源码锚点护栏全过 |
| 影响面 | 调用方仅本地实时滑窗路 B（`main.rs:9272/10466`）+ `audio/mod.rs` PoC + PoC bin；本地精确档/在线不调用（grep） |
| 测试 | `fix414_*` 3P + 1I（`kv_long.wav` 真模型复核，实跑 PASS）；`ts393_plan_empty_with_streaming_needs_revad` + 源码护栏 `ts393c_revad_branch_uses_recheck_source_guard` |
| 验证 | `cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings **92/87**=基线、mod.rs 0；全量 `cargo test` **0 failed** |
| 状态 | 阶段一交付，待主控验收；未 commit / 未 build release / 版本 0.9.3 未动 / 零凭证 |

## v0.9.3（BUILD-398 ~ BUILD-452）· 2026-09-24 ~ 09-27 · 本地实时精解质量 + 声纹 + 接缝对齐（版本 0.9.3 未升）

> 只列已出包的完成项，括号内为所在包；撤销 / 回退项不入（398 词库前缀已在 399 撤回）。测试、回归、出包明细见 `CHANGELOG.md`。

### 新功能

- 408 声纹过滤：CAM++ zh_en，自动为使用人建声纹，高置信非本人语音在解码前剔除（BUILD-409）
- 418 `-debug` 下保存整段录音，便于离线重放定因（BUILD-419）
- 399 VC++ 运行库随程序目录发布（DEC-085 app-local），干净系统免装运行库（BUILD-399）

### 优化

- 397 离线翻译 CT2 重编开 oneDNN + OpenMP、线程 `min(核数,8)`，提速 10.8~15.6×（BUILD-398）
- 405 去影子收尾（DEC-086）+ 流式显示文本增量拼接 + 日志计时守卫 + CT2 不再无故重编 + 出包脚本 exe 名修正（BUILD-409）
- 407 长静默末尾组窗；423 松键收尾统一为同一末尾组窗形状（BUILD-409 / 429）
- 413 常规窗前文只带前一片末尾后缀，末段解码更快；427 所有常规窗都带前一片后缀（BUILD-415 / 429）
- 412 相邻短段拼成连续说话单元再判声纹；432 声纹判定门槛 2.0→1.5s（注册仍 2.0s）（BUILD-415 / 432）
- 415 浮层量宽结果缓存；417 流式预览线程 `min(核,4)`（BUILD-415 / 419）
- 419 口水词规则 C：紧邻相同整段只留一份（BUILD-419）
- 420 回灌渲染改用预览原始流式文本做底稿（BUILD-420）
- 接缝对齐四步：431 半全局对齐定接续点（BUILD-432）→ 433 重叠区由预览原文裁判 + forced/估算层 → 434 前一片后缀切点只落 ≥120ms 真停顿 → 435 按读音加权对齐（新依赖 `pinyin 0.11.0`）（BUILD-435）
- 436 接缝改为锚点拼接为主（两边写法一致字为锚，前窗取到锚点、后窗取其后），433 裁判降为兜底；59 接缝回放 CER 0.090→0.079（DEC-087）（BUILD-437）
- 437 连续说话满 10s 强制切片时，优先在 8~10s 内从 10s 往前找真停顿切，片更短、解码更快（BUILD-437）
- 438 预览标点改为每 3.5s 只重打未交给精解的尾巴（删 1200ms 静默打点），说话中也能看到标点，精解段标点不被覆盖（BUILD-438）
- 440 去口水话新增规则 D：折叠「说一半重说」（如「你们的知，你们的知识」→「你们的知识」）（BUILD-441）
- 441 本地实时去重只过一遍、前移到回灌刷新前：预览与上屏同一份已去重文本（BUILD-441）
- 442 派发触发补「满 10s 时长」：连续说话 / 背景人声不停时也在录音中途派发，精解、声纹剔除、回灌不再等到松键；切点与派发后兜底切片同一函数（BUILD-442）
- 446 声纹判定最短时长 1.5s→1.0s，1 秒以上的他人短句也能在解码前剔除（BUILD-446）

> 🔙 447 碎片拼组、448 滑窗子剔除已在 BUILD-449 撤除（DEC-092：Gavin 反馈有时长损耗、明显迟滞），声纹回到 446，不再列入。
- 450 精解前剪静音 VAD 与声纹模型只加载一次、模型加载时预热，每次录音首段精解快约 0.5s（BUILD-452）
- 452 精解引擎换 llama.cpp（Vulkan / Metal / CPU 自动选，Q8_0 + f16）：回放 CER 7.87%→4.63%、精解约 2.2×；预览文字作推测解码草稿（再快 12%）；本地实时恢复词库注入；精解结果逐字流式刷新预览（平均提前 0.2s）（BUILD-452，DEC-093）
- 453 精解引擎省显存约 0.72G（上下文缓存 8 位、计算批次 128、音频编码器 Q8_0，识别与速度基本不变）；离线翻译模型用到才加载，未用翻译或走在线翻译时省约 0.6G 内存（待出包）
- 454 声纹：重叠窗口里重复出现的同一段话不再重复算声纹，每窗精解前省约 20~80ms，判定结果不变（待出包）

### Bug 修复

- 446 旁有他人短句（1~1.5s）声纹不判、精解失败时流式兜底把背景误识别写进最终文字（BUILD-446）

- 445 停顿后流式模型滞后吐出的字挂在预览末尾、与最终上屏不一致（BUILD-445）

- 443 旁有他人说话时流式预览被带偏，正确的精解反被同窗比对守卫当幻觉拒收，预览一直挂错字（BUILD-443）

- 442 背景人声连续不停顿时，剔除结果到松键都不回灌、预览一直挂着背景人声（BUILD-442）

- 439 录音中点浮层进编辑卡在「识别处理中」、编辑框不出现（382 引入）；Esc / 停止按钮取消时不再闪「识别处理中」（BUILD-441）

- 438 预览标点重打导致流式尾巴与精解前缀错位（多字/丢字）的隐患：已派发前缀冻结后根治（BUILD-438）

- 406 精解幻觉 / 变短覆盖正确预览（同窗流式比对守卫）+ 未闭合 `<标签` 漏网（BUILD-409）；410 末尾窗与 406 守卫配合修正（BUILD-410）
- 411 多片切分时整段流式记在第一片 ⇒ 最终文本重复（BUILD-415）
- 414 时间线无语音但流式非空 ⇒ 整窗送解返回空（改 VAD 复核）（BUILD-415）
- 416 413 后长句前文后缀重复（BUILD-419）
- 421 声纹剔除后兜底把他人语音放回；429 声纹整窗全剔后他人语音仍留在预览（BUILD-421 / 429）
- 426 精解解码出错被跳过，重解改指定本窗语种（BUILD-429）
- 430 焦点丢失回显窗：长文本不能滚动、窗太窄、按钮盖住文本（BUILD-435）
