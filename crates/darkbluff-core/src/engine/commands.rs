//! 指令解析。
//!
//! 设计见 docs/commands.md「指令一览」「输入解析层」。将原始输入字符串解析为 [`Command`]，
//! 规则：指令名与 ID 不区分大小写（规范化为小写）、容忍多余空白、空输入忽略、未知指令
//! 与「不需要参数」的指令多余参数以可读结果返回（不 panic）。
//!
//! 指令的**执行**（含场景在场校验、菜单生成、自动推进等）见 [`crate::engine::state`]。

/// 已解析的指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `ask [目标] [话题?]`
    Ask {
        target: Option<String>,
        topic: Option<String>,
    },
    /// `judge [目标]`
    Judge { target: Option<String> },
    /// `move [目的地]`
    Move { dest: Option<String> },
    /// `gaze`
    Gaze,
    /// `map`
    Map,
    /// `note`
    Note,
    /// `help [指令?]`
    Help { cmd: Option<String> },
    /// `quit`
    Quit,
}

/// 解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// 空输入：忽略，重新等待输入。
    Empty,
    /// 成功解析为指令。
    Ok(Command),
    /// 未知指令（拼写错误等）：`{原始输入}`，提示「未知指令：{输入}」。
    Unknown(String),
    /// 不需要参数的指令被传入了多余参数：`{指令名}`，提示「该指令不需要参数。」
    TooManyArguments(String),
}

/// 已知指令名（用于 help 列举与校验）。
pub const COMMAND_NAMES: &[&str] = &[
    "ask", "judge", "move", "gaze", "map", "note", "help", "quit",
];

/// 一句话用途说明（按指令名）。
pub fn command_summary(name: &str) -> Option<&'static str> {
    Some(match name {
        "ask" => "从在场角色收集信息；不指定话题时弹出话题菜单",
        "judge" => "审判某角色（章级操作），触发审判剧情；无参数则列出本章未审判角色",
        "move" => "在场景之间移动；无参数时列出可达场景",
        "gaze" => "切换左/右眼（影子/表面视角）",
        "map" => "打开章节树/检查点地图，可选择已经历过的 checkpoint 回滚",
        "note" => "查看玩家已见过的文本记录（对话/叙事/审判剧情）",
        "help" => "列出指令或查看某指令用法",
        "quit" => "自动保存后退出",
        _ => return None,
    })
}

/// `help` 无参数：列出全部指令名称 + 一句话说明。
pub fn help_overview() -> Vec<String> {
    let mut out = vec!["可用指令：".into()];
    for name in COMMAND_NAMES {
        if let Some(summary) = command_summary(name) {
            out.push(format!("  {name:<5} — {summary}"));
        }
    }
    out.push("输入 help [指令] 查看该指令详细用法。".into());
    out
}

/// `help [指令]`：该指令的详细用法。未知指令返回 `None`。
pub fn help_for(name: &str) -> Option<Vec<String>> {
    Some(match name {
        "ask" => vec![
            "ask — 从在场角色收集信息".into(),
            "语法：ask [目标] [话题?]".into(),
            "  不带参数：先选当前场景在场角色，再选话题。".into(),
            "  ask [目标]：列出该角色当前视角下可问的话题。".into(),
            "  ask [目标] [话题]：直接以话题 id 询问。".into(),
            "示例：ask wolf、ask wolf whereabouts".into(),
        ],
        "judge" => vec![
            "judge — 审判某角色（章级操作，不要求在场）".into(),
            "语法：judge [目标]".into(),
            "  不带参数：列出本章尚未审判的角色。".into(),
            "  judge [目标]：触发该角色的审判剧情并记录。".into(),
            "  完成本章必要审判后自动推进剧情。".into(),
            "示例：judge wolf".into(),
        ],
        "move" => vec![
            "move — 在场景之间移动".into(),
            "语法：move [目的地]".into(),
            "  不带参数：列出当前场景可达的目的地。".into(),
            "  move [目的地]：移动到指定场景（保持当前视角）。".into(),
            "示例：move market".into(),
        ],
        "gaze" => vec![
            "gaze — 切换视角".into(),
            "语法：gaze".into(),
            "  在左眼（影子世界）与右眼（表面世界）之间切换。".into(),
            "  表面恒为真命题，影子恒为假命题，取反即得真相。".into(),
        ],
        "map" => vec![
            "map — 章节树 / 检查点地图".into(),
            "语法：map".into(),
            "  浏览已经历过的 checkpoint（chapter_start / before_judgment）。".into(),
            "  选择节点可回滚到该处（破坏性，需确认）。discovered 探索记忆保留。".into(),
        ],
        "note" => vec![
            "note — 查看笔记".into(),
            "语法：note".into(),
            "  展示玩家实际见过的对话、叙事与审判剧情文本。".into(),
            "  记录基于首次查看时的快照，不随后续剧情漂移。".into(),
        ],
        "help" => vec![
            "help — 查看指令用法".into(),
            "语法：help [指令?]".into(),
            "  不带参数：列出全部指令。".into(),
            "  help [指令]：查看该指令详细用法。".into(),
        ],
        "quit" => vec![
            "quit — 退出游戏".into(),
            "语法：quit".into(),
            "  自动保存后退出。".into(),
        ],
        _ => return None,
    })
}

/// 解析原始输入。
pub fn parse(raw: &str) -> ParseOutcome {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return ParseOutcome::Empty;
    }
    let mut parts = trimmed.split_whitespace();
    let name = parts.next().unwrap().to_ascii_lowercase();
    let rest: Vec<String> = parts.map(|s| s.to_ascii_lowercase()).collect();

    match name.as_str() {
        "ask" => ParseOutcome::Ok(Command::Ask {
            target: rest.first().cloned(),
            topic: rest.get(1).cloned(),
        }),
        "judge" => ParseOutcome::Ok(Command::Judge {
            target: rest.first().cloned(),
        }),
        "move" => ParseOutcome::Ok(Command::Move {
            dest: rest.first().cloned(),
        }),
        "help" => ParseOutcome::Ok(Command::Help {
            cmd: rest.first().cloned(),
        }),
        "gaze" | "map" | "note" | "quit" => {
            if rest.is_empty() {
                ParseOutcome::Ok(match name.as_str() {
                    "gaze" => Command::Gaze,
                    "map" => Command::Map,
                    "note" => Command::Note,
                    _ => Command::Quit,
                })
            } else {
                ParseOutcome::TooManyArguments(name)
            }
        }
        _ => ParseOutcome::Unknown(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_ignored() {
        assert_eq!(parse(""), ParseOutcome::Empty);
        assert_eq!(parse("   \t  "), ParseOutcome::Empty);
    }

    #[test]
    fn parses_all_no_arg_commands() {
        assert_eq!(parse("gaze"), ParseOutcome::Ok(Command::Gaze));
        assert_eq!(parse("map"), ParseOutcome::Ok(Command::Map));
        assert_eq!(parse("note"), ParseOutcome::Ok(Command::Note));
        assert_eq!(parse("quit"), ParseOutcome::Ok(Command::Quit));
    }

    #[test]
    fn parses_ask_variants() {
        assert_eq!(
            parse("ask"),
            ParseOutcome::Ok(Command::Ask {
                target: None,
                topic: None
            })
        );
        assert_eq!(
            parse("ask wolf"),
            ParseOutcome::Ok(Command::Ask {
                target: Some("wolf".into()),
                topic: None
            })
        );
        assert_eq!(
            parse("ask wolf whereabouts"),
            ParseOutcome::Ok(Command::Ask {
                target: Some("wolf".into()),
                topic: Some("whereabouts".into()),
            })
        );
    }

    #[test]
    fn parses_judge_move_help() {
        assert_eq!(
            parse("judge wolf"),
            ParseOutcome::Ok(Command::Judge {
                target: Some("wolf".into())
            })
        );
        assert_eq!(
            parse("move market"),
            ParseOutcome::Ok(Command::Move {
                dest: Some("market".into())
            })
        );
        assert_eq!(
            parse("help ask"),
            ParseOutcome::Ok(Command::Help {
                cmd: Some("ask".into())
            })
        );
        assert_eq!(parse("help"), ParseOutcome::Ok(Command::Help { cmd: None }));
    }

    #[test]
    fn case_insensitive() {
        match parse("ASK Wolf WHEREABOUTS") {
            ParseOutcome::Ok(Command::Ask { target, topic }) => {
                assert_eq!(target.as_deref(), Some("wolf"));
                assert_eq!(topic.as_deref(), Some("whereabouts"));
            }
            _ => panic!(),
        }
        assert_eq!(parse("GAZE"), ParseOutcome::Ok(Command::Gaze));
    }

    #[test]
    fn tolerates_multiple_whitespace() {
        assert_eq!(
            parse("  ask    wolf   whereabouts  "),
            ParseOutcome::Ok(Command::Ask {
                target: Some("wolf".into()),
                topic: Some("whereabouts".into()),
            })
        );
    }

    #[test]
    fn unknown_command() {
        assert_eq!(parse("fly"), ParseOutcome::Unknown("fly".into()));
        assert_eq!(
            parse("xyzzy moon"),
            ParseOutcome::Unknown("xyzzy moon".into())
        );
    }

    #[test]
    fn no_arg_commands_reject_extra_args() {
        assert_eq!(
            parse("gaze moon"),
            ParseOutcome::TooManyArguments("gaze".into())
        );
        assert_eq!(
            parse("map now"),
            ParseOutcome::TooManyArguments("map".into())
        );
        assert_eq!(
            parse("quit please"),
            ParseOutcome::TooManyArguments("quit".into())
        );
    }

    #[test]
    fn extra_args_ignored_for_param_commands() {
        // ask/judge/move/help 多余参数被忽略
        match parse("ask wolf whereabouts now") {
            ParseOutcome::Ok(Command::Ask { target, topic }) => {
                assert_eq!(target.as_deref(), Some("wolf"));
                assert_eq!(topic.as_deref(), Some("whereabouts"));
            }
            _ => panic!(),
        }
        match parse("help ask extra") {
            ParseOutcome::Ok(Command::Help { cmd }) => assert_eq!(cmd.as_deref(), Some("ask")),
            _ => panic!(),
        }
    }

    #[test]
    fn command_names_complete() {
        assert_eq!(COMMAND_NAMES.len(), 8);
        assert!(COMMAND_NAMES.contains(&"ask"));
        assert!(COMMAND_NAMES.contains(&"quit"));
    }
}
