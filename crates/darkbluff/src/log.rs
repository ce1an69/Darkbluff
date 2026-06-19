//! 日志初始化。
//!
//! 设计见 docs/architecture.md「错误处理与日志」。
//! - `play`：日志写入文件（`{data_dir}/darkbluff.log`，按天滚动），默认 `warn`，`RUST_LOG` 可调。
//!   TUI 使用 alternate screen，故 play 模式不输出到 stderr 以免干扰渲染。
//! - `check`：不启动 TUI，日志输出到 stderr。
//!
//! 大小轮转（设计提到 5MB/保留 2 个）目前以 tracing-appender 的按天滚动近似，待后续替换为
//! 自定义按大小轮转的非阻塞 appender。

use std::path::PathBuf;

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// 默认日志级别。
const DEFAULT_LEVEL: &str = "warn";

/// 初始化日志输出到 stderr（`darkbluff check` 用）。
pub fn init_to_stderr() {
    let filter = env_filter();
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init();
}

/// 初始化日志输出到文件（`darkbluff play` 用）。`log_dir` 为日志所在目录。
/// 返回 appender 守卫；调用方需保持其存活以避免丢日志。
pub fn init_to_file(log_dir: PathBuf) -> tracing_appender::non_blocking::WorkerGuard {
    let filter = env_filter();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "darkbluff.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking))
        .try_init();
    guard
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LEVEL))
}

/// 默认日志目录：`{data_dir}/darkbluff/`。
pub fn default_log_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("darkbluff"))
}
