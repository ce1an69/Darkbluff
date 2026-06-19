//! 视图层：圆角紫色主题布局渲染。
//!
//! 布局：顶部标题条 / [左 对话转录 | 右 场景+NPC] / 底部 Claude-Code 式输入框。
//! 转录按显示宽度折行、滚动到尾；系统提示不入转录，转为输入框右侧瞬时状态。

use std::collections::VecDeque;

use darkbluff_core::engine::{ConfirmationAction, MenuKind, MenuOption, SessionState};
use darkbluff_core::world::World;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{NpcInfo, StatusKind, Suggestions};
use crate::input::CommandInput;
use crate::markdown::StyledLine;
use crate::theme;

const MIN_WIDTH: u16 = 86;
const MIN_HEIGHT: u16 = 24;
const INPUT_PROMPT: &str = "> ";
const MAX_STATUS_COLS: usize = 30;

pub struct MenuView<'a> {
    pub kind: MenuKind,
    pub options: &'a [MenuOption],
    pub selected: usize,
}

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
    pub status: Option<&'a crate::app::StatusLine>,
    pub no_motion: bool,
}

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

    draw_header(frame, header_a, state);
    draw_transcript(frame, transcript_a, state);
    draw_scene(frame, scene_a, state);
    draw_input(frame, input_a, state);

    if let Some(menu) = &state.menu {
        draw_menu(frame, inner, menu);
    }
    if let Some(action) = state.confirmation {
        draw_confirmation(frame, inner, action);
    }
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

fn draw_header(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(None, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Length(20)]).areas(inner);

    let mut spans = vec![
        Span::styled(" ◆ ", Style::default().fg(theme::MAUVE)),
        Span::styled(
            "DarkBluff",
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(state.title.to_string(), Style::default().fg(theme::SUBTEXT1)),
        Span::styled(" · ", Style::default().fg(theme::OVERLAY0)),
        Span::styled(state.scene_name.to_string(), Style::default().fg(theme::TEXT)),
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
            Span::styled(
                "Endings ",
                Style::default().fg(theme::OVERLAY0),
            ),
            Span::styled(
                format!("{found}/{total}"),
                Style::default().fg(theme::LAVENDER),
            ),
        ]))
        .alignment(Alignment::Right),
        right,
    );
}

fn draw_transcript(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(Some("Transcript"), false);
    let inner = block.inner(area);
    let width = inner.width as usize;
    let height = inner.height as usize;

    if state.transcript.is_empty() {
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("No dialogue yet. Type ", Style::default().fg(theme::SUBTEXT0)),
            Span::styled(
                "/ask",
                Style::default().fg(theme::MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to question someone.", Style::default().fg(theme::SUBTEXT0)),
        ]))
        .alignment(Alignment::Center)
        .block(block);
        frame.render_widget(hint, area);
        return;
    }

    // 从尾部倒序折行，只收集可见的 `height` 行，避免对整条转录每帧全量折行。
    let mut rows: Vec<Line<'static>> = Vec::new();
    'outer: for sl in state.transcript.iter().rev() {
        for chunk in wrap_by_width(&sl.text, width).into_iter().rev() {
            if rows.len() >= height {
                break 'outer;
            }
            rows.push(Line::styled(chunk, sl.style));
        }
    }
    rows.reverse();
    let items: Vec<ListItem> = rows.iter().map(|l| ListItem::new(l.clone())).collect();
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_scene(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let block = theme::panel(Some("Scene"), false);

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

    let title = format!("Scene · {}", state.scene_name);
    let block = theme::panel(Some(&title), false);

    let mut lines: Vec<Line<'_>> = Vec::new();
    for raw in state.scene_text.split('\n') {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        lines.push(Line::from(t.to_string()).style(Style::default().fg(theme::TEXT)));
    }

    lines.push(Line::from(""));
    lines.push(
        Line::from("PRESENT")
            .style(Style::default().fg(theme::OVERLAY1).add_modifier(Modifier::BOLD)),
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

fn npc_line(npc: &NpcInfo) -> Line<'_> {
    Line::from(vec![
        Span::styled("  ◆ ", Style::default().fg(theme::MAUVE)),
        Span::styled(
            npc.name.as_str(),
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ({})", npc.id), Style::default().fg(theme::OVERLAY0)),
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
        if t.available {
            spans.push(Span::styled(
                t.label.as_str(),
                Style::default().fg(theme::SUBTEXT0),
            ));
        } else {
            spans.push(Span::styled(
                format!("{}*", t.label),
                Style::default().fg(theme::OVERLAY0),
            ));
        }
    }
    Line::from(spans)
}

fn draw_input(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let focus = matches!(state.state, SessionState::Exploring);
    let block = theme::panel(None, focus);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.state {
        SessionState::Exploring => {
            let [left, right] =
                Layout::horizontal([Constraint::Min(0), Constraint::Length(34)]).areas(inner);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        INPUT_PROMPT,
                        Style::default().fg(theme::MAUVE).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(state.input.value(), Style::default().fg(theme::TEXT)),
                ])),
                left,
            );
            frame.render_widget(
                Paragraph::new(status_or_hint(state)).alignment(Alignment::Right),
                right,
            );

            // 光标：左边框(1) + 提示符宽 + 光标前可见宽。
            let prompt_w = UnicodeWidthStr::width(INPUT_PROMPT) as u16;
            let x = left.x + prompt_w + state.input.display_cursor();
            let max_x = left.x + left.width.saturating_sub(1);
            frame.set_cursor_position((x.min(max_x), left.y));
        }
        SessionState::Confirming => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Confirm  ", Style::default().fg(theme::LAVENDER)),
                    Span::styled(
                        "y / Enter confirm · n / Esc cancel",
                        Style::default().fg(theme::SUBTEXT0),
                    ),
                ])),
                inner,
            );
        }
        SessionState::ShowingIntro | SessionState::ShowingOutro | SessionState::Ending => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Continue  ", Style::default().fg(theme::LAVENDER)),
                    Span::styled("press Enter", Style::default().fg(theme::SUBTEXT0)),
                ])),
                inner,
            );
        }
        _ => frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Select  ", Style::default().fg(theme::LAVENDER)),
                Span::styled(
                    "↑/↓ choose · Enter confirm · Esc cancel · digits jump",
                    Style::default().fg(theme::SUBTEXT0),
                ),
            ])),
            inner,
        ),
    }

    if let Some(sg) = state.suggestions {
        draw_suggestions(frame, area, sg);
    }
}

fn status_or_hint(state: &ViewState<'_>) -> Line<'static> {
    if let Some(st) = state.status {
        let color = match st.kind {
            StatusKind::Info => theme::BLUE,
            StatusKind::Warn => theme::YELLOW,
            StatusKind::Error => theme::RED,
            StatusKind::Hint => theme::MAUVE,
        };
        return Line::from(vec![Span::styled(
            truncate_s(&st.text, MAX_STATUS_COLS),
            Style::default().fg(color),
        )]);
    }
    Line::from(vec![Span::styled(
        "Tab complete · / for commands",
        Style::default().fg(theme::OVERLAY0),
    )])
}

fn draw_suggestions(frame: &mut Frame, input_area: Rect, sg: &Suggestions) {
    let height = (sg.items.len() as u16 + 2).min(9);
    let width = 48u16.min(input_area.width.saturating_sub(2));
    let y = input_area.y.saturating_sub(height);
    let popup = Rect {
        x: input_area.x + 1,
        y,
        width,
        height,
    };
    frame.render_widget(Clear, popup);

    let title = match sg.kind {
        crate::app::SuggestKind::Command => "Commands",
        crate::app::SuggestKind::Character => "Characters",
        crate::app::SuggestKind::Scene => "Scenes",
        crate::app::SuggestKind::Topic => "Topics",
    };
    let block = theme::panel(Some(title), true);
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
    let mut list_state = ListState::default();
    list_state.select(Some(sg.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default()
                    .bg(theme::SURFACE1)
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ "),
        inner,
        &mut list_state,
    );
}

fn draw_menu(frame: &mut Frame, area: Rect, menu: &MenuView<'_>) {
    let popup = centered_rect(area, 60, (menu.options.len() as u16 + 2).min(16));
    frame.render_widget(Clear, popup);
    let title = match menu.kind {
        MenuKind::Title => "Title",
        MenuKind::AskCharacter => "Ask Character",
        MenuKind::AskTopic => "Ask Topic",
        MenuKind::JudgeCharacter => "Judge",
        MenuKind::MoveDestination => "Move",
        MenuKind::Checkpoint => "Checkpoint",
    };
    let block = theme::panel(Some(title), true);

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
    let mut state = ListState::default();
    state.select(Some(menu.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default()
                    .bg(theme::SURFACE1)
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ")
            .block(block),
        popup,
        &mut state,
    );
}

fn draw_confirmation(frame: &mut Frame, area: Rect, action: &ConfirmationAction) {
    let popup = centered_rect(area, 58, 7);
    frame.render_widget(Clear, popup);
    let block = theme::panel(Some("Confirm"), true);
    let prompt: String = match action {
        ConfirmationAction::NewGame => {
            "An existing save will be overwritten by a new game. Continue?".to_string()
        }
        ConfirmationAction::Rollback { checkpoint_id } => format!(
            "Roll back to checkpoint {checkpoint_id}? This discards progress after it (discovered memories kept). Continue?"
        ),
    };
    let text = Text::from(vec![
        Line::from(prompt).style(Style::default().fg(theme::TEXT)),
        Line::from(""),
        Line::from("y / Enter confirm     n / Esc cancel")
            .style(Style::default().fg(theme::SUBTEXT0))
            .centered(),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true }),
        popup,
    );
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
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

/// 按显示宽度折行（按字符宽度累加，超宽即换行）。
fn wrap_by_width(s: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_w && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            w = 0;
        }
        cur.push(ch);
        w += cw;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// 按显示宽度截断（尾部加 …），用于输入框右侧瞬时状态。
fn truncate_s(s: &str, max_w: usize) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_w.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

#[cfg(test)]
mod tests {
    //! 用 TestBackend 无头渲染视图，确认新布局/面板/浮层不 panic 且产出预期文本。
    use super::*;
    use crate::app::{NpcInfo, NpcTopic, SuggestKind, Suggestion, Suggestions};
    use crate::input::CommandInput;
    use crate::markdown::StyledLine;
    use darkbluff_core::engine::{ConfirmationAction, MenuKind, MenuOption, SessionState};
    use darkbluff_core::world::World;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::VecDeque;

    fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
        // 宽字符（CJK 等）占两列：ratatui 把字形放在前一格、后一格是占位 spacer。
        // 直接拼接会在字之间插入空格，故按「上一格是否宽字符」跳过 spacer。
        let w = buf.area.width as usize;
        let mut out = String::new();
        let mut skip_spacer = false;
        for (i, cell) in buf.content.iter().enumerate() {
            if (i + 1) % w == 0 {
                skip_spacer = false;
                out.push('\n');
                continue;
            }
            if skip_spacer {
                skip_spacer = false;
                continue;
            }
            out.push_str(cell.symbol());
            skip_spacer = cell
                .symbol()
                .chars()
                .next()
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0) == 2)
                .unwrap_or(false);
        }
        out
    }

    fn sample_npcs() -> Vec<NpcInfo> {
        vec![NpcInfo {
            name: "灰狼".into(),
            id: "wolf".into(),
            topics: vec![
                NpcTopic {
                    label: "昨晚的行踪".into(),
                    available: true,
                },
                NpcTopic {
                    label: "隐藏的秘密".into(),
                    available: false,
                },
            ],
        }]
    }

    #[test]
    fn renders_exploring_layout_npcs_and_palette() {
        let title = "失踪的屠夫".to_string();
        let scene_name = "酒馆".to_string();
        let scene_text = "昏黄的酒馆里飘着麦芽味，角落传来低语。".to_string();
        let transcript = VecDeque::from(vec![
            StyledLine {
                text: "灰狼 · 昨晚的行踪".into(),
                style: Style::default(),
            },
            StyledLine {
                text: "“我一直在这儿喝酒。”".into(),
                style: Style::default(),
            },
        ]);
        let npcs = sample_npcs();
        let input = CommandInput::default();
        let state = SessionState::Exploring;
        let suggestions = Suggestions {
            kind: SuggestKind::Command,
            items: vec![Suggestion {
                display: "/ask".into(),
                desc: "Question a character".into(),
                insert: "/ask ".into(),
            }],
            selected: 0,
        };
        let vs = ViewState {
            title: &title,
            scene_name: &scene_name,
            world: World::Surface,
            scene_text: &scene_text,
            npcs: &npcs,
            endings: (0, 2),
            state: &state,
            input: &input,
            transcript: &transcript,
            menu: None,
            confirmation: None,
            suggestions: Some(&suggestions),
            status: None,
            no_motion: false,
        };

        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| draw(f, &vs)).unwrap();
        let text = buffer_text(term.backend().buffer());

        for needle in [
            "Transcript", "Scene", "PRESENT", "灰狼", "DarkBluff", "Surface", "/ask", "Endings",
            "昨晚的行踪",
        ] {
            assert!(text.contains(needle), "render missing {needle:?}:\n{text}");
        }
    }

    #[test]
    fn renders_menu_and_confirmation_overlays() {
        let input = CommandInput::default();
        let state = SessionState::Title;
        let options = vec![
            MenuOption {
                id: "new_game".into(),
                label: "新游戏".into(),
            },
            MenuOption {
                id: "quit".into(),
                label: "退出".into(),
            },
        ];
        let menu = MenuView {
            kind: MenuKind::Title,
            options: &options,
            selected: 0,
        };
        let transcript = VecDeque::new();
        let empty = String::new();
        let vs = ViewState {
            title: &empty,
            scene_name: &empty,
            world: World::Surface,
            scene_text: &empty,
            npcs: &[],
            endings: (0, 0),
            state: &state,
            input: &input,
            transcript: &transcript,
            menu: Some(menu),
            confirmation: None,
            suggestions: None,
            status: None,
            no_motion: false,
        };
        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| draw(f, &vs)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("Title"), "menu title missing:\n{text}");
        assert!(text.contains("新游戏"), "menu option missing:\n{text}");

        // 确认浮层
        let action = ConfirmationAction::NewGame;
        let confirming = SessionState::Confirming;
        let vs2 = ViewState {
            title: &empty,
            scene_name: &empty,
            world: World::Surface,
            scene_text: &empty,
            npcs: &[],
            endings: (0, 0),
            state: &confirming,
            input: &input,
            transcript: &transcript,
            menu: None,
            confirmation: Some(&action),
            suggestions: None,
            status: None,
            no_motion: false,
        };
        term.draw(|f| draw(f, &vs2)).unwrap();
        let text2 = buffer_text(term.backend().buffer());
        assert!(text2.contains("Confirm"), "confirm title missing:\n{text2}");
        assert!(
            text2.contains("overwritten"),
            "english confirm prompt missing:\n{text2}"
        );
    }
}
