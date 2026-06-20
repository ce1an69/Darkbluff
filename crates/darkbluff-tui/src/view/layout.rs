//! 常规布局面板：标题条 / 对话转录 / 场景+NPC / 输入框。

use darkbluff_core::engine::SessionState;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::NpcInfo;
use crate::theme;

use super::ViewState;
use super::overlays::draw_suggestions;
use super::text::{count_visual_lines, truncate_by_width, wrap_by_width};

const INPUT_PROMPT: &str = "> ";

pub(super) fn draw_header(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(None, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(20)]).areas(inner);

    let mut spans = vec![
        Span::styled(" ◆ ", Style::default().fg(theme::MAUVE)),
        Span::styled(
            "DarkBluff",
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            state.title.to_string(),
            Style::default().fg(theme::SUBTEXT1),
        ),
        Span::styled(" · ", Style::default().fg(theme::OVERLAY0)),
        Span::styled(
            state.scene_name.to_string(),
            Style::default().fg(theme::TEXT),
        ),
        Span::raw("   "),
        Span::styled(
            format!("● {}", theme::world_label(state.world)),
            Style::default().fg(theme::world_color(state.world)),
        ),
    ];
    if state.no_motion {
        spans.push(Span::styled(
            "  no-motion",
            Style::default().fg(theme::OVERLAY0),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), left);

    let (found, total) = state.endings;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Endings ", Style::default().fg(theme::OVERLAY0)),
            Span::styled(
                format!("{found}/{total}"),
                Style::default().fg(theme::LAVENDER),
            ),
        ]))
        .alignment(Alignment::Right),
        right,
    );
}

pub(super) fn draw_transcript(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(Some("Transcript"), false);
    let inner = block.inner(area);
    let width = inner.width as usize;
    let height = inner.height as usize;

    if state.transcript.is_empty() {
        let hint = Paragraph::new(Line::from(vec![
            Span::styled(
                "No dialogue yet. Type ",
                Style::default().fg(theme::SUBTEXT0),
            ),
            Span::styled(
                "/ask",
                Style::default()
                    .fg(theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " to question someone.",
                Style::default().fg(theme::SUBTEXT0),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(block);
        frame.render_widget(hint, area);
        return;
    }

    // 只渲染可见的 `height` 视觉行，避免对整条转录每帧全量折行。
    // 打字机：reveal 预算只分给「可见的覆盖区正文行」(正序消耗)。覆盖区前 skip 行
    // (header/blank) 瞬显；正文行整行折行后跨视觉行逐字揭示，行数固定不跳(#5)。
    let n = state.transcript.len();
    let (tw_lines, tw_skip, revealed) = match state.typewriter {
        Some(tw) => (tw.lines, tw.skip, tw.revealed),
        None => (0, 0, usize::MAX),
    };

    // Pass 1：倒序累计视觉行数，定位首个可见源行 `start`(只计数，零 clone)。
    let mut start = n;
    let mut used = 0usize;
    for (i, sl) in state.transcript.iter().enumerate().rev() {
        start = i;
        used += count_visual_lines(&sl.text, width);
        if used >= height {
            break;
        }
    }

    // Pass 2：正序遍历可见源行 [start, n)。
    //   - 非覆盖区 → 正常折行。
    //   - 覆盖区前 skip 行(结构行) → 瞬显。
    //   - 覆盖区正文行(body) → 整行折行后跨视觉行逐字揭示(行数固定不跳，#5)。
    let mut reveal = revealed;
    let mut rows: Vec<Line<'static>> = Vec::new();
    for i in start..n {
        let sl = &state.transcript[i];
        let from_tail = n - i; // 1-based：末行为 1
        let in_tw = tw_lines > 0 && from_tail <= tw_lines;
        let in_skip = in_tw && from_tail > tw_lines - tw_skip;
        if in_skip {
            for chunk in wrap_by_width(&sl.text, width) {
                rows.push(Line::styled(chunk, sl.style));
            }
        } else if in_tw {
            for chunk in wrap_by_width(&sl.text, width) {
                let cw = UnicodeWidthStr::width(chunk.as_str());
                let show = reveal.min(cw);
                reveal = reveal.saturating_sub(show);
                rows.push(Line::styled(truncate_by_width(&chunk, show), sl.style));
            }
        } else {
            for chunk in wrap_by_width(&sl.text, width) {
                rows.push(Line::styled(chunk, sl.style));
            }
        }
    }

    // Pass 1 的行边界近似可能多收一源行，裁掉超出 `height` 的部分。
    let drop = rows.len().saturating_sub(height);
    let items: Vec<ListItem> = rows[drop..]
        .iter()
        .map(|l| ListItem::new(l.clone()))
        .collect();
    frame.render_widget(List::new(items).block(block), area);
}

pub(super) fn draw_scene(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(Some(&scene_title(state)), false);

    // 标题界面尚未进入任何章节：避免透出空 chapter/scene 的占位假场景。
    if matches!(state.state, SessionState::Title) {
        frame.render_widget(
            Paragraph::new(Line::from("Begin a new game to explore."))
                .style(Style::default().fg(theme::SUBTEXT0))
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    }

    let mut lines: Vec<Line<'_>> = scene_description_lines(state);
    lines.push(Line::from(""));
    lines.push(
        Line::from("PRESENT").style(
            Style::default()
                .fg(theme::OVERLAY1)
                .add_modifier(Modifier::BOLD),
        ),
    );
    if state.npcs.is_empty() {
        lines.push(Line::from("  no one to ask here").style(Style::default().fg(theme::SUBTEXT0)));
    }
    for npc in state.npcs {
        lines.push(npc_line(npc));
        lines.push(npc_topics_line(npc));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}

fn scene_title(state: &ViewState<'_>) -> String {
    if matches!(state.state, SessionState::Title) {
        "Scene".to_string()
    } else {
        format!("Scene · {}", state.scene_name)
    }
}

fn scene_description_lines(state: &ViewState<'_>) -> Vec<Line<'static>> {
    crate::markdown::render(&state.scene_text)
        .into_iter()
        .map(|sl| Line::from(sl.text).style(sl.style))
        .collect()
}

fn npc_line(npc: &NpcInfo) -> Line<'_> {
    Line::from(vec![
        Span::styled("  ◆ ", Style::default().fg(theme::MAUVE)),
        Span::styled(
            npc.name.as_str(),
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ({})", npc.id),
            Style::default().fg(theme::OVERLAY0),
        ),
    ])
}

fn npc_topics_line(npc: &NpcInfo) -> Line<'_> {
    if npc.topics.is_empty() {
        return Line::from("     —").style(Style::default().fg(theme::SUBTEXT0));
    }
    let mut spans: Vec<Span<'_>> = vec![Span::raw("     ")];
    for (i, t) in npc.topics.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::OVERLAY0)));
        }
        let (text, color) = if t.available {
            (t.label.as_str().to_string(), theme::SUBTEXT0)
        } else {
            (format!("{}*", t.label), theme::OVERLAY0)
        };
        spans.push(Span::styled(text, Style::default().fg(color)));
    }
    Line::from(spans)
}

pub(super) fn draw_input(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let focus = matches!(state.state, SessionState::Exploring);
    let block = theme::panel(None, focus);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.state {
        SessionState::Exploring => render_exploring_input(frame, inner, state),
        SessionState::Confirming => render_hint(
            frame,
            inner,
            "Confirm",
            "y / Enter confirm · n / Esc cancel",
        ),
        SessionState::ShowingIntro
        | SessionState::ShowingNarrative
        | SessionState::ShowingOutro
        | SessionState::Ending => render_hint(frame, inner, "Continue", "press Enter"),
        _ => render_hint(
            frame,
            inner,
            "Select",
            "↑/↓ choose · Enter confirm · Esc cancel · digits jump",
        ),
    }

    if let Some(sg) = state.suggestions {
        draw_suggestions(frame, area, sg);
    }
}

fn render_exploring_input(frame: &mut Frame, inner: Rect, state: &ViewState<'_>) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                INPUT_PROMPT,
                Style::default()
                    .fg(theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.input.value(), Style::default().fg(theme::TEXT)),
        ])),
        inner,
    );
    // 光标：左边框(1) + 提示符宽 + 光标前可见宽。
    let prompt_w = UnicodeWidthStr::width(INPUT_PROMPT) as u16;
    let x = inner.x + prompt_w + state.input.display_cursor();
    let max_x = inner.x + inner.width.saturating_sub(1);
    frame.set_cursor_position((x.min(max_x), inner.y));
}

fn render_hint(frame: &mut Frame, area: Rect, label: &str, hint: &str) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{label}  "), Style::default().fg(theme::LAVENDER)),
            Span::styled(hint, Style::default().fg(theme::SUBTEXT0)),
        ])),
        area,
    );
}

