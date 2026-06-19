//! 斜杠补全：候选计算、浮层导航、补全替换。

use darkbluff_core::engine::{
    Session, ask_topic_options, move_options, unjudged_character_options,
};

use crate::command;

use super::App;
use super::types::{SuggestKind, Suggestion, Suggestions};

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

    if tokens.len() <= 1
        && !ends_space
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
    let partial = if ends_space {
        ""
    } else {
        tokens.last().copied().unwrap_or("")
    };
    let arg_pos = if ends_space {
        tokens.len()
    } else {
        tokens.len().saturating_sub(1)
    };

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
        ("ask", 2) if tokens.len() >= 2 => (
            SuggestKind::Topic,
            ask_topic_options(engine, save, tokens[1]),
        ),
        ("judge", 1) => (
            SuggestKind::Character,
            unjudged_character_options(engine, save),
        ),
        ("move", 1) => (
            SuggestKind::Scene,
            move_options(engine, save)
                .into_iter()
                .filter(|(id, _)| !id.starts_with("__"))
                .collect::<Vec<_>>(),
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use darkbluff_core::content::{ContentEngine, InMemorySource};
    use darkbluff_core::engine::{Input, Selection, Session};
    use darkbluff_core::save::{FakeClock, SaveStore};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 构造一个进入 c1/tavern 探索态的会话：wolf 在场，话题 whereabouts(可见)/secret(锁定)，
    /// 审判点 judge_wolf，tavern 连通 market。
    fn exploring_session() -> Session {
        let src = InMemorySource::new()
            .insert("scenes/tavern.yaml", "id: tavern\nname: 酒馆\nconnections: [market]\ndescription:\n  surface: t.md\n  shadow: t.md\n")
            .insert("scenes/market.yaml", "id: market\nname: 集市\ndescription:\n  surface: m.md\n  shadow: m.md\n")
            .insert("t.md", "酒馆。").insert("m.md", "集市。")
            .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
            .insert("chapters/c1/chapter.yaml", "id: c1\ntitle: 首\nintro: i.md\nscenes: [tavern, market]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    appears_in: [tavern]\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after:\n          all_of: [wolf_alibi]\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n")
            .insert("chapters/c1/i.md", "开场。")
            .insert("chapters/c1/dialogues/wolf.md", "## whereabouts\n\n### [surface]\n\n在场。\n\n### [shadow]\n\n不在。\n")
            .insert("chapters/c1/judgments.yaml", "- id: judge_wolf\n  target: wolf\n  result: r.md\n")
            .insert("chapters/c1/r.md", "审判。")
            .insert("chapters/c2/chapter.yaml", "id: c2\ntitle: 终\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_end]\n")
            .insert("chapters/c2/judgments.yaml", "- id: judge_wolf_end\n  target: wolf\n  result: r2.md\n")
            .insert("chapters/c2/r2.md", "终审。");
        let engine = ContentEngine::load(&src).unwrap();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("db-suggest-{}-{n}", std::process::id()));
        let store = SaveStore::open(dir, Box::new(FakeClock::new())).unwrap();
        let mut s = Session::new(engine, store);
        s.handle(Input::Text(String::new())); // 进入标题、构建菜单
        s.handle(Input::Select(Selection::Index(0))); // 新游戏 -> ChapterIntro
        s.handle(Input::Ack); // -> Exploring@c1/tavern
        s
    }

    // ----- 纯字符串辅助 -----

    #[test]
    fn strip_slash_removes_all_leading_slashes() {
        assert_eq!(strip_slash("/ask"), "ask");
        assert_eq!(strip_slash("//ask"), "ask"); // 多斜杠
        assert_eq!(strip_slash("  ///ask wolf"), "ask wolf"); // 前导空白 + 多斜杠
        assert_eq!(strip_slash("ask"), "ask"); // 无斜杠原样
    }

    #[test]
    fn apply_completion_replaces_trailing_token() {
        assert_eq!(apply_completion("/as", "/ask "), "/ask ");
        assert_eq!(apply_completion("/ask wo", "wolf "), "/ask wolf ");
        assert_eq!(apply_completion("/ask ", "wolf "), "/ask wolf ");
    }

    #[test]
    fn filter_opts_matches_id_or_label() {
        let cands = vec![
            ("wolf".into(), "灰狼".into()),
            ("crow".into(), "乌鸦".into()),
        ];
        assert_eq!(filter_opts(cands.clone(), "wo").len(), 1); // id 前缀
        assert_eq!(filter_opts(cands.clone(), "灰").len(), 1); // label 包含
        assert!(filter_opts(cands.clone(), "zzz").is_empty()); // 无匹配
    }

    // ----- compute_suggestions（此前 bug 集中区） -----

    #[test]
    fn suggest_filters_commands_by_prefix() {
        let s = exploring_session();
        let sg = compute_suggestions("/a", &s).expect("命令补全");
        assert_eq!(sg.kind, SuggestKind::Command);
        assert!(sg.items.iter().any(|i| i.display == "/ask"));
        assert!(!sg.items.iter().any(|i| i.display == "/move")); // m 不匹配 a
    }

    #[test]
    fn suggest_ask_lists_characters_in_scene() {
        let s = exploring_session();
        let sg = compute_suggestions("/ask ", &s).expect("角色补全");
        assert_eq!(sg.kind, SuggestKind::Character);
        assert!(sg.items.iter().any(|i| i.insert == "wolf "));
    }

    #[test]
    fn suggest_ask_topic_excludes_locked() {
        let s = exploring_session();
        let sg = compute_suggestions("/ask wolf ", &s).expect("话题补全");
        assert_eq!(sg.kind, SuggestKind::Topic);
        // secret 锁定(无 wolf_alibi) -> 不出现；仅 whereabouts
        assert_eq!(sg.items.len(), 1);
        assert_eq!(sg.items[0].insert, "whereabouts ");
    }

    #[test]
    fn suggest_move_lists_reachable_scenes() {
        let s = exploring_session();
        let sg = compute_suggestions("/move ", &s).expect("场景补全");
        assert_eq!(sg.kind, SuggestKind::Scene);
        assert!(sg.items.iter().any(|i| i.insert == "market ")); // tavern 连通 market
    }

    #[test]
    fn suggest_judge_lists_only_unjudged() {
        let s = exploring_session();
        let sg = compute_suggestions("/judge ", &s).expect("审判补全");
        assert_eq!(sg.kind, SuggestKind::Character);
        assert!(sg.items.iter().any(|i| i.insert == "wolf ")); // judge_wolf 尚未审判
    }

    #[test]
    fn suggest_returns_none_for_empty_or_unknown() {
        let s = exploring_session();
        assert!(compute_suggestions("", &s).is_none());
        assert!(compute_suggestions("xyz", &s).is_none()); // 非命令、无斜杠
    }
}
