//! Darkbluff 游戏核心引擎库。
//!
//! 分层（自底向上，无循环依赖）：
//! - [`content`]：内容引擎（无状态加载/查询/校验层）。
//! - [`save`]：存档系统（纯字符串存档模型，不依赖 content）。
//! - [`engine`]：游戏引擎（条件求值、指令解析、状态机、审判/推进逻辑）——对渲染层的门面。
//!
//! 叶子模块：[`world`]（视角枚举）、[`error`]（跨层错误）。
//! TUI 渲染层见 `darkbluff-tui` crate；CLI 入口见 `darkbluff` 二进制 crate。

pub mod content;
pub mod engine;
pub mod error;
pub mod save;
pub mod world;

pub use error::{AppError, Result};
