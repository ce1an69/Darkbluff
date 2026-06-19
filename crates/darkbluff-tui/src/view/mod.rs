//! 视图层：圆角紫色主题布局渲染。
//!
//! 布局：顶部标题条 / [左 对话转录 | 右 场景+NPC] / 库部 Claude-Code 式输入框。
//! 转录按显示宽度折行、滚动到尾；系统提示不入转录，转为输入框右侧瞬时状态。
//! 按关注点拆分：[`home`]（首页）/ [`layout`]（常规面板）/ [`overlays`]（浮层）/
//! [`text`]（宽度工具）/ [`tests`]（无头渲染测试）。

mod home;
mod layout;
mod map_panel;
mod note_panel;
mod overlays;
mod text;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use std::collections::VecDeque;

use darkbluff_core::engine::{ConfirmationAction, MenuKind, MenuOption, SessionState};
use darkbluff_core::world::World;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::{AnimationView, NotePanel, Notice, NpcInfo, StatusLine, Suggestions};
use crate::input::CommandInput;
use crate::markdown::StyledLine;
use crate::theme;

const MIN_WIDTH: u16 = 86;
const MIN_HEIGHT: u16 = 24;

pub struct MenuView<'a> {
    pub kind: MenuKind,
    pub options: &'a [MenuOption],
    pub selected: usize,
}

/// map 面板里一个章节分组：标题 + 标记 + 话题进度 + 其下的可选择检查点。
#[derive(Debug, Clone, Default)]
pub struct MapGroup {
    pub title: String,
    pub ending: bool,
    pub is_current: bool,
    /// 未到过但可见的分支数（显示为 ???）。
    pub unseen_branches: usize,
    /// (已问话题数, 本章可问话题总数)。
    pub topic_progress: Option<(usize, usize)>,
    pub checkpoints: Vec<MapRow>,
}

/// map 面板里一行可选择的检查点；`flat_index` 对应引擎 checkpoint 菜单的扁平下标。
#[derive(Debug, Clone)]
pub struct MapRow {
    pub flat_index: usize,
    pub label: String,
}

/// 一帧渲染所需的全部只读视图状态（由 app 层组装）。
pub struct ViewState<'a> {
    pub title: &'a str,
    pub scene_name: &'a str,
    pub world: World,
    pub scene_text: &'a str,
    pub npcs: &'a [NpcInfo],
    pub endings: (usize, usize),
    pub state: &'a SessionState,
    pub input: &'a CommandInput,
    pub transcript: &'a VecDeque<StyledLine>,
    pub menu: Option<MenuView<'a>>,
    pub confirmation: Option<&'a ConfirmationAction>,
    pub suggestions: Option<&'a Suggestions>,
    pub status: Option<&'a StatusLine>,
    pub note: Option<&'a NotePanel>,
    pub notice: Option<&'a Notice>,
    pub map: Option<&'a [MapGroup]>,
    pub no_motion: bool,
    pub animation: Option<AnimationView>,
}

/// 渲染一帧。标题态走整页首页，其余走常规三段布局。
pub fn draw(frame: &mut Frame, state: &ViewState<'_>) {
    let area = frame.area();
    // 整屏铺底（CRUST），面板再覆 MANTLE，形成分层。
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::CRUST)),
        area,
    );

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }
    if matches!(
        state.state,
        SessionState::Title | SessionState::ChoosingSettings
    ) {
        home::draw_home(frame, area, state);
        return;
    }

    // 外层 1 格留白，面板之间靠圆角边框自然分隔。
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let [header_a, body_a, input_a] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .areas(inner);
    let [transcript_a, scene_a] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(body_a);

    layout::draw_header(frame, header_a, state);
    layout::draw_transcript(frame, transcript_a, state);
    layout::draw_scene(frame, scene_a, state);
    layout::draw_input(frame, input_a, state);

    if let Some(animation) = &state.animation {
        draw_animation(frame, header_a, animation);
    }
    if let Some(notice) = state.notice {
        draw_notice(frame, header_a, notice);
    }
    if let Some(groups) = state.map {
        let selected = state.menu.as_ref().map(|m| m.selected).unwrap_or(0);
        map_panel::draw_map_panel(frame, inner, groups, selected);
    } else if let Some(menu) = &state.menu {
        overlays::draw_menu(frame, inner, menu);
    }
    if let Some(action) = state.confirmation {
        overlays::draw_confirmation(frame, inner, action);
    }
    if let Some(panel) = state.note {
        note_panel::draw_note_panel(frame, inner, panel);
    }
}

fn draw_animation(frame: &mut Frame, area: Rect, animation: &AnimationView) {
    let pulse = if animation.progress < 0.5 {
        "◆"
    } else {
        "◇"
    };
    let text = Paragraph::new(format!(" {pulse} {} ", animation.label))
        .style(
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(text, area);
}

/// 顶部通知条：覆盖 header 区，warning（黄）/ info（蓝），瞬时（下次按键清除）。
fn draw_notice(frame: &mut Frame, area: Rect, notice: &Notice) {
    frame.render_widget(Clear, area);
    let color = if notice.warn {
        theme::YELLOW
    } else {
        theme::BLUE
    };
    let text = Paragraph::new(format!(" ⚠ {}", notice.text))
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .block(theme::panel(Some("Notice"), true));
    frame.render_widget(text, area);
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let block = theme::panel(Some("DarkBluff"), false);
    let text = Paragraph::new(format!(
        "Terminal too small (need >={MIN_WIDTH}x{MIN_HEIGHT})."
    ))
    .block(block)
    .alignment(Alignment::Center);
    frame.render_widget(text, area);
}
