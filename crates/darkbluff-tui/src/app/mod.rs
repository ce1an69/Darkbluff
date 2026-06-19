//! 应用控制器：事件循环、按键路由、场景缓存、视图状态组装。
//!
//! 按关注点拆分：[`types`]（数据类型）/ [`outcome`]（Outcome 应用）/
//! [`suggest`]（斜杠补全）。本模块只负责驱动会话与组装视图状态。

mod outcome;
mod suggest;
mod types;

pub use self::types::*;

use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use darkbluff_core::engine::{
    ConfirmationAction, Input, MenuKind, MenuOption, Selection, Session, SessionState,
};
use darkbluff_core::error::Result;
use ratatui::style::{Modifier, Style};

use crate::input::CommandInput;
use crate::markdown::StyledLine;
use self::suggest::strip_slash;
use crate::terminal::TerminalGuard;
use crate::theme;
use crate::view::{self, ViewState};

const POLL_INTERVAL: Duration = Duration::from_millis(120);

pub struct App {
    pub(super) session: Session,
    pub(super) running: bool,
    pub(super) input: CommandInput,
    /// 对话/剧情转录（markdown 渲染后的带样式行）。系统提示不入此列。
    pub(super) transcript: VecDeque<StyledLine>,
    pub(super) menu: Option<ActiveMenu>,
    pub(super) confirmation: Option<ConfirmationAction>,
    pub(super) status: Option<StatusLine>,
    pub(super) suggestions: Option<Suggestions>,
    pub(super) note_panel: Option<NotePanel>,
    pub(super) notice: Option<Notice>,
    pub(super) no_motion: bool,
    pub(super) cached_title: String,
    pub(super) cached_scene_name: String,
    pub(super) cached_scene_text: String,
    pub(super) cached_npcs: Vec<NpcInfo>,
    pub(super) cached_map: Vec<crate::view::MapGroup>,
    pub(super) dirty: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveMenu {
    pub(super) kind: MenuKind,
    pub(super) options: Vec<MenuOption>,
    pub(super) selected: usize,
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
            note_panel: None,
            notice: None,
            no_motion,
            cached_title: String::new(),
            cached_scene_name: String::new(),
            cached_scene_text: String::new(),
            cached_npcs: Vec::new(),
            cached_map: Vec::new(),
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
        // 瞬时状态：任何新按键都清除上一条错误/提示与通知条。
        self.status = None;
        self.notice = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.dispatch(Input::Quit);
            return;
        }
        // 笔记独立面板：数字切标签、Esc 关闭（会话仍处 Exploring）。
        if self.note_panel.is_some() {
            self.handle_note_key(key);
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
            SessionState::ShowingIntro
            | SessionState::ShowingNarrative
            | SessionState::ShowingOutro
            | SessionState::Ending => self.handle_ack_key(key),
            SessionState::Exploring => self.handle_command_key(key),
        }
    }

    fn handle_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // 标题态 Esc = 存档退出（其余菜单态 Esc = 取消回探索）。
                if matches!(self.session.state(), SessionState::Title) {
                    self.dispatch(Input::Quit);
                } else {
                    self.dispatch(Input::Cancel);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_menu(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_menu(1),
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

    /// 菜单上下选择（环形：首末循环）。
    fn move_menu(&mut self, delta: i32) {
        if let Some(menu) = &mut self.menu {
            let len = menu.options.len();
            if len > 0 {
                menu.selected = (menu.selected as i32 + delta).rem_euclid(len as i32) as usize;
            }
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
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char(_)
        ) {
            self.dispatch(Input::Ack);
        }
    }

    /// 笔记独立面板：1-4 切标签、Esc 关闭回探索态。
    fn handle_note_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.note_panel = None;
                self.dirty = true;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(tab) = NoteTab::from_digit(c)
                    && let Some(panel) = self.note_panel.as_mut()
                {
                    panel.tab = tab;
                    self.dirty = true;
                }
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
            KeyCode::Enter => self.submit_command(),
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Left | KeyCode::Right
            | KeyCode::Home | KeyCode::End => self.apply_edit(key.code),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.insert(c);
                self.recompute_suggestions();
            }
            _ => {}
        }
    }

    fn submit_command(&mut self) {
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

    /// 行内编辑键统一处理：执行编辑后重算补全。
    fn apply_edit(&mut self, code: KeyCode) {
        match code {
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Home => self.input.jump_start(),
            KeyCode::End => self.input.jump_end(),
            _ => return,
        }
        self.recompute_suggestions();
    }

    fn dispatch(&mut self, input: Input) {
        self.suggestions = None;
        let outcome = self.session.handle(input);
        self.process_outcome(outcome);
        self.refresh_scene();
        self.dirty = true;
    }

    /// 仅在 dispatch 后刷新：缓存场景标题/名称/描述与在场 NPC，供视图层读取。
    fn refresh_scene(&mut self) {
        let (title, scene_name, scene_text, npcs) = self.scene_snapshot();
        self.cached_title = title;
        self.cached_scene_name = scene_name;
        self.cached_scene_text = scene_text;
        self.cached_npcs = npcs;
        self.cached_map = if matches!(self.session.state(), SessionState::ChoosingCheckpoint) {
            self.compute_map_groups()
        } else {
            Vec::new()
        };
    }

    fn scene_snapshot(&self) -> (String, String, String, Vec<NpcInfo>) {
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
            .collect();
        (title, scene_name, scene_text, npcs)
    }

    /// 组装 map 面板的章节树数据：按 `discovered.chapters`（首次到达顺序）排列，每章挂
    /// 其检查点（章节开始 / 审判前）、话题进度与未到分支数（显示为 ???）。
    fn compute_map_groups(&self) -> Vec<crate::view::MapGroup> {
        use darkbluff_core::save::CheckpointKind;
        let save = self.session.save();
        let engine = self.session.engine();
        let current = &save.current_chapter;
        let reached: std::collections::HashSet<&str> =
            save.discovered.chapters.iter().map(|s| s.as_str()).collect();
        save.discovered
            .chapters
            .iter()
            .map(|ch_id| {
                let chapter = engine.get_chapter(ch_id);
                let title = chapter
                    .map(|c| c.title.clone())
                    .unwrap_or_else(|| ch_id.clone());
                let ending = chapter.map(|c| c.ending).unwrap_or(false);
                let is_current = ch_id == current;
                let unseen = engine
                    .next_targets(ch_id)
                    .iter()
                    .filter(|t| !reached.contains(*t))
                    .count();
                let asked = save
                    .discovered
                    .topics
                    .get(ch_id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                let total: usize = chapter
                    .map(|c| c.characters.iter().map(|cc| cc.topics.len()).sum())
                    .unwrap_or(0);
                let topic_progress = if total > 0 {
                    Some((asked.min(total), total))
                } else {
                    None
                };
                let checkpoints = save
                    .checkpoints
                    .iter()
                    .enumerate()
                    .filter(|(_, ck)| &ck.chapter == ch_id)
                    .map(|(i, ck)| {
                        let kind = match ck.kind {
                            CheckpointKind::ChapterStart => "章节开始",
                            CheckpointKind::BeforeJudgment => "审判前",
                        };
                        let scene_name = engine
                            .get_scene(&ck.scene)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| ck.scene.clone());
                        crate::view::MapRow {
                            flat_index: i,
                            label: format!("{kind} · {scene_name}"),
                        }
                    })
                    .collect();
                crate::view::MapGroup {
                    title,
                    ending,
                    is_current,
                    unseen_branches: unseen,
                    topic_progress,
                    checkpoints,
                }
            })
            .collect()
    }

    fn view_state(&self) -> ViewState<'_> {
        let state = self.session.state();
        let save = self.session.save();
        let engine = self.session.engine();
        ViewState {
            title: &self.cached_title,
            scene_name: &self.cached_scene_name,
            world: save.current_world,
            scene_text: &self.cached_scene_text,
            npcs: &self.cached_npcs,
            endings: (save.discovered.endings.len(), engine.ending_chapter_ids().len()),
            state,
            input: &self.input,
            transcript: &self.transcript,
            menu: is_menu_state(state)
                .then_some(self.menu.as_ref())
                .flatten()
                .map(|m| view::MenuView {
                    kind: m.kind,
                    options: &m.options,
                    selected: m.selected,
                }),
            confirmation: (matches!(state, SessionState::Confirming))
                .then_some(self.confirmation.as_ref())
                .flatten(),
            suggestions: (matches!(state, SessionState::Exploring))
                .then_some(self.suggestions.as_ref())
                .flatten(),
            status: self.status.as_ref(),
            note: self.note_panel.as_ref(),
            notice: self.notice.as_ref(),
            map: if matches!(state, SessionState::ChoosingCheckpoint) && !self.cached_map.is_empty()
            {
                Some(self.cached_map.as_slice())
            } else {
                None
            },
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
