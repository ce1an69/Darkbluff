//! 游戏引擎：条件求值、指令解析、状态机、审判与自动推进逻辑。
//!
//! 设计见 docs/commands.md、docs/content-engine.md「条件求值实现」、
//! docs/data-formats.md「自动推进章节」。引擎层依赖 [`crate::content`] 与
//! [`crate::save`]，是 TUI 之下、存档/内容之上的中间层。

pub mod ask;
pub mod chapter_flow;
pub mod commands;
pub mod condition;
pub mod hints;
pub mod judge;
pub mod logic;
pub mod map;
pub mod navigation;
pub mod note_view;
pub mod outcome;
pub mod state;
pub mod system;

// 纯求值函数实际定义在 content::condition，这里转发再导出，便于引擎内部统一从 engine 命名空间使用。
pub use crate::content::condition::{eval, topic_visible};
pub use condition::{build_factset, chapter_complete, required_judgments_complete, FactSet};
pub use logic::reconcile_save;
pub use outcome::{
    ConfirmationAction, Input, MenuKind, Message, MessageLevel, NoteView, Outcome, Selection,
    SessionState,
};
pub use state::Session;
