//! 浮层：选择菜单 / 确认对话框 / 斜杠补全浮层。

use darkbluff_core::engine::{ConfirmationAction, MenuKind};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{SuggestKind, Suggestions};
use crate::theme;

use super::text::truncate_s;
use super::MenuView;

/// 选择菜单（圆角浮层，可滚动）。
pub(super) fn draw_menu(frame: &mut Frame, area: Rect, menu: &MenuView<'_>) {
    let popup = centered_rect(area, 60, (menu.options.len() as u16 + 2).min(16));
    frame.render_widget(Clear, popup);
    let block = theme::panel(Some(menu_title(menu.kind)), true);

    let items: Vec<ListItem<'_>> = menu
        .options
        .iter()
        .enumerate()
        .map(|(i, option)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}. ", i + 1), Style::default().fg(theme::MAUVE)),
                Span::styled(option.label.as_str(), Style::default().fg(theme::TEXT)),
                Span::styled(format!("  ({})", option.id), Style::default().fg(theme::OVERLAY0)),
            ]))
        })
        .collect();

    let (list, mut state) = highlighted_list(items, menu.selected);
    frame.render_stateful_widget(list.block(block), popup, &mut state);
}

fn menu_title(kind: MenuKind) -> &'static str {
    match kind {
        MenuKind::Title => "Title",
        MenuKind::AskCharacter => "Ask Character",
        MenuKind::AskTopic => "Ask Topic",
        MenuKind::JudgeCharacter => "Judge",
        MenuKind::MoveDestination => "Move",
        MenuKind::Checkpoint => "Checkpoint",
    }
}

/// 确认对话框（NewGame / Rollback，英文提示）。
pub(super) fn draw_confirmation(frame: &mut Frame, area: Rect, action: &ConfirmationAction) {
    let popup = centered_rect(area, 58, 7);
    frame.render_widget(Clear, popup);
    let block = theme::panel(Some("Confirm"), true);

    let text = Text::from(vec![
        Line::from(confirm_prompt(action)).style(Style::default().fg(theme::TEXT)),
        Line::from(""),
        Line::from("y / Enter confirm     n / Esc cancel")
            .style(Style::default().fg(theme::SUBTEXT0))
            .centered(),
    ]);
    frame.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: true }), popup);
}

fn confirm_prompt(action: &ConfirmationAction) -> String {
    match action {
        ConfirmationAction::NewGame => {
            "An existing save will be overwritten by a new game. Continue?".to_string()
        }
        ConfirmationAction::Rollback { checkpoint_id } => format!(
            "Roll back to checkpoint {checkpoint_id}? This discards progress after it (discovered memories kept). Continue?"
        ),
    }
}

/// 斜杠补全浮层（输入框上方，按上下选择）。
pub(super) fn draw_suggestions(frame: &mut Frame, input_area: Rect, sg: &Suggestions) {
    let height = (sg.items.len() as u16 + 2).min(9);
    let width = 48u16.min(input_area.width.saturating_sub(2));
    let popup = Rect {
        x: input_area.x + 1,
        y: input_area.y.saturating_sub(height),
        width,
        height,
    };
    frame.render_widget(Clear, popup);

    let block = theme::panel(Some(suggest_title(sg.kind)), true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let items: Vec<ListItem> = sg
        .items
        .iter()
        .map(|it| {
            ListItem::new(Line::from(vec![
                Span::styled(truncate_s(&it.display, 18), Style::default().fg(theme::TEXT)),
                Span::raw("  "),
                Span::styled(it.desc.clone(), Style::default().fg(theme::OVERLAY0)),
            ]))
        })
        .collect();
    let (list, mut state) = highlighted_list(items, sg.selected);
    frame.render_stateful_widget(list, inner, &mut state);
}

fn suggest_title(kind: SuggestKind) -> &'static str {
    match kind {
        SuggestKind::Command => "Commands",
        SuggestKind::Character => "Characters",
        SuggestKind::Scene => "Scenes",
        SuggestKind::Topic => "Topics",
    }
}

/// 带选中高亮的可滚动列表（菜单与补全共用）。
fn highlighted_list<'a>(items: Vec<ListItem<'a>>, selected: usize) -> (List<'a>, ListState) {
    let mut state = ListState::default();
    state.select(Some(selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme::SURFACE1)
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    (list, state)
}

/// 在 `area` 内居中、并按可用空间夹取尺寸的矩形。
pub(super) fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4)).max(20);
    let height = height.min(area.height.saturating_sub(4)).max(5);
    let [rect] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(rect);
    rect
}
