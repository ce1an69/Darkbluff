//! Darkbluff 二进制入口。
//!
//! TUI 渲染暂未实现；当前仅 `check` 子命令完整可用。`play` 会打印提示后退出。

fn main() -> darkbluff::Result<()> {
    darkbluff::cli::run()
}
