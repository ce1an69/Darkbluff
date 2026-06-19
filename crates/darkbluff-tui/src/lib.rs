//! Darkbluff TUI 渲染层。
//!
//! 设计见 docs/architecture.md「UI 层」。本 crate 只通过 `darkbluff_core::engine`
//! 的 `Session` / `Input` / `Outcome` 契约驱动 ratatui 渲染循环，不触碰 engine 内部模块。

mod app;
mod command;
mod input;
mod markdown;
mod terminal;
mod theme;
mod view;

use std::sync::{Arc, atomic::AtomicBool};

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
    let terminate = Arc::new(AtomicBool::new(false));
    install_sigterm_handler(Arc::clone(&terminate));
    app::App::new(session, options.no_motion, terminate).run()
}

#[cfg(unix)]
fn install_sigterm_handler(terminate: Arc<AtomicBool>) {
    // 注册失败（罕见：信号已被注册、受限环境）则告警；此时 SIGTERM 回落默认处置。
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGTERM, terminate) {
        tracing::warn!("SIGTERM handler 注册失败，信号将走默认处置：{e}");
    }
}

#[cfg(not(unix))]
fn install_sigterm_handler(_: Arc<AtomicBool>) {}
