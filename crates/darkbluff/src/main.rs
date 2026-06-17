//! Darkbluff 二进制入口。
//!
//! 装配 [`darkbluff_core`] 引擎与 CLI，分发 `play`/`check` 子命令。
//! TUI 渲染层（`darkbluff-tui`）尚未接入；`play` 当前打印提示后退出。

mod cli;
mod log;

fn main() -> darkbluff_core::Result<()> {
    cli::run()
}
