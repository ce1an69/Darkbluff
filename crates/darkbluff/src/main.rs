//! Darkbluff 二进制入口。
//!
//! 装配 [`darkbluff_core`] 引擎与 CLI，分发 `play`/`check` 子命令。
//! TUI 渲染层由 `darkbluff-tui` crate 提供。

mod cli;
mod log;

fn main() -> darkbluff_core::Result<()> {
    cli::run()
}
