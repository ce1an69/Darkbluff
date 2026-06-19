//! 引擎层与存档相关的条件/事实集合助手。
//!
//! 纯条件求值（`eval` / `topic_visible`）位于 [`crate::content::condition`]，因仅依赖内容
//! 类型；本模块保留需要读取存档（[`Save`]）的运行时助手：`FactSet` 别名、
//! `build_factset`（合并 chapter_path 的事实）、`chapter_complete` / `required_judgments_complete`
//! （必要审判是否完成）。

use std::collections::HashSet;

use crate::content::models::Chapter;
use crate::save::Save;

/// 事实集合 = 已收集线索 id ∪ 已审判审判点 id。
///
/// 由存档的 `collected_clues` 与 `judgments_made` 合并得到（详见 [`build_factset`]）。
pub type FactSet = HashSet<String>;

/// 由存档合并 `chapter_path` 中所有章节（首章到当前章，含当前章）的事实：
/// 各章已收集线索 id ∪ 已审判审判点 id。
pub fn build_factset(save: &Save) -> FactSet {
    let mut facts = FactSet::new();
    for ch in &save.chapter_path {
        if let Some(clues) = save.collected_clues.get(ch) {
            facts.extend(clues.iter().cloned());
        }
        if let Some(judgs) = save.judgments_made.get(ch) {
            for j in judgs {
                facts.insert(j.judgment.clone());
            }
        }
    }
    // 叙事触发器 id 作为条件标记：取自 append-only 的 `discovered.triggers`（任何回滚都不
    // 截断），而非 `viewed_narrative`（随章清理）。这样「碎片作为事实」的语义与「不重复
    // 触发」一致——回滚不会让 when 链式依赖（T2 when:[T1]）因 T1 快照被清而失效，
    // 也不会在审判回滚后残留与 discovered 矛盾的事实。
    for t in &save.discovered.triggers {
        facts.insert(t.clone());
    }
    facts
}

/// 本章是否所有必要审判都已完成（用于 `judge` 后判断是否自动推进）。
///
/// `required: None` 表示要求本章全部审判点完成；`Some(set)` 表示要求集合内全部完成。
pub fn required_judgments_complete(
    required: Option<&[String]>,
    judgments: &FactSet,
    all_judgment_ids: &[String],
) -> bool {
    match required {
        Some(req) => req.iter().all(|id| judgments.contains(id)),
        None => all_judgment_ids.iter().all(|id| judgments.contains(id)),
    }
}

/// 便捷：给定章节与当前已审判 id 集合，判断本章必要审判是否完成。
pub fn chapter_complete(
    chapter: &Chapter,
    judgments: &FactSet,
    all_judgment_ids: &[String],
) -> bool {
    required_judgments_complete(
        chapter.required_judgments.as_deref(),
        judgments,
        all_judgment_ids,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(items: &[&str]) -> FactSet {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn required_judgments_complete_explicit() {
        let req = vec!["j1".to_string(), "j2".to_string()];
        assert!(!required_judgments_complete(Some(&req), &facts(&["j1"]), &[]));
        assert!(required_judgments_complete(
            Some(&req),
            &facts(&["j1", "j2", "extra"]),
            &[]
        ));
    }

    #[test]
    fn required_judgments_complete_implicit_all() {
        let all = vec!["j1".to_string(), "j2".to_string()];
        assert!(!required_judgments_complete(None, &facts(&["j1"]), &all));
        assert!(required_judgments_complete(None, &facts(&["j1", "j2"]), &all));
    }

    #[test]
    fn build_factset_merges_chapter_path() {
        use crate::save::schema::{JudgmentMade, Save};
        let mut save = Save::new_game("c1", "s", "t".into());
        save.chapter_path.push("c2".into());
        save.clues_mut("c1").push("clue_a".into());
        save.clues_mut("c2").push("clue_b".into());
        save.judgments_mut("c1").push(JudgmentMade {
            judgment: "judge_wolf".into(),
            result_snapshot: "x".into(),
        });
        let fs = build_factset(&save);
        assert!(fs.contains("clue_a"));
        assert!(fs.contains("clue_b"));
        assert!(fs.contains("judge_wolf"));
    }
}
