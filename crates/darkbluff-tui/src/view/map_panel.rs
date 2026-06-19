//! 章节树 / 检查点地图面板：`map` 指令进入（`ChoosingCheckpoint` 状态）。
//!
//! 渲染已到过章节为树（`★` 终章 / `▶` 当前章 / `???` 未到但可见的分支），每章下挂可选择
//! 的检查点（章节开始 / 审判前）与话题进度。选择（↑/↓ + Enter 回滚）仍由引擎的
//! `ChoosingCheckpoint` 菜单驱动——本面板按章节分组呈现同一组 checkpoint，并高亮当前选中。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use crate::theme;
use crate::view::MapGroup;

pub(super) fn draw_map_panel(frame: &mut Frame, area: Rect, groups: &[MapGroup], selected: usize) {
    frame.render_widget(Clear, area);
    let block = theme::panel(Some("Map  ·  ↑/↓ 选节点 · Enter 回滚 · Esc 取消"), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if groups.is_empty() {
        lines
            .push(Line::from("还没有可以回到的节点。").style(Style::default().fg(theme::SUBTEXT0)));
    } else {
        for g in groups {
            let marker = if g.is_current { "▶" } else { " " };
            let star = if g.ending { "  ★" } else { "" };
            lines.push(Line::from(vec![
                Span::styled(format!("{marker} "), Style::default().fg(theme::MAUVE)),
                Span::styled(
                    format!("{}{star}", g.title),
                    Style::default()
                        .fg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            if let Some((x, y)) = g.topic_progress {
                lines.push(
                    Line::from(format!("    话题 {x}/{y}"))
                        .style(Style::default().fg(theme::OVERLAY1)),
                );
            }
            if g.unseen_branches > 0 {
                lines.push(
                    Line::from(format!("    ???  {} 个未探索分支", g.unseen_branches))
                        .style(Style::default().fg(theme::OVERLAY0)),
                );
            }
            for row in &g.checkpoints {
                let is_sel = row.flat_index == selected;
                let prefix = if is_sel { "    ▶ " } else { "      " };
                let num = row.flat_index + 1;
                let style = if is_sel {
                    Style::default()
                        .fg(theme::TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::SUBTEXT0)
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{prefix}{num}. "), style),
                    Span::styled(row.label.clone(), style),
                ]));
            }
            lines.push(Line::from(""));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
