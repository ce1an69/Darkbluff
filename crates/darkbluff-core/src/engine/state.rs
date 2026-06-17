//! 会话状态机：[`Session`] 持有内容引擎、存档 store 与工作存档，通过 [`Input`] 驱动，
//! 产出 [`Outcome`]。本模块只含类型定义、构造、主分发与共用小助手；指令执行见
//! [`crate::engine::ask`] / [`crate::engine::judge`] / [`crate::engine::navigation`] /
//! [`crate::engine::map`] / [`crate::engine::system`]，章节进入见 [`crate::engine::chapter_flow`]、
//! 笔记见 [`crate::engine::note_view`]、新手引导见 [`crate::engine::hints`]。
//!
//! 设计见 docs/commands.md、docs/data-formats.md「自动推进章节」、
//! docs/save-system.md「自动保存时机」。一切外部输入的错误都转为可读 [`Outcome`]，**绝不 panic**。

use crate::content::ContentEngine;
use crate::engine::commands::{help_for, help_overview, parse, ParseOutcome};
use crate::engine::outcome::{
    ConfirmationAction, Input, MenuKind, Message, Outcome, Selection, SessionState,
};
use crate::save::Save;
use crate::save::SaveStore;

use crate::engine::hints::Hints;

use crate::engine::outcome::MenuOption;

/// 待定交互上下文。状态机负责控制何时消费这些上下文，渲染层只负责回传选择/确认。
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) action: PendingAction,
}

#[derive(Debug, Clone)]
pub(crate) enum PendingAction {
    None,
    Menu {
        _kind: MenuKind,
        options: Vec<MenuOption>,
    },
    AskTopic {
        character: String,
        options: Vec<MenuOption>,
    },
    Intro {
        create_checkpoint: bool,
    },
    Confirm(ConfirmationAction),
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            action: PendingAction::None,
        }
    }
}

/// 游戏会话。
pub struct Session {
    pub(crate) engine: ContentEngine,
    pub(crate) store: SaveStore,
    pub(crate) save: Save,
    pub(crate) state: SessionState,
    pub(crate) pending: Pending,
    pub(crate) hints: Hints,
}

impl Session {
    pub fn new(engine: ContentEngine, store: SaveStore) -> Self {
        Self {
            engine,
            store,
            save: Save::default(),
            state: SessionState::Title,
            pending: Pending::default(),
            hints: Hints::default(),
        }
    }

    pub fn engine(&self) -> &ContentEngine {
        &self.engine
    }
    pub fn save(&self) -> &Save {
        &self.save
    }
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// 是否仍在首章（chapter_path 仅含首章且当前章为首章）。
    pub(crate) fn in_first_chapter(&self) -> bool {
        self.save.chapter_path.len() == 1
            && self
                .engine
                .first_chapter_id()
                .map(|r| r == self.save.current_chapter.as_str())
                .unwrap_or(false)
    }

    /// 当前场景当前视角的描述文本（缺失时给降级提示）。
    pub(crate) fn scene_description_messages(&self) -> Vec<String> {
        let ch = &self.save.current_chapter;
        let scene = &self.save.current_scene;
        let world = self.save.current_world;
        match self.engine.get_scene_description(ch, scene, world) {
            Some(text) => vec![text.to_string()],
            None => vec!["（该视角的场景描述缺失）".into()],
        }
    }

    /// 持久化工作存档，失败时写 tracing 日志（不阻断流程）。
    pub(crate) fn persist(&self) {
        if let Err(e) = self.store.save(&self.save) {
            tracing::error!("存档保存失败: {e}");
        }
    }
    /// 持久化并返回是否成功（`do_quit` 用：失败则不退出）。
    pub(crate) fn try_persist(&self) -> bool {
        self.store
            .save(&self.save)
            .map_err(|e| tracing::error!("存档保存失败: {e}"))
            .is_ok()
    }

    /// 进入标题界面：构建新游戏/继续/退出菜单，附带探索进度。
    fn enter_title(&mut self) -> Outcome {
        self.state = SessionState::Title;
        let found = self.save.discovered.endings.len();
        let total = self.engine.ending_chapter_ids().len();
        let mut options = vec![MenuOption {
            id: "new_game".into(),
            label: "新游戏".into(),
        }];
        if self.store.has_save() {
            options.push(MenuOption {
                id: "continue".into(),
                label: "继续".into(),
            });
        }
        options.push(MenuOption {
            id: "quit".into(),
            label: "退出".into(),
        });
        self.set_menu(MenuKind::Title, options.clone());
        Outcome::MenuRequested {
            kind: MenuKind::Title,
            prompt: format!("Darkbluff — 已发现结局 {found}/{total}"),
            options,
        }
    }

    // ----- 主分发 -----

    pub fn handle(&mut self, input: Input) -> Outcome {
        match (&self.state.clone(), input) {
            // 强制退出：任意状态都走 do_quit（持久化 + QuitRequested），不绕过状态机。
            (_, Input::Quit) => self.do_quit(),

            (SessionState::ShowingIntro, Input::Ack)
            | (SessionState::ShowingIntro, Input::Cancel) => self.ack_intro(),
            (SessionState::ShowingIntro, _) => Outcome::Ignored,

            (SessionState::ShowingOutro, Input::Ack)
            | (SessionState::ShowingOutro, Input::Cancel) => self.to_ending(),
            (SessionState::ShowingOutro, _) => Outcome::Ignored,

            (SessionState::Ending, Input::Ack) | (SessionState::Ending, Input::Cancel) => {
                self.enter_title()
            }
            (SessionState::Ending, _) => Outcome::Ignored,

            // 标题界面：新游戏 / 继续 / 退出
            (SessionState::Title, Input::Select(selection)) => {
                if !self.has_pending_menu() {
                    self.enter_title()
                } else {
                    match self.selection_id(&selection).as_deref() {
                        Some("new_game") => {
                            if self.store.has_save() {
                                self.pending.action =
                                    PendingAction::Confirm(ConfirmationAction::NewGame);
                                self.state = SessionState::Confirming;
                                Outcome::ConfirmationRequested {
                                    action: ConfirmationAction::NewGame,
                                    prompt: "已有存档，新游戏将覆盖。确认？".into(),
                                }
                            } else {
                                self.start_new_game()
                            }
                        }
                        Some("continue") => match self.store.load() {
                            Ok(crate::save::LoadResult::Save(save, report)) => {
                                let mut outcome = self.continue_with(save);
                                for w in report.warning_messages() {
                                    if let Outcome::Message(ref mut message) = outcome {
                                        message.lines.insert(0, w);
                                    }
                                }
                                outcome
                            }
                            _ => Outcome::Message(Message::error(vec!["存档加载失败。".into()])),
                        },
                        Some("quit") => self.do_quit(),
                        _ => self.enter_title(),
                    }
                }
            }
            (SessionState::Title, _) => {
                if !self.has_pending_menu() {
                    self.enter_title()
                } else {
                    Outcome::Ignored
                }
            }

            (SessionState::Confirming, Input::Confirm(true)) => match self.pending.action.clone() {
                PendingAction::Confirm(ConfirmationAction::NewGame) => {
                    self.pending.action = PendingAction::None;
                    self.start_new_game()
                }
                PendingAction::Confirm(ConfirmationAction::Rollback { checkpoint_id }) => {
                    self.pending.action = PendingAction::None;
                    self.execute_rollback_confirm(&checkpoint_id)
                }
                _ => {
                    self.state = SessionState::Exploring;
                    Outcome::Message(Message::error(vec!["无效确认。".into()]))
                }
            },
            (SessionState::Confirming, Input::Confirm(false))
            | (SessionState::Confirming, Input::Cancel) => {
                let was_title = matches!(
                    self.pending.action,
                    PendingAction::Confirm(ConfirmationAction::NewGame)
                );
                self.pending.action = PendingAction::None;
                if was_title {
                    self.enter_title()
                } else {
                    self.state = SessionState::Exploring;
                    Outcome::Message(Message::info(vec!["已取消。".into()]))
                }
            }

            (SessionState::Exploring, Input::Text(line)) => self.handle_command(&line),
            (SessionState::Exploring, _) => Outcome::Ignored,

            (SessionState::ChoosingAskCharacter, Input::Select(selection)) => {
                self.pick_menu(&selection, |s, id| s.do_ask(Some(id), None))
            }
            (SessionState::ChoosingAskCharacter, Input::Cancel) => self.cancel_menu(),

            (SessionState::ChoosingAskTopic, Input::Select(selection)) => {
                let ch = match &self.pending.action {
                    PendingAction::AskTopic { character, .. } => Some(character.clone()),
                    _ => None,
                };
                let id = self.selection_id(&selection);
                match (ch, id) {
                    (Some(c), Some(t)) => self.ask_topic(&c, &t),
                    _ => Outcome::Message(Message::error(vec!["无效选择。".into()])),
                }
            }
            (SessionState::ChoosingAskTopic, Input::Cancel) => self.cancel_menu(),

            (SessionState::ChoosingJudgeCharacter, Input::Select(selection)) => {
                self.pick_menu(&selection, |s, id| s.do_judge(Some(id)))
            }
            (SessionState::ChoosingJudgeCharacter, Input::Cancel) => self.cancel_menu(),

            (SessionState::ChoosingMove, Input::Select(selection)) => {
                self.pick_menu(&selection, |s, id| s.do_move(Some(id)))
            }
            (SessionState::ChoosingMove, Input::Cancel) => self.cancel_menu(),

            (SessionState::ChoosingCheckpoint, Input::Select(selection)) => {
                if let Some(id) = self.selection_id(&selection) {
                    let action = ConfirmationAction::Rollback { checkpoint_id: id };
                    self.pending.action = PendingAction::Confirm(action.clone());
                    self.state = SessionState::Confirming;
                    Outcome::ConfirmationRequested {
                        action,
                        prompt:
                            "回滚会丢弃该 checkpoint 之后的当前流程进度，discovered 保留。确认？"
                                .into(),
                    }
                } else {
                    Outcome::Message(Message::error(vec!["无效选择。".into()]))
                }
            }
            (SessionState::ChoosingCheckpoint, Input::Cancel) => self.cancel_menu(),

            // 菜单态下其余输入静默忽略（须先 Esc 退出菜单）
            #[allow(unreachable_patterns)]
            (_, Input::Select(_))
            | (_, Input::Cancel)
            | (_, Input::Confirm(_))
            | (_, Input::Ack) => Outcome::Ignored,
            (_, Input::Text(_)) => Outcome::Ignored,
        }
    }

    /// 取菜单选择的 id，存在则执行 `f`，否则提示无效选择。
    fn pick_menu<F>(&mut self, selection: &Selection, f: F) -> Outcome
    where
        F: FnOnce(&mut Session, String) -> Outcome,
    {
        match self.selection_id(selection) {
            Some(id) => f(self, id),
            None => Outcome::Message(Message::error(vec!["无效选择。".into()])),
        }
    }

    pub(crate) fn set_menu(&mut self, kind: MenuKind, options: Vec<MenuOption>) {
        self.pending.action = PendingAction::Menu {
            _kind: kind,
            options,
        };
    }

    pub(crate) fn set_ask_topic_menu(&mut self, character: String, options: Vec<MenuOption>) {
        self.pending.action = PendingAction::AskTopic { character, options };
    }

    pub(crate) fn set_intro_pending(&mut self, create_checkpoint: bool) {
        self.pending.action = PendingAction::Intro { create_checkpoint };
    }

    fn has_pending_menu(&self) -> bool {
        matches!(
            self.pending.action,
            PendingAction::Menu { .. } | PendingAction::AskTopic { .. }
        )
    }

    pub(crate) fn selection_id(&self, selection: &Selection) -> Option<String> {
        let options = match &self.pending.action {
            PendingAction::Menu { options, .. } | PendingAction::AskTopic { options, .. } => {
                options
            }
            _ => return None,
        };
        match selection {
            Selection::Index(i) => options.get(*i).map(|o| o.id.clone()),
            Selection::Id(id) => options.iter().find(|o| o.id == *id).map(|o| o.id.clone()),
        }
    }

    pub(crate) fn cancel_menu(&mut self) -> Outcome {
        self.pending.action = PendingAction::None;
        self.state = SessionState::Exploring;
        Outcome::Message(Message::info(vec!["已取消。".into()]))
    }

    // ----- 指令解析分发 -----

    fn handle_command(&mut self, line: &str) -> Outcome {
        use crate::engine::commands::Command::*;
        match parse(line) {
            ParseOutcome::Empty => Outcome::Ignored,
            ParseOutcome::Unknown(s) => Outcome::Message(Message::error(vec![format!(
                "未知指令：{s}。输入 help 查看可用指令。"
            )])),
            ParseOutcome::TooManyArguments(_) => {
                Outcome::Message(Message::info(vec!["该指令不需要参数。".into()]))
            }
            ParseOutcome::Ok(cmd) => match cmd {
                Ask { target, topic } => self.do_ask(target, topic),
                Judge { target } => self.do_judge(target),
                Move { dest } => self.do_move(dest),
                Gaze => self.do_gaze(),
                Map => self.do_map(),
                Note => self.do_note(),
                Help { cmd } => Self::do_help(cmd),
                Quit => self.do_quit(),
            },
        }
    }

    fn do_help(cmd: Option<String>) -> Outcome {
        match cmd {
            None => Outcome::Message(Message::info(help_overview())),
            Some(c) => match help_for(&c) {
                Some(lines) => Outcome::Message(Message::info(lines)),
                None => Outcome::Message(Message::error(vec![format!(
                    "未知指令：{c}。输入 help 查看可用指令。"
                )])),
            },
        }
    }
}
