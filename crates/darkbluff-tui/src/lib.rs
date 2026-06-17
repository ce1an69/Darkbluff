//! Darkbluff TUI 渲染层。
//!
//! 设计见 docs/architecture.md「UI 层」。本 crate 只通过 `darkbluff_core::engine`
//! 的 `Session` / `Input` / `Outcome` 契约驱动 ratatui 渲染循环，不触碰 engine 内部模块。

mod app;
mod input;
mod terminal;
mod view;

use darkbluff_core::content::ContentEngine;
use darkbluff_core::engine::Session;
use darkbluff_core::error::{AppError, Result};
use darkbluff_core::save::{SaveStore, SystemClock};

pub use app::TuiOptions;

/// 启动 TUI。调用方负责先完成内容加载与校验。
pub fn run(engine: ContentEngine, options: TuiOptions) -> Result<()> {
    let save_dir = match options.save_dir {
        Some(dir) => dir,
        None => {
            SaveStore::default_dir().ok_or_else(|| AppError::Save("无法确定默认存档目录".into()))?
        }
    };
    let store = SaveStore::open(save_dir, Box::new(SystemClock))?;
    let session = Session::new(engine, store);
    app::App::new(session, options.no_motion).run()
}
