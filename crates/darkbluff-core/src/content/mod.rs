//! 内容引擎：无状态的加载 / 校验 / 查询层。
//!
//! 设计见 docs/content-engine.md。职责：从 `data/` 加载全部内容到内存、提供统一查询接口，
//! 并通过 [`check`] 执行引用完整性校验。内容引擎本身无状态；游戏状态由 [`crate::engine`] 管理。

pub mod checker;
pub mod condition;
pub mod dialogue;
pub mod engine;
pub mod loader;
pub mod models;

pub use checker::{check, CheckReport, Issue, Severity};
pub use condition::{eval, topic_visible};
pub use dialogue::DialogueBook;
pub use engine::{ChapterMeta, ContentEngine};
pub use loader::{parse_scene_override_name, strip_md_ext, strip_yaml_ext, DataSource, FilesystemSource, InMemorySource};
pub use models::*;
