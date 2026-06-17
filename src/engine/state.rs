//! 会话状态机：[`Session`] 持有内容引擎、存档 store 与工作存档，通过 [`Input`] 驱动，
//! 产出 [`Outcome`]。本模块只含类型定义、构造、主分发与共用小助手；各指令执行见
//! [`crate::engine::handlers`]、章节进入/推进见 [`crate::engine::chapter_flow`]、
//! 笔记见 [`crate::engine::note_view`]、新手引导见 [`crate::engine::hints`]。
//!
//! 设计见 docs/commands.md、docs/data-formats.md「自动推进章节」、
//! docs/save-system.md「自动保存时机」。一切外部输入的错误都转为可读 [`Outcome`]，**绝不 panic**。

use crate::content::ContentEngine;
use crate::engine::commands::{help_for, help_overview, parse, ParseOutcome};
use crate::engine::outcome::{AppState, Input, Outcome};
use crate::save::Save;
use crate::save::SaveStore;

use crate::engine::hints::Hints;

/// 菜单态下的待定上下文。
#[derive(Debug, Clone, Default)]
pub(crate) struct Pending {
    pub(crate) ask_character: Option<String>,
    pub(crate) menu: Option<Vec<crate::engine::outcome::MenuOption>>,
    pub(crate) intro_needs_checkpoint: bool,
    pub(crate) confirm_rollback_id: Option<String>,
}

/// 游戏会话。
pub struct Session {
    pub(crate) engine: ContentEngine,
    pub(crate) store: SaveStore,
    pub(crate) save: Save,
    pub(crate) state: AppState,
    pub(crate) pending: Pending,
    pub(crate) hints: Hints,
}

impl Session {
    pub fn new(engine: ContentEngine, store: SaveStore) -> Self {
        Self {
            engine,
            store,
            save: Save::default(),
            state: AppState::Title,
            pending: Pending::default(),
            hints: Hints::default(),
        }
    }

    pub fn engine(&self) -> &ContentEngine {
        &self.engine
    }
    pub fn store(&self) -> &SaveStore {
        &self.store
    }
    pub fn save(&self) -> &Save {
        &self.save
    }
    pub fn state(&self) -> &AppState {
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
        self.store.save(&self.save).map_err(|e| tracing::error!("存档保存失败: {e}")).is_ok()
    }

    // ----- 主分发 -----

    pub fn handle(&mut self, input: Input) -> Outcome {
        match (&self.state.clone(), input) {
            (AppState::ShowingIntro, Input::Ack) | (AppState::ShowingIntro, Input::Cancel) => {
                self.ack_intro()
            }
            (AppState::ShowingIntro, _) => Outcome::Ignored,

            (AppState::ShowingOutro, Input::Ack) | (AppState::ShowingOutro, Input::Cancel) => {
                self.to_ending()
            }
            (AppState::ShowingOutro, _) => Outcome::Ignored,

            (AppState::Ending, Input::Ack) | (AppState::Ending, Input::Cancel) => {
                self.state = AppState::Title;
                Outcome::Title
            }
            (AppState::Ending, _) => Outcome::Ignored,

            (AppState::Exploring, Input::Text(line)) => self.handle_command(&line),
            (AppState::Exploring, _) => Outcome::Ignored,

            (AppState::ChoosingAskCharacter, Input::Pick(i)) => self.pick_menu(i, |s, id| s.do_ask(Some(id), None)),
            (AppState::ChoosingAskCharacter, Input::Cancel) => self.cancel_menu(),

            (AppState::ChoosingAskTopic, Input::Pick(i)) => {
                let ch = self.pending.ask_character.clone();
                let id = self.menu_id(i);
                match (ch, id) {
                    (Some(c), Some(t)) => self.ask_topic(&c, &t),
                    _ => Outcome::Show(vec!["无效选择。".into()]),
                }
            }
            (AppState::ChoosingAskTopic, Input::Cancel) => self.cancel_menu(),

            (AppState::ChoosingJudgeCharacter, Input::Pick(i)) => {
                self.pick_menu(i, |s, id| s.do_judge(Some(id)))
            }
            (AppState::ChoosingJudgeCharacter, Input::Cancel) => self.cancel_menu(),

            (AppState::ChoosingMove, Input::Pick(i)) => self.pick_menu(i, |s, id| s.do_move(Some(id))),
            (AppState::ChoosingMove, Input::Cancel) => self.cancel_menu(),

            (AppState::ChoosingCheckpoint, Input::Pick(i)) => {
                if let Some(id) = self.menu_id(i) {
                    self.pending.confirm_rollback_id = Some(id);
                    self.state = AppState::Confirming;
                    Outcome::Confirm {
                        prompt: "回滚会丢弃该 checkpoint 之后的当前流程进度，discovered 保留。确认？".into(),
                    }
                } else {
                    Outcome::Show(vec!["无效选择。".into()])
                }
            }
            (AppState::ChoosingCheckpoint, Input::Cancel) => self.cancel_menu(),

            (AppState::Confirming, Input::Confirm(true)) => self.execute_rollback_confirm(),
            (AppState::Confirming, Input::Confirm(false)) | (AppState::Confirming, Input::Cancel) => {
                self.state = AppState::Exploring;
                Outcome::Show(vec!["已取消。".into()])
            }

            // 菜单态下其余输入静默忽略（须先 Esc 退出菜单）
            (_, Input::Pick(_)) | (_, Input::Cancel) | (_, Input::Confirm(_)) | (_, Input::Ack) => {
                Outcome::Ignored
            }
            (_, Input::Text(_)) => Outcome::Ignored,
        }
    }

    /// 取菜单第 i 项的 id，存在则执行 `f`，否则提示无效选择。
    fn pick_menu<F>(&mut self, i: usize, f: F) -> Outcome
    where
        F: FnOnce(&mut Session, String) -> Outcome,
    {
        match self.menu_id(i) {
            Some(id) => f(self, id),
            None => Outcome::Show(vec!["无效选择。".into()]),
        }
    }

    pub(crate) fn menu_id(&mut self, i: usize) -> Option<String> {
        self.pending
            .menu
            .as_ref()
            .and_then(|opts| opts.get(i))
            .map(|o| o.id.clone())
    }

    pub(crate) fn cancel_menu(&mut self) -> Outcome {
        self.pending.menu = None;
        self.pending.ask_character = None;
        self.state = AppState::Exploring;
        Outcome::Show(vec!["已取消。".into()])
    }

    // ----- 指令解析分发 -----

    fn handle_command(&mut self, line: &str) -> Outcome {
        use crate::engine::commands::Command::*;
        match parse(line) {
            ParseOutcome::Empty => Outcome::Ignored,
            ParseOutcome::Unknown(s) => Outcome::Show(vec![format!(
                "未知指令：{s}。输入 help 查看可用指令。"
            )]),
            ParseOutcome::TooManyArguments(_) => {
                Outcome::Show(vec!["该指令不需要参数。".into()])
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
            None => Outcome::Show(help_overview()),
            Some(c) => match help_for(&c) {
                Some(lines) => Outcome::Show(lines),
                None => Outcome::Show(vec![format!(
                    "未知指令：{c}。输入 help 查看可用指令。"
                )]),
            },
        }
    }
}
