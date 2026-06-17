use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use darkbluff_core::engine::{
    Input, MenuKind, MenuOption, Message, MessageLevel, Outcome, Selection, Session, SessionState,
};
use darkbluff_core::error::Result;

use crate::input::CommandInput;
use crate::terminal::TerminalGuard;
use crate::view::{self, Record, RecordKind, ViewState};

const POLL_INTERVAL: Duration = Duration::from_millis(120);
const MAX_RECORDS: usize = 512;

#[derive(Debug, Clone, Default)]
pub struct TuiOptions {
    pub no_motion: bool,
    pub save_dir: Option<PathBuf>,
}

pub struct App {
    session: Session,
    running: bool,
    input: CommandInput,
    records: VecDeque<Record>,
    menu: Option<ActiveMenu>,
    confirmation: Option<String>,
    no_motion: bool,
    /// 缓存的当前场景标题/名称/描述，仅在 dispatch 后刷新，避免每帧重算引擎查询。
    cached_title: String,
    cached_scene_name: String,
    cached_scene_text: String,
    /// 是否需要在下一轮重绘；仅在状态变更/按键/ resize 时置位，空闲时跳过 draw。
    dirty: bool,
}

#[derive(Debug, Clone)]
struct ActiveMenu {
    kind: MenuKind,
    prompt: String,
    options: Vec<MenuOption>,
    selected: usize,
}

impl App {
    pub fn new(session: Session, no_motion: bool) -> Self {
        Self {
            session,
            running: true,
            input: CommandInput::default(),
            records: VecDeque::new(),
            menu: None,
            confirmation: None,
            no_motion,
            cached_title: String::new(),
            cached_scene_name: String::new(),
            cached_scene_text: String::new(),
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
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            // 走引擎标准退出（任意状态持久化 + QuitRequested），不绕过状态机。
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
                // 数字直选：越界数字忽略，避免向引擎发送无效 Select 而触发菜单脱节。
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
        match key.code {
            KeyCode::Esc => self.input = CommandInput::default(),
            KeyCode::Enter => {
                let line = self.input.submit();
                if !line.trim().is_empty() {
                    self.push(RecordKind::Input, format!("> {}", line.trim()));
                }
                self.dispatch(Input::Text(line));
            }
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Home => self.input.jump_start(),
            KeyCode::End => self.input.jump_end(),
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.input.insert(c);
                }
            }
            _ => {}
        }
    }

    fn process_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Message(message) => self.push_message(message),
            Outcome::Dialogue {
                header,
                body,
                notes,
            } => {
                self.clear_overlays();
                self.push(RecordKind::Story, header);
                self.push_body(RecordKind::Story, &body);
                for note in notes {
                    self.push(RecordKind::System, note);
                }
            }
            Outcome::MenuRequested {
                kind,
                prompt,
                options,
            } => {
                self.confirmation = None;
                self.menu = Some(ActiveMenu {
                    kind,
                    prompt,
                    options,
                    selected: 0,
                });
            }
            Outcome::ConfirmationRequested { prompt, .. } => {
                self.menu = None;
                self.confirmation = Some(prompt);
            }
            Outcome::ChapterIntro { text } => self.push_chapter_card("章节开场", &text),
            Outcome::ChapterOutro { text } => self.push_chapter_card("结局收束", &text),
            Outcome::Notes(note_view) => {
                self.clear_overlays();
                self.push(RecordKind::System, "笔记".into());
                if note_view.narratives.is_empty()
                    && note_view.dialogues.is_empty()
                    && note_view.judgments.is_empty()
                {
                    self.push(RecordKind::System, "还没有记录。".into());
                }
                for n in note_view.narratives {
                    let kind = if n.is_outro { "结局" } else { "开场" };
                    self.push_note_section(format!("[{kind}] {}", n.title), &n.text);
                }
                for d in note_view.dialogues {
                    self.push_note_section(
                        format!("[对话] {} / {}", d.character_name, d.topic_label),
                        &d.text,
                    );
                }
                for j in note_view.judgments {
                    self.push_note_section(format!("[审判] {}", j.target_name), &j.text);
                }
            }
            Outcome::EndingReached {
                title,
                found,
                total,
            } => {
                self.clear_overlays();
                self.push(RecordKind::System, format!("达成结局：{title}"));
                self.push(RecordKind::System, format!("已发现结局 {found}/{total}"));
                self.push(RecordKind::System, "按 Enter 返回标题".into());
            }
            Outcome::QuitRequested => {
                self.running = false;
            }
            Outcome::Ignored => {}
        }
    }

    fn dispatch(&mut self, input: Input) {
        let outcome = self.session.handle(input);
        self.process_outcome(outcome);
        self.refresh_scene();
        self.dirty = true;
    }

    fn clear_overlays(&mut self) {
        self.menu = None;
        self.confirmation = None;
    }

    fn push_message(&mut self, message: Message) {
        // 注意：不清覆盖层。错误消息（如菜单越界选择）并不等于离开菜单；
        // 菜单/确认弹窗的可见性统一由 session.state() 派生（见 view_state）。
        let kind = match message.level {
            MessageLevel::Info => RecordKind::System,
            MessageLevel::Warning => RecordKind::Warning,
            MessageLevel::Error => RecordKind::Error,
        };
        for line in message.lines {
            self.push(kind, line);
        }
    }

    fn push_chapter_card(&mut self, title: &str, text: &str) {
        self.clear_overlays();
        self.push(RecordKind::Story, title.into());
        self.push_body(RecordKind::Story, text);
        self.push(RecordKind::System, "按 Enter 继续".into());
    }

    fn push_note_section(&mut self, label: String, text: &str) {
        self.push(RecordKind::System, label);
        self.push_body(RecordKind::Story, text);
    }

    fn push_body(&mut self, kind: RecordKind, body: &str) {
        for line in body.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.push(kind, trimmed.to_string());
            }
        }
    }

    fn push(&mut self, kind: RecordKind, text: String) {
        self.records.push_back(Record { kind, text });
        while self.records.len() > MAX_RECORDS {
            self.records.pop_front();
        }
    }

    /// 仅在 dispatch 后调用；把当前场景的标题/名称/描述缓存为 owned 字符串，
    /// draw 闭包每帧只读缓存，不再重复调用引擎查询。
    fn refresh_scene(&mut self) {
        let (title, scene_name, scene_text) = {
            let save = self.session.save();
            let engine = self.session.engine();
            let title = engine
                .get_chapter(&save.current_chapter)
                .map(|c| c.title.clone())
                .unwrap_or_else(|| "DarkBluff".to_string());
            let scene_name = engine
                .get_scene(&save.current_scene)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "未知地点".to_string());
            let scene_text = engine
                .get_scene_description(&save.current_chapter, &save.current_scene, save.current_world)
                .unwrap_or("（暂无场景描述）")
                .to_string();
            (title, scene_name, scene_text)
        };
        self.cached_title = title;
        self.cached_scene_name = scene_name;
        self.cached_scene_text = scene_text;
    }

    fn view_state(&self) -> ViewState<'_> {
        let state = self.session.state();
        let show_menu = is_menu_state(state);
        ViewState {
            title: &self.cached_title,
            scene_name: &self.cached_scene_name,
            world: self.session.save().current_world,
            scene_text: &self.cached_scene_text,
            state,
            input: &self.input,
            records: &self.records,
            menu: if show_menu {
                self.menu.as_ref().map(|m| view::MenuView {
                    kind: m.kind,
                    prompt: &m.prompt,
                    options: &m.options,
                    selected: m.selected,
                })
            } else {
                None
            },
            confirmation: if matches!(state, SessionState::Confirming) {
                self.confirmation.as_deref()
            } else {
                None
            },
            no_motion: self.no_motion,
        }
    }
}

/// 是否处于「显示菜单」的会话状态（标题 + 各 Choosing*）。
/// 菜单弹窗可见性据此派生，而非另存一份可能与引擎脱节的 self.menu。
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
