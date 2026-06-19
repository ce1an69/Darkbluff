//! TUI 本地的指令展示元数据（英文），用于斜杠补全与提示。
//!
//! 指令**名单**直接取自引擎权威源 [`darkbluff_core::engine::COMMAND_NAMES`]（解析与补全共用，
//! 不会漂移）；英文 `desc`/`args` 是纯 UI 文案，留在本层。

use darkbluff_core::engine::COMMAND_NAMES;

/// 一条指令的展示元数据。
pub struct CommandMeta {
    pub name: &'static str,
    pub desc: &'static str,
    pub args: &'static str,
}

/// 全部指令（顺序与引擎 `COMMAND_NAMES` 一致）。
pub fn all() -> Vec<CommandMeta> {
    COMMAND_NAMES
        .iter()
        .map(|&name| CommandMeta {
            name,
            desc: desc_for(name),
            args: args_for(name),
        })
        .collect()
}

/// 指令名是否被引擎识别（直接查权威名单）。
pub fn is_known(verb: &str) -> bool {
    COMMAND_NAMES.contains(&verb)
}

fn desc_for(name: &str) -> &'static str {
    match name {
        "ask" => "Question a character",
        "judge" => "Judge a character",
        "move" => "Travel to a scene",
        "gaze" => "Switch perspective",
        "map" => "Open checkpoint map",
        "note" => "Review seen notes",
        "help" => "Show command help",
        "quit" => "Save and exit",
        _ => "",
    }
}

fn args_for(name: &str) -> &'static str {
    match name {
        "ask" => "[target] [topic]",
        "judge" => "[target]",
        "move" => "[dest]",
        "help" => "[cmd]",
        _ => "",
    }
}
