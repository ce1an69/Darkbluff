use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use darkbluff_core::engine::{
    ask_topic_options, ConfirmationAction, Input, MenuKind, MenuOption, MessageLevel,
    move_options, Outcome, Selection, Session, SessionState, unjudged_character_options,
};
use darkbluff_core::error::Result;
use ratatui::style::{Modifier, Style};

use crate::command;
use crate::input::CommandInput;
use crate::markdown::StyledLine;
use crate::terminal::TerminalGuard;
use crate::theme;
use crate::view::{self, ViewState};

const POLL_INTERVAL: Duration = Duration::from_millis(120);
const MAX_TRANSCRIPT: usize = 512;

#[derive(Debug, Clone, Default)]
pub struct TuiOptions {
    pub no_motion: bool,
    pub save_dir: Option<PathBuf>,
}

/// 右侧场景面板里单个 NPC 的展示数据。
#[derive(Debug, Clone)]
pub struct NpcInfo {
    pub name: String,
    pub id: String,
    pub topics: Vec<NpcTopic>,
}
#[derive(Debug, Clone)]
pub struct NpcTopic {
    pub label: String,
    pub available: bool,
}

/// 输入框右侧的瞬时状态（错误/提示/引导）。下一次按键即清除。
#[derive(Debug, Clone)]
pub struct StatusLine {
    pub kind: StatusKind,
    pub text: String,
}
#[derive(Debug, Clone, Copy)]
pub enum StatusKind {
    Info,
    Warn,
    Error,
    Hint,
}

/// 斜杠补全浮层。
#[derive(Debug, Clone)]
pub struct Suggestions {
    pub kind: SuggestKind,
    pub items: Vec<Suggestion>,
    pub selected: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestKind {
    Command,
    Character,
    Scene,
    Topic,
}
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub display: String,
    pub desc: String,
    /// 选中后替换行尾 token 的文本（已含尾随空格）。
    pub insert: String,
}

pub struct App {
    session: Session,
    running: bool,
    input: CommandInput,
    /// 对话/剧情转录（markdown 渲染后的带样式行）。系统提示不入此列。
    transcript: VecDeque<StyledLine>,
    menu: Option<ActiveMenu>,
    confirmation: Option<ConfirmationAction>,
    status: Option<StatusLine>,
    suggestions: Option<Suggestions>,
    no_motion: bool,
    cached_title: String,
    cached_scene_name: String,
    cached_scene_text: String,
    cached_npcs: Vec<NpcInfo>,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct ActiveMenu {
    kind: MenuKind,
    options: Vec<MenuOption>,
    selected: usize,
}

impl App {
    pub fn new(session: Session, no_motion: bool) -> Self {
        Self {
            session,
            running: true,
            input: CommandInput::default(),
            transcript: VecDeque::new(),
            menu: None,
            confirmation: None,
            status: None,
            suggestions: None,
            no_motion,
            cached_title: String::new(),
            cached_scene_name: String::new(),
            cached_scene_text: String::new(),
            cached_npcs: Vec::new(),
            dirty: true,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = TerminalGuard::enter()?;
        self.dispatch(Input::Cancel);

        while self.running {
            if self.dirty {
                terminal.terminal().draw(|frame| {
                    let state = self.view_state();
                    view::draw(frame, &state);
                })?;
                self.dirty = false;
            }

            if event::poll(POLL_INTERVAL)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.dirty = true;
                        self.handle_key(key);
                    }
                    Event::Resize(_, _) => self.dirty = true,
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // 瞬时状态：任何新按键都清除上一条错误/提示。
        self.status = None;

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.dispatch(Input::Quit);
            return;
        }

        match self.session.state() {
            SessionState::Title
            | SessionState::ChoosingAskCharacter
            | SessionState::ChoosingAskTopic
            | SessionState::ChoosingJudgeCharacter
            | SessionState::ChoosingMove
            | SessionState::ChoosingCheckpoint => self.handle_menu_key(key),
            SessionState::Confirming => self.handle_confirm_key(key),
            SessionState::ShowingIntro | SessionState::ShowingOutro | SessionState::Ending => {
                self.handle_ack_key(key)
            }
            SessionState::Exploring => self.handle_command_key(key),
        }
    }

    fn handle_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.dispatch(Input::Cancel),
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(menu) = &mut self.menu {
                    menu.selected = menu.selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(menu) = &mut self.menu {
                    let last = menu.options.len().saturating_sub(1);
                    menu.selected = (menu.selected + 1).min(last);
                }
            }
            KeyCode::Enter => {
                if let Some(menu) = &self.menu {
                    let selected = menu.selected;
                    self.dispatch(Input::Select(Selection::Index(selected)));
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(menu) = &self.menu
                    && let Some(index) = digit_index(c)
                    && index < menu.options.len()
                {
                    self.dispatch(Input::Select(Selection::Index(index)));
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.dispatch(Input::Confirm(true))
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.dispatch(Input::Confirm(false))
            }
            _ => {}
        }
    }

    fn handle_ack_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char(_) => {
                self.dispatch(Input::Ack)
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        let palette_open = self.suggestions.is_some();
        match key.code {
            KeyCode::Esc => {
                self.input = CommandInput::default();
                self.recompute_suggestions();
            }
            KeyCode::Up if palette_open => self.move_suggest(-1),
            KeyCode::Down if palette_open => self.move_suggest(1),
            KeyCode::Tab if palette_open => self.complete_suggestion(),
            KeyCode::Enter => {
                // 命令补全语境下，Enter 先补全而非提交（避免提交半个 / 命令）。
                if self
                    .suggestions
                    .as_ref()
                    .is_some_and(|s| s.kind == SuggestKind::Command)
                {
                    self.complete_suggestion();
                    return;
                }
                let line = self.input.submit();
                self.suggestions = None;
                let cmd = strip_slash(&line);
                if !cmd.trim().is_empty() {
                    self.push_line(
                        format!("> {}", cmd.trim()),
                        Style::default().fg(theme::OVERLAY1).add_modifier(Modifier::DIM),
                    );
                }
                self.dispatch(Input::Text(cmd));
            }
            KeyCode::Backspace => {
                self.input.backspace();
                self.recompute_suggestions();
            }
            KeyCode::Delete => {
                self.input.delete();
                self.recompute_suggestions();
            }
            KeyCode::Left => {
                self.input.move_left();
                self.recompute_suggestions();
            }
            KeyCode::Right => {
                self.input.move_right();
                self.recompute_suggestions();
            }
            KeyCode::Home => {
                self.input.jump_start();
                self.recompute_suggestions();
            }
            KeyCode::End => {
                self.input.jump_end();
                self.recompute_suggestions();
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.input.insert(c);
                    self.recompute_suggestions();
                }
            }
            _ => {}
        }
    }

    fn process_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Dialogue {
                header,
                body,
                notes,
            } => {
                self.push_blank();
                self.push_line(
                    header,
                    Style::default()
                        .fg(theme::MAUVE)
                        .add_modifier(Modifier::BOLD),
                );
                self.push_md(&body);
                if !notes.is_empty() {
                    self.set_status(StatusKind::Hint, notes.join("  ·  "));
                }
            }
            Outcome::ChapterIntro { text } => {
                self.push_blank();
                self.push_line(
                    "▌ Intro".into(),
                    Style::default()
                        .fg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                );
                self.push_md(&text);
                self.set_status(StatusKind::Info, "Press Enter to continue".into());
            }
            Outcome::ChapterOutro { text } => {
                self.push_blank();
                self.push_line(
                    "▌ Outro".into(),
                    Style::default()
                        .fg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                );
                self.push_md(&text);
                self.set_status(StatusKind::Info, "Press Enter to continue".into());
            }
            Outcome::Notes(note_view) => {
                self.push_blank();
                self.push_line(
                    "▌ Notes".into(),
                    Style::default()
                        .fg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                );
                for n in note_view.narratives {
                    let tag = if n.is_outro { "Ending" } else { "Intro" };
                    self.push_line(
                        format!("[{tag}] {}", n.title),
                        Style::default().fg(theme::MAUVE),
                    );
                    self.push_md(&n.text);
                }
                for d in note_view.dialogues {
                    self.push_line(
                        format!("{} · {}", d.character_name, d.topic_label),
                        Style::default().fg(theme::MAUVE),
                    );
                    self.push_md(&d.text);
                }
                for j in note_view.judgments {
                    self.push_line(
                        format!("Judge · {}", j.target_name),
                        Style::default().fg(theme::PINK),
                    );
                    self.push_md(&j.text);
                }
            }
            Outcome::EndingReached {
                title,
                found,
                total,
            } => {
                self.push_blank();
                self.push_line(
                    format!("✦ Ending · {title}"),
                    Style::default()
                        .fg(theme::MAUVE)
                        .add_modifier(Modifier::BOLD),
                );
                self.push_line(
                    format!("Endings discovered  {found}/{total}"),
                    Style::default().fg(theme::SUBTEXT0),
                );
                self.set_status(StatusKind::Info, "Press Enter to return".into());
            }
            Outcome::Message(message) => {
                // 多行（如 help）进转录，可读可滚；单行瞬时反馈进输入框右侧状态。
                if message.lines.len() > 1 {
                    self.push_blank();
                    let style = Style::default().fg(theme::SUBTEXT0);
                    for line in message.lines {
                        self.push_line(line, style);
                    }
                } else {
                    let kind = match message.level {
                        MessageLevel::Info => StatusKind::Info,
                        MessageLevel::Warning => StatusKind::Warn,
                        MessageLevel::Error => StatusKind::Error,
                    };
                    self.set_status(
                        kind,
                        message.lines.into_iter().next().unwrap_or_default(),
                    );
                }
            }
            Outcome::MenuRequested {
                kind,
                options,
                ..
            } => {
                self.confirmation = None;
                self.menu = Some(ActiveMenu {
                    kind,
                    options,
                    selected: 0,
                });
            }
            Outcome::ConfirmationRequested { action, .. } => {
                self.menu = None;
                self.confirmation = Some(action);
            }
            Outcome::QuitRequested => {
                self.running = false;
            }
            Outcome::Ignored => {}
        }
    }

    fn dispatch(&mut self, input: Input) {
        self.suggestions = None;
        let outcome = self.session.handle(input);
        self.process_outcome(outcome);
        self.refresh_scene();
        self.dirty = true;
    }

    // ----- 转录 / 状态 小助手 -----

    fn push_line(&mut self, text: String, style: Style) {
        self.transcript.push_back(StyledLine { text, style });
        while self.transcript.len() > MAX_TRANSCRIPT {
            self.transcript.pop_front();
        }
    }
    fn push_md(&mut self, body: &str) {
        for sl in crate::markdown::render(body) {
            self.push_line(sl.text, sl.style);
        }
    }
    fn push_blank(&mut self) {
        self.push_line(String::new(), Style::default());
    }
    fn set_status(&mut self, kind: StatusKind, text: String) {
        self.status = Some(StatusLine { kind, text });
    }

    // ----- 斜杠补全 -----

    fn recompute_suggestions(&mut self) {
        let next = if self.input.cursor_at_end() {
            compute_suggestions(self.input.value(), &self.session)
        } else {
            None
        };
        self.suggestions = next;
    }

    fn move_suggest(&mut self, delta: i32) {
        if let Some(sg) = &mut self.suggestions {
            let last = sg.items.len().saturating_sub(1) as i32;
            let mut s = sg.selected as i32 + delta;
            if s < 0 {
                s = 0;
            }
            if s > last {
                s = last;
            }
            sg.selected = s as usize;
        }
    }

    fn complete_suggestion(&mut self) {
        let Some(insert) = self
            .suggestions
            .as_ref()
            .and_then(|s| s.items.get(s.selected))
            .map(|i| i.insert.clone())
        else {
            return;
        };
        let new_val = apply_completion(self.input.value(), &insert);
        self.input.set_value(new_val);
        self.recompute_suggestions();
    }

    /// 仅在 dispatch 后刷新：缓存场景标题/名称/描述与在场 NPC，供视图层读取。
    fn refresh_scene(&mut self) {
        let (title, scene_name, scene_text, npcs) = {
            let save = self.session.save();
            let engine = self.session.engine();
            let ch = &save.current_chapter;
            let scene = &save.current_scene;
            let world = save.current_world;
            let title = engine
                .get_chapter(ch)
                .map(|c| c.title.clone())
                .unwrap_or_else(|| "DarkBluff".to_string());
            let scene_name = engine
                .get_scene(scene)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "—".to_string());
            let scene_text = engine
                .get_scene_description(ch, scene, world)
                .unwrap_or("No description for this perspective.")
                .to_string();
            let npcs = engine
                .get_characters_in_scene(ch, scene)
                .iter()
                .map(|c| NpcInfo {
                    name: c.name.clone(),
                    id: c.id.clone(),
                    topics: engine
                        .get_topics(ch, &c.id)
                        .iter()
                        .map(|t| NpcTopic {
                            label: t.label.clone(),
                            available: t.available,
                        })
                        .collect(),
                })
                .collect::<Vec<_>>();
            (title, scene_name, scene_text, npcs)
        };
        self.cached_title = title;
        self.cached_scene_name = scene_name;
        self.cached_scene_text = scene_text;
        self.cached_npcs = npcs;
    }

    fn view_state(&self) -> ViewState<'_> {
        let state = self.session.state();
        let save = self.session.save();
        let engine = self.session.engine();
        let found = save.discovered.endings.len();
        let total = engine.ending_chapter_ids().len();
        ViewState {
            title: &self.cached_title,
            scene_name: &self.cached_scene_name,
            world: save.current_world,
            scene_text: &self.cached_scene_text,
            npcs: &self.cached_npcs,
            endings: (found, total),
            state,
            input: &self.input,
            transcript: &self.transcript,
            menu: if is_menu_state(state) {
                self.menu.as_ref().map(|m| view::MenuView {
                    kind: m.kind,
                    options: &m.options,
                    selected: m.selected,
                })
            } else {
                None
            },
            confirmation: if matches!(state, SessionState::Confirming) {
                self.confirmation.as_ref()
            } else {
                None
            },
            suggestions: if matches!(state, SessionState::Exploring) {
                self.suggestions.as_ref()
            } else {
                None
            },
            status: self.status.as_ref(),
            no_motion: self.no_motion,
        }
    }
}

/// 是否处于「显示菜单」的会话状态（标题 + 各 Choosing*）。菜单可见性据此派生。
fn is_menu_state(state: &SessionState) -> bool {
    matches!(
        state,
        SessionState::Title
            | SessionState::ChoosingAskCharacter
            | SessionState::ChoosingAskTopic
            | SessionState::ChoosingJudgeCharacter
            | SessionState::ChoosingMove
            | SessionState::ChoosingCheckpoint
    )
}

fn digit_index(c: char) -> Option<usize> {
    c.to_digit(10)
        .and_then(|n| n.checked_sub(1))
        .map(|n| n as usize)
}

/// 提交时去掉行首空白与全部前导 `/`（斜杠只是 UI 触发符，引擎按无斜杠解析；
/// 容忍 `//ask` 这类多斜杠输入）。
fn strip_slash(line: &str) -> String {
    line.trim_start().trim_start_matches('/').to_string()
}

/// 用补全文本替换输入行尾的半个 token（最后一个空白之后的部分）。
fn apply_completion(value: &str, insert: &str) -> String {
    let prefix = match value.rfind(char::is_whitespace) {
        Some(i) => &value[..i + 1],
        None => "",
    };
    format!("{prefix}{insert}")
}

/// 依当前输入与会话上下文计算斜杠补全候选。
/// 参数候选直接取自引擎的菜单构建器，保证「补全列出的」=「引擎接受的」。
fn compute_suggestions(input: &str, session: &Session) -> Option<Suggestions> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let ends_space = input.ends_with(' ');
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    // 仍在输入首个 / 命令词
    if tokens.len() <= 1 && !ends_space
        && let Some(query) = trimmed.strip_prefix('/')
    {
        let items = command::all()
            .into_iter()
            .filter(|c| query.is_empty() || c.name.starts_with(query))
            .map(|c| Suggestion {
                display: format!("/{}", c.name),
                desc: if c.args.is_empty() {
                    c.desc.to_string()
                } else {
                    format!("{} · {}", c.desc, c.args)
                },
                insert: format!("/{} ", c.name),
            })
            .collect::<Vec<_>>();
        return Suggestions::new(SuggestKind::Command, items);
    }

    // 已有动词，补全参数（候选与引擎菜单同源过滤）
    let verb = tokens.first()?.trim_start_matches('/');
    if !command::is_known(verb) {
        return None;
    }
    let partial = if ends_space {
        ""
    } else {
        tokens.last().copied().unwrap_or("")
    };
    let arg_pos = if ends_space {
        tokens.len()
    } else {
        tokens.len().saturating_sub(1)
    };
    let save = session.save();
    let engine = session.engine();
    let ch = &save.current_chapter;
    let scene = &save.current_scene;
    let (kind, candidates) = match (verb, arg_pos) {
        ("ask", 1) => (
            SuggestKind::Character,
            engine
                .get_characters_in_scene(ch, scene)
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect::<Vec<_>>(),
        ),
        ("ask", 2) if tokens.len() >= 2 => {
            (SuggestKind::Topic, ask_topic_options(engine, save, tokens[1]))
        }
        ("judge", 1) => (SuggestKind::Character, unjudged_character_options(engine, save)),
        ("move", 1) => (SuggestKind::Scene, move_options(engine, save)),
        _ => return None,
    };
    Suggestions::new(kind, filter_opts(candidates, partial))
}

/// 把引擎候选 `(id, label)` 按前缀过滤为补全项。
fn filter_opts(candidates: Vec<(String, String)>, partial: &str) -> Vec<Suggestion> {
    candidates
        .into_iter()
        .filter(|(id, label)| id.starts_with(partial) || label.contains(partial))
        .map(|(id, label)| Suggestion {
            display: format!("{label} · {id}"),
            desc: String::new(),
            insert: format!("{id} "),
        })
        .collect()
}

impl Suggestions {
    fn new(kind: SuggestKind, items: Vec<Suggestion>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self {
                kind,
                items,
                selected: 0,
            })
        }
    }
}
