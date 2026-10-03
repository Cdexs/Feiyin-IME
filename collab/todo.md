# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。

**🚀 下一包**

| 单号 | 内容 | 负责 | 文件 | 状态 |
| --- | --- | --- | --- | --- |
| `LOCALRT-WARM-CACHE-450` | Gavin 09-27「仔细review本地实时管线……让预览更新和精确识别结果能更快」「但不能影响功能」。主控日志取证：每次录音首窗精解前多等 ~400–550ms（声纹 CAM++ 与剪静音 VAD 为精解线程级缓存，线程每次录音新建 ⇒ 每次重载）。方案：两者改进程级缓存 + 随本地实时模型加载预热，模型 / 参数 / 调用顺序不变 | 主控 | speaker.rs + transcription/mod.rs | 🔄 Gavin「做」，开发中 |
| `RESEARCH-GPU-ACCEL-451` | Gavin 09-27「评估下GPU加速方案」「要同时支持amd、intel、nvidia显卡」「测」。✅ PoC 完成：llama.cpp Vulkan（780M）精解 2.4×（倍率 0.340→0.140），llama CPU 也快 12%；识别不劣于现状（sherpa 首解 28 片 6 片异常 / llama 0）。建议立项换精解引擎。全文 `collab/research/gpu-accel-451.md` | 主控 | 仅离线 | ✅ 已完成 → 立项 452 |
| `ACC-ENGINE-LLAMACPP-452` | Gavin 09-27：「换，不保留目前的 sherpa onnx 调用方式」「要做好新模型和调用方式的调优，特别是调用线程，还有其他的调用参数啊、传入方式……一定要用最优的方式来调用，最大化的使用它的功能和性能」⇒ 1.7B 精解整体换 llama.cpp（Vulkan / Metal / CPU 自动选），删 sherpa 1.7B 调用。选型 Q8_0+mmproj f16（Gavin「用 Q8+f16」）；适配层 `native/llama_asr/shim.cpp` + `llama_asr.rs` 自检通过（与 server 贪心 103/104 同）。Gavin 追加「回灌刷新你可以直接做」「注入之前的文本让它续写，还有注入词库……直接接入」⇒ 五步：①换引擎 ②预览草稿 ③词库+上下文 ④续写接缝（原裁判兜底）⑤流式回灌；完整管线回放验收后一次出包 | 主控 | transcription/mod.rs + main.rs + llama_asr.rs + shim.cpp | ✅ ①②③⑤ 完成（DEC-093；上文注入 / 续写实测劣于现状不采用）→ 合包 BUILD-452（含 450） |
| `MEM-453` | Gavin 09-29「GPU 的显存和内存占用很大……还有没有内存占用的可优化空间」→ 批 1（KV q8_0 + ubatch 128）、2（NLLB 懒加载；闲置释放因 CT2 析构死锁不可做）、5（编码器 f16→Q8_0，「目前没有用户用 FP16，不用考虑回退」）；A/B 后「可以接受」。显存省 ~0.72G、NLLB 未用时省 ~0.6G 内存 | 主控 | shim.cpp + llama_asr.rs + main.rs + fetch 脚本 + src-tauri | ✅ BUILD-454 已出包，待 Gavin 端测 |
| `VOICEPRINT-EMB-CACHE-454` | Gavin 09-29 问「声纹识别只在录音派发后端精确解码时才触发一次吗」→ 主控查码：精解前判定、重叠窗重复计算 → 建议同段只算一次 → Gavin「做」。按单元音频指纹缓存声纹向量，判定仍现算（结果逐位不变） | 主控 | speaker.rs | ✅ BUILD-454 已出包，端测看 `emb_cached=true` |
| `FORCED-ALIGN-456` | Gavin 09-29「就上 0.6B」（前序：「评估下集成强对齐模型（带时间戳输出）的方案，为了窗口重叠的处理和预览界面处理更精确」；POC-455 实测 0.6B 时间戳前后窗自洽中位 0.03s、98.4% ≤0.1s，按时间拼接去掉 5 处重复半句、≥8 字重复 1→0）。分步：A shim 对齐函数（42ailab 三件套，时间戳头只算窗长内的档）→ B Rust 封装 + 单元切分 + 实测延迟 / 显存 + 与 CrispASR 结果对账 → C 接入：精解后对齐（剪后坐标换回窗内）→ `OrderedReflow` 按时间拼接（无合格时间戳回落现行链）→ 预览半截边界 / 兜底删字 → D 模型分发（缺模型 = 功能关闭、回落现行，不阻断精解）→ E 生产代码回放验收 （Gavin 追加：「一步到位吧，不要保留旧链路当兜底了，留 371 的「按比例估算」这一层」⇒ 删 431/433/435/436 接缝估算链，只留 371 比例估算兜底；对齐模型随本地实时模型标配下载）| 主控 | shim.cpp + llama_asr.rs + transcription/mod.rs + main.rs + fetch 脚本 + src-tauri | ✅ BUILD-456 已出包（v0.9.4），待 Gavin 端测 |
| `PIPE-SPEED-457` | Gavin 09-29「整个处理的管线有没有可以优化的地方，能够把这一次加对齐模型多出来的 0.25 秒能够消化掉。甚至能够更快一些，包括预览和精确的最终结果」→「你上面的优化方案 1234 都做」。① 对齐移出关键路径（精解完先回灌，空闲时对齐再修正接缝）② 松键时最后一窗不等对齐 ③ 系统提示 / 词库前缀 KV 复用 | 主控 | main.rs + shim.cpp + mod.rs | ✅ BUILD-457 已出包（最终 main `ba491a57…` / ui `50b150b4…` / crash `7a176aa1…`；运行日志已见重拼与末窗跳过对齐）→ 待 Gavin 端测：接缝、预览微调、松键出字速度 |
| `MEM-TRIM-457` | Gavin 09-29「显存和内存的占用，还有没有可以再进一步优化的空间？但不能影响功能和性能」→「1234 都做」。① 1.7B `n_outputs_max` 512→16 ② 1.7B 上下文按实际需要分配、不够自动扩 ③ 词库计数改用已加载的 1.7B 词表，去掉单独的 tokenizer.json ④ 本地实时 VAD 分段器按需加载 | 主控 | shim.cpp + llama_asr.rs + mod.rs + main.rs + src-tauri + fetch 脚本 | ✅ BUILD-457 已出包；运行态实测工作集 1.6G→1.30G、GPU 共享 0.905G→0.666G、专用持平 3.74G |
| `QUANT-AB-457` | Gavin 09-29「第四项……你跑一下，我看一下 Q6K 的量化是不是比 Q8 的量化精确度要高？……是不是用 Q6K 的量化加上 FP16 的编解码器？」⇒ {Q8, Q6_K} × {编码器 Q8, f16} 四组，对 bf16 逐窗字差异 + 5 段人工参考 CER + 速度 + 显存；结果交 Gavin 定 | 主控 | 仅测试 | ✅ 测完：**不采纳**，保持 Q8 + Q8（DEC-096）——Q6_K + f16 与 bf16 字差 0.93%（现行 0.37%）、CER 5.00%（现行 4.80%）、慢 9%，只省 0.24G |
| `LLAMA-TUNE-458` | Gavin 09-30「你再仔细分析评估一下现在用 llama.cpp 调用 1.7B 精确模型的调用方式，看有没有可以再优化的参数，调用规格方面有没有可以再优化的地方和空间」⇒ 逐项盘现行参数（线程 / batch / ubatch / FA / KV / 输出上限 / 上下文 / 草稿推测解码 / 编码器 / 预热 / 采样）+ 实测候选项，出评估报告交 Gavin 定；Gavin 追加「另外对齐模型也是，也评估分析一下」⇒ 0.6B 对齐器同样逐项评估；Gavin「所以你也要把整个 VAD 模型调用通盘考虑进去，看有无可以优化的地方」⇒ 本地实时全部 VAD 调用一并盘点 | 主控 | 评估（shim.cpp + llama_asr.rs） | ✅ 评估完成（`collab/research/llama-tune-458.md`）：建议①草稿加语种前缀 + 上一窗精解文字（−13%、CER 不变）②VAD 兜底改用实时时间线（低优先级）；其余参数现行已最优 → Gavin「一起做」⇒ DRAFT-LANG-PREV-458 + VAD-REUSE-458 |
| `DRAFT-LANG-PREV-458` | Gavin 09-30「一起做」（LLAMA-TUNE-458 建议①）：1.7B 推测解码草稿加「语种标记 + 上一窗精解文字」。回放：每窗 −80ms（−13%）、5 段 CER 不变、终稿差异仅标点 | 主控 | shim.cpp + main.rs | ✅ 开发完成（DEC-097；生产默认复核 CER 4.80% 不变）→ ✅ BUILD-458 已出包（main `d2b5adcf…`）→ 待 Gavin 端测 |
| `VAD-REUSE-458` | Gavin 09-30「一起做」（LLAMA-TUNE-458 建议②）：末尾窗 / 说话中途切出的片段不再整窗重跑 VAD，改用录音中实时 VAD 的时间线（说话中途的片段保守地把最后一个完整语音段之后全算语音，保证不吞字）。真实 24 窗中 9 窗走兜底、每窗 39ms | 主控 | local_stream.rs + main.rs | ✅ 开发完成（DEC-097；按「开始说话」判定位置 −0.5s 补齐，真实录音 14 个中途片 0 吞字）→ ✅ BUILD-458 已出包 → 待 Gavin 端测（看 debug.log `source=timeline` 占比）|
| `PHANTOM-406-459` | 主控 09-30 查 Gavin 端测日志发现（BUILD-458，非本批引入）：录音 1 首窗流式模型在**开录 0.46s 的静音里**冒出「他有这个事」（342 已判「静音段文字」只在预览里压掉），精解正确没有它（「今天的天气很好。」8 字）；406 护栏按流式 13 字判精解「保留度 0.50 不够」⇒ **改用含幻听的流式文本**；该窗无逐字时间 ⇒ 下一窗接缝回落 371 比例估算、多切 13 字 ⇒ 原始转写「**他有这个事儿**，今天天气很好。去走一走吗」（幻听 + 丢「然后你要出」），终稿靠 LLM 润色才正常。候选修法：① 406 比对基准与兜底文本剔除 VAD 判静音时段产出的流式字 ② 兜底文本也做强制对齐，接缝用时间拼接而非比例估算 | 主控 | local_stream.rs + main.rs + llama_asr.rs | ✅ 开发完成（DEC-098；①真录音 17 段只动 2 段派发片开头、预览不变 ②兜底窗对齐：终稿去重复 / 去杂字、补回丢字，CER 不变）→ ✅ BUILD-459 已出包（main `db8c5e74…`）→ 待 Gavin 端测 |
| `VOICEPRINT-LEAK-460` | 主控 09-30 查 Gavin 端测日志（BUILD-459，Gavin 确认「在放视频 / 别人的声音」）：4 段录音声纹把他人语音全部判 DropNonUser（0.17~0.27）⇒ 滑窗精解全空（正确）⇒ `acc_parallel_result_usable` 判空 ⇒ `pretranscribed=None` ⇒ **松键后整段兜底解码**（`main.rs:16277`「Transcribing N samples … asr_model=accuracy」）**不经声纹** ⇒ 他人话照样打出来（「刚才的这番打斗……梅姨走了进来」）。非本批引入。候选修法：滑窗结果为空且原因是声纹全剔（保留语音 0、剔除 >0）⇒ 不走整段兜底、按「未检测到语音」处理 | 主控 | main.rs（收尾兜底） | ✅ BUILD-460 已出包（DEC-099），待 Gavin 端测 |
| `KOJA-406-461` | Gavin 10-03「为什么我刚才在本地实时管线上录入韩文和日文识别不了……设计时原则上要求支持中英韩日四种语言的输入」。查因（当次未开 -debug，无日志 / 录音，按代码定位）：流式预览模型是 `trilingual-zh-cantonese-en`（只认中 / 粤 / 英），说韩日语时吐中文同音乱码；406 护栏 `acc_vs_streaming` 按字 LCS 拿精解（韩 / 日文）比流式（中文乱码）⇒ 保留度≈0 ⇒ 判精解丢字 ⇒ **改用流式乱码**作该窗结果；精解为空时同样兜底成乱码。简繁转换已对含假名 / 谚文的文本跳过，不是原因。候选修法：精解文本含假名 / 谚文（或模型自判语种为 Japanese / Korean）⇒ 406 不比对、直接采纳精解，且该窗不以流式兜底 | 主控 | transcription/mod.rs（406）+ main.rs（兜底） | ⏸ 暂缓（Gavin 10-03「那暂时先不改吧」）。**已取证**（`poc461_koja_406`，生产解码路径）：日 1 + 韩 5 段精解全部正确，406 保留度全 0.00 ⇒ 全被换成流式乱码；中 / 英各 1 段正常采纳精解 |
| `STREAM-KOJA-462` | Gavin 10-03「那你还是先研究评估一下前端的流式模型有没有可以支持日文和韩文的，然后最好性能的损耗、内存占用等各方面和现在的流式模型差不多，不能太耗费资源……精度方面也不能损耗」⇒ 调研支持中英日韩的流式 / 准流式模型，对现行 `streaming-paraformer-trilingual-zh-cantonese-en` 实测对比资源（内存 / CPU / 延迟）与准确率，出评估报告交 Gavin 定 | 主控 | 评估 | ✅ 评估完成，Gavin 10-03「好，那替换吧」⇒ 转 STREAM-SV-463：推荐官方 SenseVoice-Small 2024-07-17 int8 模拟流式（`poc462_simstream`，证据 `collab/evidence/462/simstream462.md`）——5 段 CER 9.3% vs 现行 20.5%、日韩近全对、首字 0.45s vs 0.54s、CPU 实时率 0.21 vs 0.04、并发拖慢 1.7B ≤1.5%（噪声级）、模型 239MB vs 238MB；隐患：裸显示闪动 26%（稳定前缀显示降到 5% 但首字 1.14s ⇒ 拟「已定部分正常色 + 未定尾巴浅色」）、长句重解 20s→0.92s 超过 0.6s 刷新步长 ⇒ 须限重解窗口 ≤10s |
| `STREAM-SV-463` | Gavin 10-03「好，那替换吧」⇒ 本地实时管线流式预览由 `streaming-paraformer-trilingual` 换成官方 SenseVoice-Small 2024-07-17 int8 模拟流式（每 0.6s 重解当前语音段，重解窗口 ≤10s，更早部分冻结）；预览浮窗「已定部分正常色 + 未定尾巴浅色」；盘全流式文本的全部下游用途（406 护栏 / 草稿 / 幻听剥离 / 端点等）逐一对齐；模型随包与就绪检查同步 | 主控开发 → tester-1 回归出包 | 开发 | ✅ 开发完成（DEC-100；`22a6884` 后续提交）：预览 = 官方 SenseVoice 2024-07-17 模拟流式（8 线程 = 物理核）、派发点即冻结、末尾未定字浅色；performance 档同一模型；BUILD-463 已出包（tester-1：bin 1910P/0F/77I、九项 PASS、main `2fada242…`，新模型已进 Publish/models）⇒ 待 Gavin 端测（日韩预览 + 浅色尾巴目视）；旧模型目录待 Gavin 确认删除（清单 `collab/evidence/463/pending-delete-models.md`） |
| `PREVIEW-PUNCT-464` | Gavin 10-03「现在最新的这个模拟的前端预览是不带标点符号的预览吗？能不能前端的这个模拟的流式预览带标点符号？这样看起来体验更好」⇒ 取证（`poc464_preview_punct`，证据 `collab/evidence/464/preview-punct.md`）：现行 3.5s 间隔打点末尾未打点平均 40 字、后半段几乎无标点；SenseVoice 自带标点 0 字但「二比一→2B1」「8分之1」、说话中 99% 刷新挂「。」、切点处句中「。」；每次刷新都打点 21 字、耗时 +5% ⇒ 拟：每次刷新打点 + 停顿派发时补句末标点 + 日韩文不送标点模型；Gavin「按你的方案来」 | 主控开发 → tester-1 回归出包 | 开发 | ✅ 开发完成（DEC-101）：预览一变就打 + 停顿补句末标点 + 日韩不送标点模型；SenseVoice 标点 / ITN 不可分（研究结论入 DEC-101）；BUILD-464 已出包（bin 1912P/0F/78I、九项 PASS、main `289b59db…`）⇒ 待 Gavin 端测 |
| `KOJA-PUNCT-464` | 主控取证发现：最终输出标点节点（本地实时 350 剥光重打 / performance 档）用的 CT-Transformer 会把**韩文空格全吃掉**（「조금만생각을하면서…」）、日文只补句末「。」且剥掉 1.7B 自己的句中标点 ⇒ 日韩文即使识别对，上屏也不对；拟：含假名 / 谚文的文本不剥不重打（本地实时保留 1.7B 标点）、performance 档不送标点模型；Gavin「按你的方案来」 | 主控开发 → tester-1 回归出包 | 开发 | ✅ 开发完成（DEC-101）：预览一变就打 + 停顿补句末标点 + 日韩不送标点模型；SenseVoice 标点 / ITN 不可分（研究结论入 DEC-101）；BUILD-464 已出包（bin 1912P/0F/78I、九项 PASS、main `289b59db…`）⇒ 待 Gavin 端测 |
| `PUNCT-MODEL-465` | Gavin 10-03「你研究下有没可以同时支持中英日韩文字的标点模型，模型的精度、大小、资源占用起码和现在用的都差不多的（或者更优的）」 ⇒ 调研 + 实测：唯一候选 punct_cap_seg_47_language（47 语、6 层 512 维、ONNX 233MB）；对比现行 CT-Transformer（75MB）中文金标准标点 F1 / 内存 / 耗时 + 日韩英定性 | 主控 | 调研 | ✅ Gavin 10-03「那就按照你的建议来」⇒ 转 PUNCT-JAKO-466；实测（`collab/evidence/465/punct-models.md`）：中文候选明显更差（位置 F1 84.7→77.1、句号 85→58、耗时 26→100ms）⇒ 不能整体替换；日韩英候选明显更好 ⇒ 建议「中英 CT + 日韩专用候选 int8（59MB、按需加载）」 |
| `PUNCT-JAKO-466` | Gavin 10-03「那就按照你的建议来，中文英文继续用现在的标点模型，日韩文换用候选模型的压缩版。但是日韩文的标点模型要采用延迟加载，不要程序一启动就加载。如果探测到用户输入的是日文或者韩文，才会加载对应的标点模型」 ⇒ 接入 punct_cap_seg_47lang int8（59MB）作日韩专用标点：预览 + 最终标点节点遇假名 / 谚文时调用；首次探测到日韩文才加载（进程内只加载一次）；中英仍走 CT-Transformer | 主控开发 → tester-1 回归出包 | 开发 | ✅ BUILD-466 已出包（9 子项全完成）⇒ 待 Gavin 端测 |
| ↳ `466-1` | 模型文件：int8 模型 + 分词模型放 `models/punct-cap-seg-47lang-int8/` | 主控 | 文件在位、sha 记录；Publish 由 tester 复制 | ✅ `models/punct-cap-seg-47lang-int8/`：model.int8.onnx `fa630cad…`（58.7MB）+ 分词模型 `1bc15b6e…` + config + README（来源 / 量化方法） |
| ↳ `466-2` | 依赖：`ort` 2.0.0-rc.13（只开 std + load-dynamic），复用程序自带 onnxruntime.dll 1.28.2，不另打包 | 主控 | cargo check 通过；产物不新增 ORT 二进制 | ✅ `ort = "=2.0.0-rc.13"`（std + load-dynamic），复用 onnxruntime.dll 1.28.2；cargo check 0 error、warnings 90 |
| ↳ `466-3` | 日韩标点模块 `src/punctuation/jako.rs`：小写分词带原文位置 → 126 子词分窗重叠 16 → 推理 → 标点按位置插回原文 | 主控 | 单测：分窗 / 插回 / 跳过空子词 / 不叠已有标点；真模型：与 Python 参考实现标点一致、无 <Unk> | ✅ `src/punctuation/jako.rs`；单测 3 条（分窗 / 原文位置映射 / 插回不叠标点）；真模型：韩文与参考实现逐字一致，日文分词 35 子词全同、标点 34/35 同（差 1 处为 ORT 1.28 vs 1.30 近似并列），无 <Unk>、大小写保留 |
| ↳ `466-4` | 延迟加载：首次探测到假名 / 谚文才加载，进程内一次；失败只记一次、不重试、原样返回 | 主控 | 护栏：只在判出日韩文的分支调用；日志记首次加载耗时 | ✅ 只在判出日韩文后调用（护栏 `jako466_only_called_after_jako_detection`）；最终输出同步加载（实测约 0.5s，仅首次）；预览**后台**加载、未就绪先原样显示，不卡预览；失败只记一次 |
| ↳ `466-5` | 本地实时预览接入：日韩尾巴改用日韩标点（说话中不挂句末、停顿补句末照旧） | 主控 | 端到端：日韩预览带标点、韩文空格完好 | ✅ `preview_display` 日韩尾巴走 `punctuate_if_ready`；端到端：韩文预览「… 편할 거야.」空格完好、日文带「、。」 |
| ↳ `466-6` | 最终输出接入（本地实时 + **performance 档**，Gavin 追加）：`apply_local_punctuation` 遇日韩文改用日韩标点 | 主控 | 真模型：日韩带标点；中英逐位不变 | ✅ `apply_local_punctuation` 日韩文走 `punctuate`（本地实时 + performance 同一节点）；真模型：韩「봤는데, 오늘은 … 생각이에요.」、日「場合は、…買う。」、中文仍 CT「…五点，我们明天见。」 |
| ↳ `466-7` | tester-1 全量回归 + 出包（Publish 带新模型目录） | 主控 | 回归全绿、九项 PASS | ✅ bin 1916P/0F/79I、root 2004P、src-tauri 92P、Vitest 100P；三条真模型全绿（加载 519ms）；九项 PASS；main `ca0fb60b…`；Publish 带新模型（`fa630cad…`）；onnxruntime 仅原有 2 个文件；冒烟日志日韩模型加载 0 次（启动不加载） |
| ↳ `466-8` | 文档：DEC-102 / 五文档 / MACOS-HANDOFF（新依赖 + 新模型目录 + macOS 动态库路径） | 主控 | 逐份落盘 | ✅ DEC-102、troubleshooting（INT8-ORT-NUMERICS-466）、progress / CHANGELOG / handoffs / logs / MACOS-HANDOFF |
| ↳ `466-9` | 清理：临时 Python 环境与下载的候选模型（结论入 DEC 后，先存清单） | 主控 | 清单在、目录已删 | ✅ 清单 `collab/evidence/465/cleanup-466.txt`；poc465（280M）/ poc465hf（224M）/ venv464（878M）已删，共约 1.4G |
| `MODEL-CLEAN-467` | Gavin 10-03「你整理下目前哪些旧的的模型目前产品已经不用了，列个清单我确认下，没问题就准备清除」 | 主控 | 清理 | 🔄 清单待确认（`collab/evidence/467/model-cleanup-candidates.md`） |
| ↳ `467-1` | 盘点 models/ 与 Publish/models/ 全部目录、逐个核对正式代码引用 | 主控 | 每个目录有结论 | ✅ 6 项停用（约 2.25G）、9 项在用、3 项测试录音保留 |
| ↳ `467-2` | 待删清单交 Gavin 确认 | 主控 | Gavin 明确同意 | 🔄 待确认 |
| ↳ `467-3` | 删除（先存逐文件清单；等 BUILD-466 出包结束再动 Publish，避免与构建冲突） | 主控 | 清单在、目录已删、在用模型校验值不变 | ⬜ |
| ↳ `467-4` | 文档：logs / todo / MACOS-HANDOFF（模型目录变化） | 主控 | 逐份落盘 | ⬜ |
| `TEST-GAP-438-TAILSTART` | 护栏缺口（BUILD-438 消融 438b 未红）：`preview_display` 重打时忽略 `tail_start` 改为整段重打，`fix438_*` 全绿——护栏只直调纯函数、唯一进 `preview_display` 的用例 `engine=None`。主控已读码确认接线正确（不影响本包）。补：带真 CT-Transformer 或可注入打点函数的端到端用例 | 待定（非作者 coder） | local_stream.rs 测试区 | ⏳ 攒批补 |
| `RT-STOP-LATENCY-MULTIWIN` | BUILD-398 #9 遗留：长录音切多窗串行解码，停止时队列压窗 ⇒ 松键到上屏 4.5s（16:44Z 23.85s 录音）；后半「无语音整窗送解」已由 414 修 | 待定 | — | ⏳ 待更多端测数据再定 |

**⏸ 挂起**

| 单号 | 内容 | 负责 | 文件 | 状态 |
| --- | --- | --- | --- | --- |
| `POC-QWEN3-PREFIX-424` | Gavin 灵感：已识别精确文本作 Qwen3 **输出前缀**（assistant `<asr_text>` 后）续写，替代系统提示注入；sherpa 加 per-stream `prefix` 补丁，session wav 离线回放，甲（末尾一截）/乙（整片）两种取法 vs 现状 | coder-1/2 | 仅测试 + sherpa 补丁 | ⏸ **暂不上生产**（Gavin 2026-09-25「先不上」）：甲 CER 小幅改善、官方形态真录音失控、需自维护 sherpa 双平台 fork；补丁 `collab/evidence/424/sherpa-prefix.patch` 留存 |

> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-26）

| 项 | 状态 |
| --- | --- |
| 版本 | 0.9.3（未动）；最新包 **BUILD-449**（撤除 447/448，声纹回到 446，DEC-092），待 Gavin 端测；未 push |
| Worker | 09-27 三方重启就绪（OpenCode `deepseek-v4.1-flash`，非 Codex，无需 `/permissions`）；三方空闲，待派。438 已于 `2252ce0` 入库（原「下一包」行 🔄 已派 为状态漂移，已删） |
| 文档 | 09-26 归档 handoffs 09-25 共 52 条；已完成功能按版本移入 `progress.md`「v0.9.3（BUILD-398 ~ BUILD-435）」；撤销项与过程原文移入 `todo-archive.md`【归档十】 |

---

## 🔴 待做

### 🧊 待观察 · FORCED-ALIGN-372（设想，**未立项**，等 371 端测结果）

> 🆕 **2026-09-29 重新评估**（Gavin「评估下集成强对齐模型（带时间戳输出）的方案」）：换 llama.cpp 后**可行**——公开 API 即可跑（llama.cpp-omni PR #115），`42ailab/Qwen3-ForcedAligner-0.6B-GGUF` 现成可用；代价估 +1.3~1.6G 显存、每窗 +0.15~0.3s。建议先半天 PoC（成本 + 回放接缝 A/B）再立项。详见 research 文档第八节。⏸ 等 Gavin 定是否做 PoC。

Gavin 2026-09-22 原话：
> 能不能借助第三方工具，或者说再借助一个小模型来干这个事呢？目的就是能够针对 acc 模型的片段和输出文本，建立很精确的位置对应关系、映射关系。
> 你先把讨论的用单独小模型做精确对齐的设想方案记下来，我先用目前的机制端侧看效果，如果确实有问题，再进一步扩展实施设想方案

**要解决的**：滑窗合并时「切片 → 文本位置」目前是**估算**（层② 时长比率），不准 ⇒ 落兜底 ⇒ **某句重复**。
（丢字已由层① 硬约束堵死，不是本条要解决的。）

**设想**：小模型出「带时间戳的文本」→ 边界时间查到小模型的字位置 → 两份转写做**全局对齐**映射到 acc 文本位置。
关键是**全局对齐两端都锚死、无未知偏移** ⇒ 周期性内容不构成歧义（现算法正是栽在"未知偏移 + 周期性"）。

**立项前必验（15 分钟 PoC）**：本地 `sherpa-onnx-sense-voice-funasr-nano-int8` 在 sherpa 1.13.8 里**填不填 timestamps**。
整个方案押在这一个事实上；不填则退路是离线 paraformer（有 predictor，天然出 token 时长）。

**决策顺序**：371 出包 → Gavin 端测 → 重复不碍眼 ⇒ **不做**；碍眼 ⇒ 先跑 PoC 验时间戳 ⇒ 验通再立项。
**全文（含已排除的 5 条路及理由、替代取舍、实测数据）**：`collab/research/forced-alignment-372.md`

---

### ⏸ v0.9.3 收尾三件 · 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| `.512.bak` 删不删 | 两处共 **1.12 GB**，是 KV 1024 的回滚路径；端测确认 1024 无问题后可删。属不可逆操作，须单独成轮确认 |

### 🔄 v0.9.3 本地流式实时模型 · 剩余风险项

| 项 | 内容 |
| --- | --- |
| 交叉场景未验 | **本地流式 + 开启翻译**：代码上通（复用同一 `run_pipeline_core`，无 `LocalRealtime` 特判绕过翻译），状态文案也统一为「识别处理中...」，但**从未实际测过**，已列入端测清单 |
| 贯穿约束 | 🔴 Gavin 2026-09-20 强调 **千万不能改坏现有管线**。238/238-B 是零行为变更提取，唯一硬证据是全量回归与基线逐位吻合，**每轮必跑** |
| 端测清单 | `collab/e2e-checklist-local-realtime.md`（10 项） |

### ✅ v0.9.1 / v0.9.2 两批已出包并销项 → 全文见 `todo-archive.md` 末尾

### 🔴 ITN-FIX-LIANGDIAN-223 · `这两点一个都不能少` → `这2.1个都不能少`（Gavin 2026-09-17 端测报）

| 项 | 内容 |
| --- | --- |
| 复现 | 「这**两点一**个都不能少」→「这**2.1**个都不能少」；「这两点**一点**都不能少」同病 |
| 语义 | 「两点」= 两个要点（量词），「一个都不能少」是独立短语。ITN 把 `两点一` 当成了小数 2.1 |
| 🔴 取证先行 | 主控初查**未定位**：乙型 `try_parse_implicit_decimal`（`itn.rs:1607`）第 1624 行已有护栏 —— `date_suffixes` 命中且非「度」即 `return None`，而「点」在 `date_time.triggers.suffix` 里，**理论上乙型不该接手**。所以真凶可能是甲型/丙型/`parse_cn_number` 的小数点处理，**必须实测定位，禁止照「乙型越界」这个假设动手** |
| 同族 | `[ITN-LOCAL-RULE-OVERREACH-001]`（局部规则在更长上下文越界）。护栏必须配边界外用例 |
| 🔴 边界外红线 | 真小数不得被误挡：`两点一五`→2.15、`三点五`、`两点一度`（度是温度单位兼 date_suffix）、`下午两点一刻` |
| 影响文件 | `src/itn.rs`（与 221/222 同文件，**必须串行**） |

### PROMPT-OPT-204 · `f3_lists` 精简（DEC-059 管辖，最高风险）

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/llm/mod.rs` 的 `f3_rules_text` / `INLINE_SEPARATOR_RULES(_NO_PUNCT)` |
| 现状 | 多行场景 13,789 字符 = doc 场景提示词总量 55%；单行场景 4,094 |
| 风险 | **最高** —— F3 + DEC-060 核心规则，直接决定列表化行为，刚被端测打过 |
| 硬要求 | 🔴 DEC-059：**真实 API A/B 实证**，不接受静态论证。已有 `PROMPT-LAB` 的 `lab_ab` 可直接跑（比开单时条件好） |
| 待定 | A/B 谁来跑：主控用生产 key 跑（花钱）／ Gavin 端测充当 A/B |
| 详情 | `todo-archive.md` §「提示词优化三单」（202 已完成、203 已撤单） |

### TEST-SYNC-194 · FIX-192 顺序护栏

钉死「先扩窗再建 EDIT」的顺序，防以后被改回去。按分工派**非作者的空闲 coder**（不是 coder-1）。

### ESC-178 · H1 机制定案

消息路由层为何不响应仍未查明，现在的轮询旁路是绕行不是根治。需一次带 debug.log 的端测。

### OVERLAY-149-PROBE · 探针删除

10~11 处临时探针，等圆角机制定案后清理。

### VERBOSE-195 遗留（需 Rust 改动，等 Worker 额度）

| # | 项 | 说明 |
| --- | --- | --- |
| 1 | **翻译版 L0-1(A) 保真底线** | 翻译路径目前只有 `UNIT_SYMBOL_PROTECTION_TRANSLATE` 护数字单位，否定/情态/限定词/主命题全裸奔。照该常量先例做翻译版 L0-1(A) |
| 2 | CONDENSE 条款重复 7 份 | 运行时只注入 1 份，token 成本不变，代价是维护性（改一处要改 7 处）。根治 = 代码侧按 kind 统一注入，或 toml 加共享段 |
| 3 | pi desktop exe 名待坐实 | 四条已按联网取证录入，非本机核实，端测时看 `Scene context: app_exe=` 日志坐实 |

### ⏸ MODEL-DOWNLOADER-402 · 模型下载器（Gavin 2026-09-24 选「做模型下载器」，方案待确认）

背景：400 核查程序**无任何模型自动下载器**，安装包不带模型（Gavin 定）⇒ 本地三档 / VAD / 标点 / 离线翻译装完不可用。
主控方案草案：内置模型清单（子目录 / 文件 / 大小 / sha256 / 主源 + 镜像）→ 设置界面按功能提示缺失模型、一键下载（进度、断点续传 HTTP Range、sha256 校验、临时目录下完再原子改名）→ 平台中立模块（macOS 共用）。🆕 **Gavin 已定**：托管=「直接用原始出处」（HF / GitHub 原始地址 + hf-mirror 国内镜像兜底，不自建托管；代价：文件不受控，靠 sha256 校验兜）；首次引导=「首次启动弹窗引导」（弹设置界面模型页、默认勾当前档位所需模型、一键下载）。下一步：coder-2 做模型清单调研 `MODEL-MANIFEST-RESEARCH-402A`，再出实施方案。⏸ **09-24 Gavin：「安装脚本的事儿往后再说，现在我都是通过压缩包形式发布的」⇒ 402A 暂停（已查内容落盘 research/model-manifest-402.md）**。

### 发布遗留（RELEASE-210）

| # | 项 | 说明 |
| --- | --- | --- |
| 1b | 🔴 **命名不一致待定** | 产品英文名已改 FlashVoice Input，但这些**还是旧名**：仓库名 `Feiyin-IME`、产物名 `feiyin-ime.exe` / `feiyin-ime-ui.exe`、GitHub Release 标题「飞音智能语音输入 v0.9.0」、安装包脚本 `voice-ime.iss`。**改产物名会动构建链和用户升级路径，等 Gavin 决定改到哪一层** |
| 2 | Release 挂安装包 | 本次 Release 未挂二进制。`voice-ime.iss` 是 Inno Setup 脚本但未构建。需对外分发时：出 release 包 → 构建 setup.exe → upload asset |

---

## 📋 待派发 / 待排期

| 编号 | 一句话 | 前置 / 风险 | 详情（`todo-archive.md`） |
| --- | --- | --- | --- |
| ITN-IDIOM-COVER-032 | 六个量级单位字成语（十全十美 / 千方百计等）在 `itn.rs` 零覆盖，先测现状再定改不改 | 与任何 ITN 任务互斥（同 `src/itn.rs`）。**取证前不动代码** | §「ITN-IDIOM-COVER-032」 |
| LLM-USAGE-OBSERVABILITY-001 | 补解析 LLM 响应里的缓存 usage 字段，让提示词成本可量化 | 无前置，改动极小。**实施前查证 DeepSeek 官方字段名**，跨 endpoint 用 `Option` 容错 | §「LLM-USAGE-OBSERVABILITY-001」 |
| SCENE-FORMAT-DIMS-001 | 把 `multiline_safe` 单 bool 拆成「换行 / 列表 / 标题 / 表格」多维度 | 动的是刚稳定的 prompt 构建链。**迁移成本随场景块增长，越晚越贵** | §「SCENE-FORMAT-DIMS-001」 |
| SCENE-FOCUS-PROBE-001 | 用 UIA / AX 探测焦点控件类型，解决「同一窗口内 Enter 语义不同」 | 🔴 **PoC 门禁**：Chrome / Electron 拿不拿得到无障碍信息是头号风险（chrome.exe 占真实听写量 36%） | §「SCENE-FOCUS-PROBE-001」 |
| ITN-V2-P6 | 75 个能产语法族从保护表移出交文法处理（治「五毛钱保持、三毛钱变 3毛钱」） | Gavin 2026-08-04 拍板暂缓，等端测反馈。分类扫描已完成 | §「ITN-V2-P6」 |
| I18N-HANT-GAP-001 | 繁体中文缺 `voice_asr_model_accuracy` 一个 key | 改动极小，**须顺带核 `getTranslations` 有无兜底**，无兜底则优先级上调 | §「I18N-HANT-GAP-001」 |
| ITN-COLLISION-TYPEB-001 | ITN 碰撞 B 型 | Gavin 2026-07-30：先建待办，以后再动手 | §「ITN-COLLISION-TYPEB-001」 |

---

## ⏸ 待 Gavin 拍板

| 事项 | 一句话 | 详情 |
| --- | --- | --- |
| ITN-FIX-BIGNUM-027 遗留三条 | `一万亿`→`10000亿` 是否改；`十万个为什么`→`10万个为什么` 专名被改写是否开单；`两万五百`→`20500` 备查 | §「ITN-FIX-BIGNUM-027」 |
| RESEARCH-SCENE-COVERAGE-001 | 场景词表扩展研究，三项已拍板落地，剩余争议项待定 | §「RESEARCH-SCENE-COVERAGE-001」 |
| 领域级泛化关键词第二批 | `思维导图` / `白板` / `表格` 是否收进词表 | §「领域级泛化关键词第二批」 |
| FORMAT 保底层（A 方案） | 不开 LLM 的用户要不要规则层语气词去除保底 | §「等 Gavin 拍板」 |
| `qwen3_asr_url` 默认值 | 维持 dashscope 还是留空强制用户配置 | §「等 Gavin 拍板」 |
| TELEGRAM-RESTART-001 | Telegram 通道恢复路线（降级 CLI / 等官方放开 / 手动轮询） | §「未排期任务」 |
| `src/main - 副本.rs` 99KB 残留 | 未入 git 的旧副本，**删除需单独确认**（不可逆操作纪律） | §「待办队列」 |
| api_key 明文存 `config.toml` | 是否处理 | §「待办队列」 |

---

## 已知遗留问题（低优先，非阻塞）

| 编号 | 问题 | 位置 |
| --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频分段中段语义级幻觉，三重兜底全拦不住。**挂起**（accuracy 已从 UI 隐藏，路径不可达）。🔴 未来若重开 accuracy 或引入其他 LLM-decoder 类 ASR，必须同批立项 | `src/transcription/mod.rs` |
| TEST-FIX-002 / 003 | `App.test.tsx` 缺 `@tauri-apps/api/core` mock（2 例 FAIL）；`Wordbook.test.tsx` `getByRole("dialog")` 无 role 报错 | `ui/src/` |
| TECH-DEBT-001 | `parse_version` 主程序与 Tauri 侧实现不一致，prerelease 处理有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` |
| ACC-DEGRADE-UI-001 | accuracy 静默降级时 UI 仍显示 accuracy（可观测性缺口） | `src/transcription/mod.rs` |
| MOJIBAKE-COMMENT-001 | `main.rs` 5 处历史 mojibake 注释（`2483 / 2673 / 2689 / 2722 / 2962`），清理须 UTF-8 无 BOM 读写 | `src/main.rs` |
| AUTOLEARN-REACH-001 | `218` 只修了闸门③（日志可见）。剩余三道按设计保留：落库 source=`system`（Gavin 09-17 拍板不改）／阈值 2 不上界面（DEC-031）／~~只有在线流式路径有 `last_streaming_text`，本地模型路径自动学习不可达~~ 🔴 **2026-09-21 实测证伪、本条作废**：自学习在本地实时档每次都触发（BUILD-321 日志 4 条）。真因是候选抽取过宽，见 AUTOLEARN-CANDIDATE-327 | `src/wordbook/mod.rs` + `src/main.rs` |

---

## 未排期（有想法，没排期）

| 编号 | 任务 |
| --- | --- |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 选定方案 2）：注入完成后 overlay 显示「纠错」按钮 → 编辑 → 入词库。背景：WM_GETTEXT 在现代应用读不回，自动学习路径实际失效 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补 i18n（App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看小按钮 |
| QWEN3-CORPUS-BIAS-001 | 在线模型接入词库偏置（官方 `corpus.text` 通道，max 10K tokens）。两份官方文档记载矛盾，实施前需实测 |
| QWEN3-STREAM-V2 | 在线 ASR 真流式演进 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 |
| DEC-014 | WebView2 自动安装（Win10 用户） |
| CRASH-EMAIL-001 | crash reporter 邮箱设置（需 SMTP） |
| Phase 3 演进评估 | 场景感知后续方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底、个性化风格学习。仅记录不排期 |

---

## 🍎 macOS 侧

Phase 4 完整规划见 `collab/research/macos-phase4-plan-001.md`，逐任务状态见 `todo-archive.md` §「macOS 侧 Phase 4 管线实现规划」。

| 项 | 状态 |
| --- | --- |
| A 探针 / A+ 修热键 / C-OVERLAY / C-WIRE / CFGGATE / E-BUNDLE | ✅ 已验收 |
| MACOS-P4-AXINJECT-002（堵 AX 静默丢词） | 🔄 进行中 |
| MACOS-P4-OVERLAY-WIRE-002（抽纯函数使七分支可单测） | 🔄 进行中 |
| B-NEUTRAL / C-HOST / C-TRAY / D-SCENE / D-PERM / D-AUTOLAUNCH / MAC-008~013 | ⏸ 待拍板或待排期 |

**⏳ 阻塞在 Gavin 决策上的 5 项**：阶段 B 归属与 Windows 零回归验证方 ／ 事件宿主选型（建议复议 DEC-015 的 Tauri，改用 winit）／ 四个浮层是否进第一版 ／ Apple Developer 账号 ／ AX 回读是否提前。

**边界**：阶段 B 独占 `src/main.rs`；C 的 HOST 与 TRAY 必须串行；D 的 SCENE / PERM / AUTOLAUNCH 可三路并行，交汇点 `macos/mod.rs` 的 re-export 由主控统一改一次。

---

## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 `progress.md`**，不在此保留历史
3. **测试同步 / 构建 / 出包任务归入 `CHANGELOG.md`**，不在此详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）
6. 🔴 **单条待办不超过 8 行**：背景、取证、方案推演一律写进 `todo-archive.md`，这里只留「是什么 + 前置/风险 + 指针」
7. 🔴 **本文件行数上限 250 行** —— 它每次 session 启动都会被完整读进上下文，超了立刻归档

### 🆕 LOCAL-RT-ENGINE-239 追加要求 · 切档模型加载提示（Gavin 2026-09-20）

| 项 | 内容 |
| --- | --- |
| 需求 | 切到本地 realtime 档位时要预加载两个模型（约 6s），期间用**现有 overlay 信息提示窗口**显示提示，**短暂显示后自动关闭，不要久留** |
| 现成机制 | `OverlayStatus::Info(String)`（蓝点白字，BUG-119 引入）。照抄 `main.rs:6791-6800` 的 `PipelineEvent::NoSpeech` 发送写法即可 |
| 发送方 | 🔴 **后端**（热重载在 `main.rs:7111` `Transcriber::new`），不是 UI 侧 |
| 文案 | 需补 i18n 新 key，三份 locale |

### 🆕 I18N-DRIFT-HANT-001 · 繁中档位文案与简中语义不一致（低优先）

`ui/src/i18n/zh-Hant.ts:29` `voice_asr_model_performance` = 「效能最優」，
而简中是「本地模型 - 快速」、英文是 `Local Model - Fast`。**繁中丢了「本地模型」语义**。
既有漂移，非本批引入；`LOCAL-RT-UI-240` 明令不许顺手改，单独排期。

---

## ⏸ 挂起中（等外部条件，非阻塞出包）

### GATE-ATTACK-PROBE-335 · 电平闸 attack 是否吃掉首字爆破音 —— ⏸ 挂起

**挂起原因**（Gavin 2026-09-21）：「我现在环境有声音，没办法做到静默啊。」⇒ 有底噪 ⇒ 闸一直开着
⇒ 「冷起振」这个被测条件不成立。**等 Gavin 说环境安静了再跑**。
🔴 **恢复流程、采集脚本、为什么这么设计、已就绪的两个 manual 测试与两个 py 量具、
预备读数（attack 31/326ms，首 30ms 亏欠 42.9/35.9dB）—— 全文见 `todo-archive.md`
§「【归档三】GATE-ATTACK-PROBE-335 全文」**。别凭这 8 行开跑，会漏掉「不可拍手做标记」这类硬约束。
**零成本旁证**（已请 Gavin 日常端测留意）：环境吵 ⇒ 闸开 ⇒ 首字应不易错。
反馈「『你』不再被听成『按』」⇒ 强旁证；「照样出错」⇒ 与闸无关，线索划掉。
### 已知遗留（非本批引入）
- KV `max_total_len=4096` 与 `HOTWORDS_MAX_TOTAL_TOKENS=3000` 耦合，改需同批核算（注释已写明）
- `MAX_WHITESPACE_SEGMENTS=4` 等其余候选闸门未复核（本批只改了长度上限 30→12）
