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
}

impl RemoteComposer {
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    pub(crate) fn tentative(&self) -> usize {
        self.tentative
    }

    /// 进入收尾（= 本机 `Processing`）：此后回灌只更新状态。
    pub(crate) fn suppress(&mut self) {
        self.suppressed = true;
    }

    /// 收尾预览（= 本机 `StreamingFinalPreview`）：显示 = 权威前缀 + 预览收尾全文，浅色清零。
    pub(crate) fn final_preview(&mut self, text: &str) {
        self.display = compose_with_acc_for_gen(self.acc.as_ref(), REMOTE_GEN, text);
        self.tentative = 0;
    }

    /// 消费一个会话事件；显示文本有变且应推送 ⇒ `true`。
    pub(crate) fn on_event(&mut self, ev: PipelineEvent) -> bool {
        match ev {
            PipelineEvent::StreamingText(_, text, _words, tentative) => {
                self.display = compose_with_acc_for_gen(self.acc.as_ref(), REMOTE_GEN, &text);
                self.raw = text;
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
                ..
            } => {
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
    }
}
