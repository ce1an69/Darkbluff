//! CLI 参数解析与子命令分发。
//!
//! 设计见 docs/architecture.md「CLI 与运行模式」。
//! - `darkbluff` / `darkbluff play`：进入游戏。
//! - `darkbluff check`：离线校验 `data/` 内容，不启动 TUI。
//! - `--no-motion`：本次运行关闭过渡动画。
//! - `--data-dir <path>`：覆盖内容数据目录（默认当前目录下的 `data/`）。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::log;

use darkbluff_core::content::{ContentEngine, FilesystemSource, check};
use darkbluff_core::error::{AppError, Result};

/// 命令行参数。
#[derive(Parser, Debug)]
#[command(name = "darkbluff", about = "DarkBluff —— 双瞳推理游戏")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// 本次运行关闭过渡动画（临时，不写设置文件；TUI 实装后生效）
    #[arg(long, global = true)]
    no_motion: bool,

    /// 指定内容数据目录（默认 ./data）
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// 进入游戏（标题界面）
    Play,
    /// 离线校验 data/ 内容，不启动 TUI
    Check,
}

/// CLI 主入口。
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.clone().unwrap_or(Command::Play) {
        Command::Play => {
            // play 模式日志写文件（TUI 用 alternate screen，不写 stderr）
            if let Some(dir) = log::default_log_dir() {
                let _guard = log::init_to_file(dir);
                return run_play(&cli);
            }
            run_play(&cli)
        }
        Command::Check => {
            log::init_to_stderr();
            run_check(&cli)
        }
    }
}

fn resolve_data_dir(cli: &Cli) -> Result<PathBuf> {
    if let Some(d) = &cli.data_dir {
        return Ok(d.clone());
    }
    // 默认：当前目录下的 data/（开发模式）。发布模式应来自 include_dir!（未实现）。
    Ok(PathBuf::from("data"))
}

fn run_play(cli: &Cli) -> Result<()> {
    let engine = load_checked_engine(cli)?;
    darkbluff_tui::run(
        engine,
        darkbluff_tui::TuiOptions {
            no_motion: cli.no_motion,
            save_dir: None,
        },
    )
}

fn load_checked_engine(cli: &Cli) -> Result<ContentEngine> {
    let data_dir = resolve_data_dir(cli)?;
    if cli.data_dir.is_none() && !data_dir.exists() {
        return Err(AppError::Content(format!(
            "默认内容目录不存在：{}。当前仓库尚未包含正式 data/，请使用 --data-dir 指向内容目录，例如 crates/darkbluff-core/tests/fixtures/data",
            data_dir.display()
        )));
    }

    let src = FilesystemSource::new(&data_dir)?;
    let engine = ContentEngine::load(&src)?;
    let report = check(&engine);
    if report.has_errors() {
        let errors = report.errors().count();
        let warnings = report.warnings().count();
        tracing::warn!(errors, warnings, "内容校验未通过，已阻止 play 启动");
        return Err(darkbluff_core::error::AppError::Content(format!(
            "内容校验未通过：{errors} 个错误，{warnings} 个警告。请先运行 darkbluff check --data-dir {}",
            data_dir.display()
        )));
    }
    Ok(engine)
}

fn run_check(cli: &Cli) -> Result<()> {
    let data_dir = resolve_data_dir(cli)?;
    let src = FilesystemSource::new(&data_dir)?;
    let engine = ContentEngine::load(&src)?;

    let report = check(&engine);

    // 错误优先输出
    let mut issues = report.issues.clone();
    issues.sort_by_key(|i| !i.severity.is_error());
    for issue in &issues {
        println!("[{}] {}", issue.severity.label(), issue.message);
    }

    let errors = report.errors().count();
    let warnings = report.warnings().count();
    if errors > 0 {
        tracing::warn!(errors, warnings, "内容校验未通过");
        eprintln!("校验未通过：{errors} 个错误，{warnings} 个警告");
        std::process::exit(1);
    }
    println!("校验通过（{warnings} 个警告）");
    Ok(())
}
