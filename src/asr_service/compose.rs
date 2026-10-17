//! ASR-SERVER-491 S3：远程会话的显示文本合成（预览 + 精解回灌）。
//!
//! 本机浮层在 controller 消费循环里用进程级全局状态（`ACC_REFLOW_STATE` / `ACC_REFLOW_FAST` /
//! `ACC_REFLOW_LAST_REFLOW_SEQ` 等）把预览文字与精解回灌拼成显示文本。远程会话可能多路并行，
//! 故把**同一套判定**搬成每会话一份的状态：所用的都是 controller 臂调用的同一批纯函数 / 状态机
//!（`reflow_action`、`partial_reflow_boundary`、`reflow_preview`、`ReflowFastState`），
//! 只去掉了渲染、编辑态与代际门（远程没有浮层、没有编辑、一个会话一代）。
//!
//! 对照（`main.rs` controller 臂）：
//! - `StreamingText`：存原始预览底稿，显示 = `compose_with_acc_for_gen(权威状态, 底稿)`；
//! - `PreviewReflow { partial }`：边界现算 + 回缩容差，过 ⇒ `note_partial` + 应用；
//! - `PreviewReflow { replace_all }`：`ReflowFastState::on_text`；
//! - `ReflowCommit`：`ReflowFastState::on_bound`；
//! - 老逐片路径（382 起无发送方）不搬。
//!
//! S9：显示文本的逐字时间同步维护（显示 = 精解前缀 + 预览尾巴，严格拼接 ⇒ 时间同样拼接）。
//! 进来的时间已是**录音时间**（精解时间由会话按片映射换算后再交给本模块）。

use crate::transcription::char_times::{CharTime, TsSource};
use crate::transcription::qwen_inference::WordTiming;
use crate::{
    compose_with_acc_for_gen, partial_reflow_boundary, reflow_action, reflow_monotonic_key,
    reflow_preview, reflow_stop_state, PipelineEvent, ReflowAction, ReflowFastOutcome,
    ReflowFastState, PARTIAL_SHRINK_TOLERANCE,
};

/// 远程会话固定代际（一个会话一代；事件里的代际由本会话线程自己盖章）。
pub(crate) const REMOTE_GEN: u64 = 1;

#[derive(Default)]
pub(crate) struct RemoteComposer {
    /// 原始预览底稿（`committed_len` 的坐标系）。
    raw: String,
    /// 当前显示文本（= 本机 `last_streaming_text` 镜像）。
    display: String,
    /// 权威状态 `(gen, 精解文本, 替换边界)`（= 本机 `ACC_REFLOW_STATE`）。
    acc: Option<(u64, String, usize)>,
    fast: ReflowFastState,
    /// = 本机 `ACC_REFLOW_LAST_REFLOW_SEQ`（初值 -1）。
    last_reflow_seq: Option<i64>,
    /// 末尾未定字数（预览浅色）。
    tentative: usize,
    /// = 本机 `ACC_REFLOW_SUPPRESS`：音频已收完、进入收尾 ⇒ 只更新状态不再推送。
    suppressed: bool,
    /// S9：与 `raw` 逐字对应的预览粗略时间。
    raw_times: Vec<Option<CharTime>>,
    /// S9：与权威状态 `acc` 的精解文本逐字对应的时间。
    acc_times: Vec<Option<CharTime>>,
    /// S9：最近收到的一份精解文本及其时间（`ReflowFastState` 可能晚些才应用它 ⇒ 按文本认领）。
    last_acc: Option<(String, Vec<Option<CharTime>>)>,
    /// S9：与 `display` 逐字对应的时间。
    display_times: Vec<Option<CharTime>>,
}

impl RemoteComposer {
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    pub(crate) fn tentative(&self) -> usize {
        self.tentative
    }

    /// S9：与 [`Self::display`] 逐字对应的时间（`None` = 暂无，输出前补估算）。
    pub(crate) fn display_times(&self) -> &[Option<CharTime>] {
        &self.display_times
    }

    /// 进入收尾（= 本机 `Processing`）：此后回灌只更新状态。
    pub(crate) fn suppress(&mut self) {
        self.suppressed = true;
    }

    /// 收尾预览（= 本机 `StreamingFinalPreview`）：显示 = 权威前缀 + 预览收尾全文，浅色清零。
    pub(crate) fn final_preview(&mut self, text: &str) {
        self.display = compose_with_acc_for_gen(self.acc.as_ref(), REMOTE_GEN, text);
        self.tentative = 0;
        // 收尾全文与最后一次预览底稿的差别只在末尾几个字 ⇒ 时间搬过来。
        let tail = crate::transcription::char_times::transfer(&self.raw, &self.raw_times, text);
        self.display_times = self.compose_times(&tail);
    }

    /// S9：显示时间 = 精解前缀时间 + 底稿边界后的时间（与 `compose_with_acc_for_gen` 同一拼法）。
    fn compose_times(&self, raw_times: &[Option<CharTime>]) -> Vec<Option<CharTime>> {
        match &self.acc {
            Some((g, acc, committed)) if *g == REMOTE_GEN && !acc.is_empty() => self
                .acc_times
                .iter()
                .copied()
                .chain(raw_times.iter().skip(*committed).copied())
                .collect(),
            _ => raw_times.to_vec(),
        }
    }

    /// 消费一个会话事件；显示文本有变且应推送 ⇒ `true`。
    pub(crate) fn on_event(&mut self, ev: PipelineEvent) -> bool {
        match ev {
            PipelineEvent::StreamingText(_, text, words, tentative) => {
                self.display = compose_with_acc_for_gen(self.acc.as_ref(), REMOTE_GEN, &text);
                self.raw_times = rough_times(&text, &words);
                self.raw = text;
                self.display_times = self.compose_times(&self.raw_times);
                self.tentative = tentative;
                !self.suppressed
            }
            PipelineEvent::PreviewReflow {
                seg_index,
                reflow_seq,
                committed_len,
                boundary_usable,
                has_hole,
                acc_text,
                replace_all,
                partial,
                acc_times,
                ..
            } => {
                let n = acc_text.chars().count();
                self.last_acc = Some((
                    acc_text.clone(),
                    acc_times
                        .filter(|t| t.len() == n)
                        .unwrap_or_else(|| vec![None; n]),
                ));
                let key = reflow_monotonic_key(replace_all, seg_index, reflow_seq);
                let action = reflow_action(
                    false,
                    reflow_stop_state(false, false),
                    key,
                    self.last_reflow_seq.unwrap_or(-1),
                    has_hole,
                    acc_text.is_empty(),
                );
                if action != ReflowAction::Applied || !replace_all && !partial {
                    return false;
                }
                if partial {
                    if self.suppressed {
                        return false;
                    }
                    let (prev_acc, prev_len) = self
                        .acc
                        .as_ref()
                        .map(|(_, a, l)| (a.as_str(), *l))
                        .unwrap_or(("", 0));
                    let cur = self.display.chars().count();
                    let Some(b) = partial_reflow_boundary(
                        prev_acc,
                        prev_len,
                        &acc_text,
                        &self.raw,
                        committed_len,
                    )
                    .filter(|&b| {
                        reflow_preview(&acc_text, &self.raw, b).chars().count()
                            + PARTIAL_SHRINK_TOLERANCE
                            >= cur
                    }) else {
                        return false;
                    };
                    self.last_reflow_seq = Some(key);
                    self.fast.note_partial(REMOTE_GEN, seg_index);
                    self.apply(&acc_text, b);
                    return true;
                }
                self.last_reflow_seq = Some(key);
                let outcome = self.fast.on_text(
                    REMOTE_GEN,
                    seg_index,
                    acc_text,
                    committed_len,
                    boundary_usable,
                    self.suppressed,
                );
                self.take(outcome)
            }
            PipelineEvent::ReflowCommit {
                seg_index,
                committed_len,
                ..
            } => {
                let outcome =
                    self.fast
                        .on_bound(REMOTE_GEN, seg_index, committed_len, self.suppressed);
                self.take(outcome)
            }
            _ => false,
        }
    }

    fn take(&mut self, outcome: ReflowFastOutcome) -> bool {
        match outcome {
            ReflowFastOutcome::Apply {
                text,
                committed_len,
                render,
                ..
            } => {
                self.apply(&text, committed_len);
                render
            }
            ReflowFastOutcome::None => false,
        }
    }

    /// = 本机 `render_authoritative_reflow` 的状态部分：记权威状态，显示 = 精解 + 底稿边界后的部分。
    fn apply(&mut self, acc_text: &str, committed_len: usize) {
        self.display = reflow_preview(acc_text, &self.raw, committed_len);
        self.acc = Some((REMOTE_GEN, acc_text.to_string(), committed_len));
        self.acc_times = match &self.last_acc {
            Some((t, times)) if t == acc_text => times.clone(),
            _ => vec![None; acc_text.chars().count()],
        };
        self.display_times = self.compose_times(&self.raw_times);
    }
}

/// S9（T5）：预览每字一条的 `WordTiming`（毫秒）⇒ 逐字粗略时间。词表与文本对不上（本机口径的空表等）⇒ 全 `None`。
fn rough_times(text: &str, words: &[WordTiming]) -> Vec<Option<CharTime>> {
    let n = text.chars().count();
    let per_char = words.len() == n
        && words
            .iter()
            .all(|w| w.text.chars().count() == 1 && w.punctuation.is_empty());
    if !per_char {
        return vec![None; n];
    }
    words
        .iter()
        .map(|w| {
            Some(CharTime {
                begin: w.begin_time as f32 / 1000.0,
                end: w.end_time as f32 / 1000.0,
                src: TsSource::Rough,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 预览按字给的 `WordTiming`：每字 `step_ms` 毫秒，一字一词、标点为空。
    fn per_char(text: &str, step_ms: i64) -> Vec<WordTiming> {
        text.chars()
            .enumerate()
            .map(|(i, c)| WordTiming {
                begin_time: i as i64 * step_ms,
                end_time: (i as i64 + 1) * step_ms,
                text: c.to_string(),
                punctuation: String::new(),
            })
            .collect()
    }

    fn streaming(text: &str, words: Vec<WordTiming>) -> PipelineEvent {
        PipelineEvent::StreamingText(REMOTE_GEN, text.to_string(), words, 0)
    }

    fn aligned(n: usize, t0: f32) -> Vec<Option<CharTime>> {
        (0..n)
            .map(|i| {
                Some(CharTime {
                    begin: t0 + i as f32 * 0.5,
                    end: t0 + (i + 1) as f32 * 0.5,
                    src: TsSource::Aligned,
                })
            })
            .collect()
    }

    fn reflow(
        seq: usize,
        committed_len: usize,
        acc: &str,
        acc_times: Option<Vec<Option<CharTime>>>,
    ) -> PipelineEvent {
        PipelineEvent::PreviewReflow {
            generation: REMOTE_GEN,
            seg_index: seq,
            reflow_seq: Some(seq),
            committed_len,
            boundary_usable: true,
            has_hole: false,
            acc_text: acc.to_string(),
            replace_all: true,
            decode_done_at: None,
            non_user_hide: false,
            partial: false,
            acc_times,
        }
    }

    /// S9 不变量：`display_times` 条数 == 显示文本字数。
    fn assert_len(c: &RemoteComposer, tag: &str) {
        assert_eq!(
            c.display_times().len(),
            c.display().chars().count(),
            "S9 不变量破坏 @{tag}：display={:?}",
            c.display()
        );
    }

    /// S9（T5）：预览每字一条词 ⇒ `display_times` 全 `Some`、来源 `Rough`、条数 == 显示字数，毫秒换算成秒。
    #[test]
    fn s9_streaming_per_char_words_give_rough_times() {
        let mut c = RemoteComposer::default();
        let text = "今天天气不错";
        assert!(c.on_event(streaming(text, per_char(text, 100))));
        assert_len(&c, "StreamingText");
        let t = c.display_times();
        assert_eq!(t.len(), 6);
        assert!(t
            .iter()
            .all(|x| x.is_some_and(|x| x.src == TsSource::Rough)));
        assert_eq!(t[2].unwrap().begin, 0.2);
        assert_eq!(t[2].unwrap().end, 0.3);
    }

    /// S9：词表不是一字一词（空表 / 条数不符 / 词带标点 / 词含多字）⇒ 全 `None`，条数仍 == 显示字数。
    #[test]
    fn s9_streaming_non_per_char_words_give_none() {
        let text = "今天天气不错";
        // 空表（本机口径）。
        let mut c = RemoteComposer::default();
        c.on_event(streaming(text, Vec::new()));
        assert_len(&c, "空表");
        assert_eq!(c.display_times().len(), 6);
        assert!(c.display_times().iter().all(|x| x.is_none()));
        // 条数不符。
        let mut c = RemoteComposer::default();
        c.on_event(streaming(text, per_char("今天", 100)));
        assert_len(&c, "条数不符");
        assert!(c.display_times().iter().all(|x| x.is_none()));
        // 词带标点。
        let mut w = per_char(text, 100);
        w[1].punctuation = "，".into();
        let mut c = RemoteComposer::default();
        c.on_event(streaming(text, w));
        assert_len(&c, "词带标点");
        assert!(c.display_times().iter().all(|x| x.is_none()));
        // 一词多字（条数相等但某词含 2 字）。
        let mut w = per_char(text, 100);
        w[0].text = "今天".into();
        let mut c = RemoteComposer::default();
        c.on_event(streaming(text, w));
        assert_len(&c, "词含多字");
        assert!(c.display_times().iter().all(|x| x.is_none()));
    }

    /// S9：精解回灌被应用 ⇒ 显示 = 精解 + 预览尾巴，`display_times` = 精解时间 ++ 预览时间[边界..]；
    /// 之后的预览更新、`ReflowCommit` 改边界、`final_preview` 都保持条数不变量。
    #[test]
    fn s9_reflow_apply_concats_acc_times_and_raw_tail() {
        let mut c = RemoteComposer::default();
        let raw = "今天天气不错我们去公园";
        c.on_event(streaming(raw, per_char(raw, 100)));
        assert_len(&c, "预览");

        // 精解 6 字（替换预览前 6 字），带时间（录音秒，已 Aligned）。
        let acc = "今天天气真好";
        let acc_t = aligned(6, 10.0);
        assert!(c.on_event(reflow(1, 6, acc, Some(acc_t.clone()))));
        assert_eq!(c.display(), "今天天气真好我们去公园");
        assert_len(&c, "回灌应用");
        let t = c.display_times();
        assert_eq!(&t[..6], &acc_t[..], "前缀 = 精解时间");
        assert!(t[6..]
            .iter()
            .all(|x| x.is_some_and(|x| x.src == TsSource::Rough)));
        assert_eq!(
            t[6].unwrap().begin,
            0.6,
            "尾巴 = 预览第 7 个字（下标 6）的时间"
        );
        assert_eq!(t.len(), 11);

        // 预览继续增长（多出 2 字）⇒ 前缀不变、尾巴跟着变长。
        let raw2 = "今天天气不错我们去公园玩耍";
        c.on_event(streaming(raw2, per_char(raw2, 100)));
        assert_eq!(c.display(), "今天天气真好我们去公园玩耍");
        assert_len(&c, "预览增长后");
        assert_eq!(&c.display_times()[..6], &acc_t[..]);

        // ReflowCommit 给出准确边界 8 ⇒ 同一份精解文本按新边界重渲，时间仍与文字对齐。
        c.on_event(PipelineEvent::ReflowCommit {
            generation: REMOTE_GEN,
            seg_index: 1,
            committed_len: Some(8),
        });
        assert_eq!(c.display(), "今天天气真好去公园玩耍");
        assert_len(&c, "ReflowCommit");
        assert_eq!(&c.display_times()[..6], &acc_t[..], "认领同一份精解时间");
        assert_eq!(c.display_times()[6].unwrap().begin, 0.8);

        // 终稿预览全文：与最后一份预览底稿只差末尾，时间搬过来，条数仍对齐。
        c.final_preview("今天天气不错我们去公园玩耍啊");
        assert_len(&c, "final_preview");
        assert_eq!(&c.display_times()[..6], &acc_t[..]);
    }

    /// S9：回灌的 `acc_times` 缺失 / 条数与 `acc_text` 不符 ⇒ 精解部分全 `None`（不错位），条数不变量照样成立。
    #[test]
    fn s9_reflow_missing_or_bad_acc_times_are_none() {
        for (label, bad) in [("缺失", None), ("条数不符", Some(aligned(3, 0.0)))] {
            let mut c = RemoteComposer::default();
            let raw = "今天天气不错我们去公园";
            c.on_event(streaming(raw, per_char(raw, 100)));
            assert!(c.on_event(reflow(1, 6, "今天天气真好", bad)), "{label}");
            assert_eq!(c.display(), "今天天气真好我们去公园", "{label}");
            assert_len(&c, label);
            assert!(
                c.display_times()[..6].iter().all(|x| x.is_none()),
                "{label}"
            );
            assert!(
                c.display_times()[6..].iter().all(|x| x.is_some()),
                "{label}"
            );
        }
    }

    /// S9：`final_preview` 在无精解状态下（预览无时间 / 有时间两种）也保持条数不变量。
    #[test]
    fn s9_final_preview_keeps_len_invariant_without_acc() {
        let mut c = RemoteComposer::default();
        let raw = "你好世界";
        c.on_event(streaming(raw, Vec::new()));
        assert_len(&c, "预览无时间");
        c.final_preview("你好世界啊");
        assert_eq!(c.display(), "你好世界啊");
        assert_len(&c, "final_preview 无精解");

        let mut c = RemoteComposer::default();
        c.on_event(streaming(raw, per_char(raw, 100)));
        c.final_preview("你好世界啊");
        assert_len(&c, "final_preview 有预览时间");
        // 末尾新增的「啊」没有预览时间，其余沿用。
        let t = c.display_times();
        assert!(t[..4].iter().all(|x| x.is_some()));
    }

    /// S9：空精解（`acc_text` 空 ⇒ 被 `reflow_action` 跳过）不改显示，也不破坏条数不变量。
    #[test]
    fn s9_empty_acc_reflow_is_skipped_and_len_holds() {
        let mut c = RemoteComposer::default();
        let raw = "你好世界";
        c.on_event(streaming(raw, per_char(raw, 100)));
        assert!(!c.on_event(reflow(1, 2, "", Some(Vec::new()))));
        assert_eq!(c.display(), raw);
        assert_len(&c, "空精解");
    }
}
