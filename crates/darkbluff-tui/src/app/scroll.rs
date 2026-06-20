//! 转录滚动：偏移调整、按面板几何钳制、鼠标滚轮路由。
//!
//! 滚动状态 `transcript_offset` 字段仍属 [`App`]（在 [`super`]），本模块只放与之
//! 相关的方法与纯函数：滚轮/键盘改偏移、渲染前钳制、可滚动状态判定。

use crossterm::event::{MouseEvent, MouseEventKind};
use darkbluff_core::engine::SessionState;
use ratatui::layout::{Rect, Size};

use crate::view;

use super::App;

/// 鼠标滚轮每格滚动的视觉行数。
pub(super) const SCROLL_STEP: usize = 3;
/// 键盘 PageUp/PageDown 每次滚动的视觉行数。
pub(super) const PAGE_STEP: i32 = 10;

impl App {
    /// 鼠标事件：仅转录可见态（探索 + 剧情展示）响应滚轮；菜单/确认浮层盖住转录时忽略。
    /// 返回是否消费了事件（用于触发重绘）。offset 的钳制在渲染前由 [`App::clamp_transcript_offset`] 完成。
    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if !is_transcript_scrollable(self.session.state()) {
            return false;
        }
        match mouse.kind {
            // 滚轮上 → 看更早内容（offset 增）；滚轮下 → 回底部方向（offset 减）。
            MouseEventKind::ScrollUp => {
                self.scroll_transcript(SCROLL_STEP as i32);
                true
            }
            MouseEventKind::ScrollDown => {
                self.scroll_transcript(-(SCROLL_STEP as i32));
                true
            }
            _ => false,
        }
    }

    /// 调整转录偏移：`delta>0` 向上（看更早），`delta<0` 向下（回底部方向）。
    /// 滚轮(±SCROLL_STEP)与键盘 PageUp/Down(±PAGE_STEP)共用，故取带符号 delta。
    pub(super) fn scroll_transcript(&mut self, delta: i32) {
        self.transcript_offset = apply_scroll_delta(self.transcript_offset, delta);
    }

    /// 渲染前按当前终端尺寸钳制偏移到 [0, max]。offset=0 时直接返回（无需全文计数）。
    pub(super) fn clamp_transcript_offset(&mut self, size: Size) {
        if self.transcript_offset == 0 {
            return;
        }
        let area = Rect::new(0, 0, size.width, size.height);
        let Some(rect) = view::transcript_text_rect(area) else {
            return;
        };
        let width = rect.width.max(1) as usize;
        let height = rect.height as usize;
        let total: usize = self
            .transcript
            .iter()
            .map(|sl| view::count_visual_lines(&sl.text, width))
            .sum();
        self.transcript_offset = clamp_offset(self.transcript_offset, total, height);
    }
}

/// 转录可见且可滚动：探索态 + 剧情展示态（菜单/确认浮层盖住转录时不可滚）。
fn is_transcript_scrollable(state: &SessionState) -> bool {
    matches!(state, SessionState::Exploring) || state.is_ack()
}

/// 纯函数：按带符号 delta 调整偏移（饱和）。`delta>0` 向上（看更早），`<0` 向下（回底）。
fn apply_scroll_delta(cur: usize, delta: i32) -> usize {
    if delta >= 0 {
        cur.saturating_add(delta as usize)
    } else {
        cur.saturating_sub((-delta) as usize)
    }
}

/// 纯函数：钳制偏移到 [0, total-height]（transcript 可向上滚动的最大量）。
fn clamp_offset(offset: usize, total: usize, height: usize) -> usize {
    offset.min(total.saturating_sub(height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use darkbluff_core::engine::SessionState;

    #[test]
    fn scroll_delta_positive_grows_saturating() {
        assert_eq!(apply_scroll_delta(5, 3), 8);
        assert_eq!(apply_scroll_delta(usize::MAX, 10), usize::MAX);
    }

    #[test]
    fn scroll_delta_negative_shrinks_saturating() {
        assert_eq!(apply_scroll_delta(5, -3), 2);
        assert_eq!(apply_scroll_delta(1, -5), 0);
        assert_eq!(apply_scroll_delta(0, -3), 0);
    }

    #[test]
    fn clamp_offset_within_and_over_bounds() {
        assert_eq!(clamp_offset(0, 40, 18), 0);
        assert_eq!(clamp_offset(10, 40, 18), 10);
        assert_eq!(clamp_offset(22, 40, 18), 22); // 上限
        assert_eq!(clamp_offset(999, 40, 18), 22); // 超上限 → 钳到 22
        assert_eq!(clamp_offset(3, 5, 18), 0); // 总行数 < 可见高 → 不可滚
    }

    #[test]
    fn transcript_scrollable_only_in_exploring_and_ack_states() {
        let scrollable = [
            SessionState::Exploring,
            SessionState::ShowingIntro,
            SessionState::ShowingNarrative,
            SessionState::ShowingOutro,
            SessionState::Ending,
        ];
        let not_scrollable = [
            SessionState::Title,
            SessionState::ChoosingSettings,
            SessionState::ChoosingAskCharacter,
            SessionState::ChoosingAskTopic,
            SessionState::ChoosingJudgeCharacter,
            SessionState::ChoosingMove,
            SessionState::ChoosingCheckpoint,
            SessionState::Confirming,
        ];
        for s in scrollable {
            assert!(is_transcript_scrollable(&s), "{s:?} 应可滚动");
        }
        for s in not_scrollable {
            assert!(!is_transcript_scrollable(&s), "{s:?} 不应可滚动");
        }
    }
}
