//! 标题态整页首页：双色块状 logo + 菜单，垂直居中。

use darkbluff_core::engine::MenuOption;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::theme;

use super::ViewState;

/// 整页首页：logo(6 行) + 菜单，整体垂直居中。
pub(super) fn draw_home(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    frame.render_widget(Block::default().style(Style::default().bg(theme::CRUST)), area);

    let options: &[MenuOption] = state.menu.as_ref().map(|m| m.options).unwrap_or(&[]);
    let selected = state.menu.as_ref().map(|m| m.selected).unwrap_or(0);
    let top_y = area.y + area.height.saturating_sub(6 + 2 + options.len() as u16) / 2;

    render_logo(frame, area, top_y);
    render_menu(frame, area, top_y + 6 + 2, options, selected);
}

fn render_logo(frame: &mut Frame, area: Rect, top_y: u16) {
    let logo = theme::logo();
    let logo_w = 80u16.min(area.width);
    let logo_x = area.x + area.width.saturating_sub(logo_w) / 2;
    for (i, row) in logo.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(logo_line(row)),
            Rect {
                x: logo_x,
                y: top_y + i as u16,
                width: logo_w,
                height: 1,
            },
        );
    }
}

fn render_menu(frame: &mut Frame, area: Rect, mut y: u16, options: &[MenuOption], selected: usize) {
    for (i, opt) in options.iter().enumerate() {
        let sel = i == selected;
        let style = if sel {
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT)
        };
        let marker = if sel { "▶ " } else { "  " };
        let line = Line::from(format!(
            "{marker}{}",
            title_option_label(&opt.id, opt.label.as_str())
        ))
        .style(style);
        render_centered(frame, area, y, line);
        y += 1;
    }
}

/// 一行 logo：DARK 部分暗紫、BLUFF 部分暗蓝。
fn logo_line(row: &str) -> Line<'static> {
    let dark: String = row.chars().take(theme::LOGO_DARK_COLS).collect();
    let bluff: String = row.chars().skip(theme::LOGO_DARK_COLS).collect();
    Line::from(vec![
        Span::styled(dark, Style::default().fg(theme::TITLE_DARK)),
        Span::styled(bluff, Style::default().fg(theme::TITLE_BLUFF)),
    ])
}

/// 标题菜单项英文标签（按引擎 option id 映射，未知则回退原标签）。
fn title_option_label<'a>(id: &str, label: &'a str) -> &'a str {
    match id {
        "new_game" => "New Game",
        "continue" => "Continue",
        "quit" => "Quit",
        _ => label,
    }
}

fn render_centered(frame: &mut Frame, area: Rect, y: u16, line: Line<'_>) {
    let w = line.width() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    frame.render_widget(
        Paragraph::new(line),
        Rect {
            x,
            y,
            width: w.max(1).min(area.width),
            height: 1,
        },
    );
}
