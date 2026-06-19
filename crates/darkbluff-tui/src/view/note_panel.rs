//! 笔记独立面板：四标签（叙事 / 对话 / 审判 / 心声）覆盖渲染。
//!
//! `note` 指令打开此面板（会话仍处 `Exploring`）；1-4 切标签、Esc 关闭。

use darkbluff_core::engine::NoteView;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{NotePanel, NoteTab};
use crate::markdown;
use crate::theme;

pub(super) fn draw_note_panel(frame: &mut Frame, area: Rect, panel: &NotePanel) {
    frame.render_widget(Clear, area);
    let block = theme::panel(Some("Notes  ·  1-4 tab · Esc close"), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [tabs_a, body_a] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    frame.render_widget(tabs_line(panel.tab), tabs_a);

    let lines = tab_lines(&panel.view, panel.tab);
    if lines.is_empty() {
        frame.render_widget(
            Paragraph::new("— nothing here yet —")
                .style(Style::default().fg(theme::SUBTEXT0))
                .alignment(Alignment::Center),
            body_a,
        );
        return;
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body_a);
}

fn tabs_line(tab: NoteTab) -> Paragraph<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, t) in NoteTab::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let label = format!("{}. {}", i + 1, t.label());
        let style = if t == tab {
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::OVERLAY0)
        };
        spans.push(Span::styled(label, style));
    }
    Paragraph::new(Line::from(spans))
}

fn tab_lines(view: &NoteView, tab: NoteTab) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    match tab {
        NoteTab::Narrative => {
            for n in &view.narratives {
                let tag = if n.is_outro { "结局" } else { "开场" };
                out.push(head(format!("[{tag}] {}", n.title), theme::LAVENDER));
                out.extend(md(&n.text));
                out.push(blank());
            }
        }
        NoteTab::Dialogue => {
            for d in &view.dialogues {
                let w = theme::world_label(d.world);
                out.push(head(
                    format!("{} · {} [{}]", d.character_name, d.topic_label, w),
                    theme::MAUVE,
                ));
                out.extend(md(&d.text));
                out.push(blank());
            }
        }
        NoteTab::Judgment => {
            for j in &view.judgments {
                out.push(head(format!("审判 · {}", j.target_name), theme::PINK));
                out.extend(md(&j.text));
                out.push(blank());
            }
        }
        NoteTab::Voice => {
            for v in &view.voices {
                out.push(head(format!("▌ {}", v.label), theme::PINK));
                out.extend(md(&v.text));
                out.push(blank());
            }
        }
    }
    out
}

fn head(text: String, color: ratatui::style::Color) -> Line<'static> {
    Line::from(text).style(Style::default().fg(color).add_modifier(Modifier::BOLD))
}

fn md(text: &str) -> Vec<Line<'static>> {
    markdown::render(text)
        .into_iter()
        .map(|sl| Line::styled(sl.text, sl.style))
        .collect()
}

fn blank() -> Line<'static> {
    Line::from("")
}
