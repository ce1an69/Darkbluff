//! TUI 本地的指令展示元数据（中文），用于斜杠补全与提示。
//!
//! 指令**名单**直接取自引擎权威源 [`darkbluff_core::engine::COMMAND_NAMES`]（解析与补全共用，
//! 不会漂移）；中文 `desc`/`args` 是纯 UI 文案，留在本层。

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
        "ask" => "询问在场角色",
        "judge" => "审判角色",
        "move" => "前往其他场景",
        "gaze" => "切换视角",
        "map" => "打开章节地图",
        "note" => "查看笔记",
        "help" => "查看指令帮助",
        "quit" => "存档并退出",
        _ => "",
    }
}

fn args_for(name: &str) -> &'static str {
    match name {
        "ask" => "[对象] [话题]",
        "judge" => "[对象]",
        "move" => "[目的地]",
        "help" => "[指令]",
        _ => "",
    }
}
