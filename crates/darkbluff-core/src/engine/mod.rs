//! 游戏引擎：条件求值、指令解析、状态机、审判与自动推进逻辑。
//!
//! 设计见 docs/commands.md、docs/content-engine.md「条件求值实现」、
//! docs/data-formats.md「自动推进章节」。引擎层依赖 [`crate::content`] 与
//! [`crate::save`]，是 TUI 之下、存档/内容之上的中间层。

mod ask;
mod chapter_flow;
mod commands;
mod condition;
mod hints;
mod judge;
mod logic;
mod map;
mod navigation;
mod note_view;
mod outcome;
mod state;
mod system;
mod trigger;

pub use outcome::{
    ConfirmationAction, Input, MenuKind, MenuOption, Message, MessageLevel, NoteView, Outcome,
    Selection, SessionState,
};
pub use state::Session;

/// 已知指令名（解析与渲染层共用，避免两处名单漂移）。
pub use commands::COMMAND_NAMES;
/// 渲染层补全用的候选构建器：与引擎菜单同源过滤（可见话题 / 未审判角色 / 可达场景）。
pub use logic::{ask_topic_options, move_options, unjudged_character_options};
