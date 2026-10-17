//! ASR-SERVER-491 S9：逐字时间戳的纯函数工具（设计 §12.3 / §12.4）。
//!
//! - `transfer`：文字处理节点「进文字、出文字」，在外面前后对比把旧时间搬到新文字上（T6）。
//! - `fill_missing`：没时间的字按前后已知时间均分估算补齐，标 `Estimated`。
//! - `SliceMap`：精解窗口跑在「虚拟时间轴」（派发切片首尾相接）上，换算回录音真实时间。
//! - `rough_from_marks`：SenseVoice 逐 token 起点 → 逐字粗略时间（T5）。
//! - `group_words`：逐字时间 → 阿里格式的 `words`（英文 / 数字连写成一个词，标点挂前一词）。
//!
//! 全部是无状态纯函数 / 小结构，仅依赖 std，不碰任何节点。

/// 时间来源：越靠后越不可靠。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsSource {
    /// 强制对齐。
    Aligned,
    /// 预览粗略（SenseVoice CTC 时间）。
    Rough,
    /// 估算（均分 / 补齐）。
    Estimated,
}

impl TsSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TsSource::Aligned => "aligned",
            TsSource::Rough => "rough",
            TsSource::Estimated => "estimated",
        }
    }

    fn rank(self) -> u8 {
        match self {
            TsSource::Aligned => 0,
            TsSource::Rough => 1,
            TsSource::Estimated => 2,
        }
    }

    /// 两者取更差的一个（Aligned < Rough < Estimated）。
    pub(crate) fn worst(self, other: Self) -> Self {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }
}

/// 单字时间，单位秒。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharTime {
    pub begin: f32,
    pub end: f32,
    pub src: TsSource,
}

/// 把 `[b, e]` 均分成 k 份，返回第 i 个边界；两端精确取 b / e，避免浮点误差。
fn bound(b: f32, e: f32, i: usize, k: usize) -> f32 {
    if i == 0 {
        b
    } else if i >= k {
        e
    } else {
        b + (e - b) * (i as f32) / (k as f32)
    }
}

// ───────────────────────── T6：时间搬运 ─────────────────────────

/// 字符级 diff 的编辑项（下标均为绝对下标）。
enum Op {
    Eq(usize, usize),
    Del(usize),
    Ins(usize),
}

/// LCS 的格子数上限（中段 old × new）；超过则整段当一次替换，防止长文本 O(n·m) 爆内存 / 耗时。
const LCS_CELL_CAP: usize = 4_000_000;

fn diff_ops(old: &[char], new: &[char]) -> Vec<Op> {
    let mut pre = 0;
    while pre < old.len() && pre < new.len() && old[pre] == new[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < old.len() - pre
        && suf < new.len() - pre
        && old[old.len() - 1 - suf] == new[new.len() - 1 - suf]
    {
        suf += 1;
    }
    let (o, n) = (&old[pre..old.len() - suf], &new[pre..new.len() - suf]);
    let (mo, mn) = (o.len(), n.len());

    let mut ops: Vec<Op> = (0..pre).map(|i| Op::Eq(i, i)).collect();
    if mo == 0 || mn == 0 || mo * mn > LCS_CELL_CAP {
        ops.extend((0..mo).map(|i| Op::Del(pre + i)));
        ops.extend((0..mn).map(|j| Op::Ins(pre + j)));
    } else {
        // dp[i][j] = o[i..] 与 n[j..] 的 LCS 长度
        let w = mn + 1;
        let mut dp = vec![0u32; (mo + 1) * w];
        for i in (0..mo).rev() {
            for j in (0..mn).rev() {
                dp[i * w + j] = if o[i] == n[j] {
                    dp[(i + 1) * w + j + 1] + 1
                } else {
                    dp[(i + 1) * w + j].max(dp[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < mo && j < mn {
            if o[i] == n[j] {
                ops.push(Op::Eq(pre + i, pre + j));
                i += 1;
                j += 1;
            } else if dp[(i + 1) * w + j] >= dp[i * w + j + 1] {
                ops.push(Op::Del(pre + i));
                i += 1;
            } else {
                ops.push(Op::Ins(pre + j));
                j += 1;
            }
        }
        ops.extend((i..mo).map(|x| Op::Del(pre + x)));
        ops.extend((j..mn).map(|x| Op::Ins(pre + x)));
    }
    let (ol, nl) = (old.len(), new.len());
    ops.extend((0..suf).map(|k| Op::Eq(ol - suf + k, nl - suf + k)));
    ops
}

/// 把 `old` 的逐字时间搬到 `new` 上。输出长度恒等于 `new` 的字数。
///
/// - 没变的字：时间原样搬（含 `None`）。
/// - 被替换的一串：新串均分旧串的 `[最早起点, 最晚终点]`，来源取旧串里最差的。
/// - 纯插入：零长时间落在新文字里前一个字的终点；插在最前面则取后面第一个有时间字的起点。
///   标点 / 空白继承邻字来源，字母数字记 `Estimated`。
/// - 纯删除：时间随字消失。
pub(crate) fn transfer(
    old: &str,
    old_times: &[Option<CharTime>],
    new: &str,
) -> Vec<Option<CharTime>> {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let mut out: Vec<Option<CharTime>> = vec![None; new_chars.len()];
    if old_times.len() != old_chars.len() {
        return out;
    }

    let mut pure_ins = vec![false; new_chars.len()];
    // 相邻的删 / 插（不论顺序、不被 Equal 隔开）合成一个 hunk
    let mut dels: Vec<usize> = Vec::new();
    let mut ins: Vec<usize> = Vec::new();
    let mut flush =
        |dels: &mut Vec<usize>, ins: &mut Vec<usize>, out: &mut Vec<Option<CharTime>>| {
            if !ins.is_empty() {
                if dels.is_empty() {
                    for &j in ins.iter() {
                        pure_ins[j] = true;
                    }
                } else {
                    let known: Vec<CharTime> = dels.iter().filter_map(|&i| old_times[i]).collect();
                    if let Some(first) = known.first() {
                        let b = known.iter().fold(first.begin, |a, t| a.min(t.begin));
                        let e = known.iter().fold(first.end, |a, t| a.max(t.end));
                        let src = known.iter().fold(first.src, |a, t| a.worst(t.src));
                        let k = ins.len();
                        for (idx, &j) in ins.iter().enumerate() {
                            out[j] = Some(CharTime {
                                begin: bound(b, e, idx, k),
                                end: bound(b, e, idx + 1, k),
                                src,
                            });
                        }
                    }
                }
            }
            dels.clear();
            ins.clear();
        };
    for op in diff_ops(&old_chars, &new_chars) {
        match op {
            Op::Eq(i, j) => {
                flush(&mut dels, &mut ins, &mut out);
                out[j] = old_times[i];
            }
            Op::Del(i) => dels.push(i),
            Op::Ins(j) => ins.push(j),
        }
    }
    flush(&mut dels, &mut ins, &mut out);

    // 纯插入：此时 Equal / Replace 都已落位，按新文字顺序逐个补
    for j in 0..new_chars.len() {
        if !pure_ins[j] {
            continue;
        }
        let punct = !new_chars[j].is_alphanumeric();
        out[j] = if j > 0 {
            out[j - 1].map(|p| CharTime {
                begin: p.end,
                end: p.end,
                src: if punct { p.src } else { TsSource::Estimated },
            })
        } else {
            out[1..].iter().flatten().next().map(|n| CharTime {
                begin: n.begin,
                end: n.begin,
                src: if punct { n.src } else { TsSource::Estimated },
            })
        };
    }
    out
}

// ───────────────────────── 估算补齐 ─────────────────────────

/// 把没时间的字补齐：已知的原样保留，每段连续缺失在前一已知终点与后一已知起点之间均分（`Estimated`）。
/// 开头缺失以 `lo` 为起点、结尾缺失以 `hi` 为终点；全缺则铺满 `[lo, hi]`。
pub(crate) fn fill_missing(times: &[Option<CharTime>], lo: f32, hi: f32) -> Vec<CharTime> {
    let mut out: Vec<CharTime> = Vec::with_capacity(times.len());
    let mut i = 0;
    while i < times.len() {
        if let Some(t) = times[i] {
            out.push(t);
            i += 1;
            continue;
        }
        let mut j = i;
        while j < times.len() && times[j].is_none() {
            j += 1;
        }
        let prev_end = out.last().map(|t| t.end);
        let next_begin = times.get(j).and_then(|t| t.map(|t| t.begin));
        let (p, q) = match (prev_end, next_begin) {
            (Some(p), Some(q)) => (p, q.max(p)),
            (None, Some(q)) => (lo.min(q), q),
            (Some(p), None) => (p, hi.max(p)),
            (None, None) => (lo, hi.max(lo)),
        };
        let k = j - i;
        for idx in 0..k {
            out.push(CharTime {
                begin: bound(p, q, idx, k),
                end: bound(p, q, idx + 1, k),
                src: TsSource::Estimated,
            });
        }
        i = j;
    }
    out
}

// ───────────────────────── 虚拟时间轴 → 录音真实时间 ─────────────────────────

const SAMPLE_RATE: f64 = 16_000.0;

#[derive(Clone, Debug)]
struct Slice {
    /// 本切片在虚拟轴上的起点（采样）。
    vstart: usize,
    virt_len: usize,
    pcm_start: usize,
    pcm_end: usize,
    pad_before: usize,
}

/// 虚拟时间轴 = 派发切片首尾相接（16 kHz）。每个切片音频 = `pad_before` 个零 + 录音 `[pcm_start, pcm_end)` + 尾部零。
#[derive(Clone, Debug, Default)]
pub(crate) struct SliceMap {
    slices: Vec<Slice>,
    total: usize,
}

impl SliceMap {
    /// 追加一个切片；`virt_len` 含前后补零的总长。
    pub(crate) fn push(
        &mut self,
        virt_len: usize,
        pcm_start: usize,
        pcm_end: usize,
        pad_before: usize,
    ) {
        self.slices.push(Slice {
            vstart: self.total,
            virt_len,
            pcm_start,
            pcm_end,
            pad_before,
        });
        self.total += virt_len;
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }

    /// 虚拟秒 → 录音真实秒。落在补零区贴到最近的录音边界；超出末尾取最后切片的 `pcm_end`；空映射原样返回。
    pub(crate) fn to_real(&self, virt_secs: f32) -> f32 {
        let Some(last) = self.slices.last() else {
            return virt_secs;
        };
        let v = ((virt_secs as f64) * SAMPLE_RATE).round().max(0.0) as usize;
        // 第一个终点在 v 之后的切片（零长切片自然被跳过）
        let k = self.slices.partition_point(|s| s.vstart + s.virt_len <= v);
        let real = match self.slices.get(k) {
            Some(s) => {
                let span = s.pcm_end.saturating_sub(s.pcm_start);
                let off = (v - s.vstart).saturating_sub(s.pad_before);
                s.pcm_start + off.min(span)
            }
            None => last.pcm_end,
        };
        (real as f64 / SAMPLE_RATE) as f32
    }

    /// 原地把每个有时间的字换算成录音真实时间；保证 `end >= begin`，来源不变。
    pub(crate) fn map_times(&self, times: &mut [Option<CharTime>]) {
        for t in times.iter_mut().flatten() {
            t.begin = self.to_real(t.begin);
            t.end = self.to_real(t.end).max(t.begin);
        }
    }
}

// ───────────────────────── T5：SenseVoice 粗略时间 ─────────────────────────

/// `marks[i] = (第 i 个 token 的起点秒（相对解码窗口）, 该 token 在 text 里的结束字节)`，按序且结束字节递增。
/// 每个字落在哪个 token 就取该 token 的 `[本 token 起点, 下一 token 起点)`，最后一个 token 终点取 `window_end`。
/// 再整体加 `offset` 换成会话时间；来源 `Rough`。没有 marks 时在 `[offset, offset+window_end]` 均分（`Estimated`）。
pub(crate) fn rough_from_marks(
    text: &str,
    marks: &[(f32, usize)],
    offset: f32,
    window_end: f32,
) -> Vec<CharTime> {
    if marks.is_empty() {
        let k = text.chars().count();
        return (0..k)
            .map(|i| CharTime {
                begin: bound(offset, offset + window_end, i, k),
                end: bound(offset, offset + window_end, i + 1, k),
                src: TsSource::Estimated,
            })
            .collect();
    }
    text.char_indices()
        .map(|(b, _)| {
            let i = marks.partition_point(|m| m.1 <= b).min(marks.len() - 1);
            let begin = marks[i].0;
            let end = marks.get(i + 1).map_or(window_end, |m| m.0).max(begin);
            CharTime {
                begin: begin + offset,
                end: end + offset,
                src: TsSource::Rough,
            }
        })
        .collect()
}

// ───────────────────────── 阿里格式成词 ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WordOut {
    pub text: String,
    pub punctuation: String,
    pub begin_ms: i64,
    pub end_ms: i64,
    pub src: TsSource,
}

struct WordAcc {
    text: String,
    punctuation: String,
    begin: f32,
    end: f32,
    src: TsSource,
}

impl WordAcc {
    fn extend(&mut self, t: CharTime) {
        self.begin = self.begin.min(t.begin);
        self.end = self.end.max(t.end);
        self.src = self.src.worst(t.src);
    }
}

fn to_ms(secs: f32) -> i64 {
    ((secs as f64) * 1000.0).round() as i64
}

/// 逐字时间 → 阿里格式 `words`。ASCII 字母数字连写成一个词（`2026`、`Hello`），其余字母数字（汉字等）一字一词；
/// 标点 / 空白挂在前一个词的 `punctuation`，开头的标点并入第一个词的 `text`（不计入其时间）。
/// 不变量：所有词 `text + punctuation` 依次拼接 == 输入文字。`times` 长度与字数不符时返回空。
pub(crate) fn group_words(text: &str, times: &[CharTime]) -> Vec<WordOut> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() != times.len() || chars.is_empty() {
        return Vec::new();
    }
    let mk = |w: WordAcc| WordOut {
        text: w.text,
        punctuation: w.punctuation,
        begin_ms: to_ms(w.begin),
        end_ms: to_ms(w.end),
        src: w.src,
    };

    if !chars.iter().any(|c| c.is_alphanumeric()) {
        let mut w = WordAcc {
            text: text.to_string(),
            punctuation: String::new(),
            begin: times[0].begin,
            end: times[0].end,
            src: times[0].src,
        };
        times[1..].iter().for_each(|&t| w.extend(t));
        return vec![mk(w)];
    }

    let mut words: Vec<WordAcc> = Vec::new();
    let mut lead = String::new();
    let mut prev_ascii_alnum = false;
    for (&c, &t) in chars.iter().zip(times) {
        let ascii_alnum = c.is_ascii_alphanumeric();
        if c.is_alphanumeric() {
            match words.last_mut() {
                Some(w) if ascii_alnum && prev_ascii_alnum && w.punctuation.is_empty() => {
                    w.text.push(c);
                    w.extend(t);
                }
                _ => {
                    let mut text = std::mem::take(&mut lead);
                    text.push(c);
                    words.push(WordAcc {
                        text,
                        punctuation: String::new(),
                        begin: t.begin,
                        end: t.end,
                        src: t.src,
                    });
                }
            }
        } else if let Some(w) = words.last_mut() {
            w.punctuation.push(c);
        } else {
            lead.push(c);
        }
        prev_ascii_alnum = ascii_alnum;
    }
    words.into_iter().map(mk).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ct(b: f32, e: f32) -> CharTime {
        CharTime {
            begin: b,
            end: e,
            src: TsSource::Aligned,
        }
    }
    fn cts(b: f32, e: f32, src: TsSource) -> CharTime {
        CharTime {
            begin: b,
            end: e,
            src,
        }
    }
    /// 第 i 个字占 [i, i+1] 秒。
    fn unit(n: usize) -> Vec<Option<CharTime>> {
        (0..n).map(|i| Some(ct(i as f32, i as f32 + 1.0))).collect()
    }
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    // ---- TsSource ----

    #[test]
    fn src_str_and_worst() {
        assert_eq!(TsSource::Aligned.as_str(), "aligned");
        assert_eq!(TsSource::Rough.as_str(), "rough");
        assert_eq!(TsSource::Estimated.as_str(), "estimated");
        assert_eq!(TsSource::Aligned.worst(TsSource::Rough), TsSource::Rough);
        assert_eq!(
            TsSource::Estimated.worst(TsSource::Rough),
            TsSource::Estimated
        );
        assert_eq!(TsSource::Rough.worst(TsSource::Aligned), TsSource::Rough);
        assert_eq!(
            TsSource::Aligned.worst(TsSource::Aligned),
            TsSource::Aligned
        );
    }

    // ---- transfer ----

    #[test]
    fn transfer_equal_copies() {
        let t = unit(4);
        assert_eq!(transfer("你好世界", &t, "你好世界"), t);
    }

    #[test]
    fn transfer_replace_same_len() {
        // 「二零二六年」→「2026年」：新串逐字占旧串对应时间
        let t = unit(5);
        let r = transfer("二零二六年", &t, "2026年");
        assert_eq!(r.len(), 5);
        for i in 0..5 {
            assert_eq!(r[i], Some(ct(i as f32, i as f32 + 1.0)));
        }
    }

    #[test]
    fn transfer_replace_shorter_splits_span() {
        // 「一百二十」[0,4] → 「120」：三个字均分 [0,4]
        let t = unit(4);
        let r = transfer("一百二十", &t, "120");
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].unwrap().begin, 0.0);
        assert_eq!(r[2].unwrap().end, 4.0);
        assert!(close(r[0].unwrap().end, 4.0 / 3.0));
        assert_eq!(r[0].unwrap().end, r[1].unwrap().begin);
        assert_eq!(r[1].unwrap().end, r[2].unwrap().begin);
    }

    #[test]
    fn transfer_replace_longer_and_worst_src() {
        let mut t = unit(3);
        t[1] = Some(cts(1.0, 2.0, TsSource::Rough));
        // 「零一二」中间「一」被替换成两字
        let r = transfer("零一二", &t, "零AB二");
        assert_eq!(r.len(), 4);
        assert_eq!(r[1].unwrap(), cts(1.0, 1.5, TsSource::Rough));
        assert_eq!(r[2].unwrap(), cts(1.5, 2.0, TsSource::Rough));
        assert_eq!(r[3], t[2]);
    }

    #[test]
    fn transfer_replace_span_uses_min_max_of_known() {
        // 被替换的 3 字里中间一个无时间：span 仍取已知的最早起点 / 最晚终点
        let t = vec![Some(ct(0.0, 1.0)), None, Some(ct(2.0, 3.0))];
        let r = transfer("甲乙丙", &t, "X");
        assert_eq!(r, vec![Some(ct(0.0, 3.0))]);
    }

    #[test]
    fn transfer_insert_punctuation_takes_prev_end() {
        let mut t = unit(4);
        t[1] = Some(cts(1.0, 2.0, TsSource::Rough));
        let r = transfer("你好世界", &t, "你好，世界");
        assert_eq!(r.len(), 5);
        assert_eq!(r[2].unwrap(), cts(2.0, 2.0, TsSource::Rough));
        assert_eq!(r[3], t[2]);
    }

    #[test]
    fn transfer_insert_alnum_is_estimated() {
        let t = unit(2);
        let r = transfer("你好", &t, "你好a");
        assert_eq!(r[2].unwrap(), cts(2.0, 2.0, TsSource::Estimated));
    }

    #[test]
    fn transfer_insert_at_start_uses_next_begin() {
        let mut t = unit(2);
        t[0] = Some(cts(0.5, 1.0, TsSource::Rough));
        let r = transfer("你好", &t, "，你好");
        assert_eq!(r[0].unwrap(), cts(0.5, 0.5, TsSource::Rough));
        assert_eq!(r[1], t[0]);
        // 连续插在最前面：第二个跟随第一个的终点
        let r = transfer("你好", &t, "“，你好");
        assert_eq!(r[0].unwrap().begin, 0.5);
        assert_eq!(r[1].unwrap().begin, 0.5);
    }

    #[test]
    fn transfer_insert_at_start_skips_none_neighbour() {
        let t = vec![None, Some(ct(3.0, 4.0))];
        let r = transfer("你好", &t, "，你好");
        assert_eq!(r[0].unwrap().begin, 3.0);
        assert_eq!(r[1], None);
    }

    #[test]
    fn transfer_delete_filler_drops_time() {
        let t = unit(3);
        let r = transfer("嗯你好", &t, "你好");
        assert_eq!(r, vec![t[1], t[2]]);
        // 全删
        assert!(transfer("嗯", &unit(1), "").is_empty());
    }

    #[test]
    fn transfer_none_propagation() {
        // Equal 的 None 照搬
        let t = vec![None, Some(ct(1.0, 2.0))];
        assert_eq!(transfer("你好", &t, "你好"), t);
        // 插在 None 字后面 ⇒ None
        let r = transfer("你好", &t, "你，好");
        assert_eq!(r[0], None);
        assert_eq!(r[1], None);
        assert_eq!(r[2], t[1]);
        // 被替换的全是 None ⇒ 新字全 None
        let r = transfer("二零", &[None, None], "20");
        assert_eq!(r, vec![None, None]);
        // 空旧文字 + 插入 ⇒ 没有可参照 ⇒ None
        assert_eq!(transfer("", &[], "abc"), vec![None, None, None]);
    }

    #[test]
    fn transfer_len_mismatch_all_none() {
        let r = transfer("你好", &unit(3), "你好吗");
        assert_eq!(r, vec![None, None, None]);
        assert!(transfer("你好", &unit(1), "").is_empty());
    }

    #[test]
    fn transfer_mixed_edits_keep_alignment() {
        // 去口水词 + 数字规整 + 插标点同时发生（口水词与替换段被 Equal 隔开）
        let old = "嗯你好二零二六年";
        let t = unit(old.chars().count());
        let r = transfer(old, &t, "你好2026年，");
        assert_eq!(r.len(), 8);
        assert_eq!(r[0], t[1]); // 你
        assert_eq!(r[1], t[2]); // 好
        assert_eq!(r[2].unwrap().begin, 3.0); // 2 ← 二
        assert_eq!(r[5].unwrap().end, 7.0); // 6 ← 六
        assert_eq!(r[6].unwrap(), ct(7.0, 8.0)); // 年
        assert_eq!(r[7].unwrap(), cts(8.0, 8.0, TsSource::Aligned)); // ，取「年」终点
    }

    #[test]
    fn transfer_filler_adjacent_to_replace_merges_into_span() {
        // 「嗯」与「二零二六」之间没有 Equal ⇒ 同一 hunk，新串占整段 [0,5]
        let t = unit(5);
        let r = transfer("嗯二零二六", &t, "2026");
        assert_eq!(r[0].unwrap().begin, 0.0);
        assert_eq!(r[3].unwrap().end, 5.0);
    }

    #[test]
    fn transfer_lcs_cap_path_returns_right_len_fast() {
        // 中段 3001 × 3002 > 4M ⇒ 整段当一次替换，不跑 DP
        let old: String = std::iter::once('x')
            .chain(std::iter::repeat('a').take(3000))
            .collect();
        let new: String = std::iter::once('q')
            .chain(std::iter::repeat('a').take(3000))
            .chain(std::iter::once('w'))
            .collect();
        let t = unit(old.chars().count());
        let start = std::time::Instant::now();
        let r = transfer(&old, &t, &new);
        assert_eq!(r.len(), new.chars().count());
        assert!(r.iter().all(|x| x.is_some()));
        assert!(start.elapsed().as_secs() < 5);
        assert_eq!(r[0].unwrap().begin, 0.0);
        assert!(close(r.last().unwrap().unwrap().end, 3001.0));
    }

    #[test]
    fn transfer_lcs_boundary_runs_dp() {
        // 2000 × 2000 = 4M，恰好走 DP：交替字符，长度正确且大部分字保留原时间
        let old: String = (0..2000)
            .map(|i| if i % 2 == 0 { 'a' } else { 'b' })
            .collect();
        let new: String = (0..2000)
            .map(|i| if i % 2 == 0 { 'b' } else { 'a' })
            .collect();
        let t = unit(2000);
        let r = transfer(&old, &t, &new);
        assert_eq!(r.len(), 2000);
        assert!(r.iter().all(|x| x.is_some()));
    }

    // ---- fill_missing ----

    #[test]
    fn fill_leading_middle_trailing() {
        let t = vec![
            None,
            None,
            Some(ct(2.0, 3.0)),
            None,
            Some(ct(5.0, 6.0)),
            None,
        ];
        let r = fill_missing(&t, 1.0, 8.0);
        assert_eq!(r.len(), 6);
        // 开头 2 字铺 [1,2]
        assert_eq!(r[0], cts(1.0, 1.5, TsSource::Estimated));
        assert_eq!(r[1], cts(1.5, 2.0, TsSource::Estimated));
        assert_eq!(r[2], ct(2.0, 3.0));
        // 中间 1 字铺 [3,5]
        assert_eq!(r[3], cts(3.0, 5.0, TsSource::Estimated));
        assert_eq!(r[4], ct(5.0, 6.0));
        // 结尾 1 字铺 [6,8]
        assert_eq!(r[5], cts(6.0, 8.0, TsSource::Estimated));
    }

    #[test]
    fn fill_all_none_and_empty() {
        let r = fill_missing(&[None, None, None, None], 0.0, 2.0);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].begin, 0.0);
        assert_eq!(r[3].end, 2.0);
        assert!(r.iter().all(|t| t.src == TsSource::Estimated));
        assert!(fill_missing(&[], 0.0, 1.0).is_empty());
    }

    #[test]
    fn fill_q_before_p_collapses() {
        // 下一已知起点早于上一已知终点 ⇒ 零长，不倒退
        let t = vec![Some(ct(0.0, 3.0)), None, Some(ct(2.0, 4.0))];
        let r = fill_missing(&t, 0.0, 4.0);
        assert_eq!(r[1], cts(3.0, 3.0, TsSource::Estimated));
    }

    #[test]
    fn fill_lo_after_first_known_and_hi_before_last() {
        // 开头：lo 比后面已知起点还晚 ⇒ p = q；结尾：hi 比前面已知终点还早 ⇒ q = p
        let t = vec![None, Some(ct(1.0, 2.0)), None];
        let r = fill_missing(&t, 5.0, 0.5);
        assert_eq!(r[0], cts(1.0, 1.0, TsSource::Estimated));
        assert_eq!(r[2], cts(2.0, 2.0, TsSource::Estimated));
        // 全缺且 hi < lo ⇒ 全落在 lo
        let r = fill_missing(&[None, None], 3.0, 1.0);
        assert!(r.iter().all(|t| t.begin == 3.0 && t.end == 3.0));
    }

    // ---- SliceMap ----

    /// 切片 A：补零前 1600（0.1s）+ 录音 [16000, 48000) + 尾补 1600 = 35200；
    /// 切片 B：前补 800 + 录音 [100000, 116000) + 尾补 800 = 17600，虚拟起点 35200。
    fn two_slices() -> SliceMap {
        let mut m = SliceMap::default();
        m.push(35_200, 16_000, 48_000, 1_600);
        m.push(17_600, 100_000, 116_000, 800);
        m
    }

    #[test]
    fn slicemap_maps_across_two_slices() {
        let m = two_slices();
        assert!(!m.is_empty());
        // 前补零区 ⇒ 贴到录音起点
        assert!(close(m.to_real(0.0), 1.0));
        assert!(close(m.to_real(0.1), 1.0));
        // 录音内部：虚拟 0.6s ⇒ 偏移 0.5s ⇒ 1.5s
        assert!(close(m.to_real(0.6), 1.5));
        // 尾补零区（虚拟 2.15s）⇒ 贴到录音终点 3.0
        assert!(close(m.to_real(2.15), 3.0));
        // 切片 B 起点 2.2s ⇒ 前补零区 ⇒ 6.25
        assert!(close(m.to_real(2.2), 6.25));
        // B 内部：2.2 + 0.05 + 0.5 = 2.75 ⇒ 6.25 + 0.5
        assert!(close(m.to_real(2.75), 6.75));
        // B 尾补零 / 超出末尾 ⇒ 7.25
        assert!(close(m.to_real(3.28), 7.25));
        assert!(close(m.to_real(100.0), 7.25));
        // 负数夹到 0
        assert!(close(m.to_real(-1.0), 1.0));
    }

    #[test]
    fn slicemap_empty_is_identity() {
        let m = SliceMap::default();
        assert!(m.is_empty());
        assert_eq!(m.to_real(1.234), 1.234);
        let mut t = vec![Some(ct(0.5, 1.0)), None];
        m.map_times(&mut t);
        assert_eq!(t, vec![Some(ct(0.5, 1.0)), None]);
    }

    #[test]
    fn slicemap_map_times_keeps_src_and_end_ge_begin() {
        let m = two_slices();
        let mut t = vec![
            Some(cts(0.6, 0.7, TsSource::Rough)),
            None,
            // begin 落在 B、end 落在 A（异常输入）⇒ end 夹到 begin
            Some(cts(2.75, 0.1, TsSource::Estimated)),
        ];
        m.map_times(&mut t);
        let a = t[0].unwrap();
        assert!(close(a.begin, 1.5) && close(a.end, 1.6));
        assert_eq!(a.src, TsSource::Rough);
        assert_eq!(t[1], None);
        let c = t[2].unwrap();
        assert!(close(c.begin, 6.75));
        assert_eq!(c.end, c.begin);
        assert_eq!(c.src, TsSource::Estimated);
    }

    #[test]
    fn slicemap_zero_len_slice_skipped() {
        let mut m = SliceMap::default();
        m.push(0, 0, 0, 0);
        m.push(16_000, 32_000, 48_000, 0);
        assert!(close(m.to_real(0.5), 2.5));
    }

    // ---- rough_from_marks ----

    #[test]
    fn rough_normal_with_offset() {
        // token0 =「你好」(字节 0..6)，token1 =「世界」(6..12)
        let marks = [(0.0, 6), (0.5, 12)];
        let r = rough_from_marks("你好世界", &marks, 10.0, 1.0);
        assert_eq!(r.len(), 4);
        assert_eq!((r[0].begin, r[0].end), (10.0, 10.5));
        assert_eq!((r[1].begin, r[1].end), (10.0, 10.5));
        assert_eq!((r[2].begin, r[2].end), (10.5, 11.0));
        assert!(r.iter().all(|t| t.src == TsSource::Rough));
    }

    #[test]
    fn rough_char_after_last_mark_uses_last_token() {
        let marks = [(0.0, 6), (0.5, 12)];
        let r = rough_from_marks("你好世界吗", &marks, 0.0, 1.0);
        assert_eq!(r.len(), 5);
        assert_eq!((r[4].begin, r[4].end), (0.5, 1.0));
    }

    #[test]
    fn rough_empty_marks_and_text() {
        let r = rough_from_marks("你好", &[], 2.0, 1.0);
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].begin, r[0].end), (2.0, 2.5));
        assert_eq!((r[1].begin, r[1].end), (2.5, 3.0));
        assert!(r.iter().all(|t| t.src == TsSource::Estimated));
        assert!(rough_from_marks("", &[(0.0, 3)], 0.0, 1.0).is_empty());
        assert!(rough_from_marks("", &[], 0.0, 1.0).is_empty());
    }

    #[test]
    fn rough_end_not_before_begin() {
        // window_end 比最后 token 起点还早 ⇒ 零长
        let r = rough_from_marks("好", &[(2.0, 3)], 1.0, 1.0);
        assert_eq!((r[0].begin, r[0].end), (3.0, 3.0));
    }

    #[test]
    fn rough_ascii_tokens() {
        // 英文 token 多字节宽度为 1：「hi 你」marks: "hi"(0..2) " 你"(2..6)
        let marks = [(0.0, 2), (0.4, 6)];
        let r = rough_from_marks("hi 你", &marks, 0.0, 1.0);
        assert_eq!(r.len(), 4);
        assert_eq!((r[1].begin, r[1].end), (0.0, 0.4));
        assert_eq!((r[2].begin, r[2].end), (0.4, 1.0));
    }

    // ---- group_words ----

    fn uniform(text: &str) -> Vec<CharTime> {
        (0..text.chars().count())
            .map(|i| ct(i as f32 * 0.1, (i + 1) as f32 * 0.1))
            .collect()
    }
    fn concat(words: &[WordOut]) -> String {
        words
            .iter()
            .map(|w| format!("{}{}", w.text, w.punctuation))
            .collect()
    }

    #[test]
    fn group_invariant_concat_equals_input() {
        let cases = [
            "你好，世界。",
            "Hello world 2026年，OK!",
            "，，你好",
            "  hello",
            "abc123 def, 测试 ！",
            "a-b",
            "x,y",
            "今天是2026年10月17日，温度23.5度；iPhone 15 Pro Max 好用吗？",
            "“你好”，他说。 (ok) [x] 价格￥100",
            "。。。",
            "，",
            "e-mail: foo#bar.com 你",
            "é你é",
            "\t换行\n结束\n",
            "",
        ];
        for s in cases {
            let w = group_words(s, &uniform(s));
            assert_eq!(concat(&w), s, "case {s:?}");
        }
    }

    #[test]
    fn group_ascii_runs_and_cjk_split() {
        let s = "Hello world 2026年，好";
        let w = group_words(s, &uniform(s));
        let texts: Vec<_> = w
            .iter()
            .map(|x| (x.text.as_str(), x.punctuation.as_str()))
            .collect();
        assert_eq!(
            texts,
            vec![
                ("Hello", " "),
                ("world", " "),
                ("2026", ""),
                ("年", "，"),
                ("好", "")
            ]
        );
    }

    #[test]
    fn group_punct_splits_ascii_run() {
        let s = "a-b x,y";
        let w = group_words(s, &uniform(s));
        let texts: Vec<_> = w
            .iter()
            .map(|x| (x.text.as_str(), x.punctuation.as_str()))
            .collect();
        assert_eq!(texts, vec![("a", "-"), ("b", " "), ("x", ","), ("y", "")]);
    }

    #[test]
    fn group_leading_punct_prepended_without_time() {
        let s = "，“你好";
        let w = group_words(s, &uniform(s));
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].text, "，“你");
        // 时间只来自「你」(下标 2)：[0.2, 0.3]
        assert_eq!((w[0].begin_ms, w[0].end_ms), (200, 300));
        assert_eq!(w[1].text, "好");
    }

    #[test]
    fn group_time_and_src_aggregation() {
        // 「ab」两字：Aligned + Estimated ⇒ 词来源 Estimated，时间取 min / max；后面标点不延长
        let times = vec![
            cts(1.2346, 1.5, TsSource::Aligned),
            cts(1.5, 2.0, TsSource::Estimated),
            cts(2.0, 9.0, TsSource::Aligned),
        ];
        let w = group_words("ab，", &times);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].text, "ab");
        assert_eq!(w[0].punctuation, "，");
        assert_eq!((w[0].begin_ms, w[0].end_ms), (1235, 2000));
        assert_eq!(w[0].src, TsSource::Estimated);
    }

    #[test]
    fn group_no_alnum_single_word() {
        let times = vec![
            cts(0.5, 1.0, TsSource::Aligned),
            cts(1.0, 2.5, TsSource::Rough),
        ];
        let w = group_words("。，", &times);
        assert_eq!(
            w,
            vec![WordOut {
                text: "。，".into(),
                punctuation: String::new(),
                begin_ms: 500,
                end_ms: 2500,
                src: TsSource::Rough,
            }]
        );
    }

    #[test]
    fn group_len_mismatch_or_empty() {
        assert!(group_words("你好", &uniform("你"),).is_empty());
        assert!(group_words("", &[]).is_empty());
    }
}
