use std::collections::VecDeque;

use darkbluff_core::engine::{MenuKind, MenuOption, SessionState};
use darkbluff_core::world::World;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::input::CommandInput;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;
/// Exploring 态指令行提示符；光标偏移据此推导，避免硬编码字面量。
const INPUT_PROMPT: &str = "> ";

#[derive(Debug, Clone, Copy)]
pub enum RecordKind {
    Input,
    Story,
    System,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Record {
    pub kind: RecordKind,
    pub text: String,
}

pub struct MenuView<'a> {
    pub kind: MenuKind,
    pub prompt: &'a str,
    pub options: &'a [MenuOption],
    pub selected: usize,
}

pub struct ViewState<'a> {
    pub title: &'a str,
    pub scene_name: &'a str,
    pub world: World,
    pub scene_text: &'a str,
    pub state: &'a SessionState,
    pub input: &'a CommandInput,
    pub records: &'a VecDeque<Record>,
    pub menu: Option<MenuView<'a>>,
    pub confirmation: Option<&'a str>,
    pub no_motion: bool,
}

pub fn draw(frame: &mut Frame, state: &ViewState<'_>) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }

    let [header, body, input] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .areas(area);

    let [scene_area, log_area] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);

    draw_header(frame, header, state);
    draw_scene(frame, scene_area, state);
    draw_log(frame, log_area, state);
    draw_input(frame, input, state);

    if let Some(menu) = &state.menu {
        draw_menu(frame, area, menu);
    }
    if let Some(prompt) = state.confirmation {
        draw_confirmation(frame, area, prompt);
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let block = Block::bordered().title("DarkBluff");
    let text = Paragraph::new("终端窗口太小，请至少调整到 80x24。")
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(text, area);
}

fn draw_header(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let world = match state.world {
        World::Surface => Span::styled("右眼·表面", Style::default().fg(Color::Cyan)),
        World::Shadow => Span::styled("左眼·影子", Style::default().fg(Color::Magenta)),
    };
    let motion = if state.no_motion {
        " · 动画关闭"
    } else {
        ""
    };
    let line = Line::from(vec![
        Span::styled(" DarkBluff ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(state.title),
        Span::raw(" / "),
        Span::raw(state.scene_name),
        Span::raw(" / "),
        world,
        Span::raw(motion),
    ]);
    let block = Block::bordered();
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_scene(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let title = format!(" 场景 · {} ", state.scene_name);
    let block = Block::bordered().title(title);
    let paragraph = Paragraph::new(state.scene_text)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_log(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let height = area.height.saturating_sub(2) as usize;
    let skip = state.records.len().saturating_sub(height);
    let items = state.records.iter().skip(skip).map(record_item);
    let block = Block::bordered().title(" 记录 ");
    frame.render_widget(List::new(items).block(block), area);
}

fn record_item(record: &Record) -> ListItem<'_> {
    let tag = match record.kind {
        RecordKind::Input => Span::styled("输入", Color::Green),
        RecordKind::Story => Span::styled("叙事", Color::White),
        RecordKind::System => Span::styled("系统", Color::Blue),
        RecordKind::Warning => Span::styled("提示", Color::Yellow),
        RecordKind::Error => Span::styled("错误", Color::Red),
    };
    ListItem::new(Line::from(vec![
        Span::raw("["),
        tag,
        Span::raw("] "),
        Span::raw(record.text.as_str()),
    ]))
}

fn draw_input(frame: &mut Frame, area: Rect, state: &ViewState<'_>) {
    let (title, content) = match state.state {
        SessionState::Title => (
            " 标题 ",
            "↑/↓ 选择，Enter 确认，可按数字直选".to_string(),
        ),
        SessionState::Exploring => (
            " 指令 ",
            format!("{}{}", INPUT_PROMPT, state.input.value()),
        ),
        SessionState::Confirming => (" 确认 ", "按 y/Enter 确认，n/Esc 取消".to_string()),
        SessionState::ShowingIntro | SessionState::ShowingOutro | SessionState::Ending => {
            (" 继续 ", "按 Enter 继续".to_string())
        }
        _ => (
            " 选择 ",
            "↑/↓ 选择，Enter 确认，Esc 取消，可按数字直选".to_string(),
        ),
    };
    let block = Block::bordered().title(title);
    frame.render_widget(Paragraph::new(content).block(block), area);

    if matches!(state.state, SessionState::Exploring) {
        // 左边框(1) + 提示符宽度 + 光标可见宽度，避免与 INPUT_PROMPT 隐式耦合。
        let prompt_w = UnicodeWidthStr::width(INPUT_PROMPT) as u16;
        let x = area.x + 1 + prompt_w + state.input.display_cursor();
        let y = area.y + 1;
        let max_x = area.x + area.width.saturating_sub(2);
        frame.set_cursor_position((x.min(max_x), y));
    }
}

fn draw_menu(frame: &mut Frame, area: Rect, menu: &MenuView<'_>) {
    let popup = centered_rect(area, 62, menu.options.len() as u16 + 5);
    frame.render_widget(Clear, popup);

    let title = match menu.kind {
        MenuKind::Title => " 标题 ",
        MenuKind::AskCharacter => " 选择角色 ",
        MenuKind::AskTopic => " 选择话题 ",
        MenuKind::JudgeCharacter => " 选择审判对象 ",
        MenuKind::MoveDestination => " 选择目的地 ",
        MenuKind::Checkpoint => " 选择检查点 ",
    };
    let block = Block::bordered().title(title);
    let mut lines = vec![Line::from(menu.prompt.to_string()), Line::from("")];
    for (i, option) in menu.options.iter().enumerate() {
        let marker = if i == menu.selected { ">" } else { " " };
        let style = if i == menu.selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} {}. ", i + 1), style),
            Span::styled(option.label.as_str(), style),
            Span::styled(
                format!(" ({})", option.id),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), popup);
}

fn draw_confirmation(frame: &mut Frame, area: Rect, prompt: &str) {
    let popup = centered_rect(area, 60, 7);
    frame.render_widget(Clear, popup);
    let text = Text::from(vec![
        Line::from(prompt),
        Line::from(""),
        Line::from("y / Enter 确认    n / Esc 取消").centered(),
    ]);
    let block = Block::bordered().title(" 确认 ");
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: true }),
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
