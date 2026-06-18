//! 终端生命周期管理：raw mode + alternate screen 的 RAII 守卫。
//!
//! 经 `ratatui::try_init` / `ratatui::restore` 接管进出：`try_init` 会安装 panic hook，
//! 即便运行期 panic 也会先恢复终端；失败（如非 TTY 环境）返回 `io::Error` 而非 panic。
//! 不捕获鼠标（本 TUI 无鼠标交互）。

use ratatui::DefaultTerminal;

use darkbluff_core::error::Result;

/// 进入 raw mode + alternate screen，返回终端句柄；Drop 时自动恢复。
pub struct TerminalGuard {
    terminal: DefaultTerminal,
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        let terminal = ratatui::try_init()?;
        Ok(Self { terminal })
    }

    pub fn terminal(&mut self) -> &mut DefaultTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
