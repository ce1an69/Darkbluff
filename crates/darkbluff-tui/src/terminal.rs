//! 终端生命周期管理：raw mode + alternate screen + 鼠标滚轮捕获 的 RAII 守卫。
//!
//! 经 `ratatui::try_init` / `ratatui::restore` 接管进出：`try_init` 会安装 panic hook，
//! 即便运行期 panic 也会先恢复终端；失败（如非 TTY 环境）返回 `io::Error` 而非 panic。
//!
//! 捕获鼠标仅为滚轮滚动转录。代价：捕获期间终端原生的鼠标文本选择被禁用；
//! 多数终端按住 **Shift** 再拖拽可绕过捕获做选择/复制（通用约定）。

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use ratatui::DefaultTerminal;
use std::io::stdout;

use darkbluff_core::error::Result;

/// 进入 raw mode + alternate screen，返回终端句柄；Drop 时自动恢复。
pub struct TerminalGuard {
    terminal: DefaultTerminal,
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        let terminal = ratatui::try_init()?;
        // 捕获鼠标以接收滚轮事件。失败记日志但不致命（部分环境/重定向下不支持，退化为无滚轮）。
        if let Err(e) = execute!(stdout(), EnableMouseCapture) {
            tracing::warn!("启用鼠标捕获失败（滚轮将不可用）: {e}");
        }
        Ok(Self { terminal })
    }

    pub fn terminal(&mut self) -> &mut DefaultTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // 先关鼠标捕获，再退出 alt screen / raw mode，避免终端残留鼠标报告。
        if let Err(e) = execute!(stdout(), DisableMouseCapture) {
            tracing::warn!("关闭鼠标捕获失败: {e}");
        }
        ratatui::restore();
    }
}
