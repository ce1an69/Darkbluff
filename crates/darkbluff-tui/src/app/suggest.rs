//! 斜杠补全：候选计算、浮层导航、补全替换。

use darkbluff_core::engine::{ask_topic_options, move_options, Session, unjudged_character_options};

use crate::command;

use super::types::{SuggestKind, Suggestion, Suggestions};
use super::App;

impl App {
    /// 输入变更后重算补全候选（仅光标在行尾时）。
    pub(super) fn recompute_suggestions(&mut self) {
        let next = if self.input.cursor_at_end() {
            compute_suggestions(self.input.value(), &self.session)
        } else {
            None
        };
        self.suggestions = next;
    }

    /// 浮层内上下移动选中项（夹取到边界）。
    pub(super) fn move_suggest(&mut self, delta: i32) {
        if let Some(sg) = &mut self.suggestions {
            let last = sg.items.len().saturating_sub(1) as i32;
            let mut s = sg.selected as i32 + delta;
            if s < 0 {
                s = 0;
            }
            if s > last {
                s = last;
            }
            sg.selected = s as usize;
        }
    }

    /// 用选中项的 insert 文本替换行尾半个 token。
    pub(super) fn complete_suggestion(&mut self) {
        let Some(insert) = self
            .suggestions
            .as_ref()
            .and_then(|s| s.items.get(s.selected))
            .map(|i| i.insert.clone())
        else {
            return;
        };
        let new_val = apply_completion(self.input.value(), &insert);
        self.input.set_value(new_val);
        self.recompute_suggestions();
    }
}

/// 依当前输入与会话上下文计算斜杠补全候选。
/// 参数候选取自引擎的菜单构建器，保证「补全列出的」=「引擎接受的」。
fn compute_suggestions(input: &str, session: &Session) -> Option<Suggestions> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let ends_space = input.ends_with(' ');
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    if tokens.len() <= 1 && !ends_space
        && let Some(query) = trimmed.strip_prefix('/')
    {
        return command_suggestions(query);
    }
    arg_suggestions(&tokens, ends_space, session)
}

/// 首个 `/` 命令词补全。
fn command_suggestions(query: &str) -> Option<Suggestions> {
    let items = command::all()
        .into_iter()
        .filter(|c| query.is_empty() || c.name.starts_with(query))
        .map(|c| Suggestion {
            display: format!("/{}", c.name),
            desc: if c.args.is_empty() {
                c.desc.to_string()
            } else {
                format!("{} · {}", c.desc, c.args)
            },
            insert: format!("/{} ", c.name),
        })
        .collect();
    Suggestions::new(SuggestKind::Command, items)
}

/// 已知动词的参数补全（角色 / 话题 / 场景）。
fn arg_suggestions(tokens: &[&str], ends_space: bool, session: &Session) -> Option<Suggestions> {
    let verb = tokens.first()?.trim_start_matches('/');
    if !command::is_known(verb) {
        return None;
    }
    let partial = if ends_space { "" } else { tokens.last().copied().unwrap_or("") };
    let arg_pos = if ends_space { tokens.len() } else { tokens.len().saturating_sub(1) };

    let save = session.save();
    let engine = session.engine();
    let ch = &save.current_chapter;
    let scene = &save.current_scene;
    let (kind, candidates) = match (verb, arg_pos) {
        ("ask", 1) => (
            SuggestKind::Character,
            engine
                .get_characters_in_scene(ch, scene)
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect::<Vec<_>>(),
        ),
        ("ask", 2) if tokens.len() >= 2 => {
            (SuggestKind::Topic, ask_topic_options(engine, save, tokens[1]))
        }
        ("judge", 1) => (SuggestKind::Character, unjudged_character_options(engine, save)),
        ("move", 1) => (SuggestKind::Scene, move_options(engine, save)),
        _ => return None,
    };
    Suggestions::new(kind, filter_opts(candidates, partial))
}

/// 把引擎候选 `(id, label)` 按前缀过滤为补全项。
fn filter_opts(candidates: Vec<(String, String)>, partial: &str) -> Vec<Suggestion> {
    candidates
        .into_iter()
        .filter(|(id, label)| id.starts_with(partial) || label.contains(partial))
        .map(|(id, label)| Suggestion {
            display: format!("{label} · {id}"),
            desc: String::new(),
            insert: format!("{id} "),
        })
        .collect()
}

/// 提交时去掉行首空白与全部前导 `/`（斜杠只是 UI 触发符；容忍 `//ask`）。
pub(super) fn strip_slash(line: &str) -> String {
    line.trim_start().trim_start_matches('/').to_string()
}

/// 用补全文本替换输入行尾的半个 token（最后一个空白之后的部分）。
fn apply_completion(value: &str, insert: &str) -> String {
    let prefix = match value.rfind(char::is_whitespace) {
        Some(i) => &value[..i + 1],
        None => "",
    };
    format!("{prefix}{insert}")
}
