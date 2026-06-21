//! 浮层：选择菜单 / 确认对话框 / 斜杠补全浮层。

use darkbluff_core::engine::{ConfirmationAction, MenuKind};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Widget, Wrap};

use crate::app::{SuggestKind, Suggestions};
use crate::theme;

use super::MenuView;
use super::text::truncate_s;

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
                Span::styled(
                    format!("  ({})", option.id),
                    Style::default().fg(theme::OVERLAY0),
                ),
            ]))
        })
        .collect();

    let (list, mut state) = highlighted_list(items, menu.selected);
    frame.render_stateful_widget(list.block(block), popup, &mut state);
}

fn menu_title(kind: MenuKind) -> &'static str {
    match kind {
        MenuKind::Title => "标题",
        // Settings 实际由 view::home::draw_home 渲染（ChoosingSettings 走标题子屏），
        // 此分支不会被 draw_menu 触达，仅为 match 穷尽性保留。
        MenuKind::Settings => "设置",
        MenuKind::AskCharacter => "询问角色",
        MenuKind::AskTopic => "选择话题",
        MenuKind::JudgeCharacter => "审判",
        MenuKind::MoveDestination => "前往",
        MenuKind::Checkpoint => "检查点",
    }
}

/// 确认对话框（NewGame / Rollback，中文提示）。高度随提示换行行数自适应，
/// 避免短提示时框过高、底部留白过多。
pub(super) fn draw_confirmation(frame: &mut Frame, area: Rect, action: &ConfirmationAction) {
    let prompt = confirm_prompt(action);
    let popup_w = 58u16.min(area.width.saturating_sub(4)).max(20);
    // 提示行 + 空行 + 按键行 + 上下边框
    let height = wrapped_height(&prompt, popup_w.saturating_sub(2)) + 1 + 1 + 2;
    let popup = centered_rect(area, popup_w, height);
    frame.render_widget(Clear, popup);
    let block = theme::panel(Some("确认"), true);

    let text = Text::from(vec![
        Line::from(prompt).style(Style::default().fg(theme::TEXT)),
        Line::from(""),
        Line::from("y / Enter 确认     n / Esc 取消")
            .style(Style::default().fg(theme::SUBTEXT0))
            .centered(),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: true }),
        popup,
    );
}

/// 用 ratatui 自身的 wrap 渲染统计文本在给定内容宽度下占用的行数，
/// 保证与实际渲染（含 word-break / trim）完全一致。
fn wrapped_height(text: &str, width: u16) -> u16 {
    let width = width.max(1);
    let max_h = 64u16;
    let mut buf = Buffer::empty(Rect::new(0, 0, width, max_h));
    Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .render(Rect::new(0, 0, width, max_h), &mut buf);
    let w = width as usize;
    let mut last = 1u16;
    for y in 0..max_h {
        let row = &buf.content[(y as usize * w)..((y as usize + 1) * w)];
        if row.iter().any(|c| c.symbol() != " ") {
            last = y + 1;
        }
    }
    last
}

fn confirm_prompt(action: &ConfirmationAction) -> String {
    match action {
        ConfirmationAction::NewGame => {
            "开始新游戏将覆盖现有存档。继续吗？".to_string()
        }
        ConfirmationAction::Rollback { checkpoint_id } => format!(
            "回滚到检查点 {checkpoint_id}？这将丢弃其后的进度（已发现的记忆保留）。继续吗？"
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
                Span::styled(
                    truncate_s(&it.display, 18),
                    Style::default().fg(theme::TEXT),
                ),
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
        SuggestKind::Command => "指令",
        SuggestKind::Character => "角色",
        SuggestKind::Scene => "场景",
        SuggestKind::Topic => "话题",
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
