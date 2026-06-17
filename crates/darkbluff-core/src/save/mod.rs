//! 存档系统：单存档自动保存、检查点回滚、原子写入与版本迁移。
//!
//! 设计见 docs/save-system.md。存档采用「存储事实为权威」模型，`checkpoints`
//! 只记录三个权威数组在创建时刻的长度，回滚按长度截断。存档层是纯字符串模型，
//! 不依赖 [`crate::content`]。

pub mod atomic;
pub mod checkpoint;
pub mod clock;
pub mod migration;
pub mod schema;
pub mod snapshot;
pub mod store;

pub use clock::{Clock, FakeClock, SystemClock};
pub use schema::*;
pub use store::{LoadReport, LoadResult, SaveStore};
