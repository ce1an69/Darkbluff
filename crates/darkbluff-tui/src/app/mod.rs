//! 应用控制器：事件循环、按键路由、场景缓存、视图状态组装。
//!
//! 按关注点拆分：[`types`]（数据类型）/ [`outcome`]（Outcome 应用）/
//! [`suggest`]（斜杠补全）。本模块只负责驱动会话与组装视图状态。

mod outcome;
mod scroll;
mod suggest;
mod types;

pub use self::types::*;

use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use darkbluff_core::engine::{
    ConfirmationAction, Input, MenuKind, MenuOption, Selection, Session, SessionState,
};
use darkbluff_core::error::Result;

use self::suggest::strip_slash;
use crate::input::CommandInput;
use crate::markdown::StyledLine;
use crate::terminal::TerminalGuard;
use crate::view::{self, ViewState};
use unicode_width::UnicodeWidthStr;

const POLL_INTERVAL: Duration = Duration::from_millis(120);
const ANIMATION_TICK: Duration = Duration::from_millis(33);
const FULL_ANIMATION: Duration = Duration::from_millis(240);
const REDUCED_ANIMATION: Duration = Duration::from_millis(90);

pub struct App {
    pub(super) session: Session,
    terminate: Arc<AtomicBool>,
    pub(super) running: bool,
    pub(super) input: CommandInput,
    /// 对话/剧情转录（markdown 渲染后的带样式行）。系统提示不入此列。
    pub(super) transcript: VecDeque<StyledLine>,
    /// 转录滚动偏移（从末尾往上计的视觉行数；0=贴底）。新对话到达归零；
    /// 渲染前由 [`App::clamp_transcript_offset`] 按面板几何钳制到 [0, max]。
    pub(super) transcript_offset: usize,
    pub(super) menu: Option<ActiveMenu>,
    pub(super) confirmation: Option<ConfirmationAction>,
    pub(super) suggestions: Option<Suggestions>,
    pub(super) note_panel: Option<NotePanel>,
    pub(super) notice: Option<Notice>,
    pub(super) force_no_motion: bool,
    pub(super) motion: EffectiveMotion,
    pub(super) animation: Option<Animation>,
    /// 打字机：剧情类文本逐字揭示（motion=Off 时不启动）。
    pub(super) typewriter: Option<Typewriter>,
    /// 本次 dispatch 待传 skip（apply_* 填入，process_outcome 取用后归零）。
    pending_tw_skip: usize,
    /// transcript 累计 push 次数（不受 MAX_TRANSCRIPT FIFO 封顶影响），用于度量单次 dispatch 新增行数。
    pub(super) transcript_pushes: usize,
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
    pub fn new(session: Session, no_motion: bool, terminate: Arc<AtomicBool>) -> Self {
        let motion = EffectiveMotion::from_settings(session.settings().motion, no_motion);
        Self {
            session,
            terminate,
            running: true,
            input: CommandInput::default(),
            transcript: VecDeque::new(),
            transcript_offset: 0,
            menu: None,
            confirmation: None,
            suggestions: None,
            note_panel: None,
            notice: None,
            force_no_motion: no_motion,
            motion,
            animation: None,
            typewriter: None,
            pending_tw_skip: 0,
            transcript_pushes: 0,
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
            if self.terminate.swap(false, Ordering::Relaxed) {
                self.dispatch(Input::ForceQuit);
                continue;
            }
            if self.dirty {
                // 渲染前按当前终端尺寸钳制滚动偏移（视图层不再写回，保持无副作用）。
                let size = terminal.terminal().size()?;
                self.clamp_transcript_offset(size);
                terminal.terminal().draw(|frame| {
                    let state = self.view_state();
                    view::draw(frame, &state);
                })?;
                self.dirty = false;
            }
            self.update_animation();
            self.update_typewriter();
            let poll_interval = if self.animation.is_some()
                || self.typewriter.as_ref().is_some_and(|t| t.is_active())
            {
                ANIMATION_TICK
            } else {
                POLL_INTERVAL
            };
            if event::poll(poll_interval)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.dirty = true;
                        self.handle_key(key);
                    }
                    Event::Mouse(mouse) => {
                        if self.handle_mouse(mouse) {
                            self.dirty = true;
                        }
                    }
                    Event::Resize(_, _) => self.dirty = true,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn update_animation(&mut self) {
        if self.animation.as_ref().is_some_and(|a| a.done()) {
            self.animation = None;
            self.dirty = true;
        } else if self.animation.is_some() {
            self.dirty = true;
        }
    }

    /// 推进打字机揭示宽度（每 ANIMATION_TICK 一次）；完成后清除。
    fn update_typewriter(&mut self) {
        let Some(tw) = self.typewriter.as_mut() else {
            return;
        };
        // Off 中途到达（设置切换）：立即完成，避免 step 加法溢出 / 卡死。
        if self.motion.is_off() {
            self.typewriter = None;
            self.dirty = true;
            return;
        }
        // 速度：目标列/秒，按 ANIMATION_TICK(33ms) 折算为每 tick 列数。
        //   Full ≈ 30 列/秒 → 1 列/tick；Reduced ≈ 90 列/秒 → 3 列/tick。
        let step = match self.motion {
            EffectiveMotion::Full => 1,
            EffectiveMotion::Reduced => 3,
            EffectiveMotion::Off => unreachable!(),
        };
        tw.revealed = tw.revealed.saturating_add(step).min(tw.total);
        if !tw.is_active() {
            self.typewriter = None;
        }
        self.dirty = true;
    }

    /// 为本次 dispatch 新增的 `added` 行启动打字机；先结束旧的避免范围混乱。
    /// `skip` = 前导结构行数（blank / header），瞬显不逐字，不计入 total。
    pub(super) fn start_typewriter(&mut self, added: usize, skip: usize) {
        self.typewriter = None;
        let body_lines = added.saturating_sub(skip);
        if body_lines == 0 {
            return;
        }
        let total = self
            .transcript
            .iter()
            .rev()
            .take(body_lines)
            .map(|sl| UnicodeWidthStr::width(sl.text.as_str()))
            .sum();
        self.typewriter = Some(Typewriter {
            lines: added,
            skip,
            revealed: 0,
            total,
        });
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // 通知条：任何新按键都清除上一条。
        self.notice = None;
        // 打字机播放中：任意键立即全显。
        if self.typewriter.as_ref().is_some_and(|t| t.is_active()) {
            self.typewriter = None;
            self.dirty = true;
            // Showing* / Ending 态：仅跳过打字机（不触发 Ack），避免连按推进时
            // 新文本 revealed=0 闪空；玩家再按一次才推进。
            if self.session.state().is_ack() {
                return;
            }
            // Exploring 等态：不吞键，继续正常处理（字符进命令输入等）。
        }
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
            | SessionState::ChoosingSettings
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
            // 设置菜单：左右（或 h/l）循环切换当前维度行的取值。
            KeyCode::Left | KeyCode::Char('h') => self.cycle_setting_key(-1),
            KeyCode::Right | KeyCode::Char('l') => self.cycle_setting_key(1),
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

    /// 设置菜单专用：对当前光标维度行发 Cycle；非 Settings 菜单忽略（保持原忽略行为）。
    fn cycle_setting_key(&mut self, delta: i32) {
        if let Some(menu) = &self.menu
            && matches!(menu.kind, MenuKind::Settings)
        {
            let dim = menu.options[menu.selected].id.clone();
            self.dispatch(Input::Cycle(Selection::Id(dim), delta));
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
        // 滚动键不推进剧情，只滚转录（剧情展示态可回看上文后再按 Enter 继续）。
        match key.code {
            KeyCode::PageUp => {
                self.scroll_transcript(scroll::PAGE_STEP);
                return;
            }
            KeyCode::PageDown => {
                self.scroll_transcript(-scroll::PAGE_STEP);
                return;
            }
            _ => {}
        }
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
            // 滚动键：与补全浮层互不冲突（浮层用 ↑/↓ 选词）。
            KeyCode::PageUp => self.scroll_transcript(scroll::PAGE_STEP),
            KeyCode::PageDown => self.scroll_transcript(-scroll::PAGE_STEP),
            KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => self.apply_edit(key.code),
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
            self.dispatch(Input::Text(cmd));
        }
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
        let before_scene = self.session.save().current_scene.clone();
        let before_world = self.session.save().current_world;
        let before_chapter = self.session.save().current_chapter.clone();
        let outcome = self.session.handle(input);
        self.start_animation_for(&outcome, &before_chapter, &before_scene, before_world);
        self.process_outcome(outcome);
        self.refresh_motion();
        self.refresh_scene();
        self.dirty = true;
    }

    fn start_animation_for(
        &mut self,
        outcome: &darkbluff_core::engine::Outcome,
        before_chapter: &str,
        before_scene: &str,
        before_world: darkbluff_core::world::World,
    ) {
        let save = self.session.save();
        let label = match outcome {
            darkbluff_core::engine::Outcome::ChapterIntro { .. } => Some("Chapter"),
            darkbluff_core::engine::Outcome::ChapterOutro { .. }
            | darkbluff_core::engine::Outcome::EndingReached { .. } => Some("Ending"),
            darkbluff_core::engine::Outcome::Narrative { .. } => Some("Voice"),
            // 仅在「已在章节内」时才用存档 diff 兜底；Title 态 before_chapter 为空
            //（继续/新游戏前的默认存档），避免读档/开局误触发移动动画。
            _ if !before_chapter.is_empty() && save.current_chapter != before_chapter => {
                Some("Chapter")
            }
            _ if !before_chapter.is_empty() && save.current_world != before_world => {
                Some("Gaze")
            }
            _ if !before_chapter.is_empty() && save.current_scene != before_scene => {
                Some("Move")
            }
            _ => None,
        };
        if let Some(label) = label {
            self.start_animation(label);
        }
    }

    fn start_animation(&mut self, label: &'static str) {
        let duration = match self.motion {
            EffectiveMotion::Full => FULL_ANIMATION,
            EffectiveMotion::Reduced => REDUCED_ANIMATION,
            EffectiveMotion::Off => return,
        };
        self.animation = Some(Animation {
            label,
            started: Instant::now(),
            duration,
        });
    }

    fn refresh_motion(&mut self) {
        self.motion =
            EffectiveMotion::from_settings(self.session.settings().motion, self.force_no_motion);
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
        let reached: std::collections::HashSet<&str> = save
            .discovered
            .chapters
            .iter()
            .map(|s| s.as_str())
            .collect();
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
            endings: (
                save.discovered.endings.len(),
                engine.ending_chapter_ids().len(),
            ),
            state,
            input: &self.input,
            transcript: &self.transcript,
            offset: self.transcript_offset,
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
            note: self.note_panel.as_ref(),
            notice: self.notice.as_ref(),
            map: if matches!(state, SessionState::ChoosingCheckpoint) && !self.cached_map.is_empty()
            {
                Some(self.cached_map.as_slice())
            } else {
                None
            },
            no_motion: self.motion.is_off(),
            animation: self.animation.as_ref().map(Animation::view),
            typewriter: self
                .typewriter
                .as_ref()
                .filter(|t| t.is_active())
                .map(|t| t.as_view()),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Animation {
    label: &'static str,
    started: Instant,
    duration: Duration,
}

impl Animation {
    fn done(&self) -> bool {
        self.started.elapsed() >= self.duration
    }

    fn view(&self) -> AnimationView {
        let elapsed = self.started.elapsed().as_secs_f32();
        let total = self.duration.as_secs_f32().max(f32::EPSILON);
        AnimationView {
            label: self.label,
            progress: (elapsed / total).clamp(0.0, 1.0),
        }
    }
}

/// 打字机：覆盖 transcript 末尾 `lines` 行，按显示宽度逐 tick 揭示。
#[derive(Debug, Clone)]
pub(super) struct Typewriter {
    /// 覆盖末尾多少行。
    pub(super) lines: usize,
    /// 前导结构行数（header / blank），瞬显不逐字，不计入 total。
    pub(super) skip: usize,
    /// 已揭示的总显示宽度（列）。
    pub(super) revealed: usize,
    /// 总显示宽度（body_lines 行宽度之和）。
    total: usize,
}

impl Typewriter {
    fn is_active(&self) -> bool {
        self.revealed < self.total
    }

    fn as_view(&self) -> TypewriterView {
        TypewriterView {
            lines: self.lines,
            skip: self.skip,
            revealed: self.revealed,
        }
    }
}

/// 是否处于「显示菜单」的会话状态（标题 + 各 Choosing*）。菜单可见性据此派生。
fn is_menu_state(state: &SessionState) -> bool {
    matches!(
        state,
        SessionState::Title
            | SessionState::ChoosingSettings
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
