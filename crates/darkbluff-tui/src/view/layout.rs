//! 常规布局面板：标题条 / 对话转录 / 场景+NPC / 输入框。

use std::collections::VecDeque;

use darkbluff_core::engine::SessionState;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::NpcInfo;
use crate::markdown::StyledLine;
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
            "  禁用动画",
            Style::default().fg(theme::OVERLAY0),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), left);

    let (found, total) = state.endings;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("结局 ", Style::default().fg(theme::OVERLAY0)),
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
    if state.transcript.is_empty() {
        render_empty_transcript(frame, area);
        return;
    }
    let offset = state.offset;
    let title = transcript_title(offset);
    let block = theme::panel(Some(&title), false);
    let inner = block.inner(area);
    let width = inner.width as usize;
    let height = inner.height as usize;

    // offset 已由 app 层钳制；只渲染覆盖可见窗的源行 [start, n) 并取窗显示（无写回、渲染无副作用）。
    let start = find_window_start(state.transcript, width, height, offset);
    let rows = build_transcript_rows(state, start, width);
    let items = window_items(&rows, height, offset);
    frame.render_widget(List::new(items).block(block), area);
}

/// 空转录提示（transcript 仅增长，空即贴底，无 offset 概念）。
fn render_empty_transcript(frame: &mut Frame, area: Rect) {
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("暂无对话。输入 ", Style::default().fg(theme::SUBTEXT0)),
        Span::styled(
            "/ask",
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" 询问他人。", Style::default().fg(theme::SUBTEXT0)),
    ]))
    .alignment(Alignment::Center)
    .block(theme::panel(Some("对话记录"), false));
    frame.render_widget(hint, area);
}

/// offset>0 时标题加位置指示（↑N = 末尾之上 N 视觉行）。
fn transcript_title(offset: usize) -> String {
    if offset > 0 {
        format!("对话记录 ↑{offset}")
    } else {
        "对话记录".to_string()
    }
}

/// Pass 1：从末尾倒序累计视觉行数，定位首个需渲染的源行 `start`。
/// `need = height + offset`：窗完整覆盖所需；累计达标即早退（offset=0 时仅扫约 height 行）。
fn find_window_start(
    transcript: &VecDeque<StyledLine>,
    width: usize,
    height: usize,
    offset: usize,
) -> usize {
    let need = height.saturating_add(offset);
    let mut cum = 0usize;
    for (i, sl) in transcript.iter().enumerate().rev() {
        cum += count_visual_lines(&sl.text, width);
        if cum >= need {
            return i;
        }
    }
    0 // 转录总行数 < need：从头渲染
}

/// Pass 2：正序遍历源行 [start, n) 折行成视觉行。
/// 打字机：覆盖区前 skip 行(结构行)瞬显；正文行整行折行后跨视觉行逐字揭示(行数固定不跳，#5)。
fn build_transcript_rows(state: &ViewState<'_>, start: usize, width: usize) -> Vec<Line<'static>> {
    let n = state.transcript.len();
    let (tw_lines, tw_skip, revealed) = match state.typewriter {
        Some(tw) => (tw.lines, tw.skip, tw.revealed),
        None => (0, 0, usize::MAX),
    };
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
    rows
}

/// 取窗 [begin, end)：末尾 `offset` 行之上、高 `height` 的视窗（offset=0 即贴底）。
fn window_items(rows: &[Line<'static>], height: usize, offset: usize) -> Vec<ListItem<'static>> {
    let total = rows.len();
    let end = total.saturating_sub(offset);
    let begin = end.saturating_sub(height);
    rows[begin..end]
        .iter()
        .map(|l| ListItem::new(l.clone()))
        .collect()
}

pub(super) fn draw_scene(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(Some(&scene_title(state)), false);

    // 标题界面尚未进入任何章节：避免透出空 chapter/scene 的占位假场景。
    if matches!(state.state, SessionState::Title) {
        frame.render_widget(
            Paragraph::new(Line::from("开始新游戏以探索。"))
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
        Line::from("在场").style(
            Style::default()
                .fg(theme::OVERLAY1)
                .add_modifier(Modifier::BOLD),
        ),
    );
    if state.npcs.is_empty() {
        lines.push(Line::from("  此处无人可询问").style(Style::default().fg(theme::SUBTEXT0)));
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
        "场景".to_string()
    } else {
        format!("场景 · {}", state.scene_name)
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
            "确认",
            "y / Enter 确认 · n / Esc 取消",
        ),
        s if s.is_ack() => render_hint(frame, inner, "继续", "按 Enter 继续 · 滚轮/PageUp 滚动"),
        _ => render_hint(
            frame,
            inner,
            "选择",
            "↑/↓ 选择 · Enter 确认 · Esc 取消 · 数字键跳转",
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

