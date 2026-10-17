# 任务书 · ASR-SERVER-491 S9（逐字时间戳 T1–T6）

> 依据：`docs/ASR-SERVER-DESIGN.md` §12（12.1–12.4）。本文件是实现方案 + 验收基准；偏离处在设计文档 §12.5 留档。
> 范围：**只动远程会话**。本机录音的文字、时间、回灌逐位不变。

## 0. 先纠正 §12.1 的一处说法

§12.1 写「坐标系是对的」——**对内拼接是对的，对外输出不对**。
精解窗口的时间是「虚拟时间轴」：各派发片**首尾相接**（`localrt_session.rs` `slice_cum_end`）。
每片 = 前置零 `pad_before` + 录音 `[pcm_start, pcm_end)` + 后置零（`vad.rs` `pad_and_extract_with_spans`，前后各 ≤200ms 静音补零）。
所以虚拟时间每过一片就比真实录音时间多出这片的补零，长句会累积偏几百毫秒到几秒。

修法：派发时把每片的真实位置（`spans`）一并记下，远程会话持一张「片映射」，对外输出前把虚拟秒换算成录音秒：
`真实样本 = pcm_start + clamp(片内偏移 − pad_before, 0, pcm_end − pcm_start)`。

## 1. 数据形状

- 逐字时间 `CharTime { begin, end: f32 秒, src: TsSource }`，`TsSource = Aligned | Rough | Estimated`（差到好：Estimated 最差）。
- 一段文字的时间 = `Vec<Option<CharTime>>`，与 `text.chars()` 一一对应；`None` = 暂无，输出前一律补成 `Estimated`。

## 2. 逐条实现

| # | 设计要求 | 实现 | 本机影响 |
| --- | --- | --- | --- |
| T1 | 拼接器已定稿部分也留时间 | `OrderedReflow` 加 `committed_times: Vec<Option<(f32,f32)>>`，各 `committed.push_str` 分支同步追加；`refine_times` 回退时同步截断；新增 `full_times()` / `finish_timed()`，原 `finish()` 不动 | 只多维护一个数组，文字逐位不变 |
| T2 | 只对远程会话照常对齐最后一窗 | 解码线程跳过最后一窗对齐的条件加 `!recognizer.is_remote()` | 本机不变 |
| T3 | 无时间的接缝补对齐，补不出标估算 | **偏离（留档）**：不补对齐，直接按相邻已知字插值标 `Estimated`。理由见 §4 | 不影响 |
| T4 | 整段兜底对整段补对齐 | 精解为空、退回预览收尾文本时：录音 ≤30s ⇒ 对整段录音对齐一次（真实时间轴，无需换算）；失败 / 超长 ⇒ 用预览的粗略时间搬过来；再不行标估算 | 不影响 |
| T5 | 预览带 SenseVoice 粗略时间 | `local_stream` 仅 `PreviewRole::Remote` 时维护「已确认区 + 当前句」每字粗略时间，`on_result` 的 `words` 按字给出（本机仍传空） | 本机不变 |
| T6 | 文字节点改字后时间搬运 | 纯函数 `transfer(旧文, 旧时间, 新文)`（规则见设计 §12.3），套在：去口水重复、繁简规整、预览打点、S10 后处理节点 | 不影响 |

### 接线

1. **片映射**：`PreviewRole::Remote { vad, slices }`，`slices: &Mutex<SliceMap>`；三处 `on_segment(` 调用**之前**各加一行 `record_slices(...)`（本机 `None` 不记）。记录先于派发 ⇒ 精解回灌到达时映射必已就绪。
2. **精解时间随回灌走**：`PipelineEvent::PreviewReflow` 加字段 `acc_times: Option<Vec<Option<CharTime>>>`（虚拟轴，与 `acc_text` 逐字对应）。只在 `recognizer.is_remote()` 时填（本机恒 `None`，不算不分配）。去口水重复改了字 ⇒ 先 `transfer`。
3. **收尾**：`run_acc_windows` 返回值加第 6 项 `Option<Vec<Option<CharTime>>>`（远程 = `committed + last_window` 逐字时间；本机 `None`）。main.rs 解构加一个 `_`。
4. **会话合成**：`RemoteComposer` 同步维护显示文本的逐字时间（显示 = 精解前缀 + 预览尾巴，严格拼接 ⇒ 时间同样拼接）；终稿 = `transfer(去口水前, 时间, 去口水后)` ⇒ `transfer(…, 繁简规整后)` ⇒ `fill_missing`。
5. **对外**：`ResultPayload.words: Vec<ResultWord>`（frame.rs，内核与网关共用），网关原样放进 `sentence.words`，`sentence.begin_time/end_time` 取首末字，来源放 `output.feiyin.ts_source`（与 words 一一对应）。

### 成词规则（§12.4「按阿里格式」）

- 汉字等非 ASCII 字母数字：一字一词；连续 ASCII 字母数字：合成一词（「2026」「hello」）。
- 标点、空白挂到前一词的 `punctuation`；开头的标点 / 空白并进第一个词的 `text`。
- **不变量**：各词 `text + punctuation` 依次拼接 == 原文（电脑客户端 `StreamingAsrState` 要求词表与文本逐字对齐）。
- `begin_time / end_time`：毫秒，相对本会话第一帧音频；词的时间 = 其 `text` 部分各字的最早起点 / 最晚终点。

## 3. 分工

| 部分 | 谁做 |
| --- | --- |
| `src/transcription/char_times.rs` 纯函数（transfer / fill_missing / SliceMap / rough_from_marks / group_words）+ 单测 | 子 Agent（按本节 §5 接口） |
| T1 拼接器、T2、回灌带时间、片映射接线、T5 预览、会话合成、frame / 网关 | 主控 |
| 测试补充（拼接器时间不变量、会话合成）、MACOS-HANDOFF、日志 | 子 Agent |
| 验收 `collab/acceptance-491-S9.md` | 主控 |

## 4. T3 偏离理由

拼接时没有时间的字只有三类：① 对齐器对这段返回不合格（`spans_valid` 拒绝）——同一段音频 + 同一段文字再对一次结果相同，补对齐无意义；
② 兜底文字经声纹 / 远场剔除、或末尾 pending 兜底——文字与整窗音频已不一一对应，硬对齐会给出错误时间；
③ 半截结果（预览期）——终稿时会被正式结果替换。
三类都不是「再对一次就能对上」。改为按两侧已知字插值并明确标 `Estimated`，满足 §12.4「定稿句每字都有时间，补不出的标 estimated」。

## 5. `char_times.rs` 接口（子 Agent 按此实现）

见文件头注释；签名：

```rust
pub(crate) enum TsSource { Aligned, Rough, Estimated }      // as_str(): "aligned" / "rough" / "estimated"；worst(a, b)
pub(crate) struct CharTime { pub begin: f32, pub end: f32, pub src: TsSource }
pub(crate) fn transfer(old: &str, old_times: &[Option<CharTime>], new: &str) -> Vec<Option<CharTime>>;
pub(crate) fn fill_missing(times: &[Option<CharTime>], lo: f32, hi: f32) -> Vec<CharTime>;
pub(crate) struct SliceMap { .. }   // push(virt_len, pcm_start, pcm_end, pad_before) / to_real(virt_secs) / map_times(&mut [Option<CharTime>])
pub(crate) fn rough_from_marks(text: &str, marks: &[(f32, usize)], offset: f32, window_end: f32) -> Vec<CharTime>;
pub(crate) struct WordOut { pub text: String, pub punctuation: String, pub begin_ms: i64, pub end_ms: i64, pub src: TsSource }
pub(crate) fn group_words(text: &str, times: &[CharTime]) -> Vec<WordOut>;
```
