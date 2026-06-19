//! 条件表达式的纯求值（不依赖存档）。
//!
//! 这些函数只依赖 [`crate::content::models`] 的类型与一个事实集合
//! （`HashSet<String>`），因此放在内容层，供 [`crate::content::engine`] 与
//! [`crate::engine`] 共用，避免内容层反向依赖引擎层。
//!
//! 存档相关的事实集合构造（`build_factset`）位于 [`crate::engine::condition`]。

use std::collections::HashSet;

use crate::content::models::{Condition, Topic};

/// 评估条件表达式。
///
/// 约定（数学约定）：空 `all_of` 恒真（空真）、空 `any_of` 恒假。未知 id（内容已删除）
/// 不在集合中 → 视为 false，安全降级。
pub fn eval(cond: &Condition, facts: &HashSet<String>) -> bool {
    match cond {
        Condition::Fact(id) => facts.contains(id),
        Condition::AllOf(ids) => ids.iter().all(|id| facts.contains(id)),
        Condition::AnyOf(ids) => ids.iter().any(|id| facts.contains(id)),
        Condition::Not(id) => !facts.contains(id),
    }
}

/// 话题在当前事实集合下是否可见（菜单展示 / `ask` 校验用）。
///
/// - `available: true` → 恒可见。
/// - `available: false` + `unlock_after` → 条件求值为真时可见。
/// - `available: false` 且无 `unlock_after` → 永久不可问（返回 false）。
pub fn topic_visible(topic: &Topic, facts: &HashSet<String>) -> bool {
    if topic.available {
        true
    } else {
        topic
            .unlock_after
            .as_ref()
            .map_or(false, |c| eval(c, facts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn eval_variants() {
        let f = facts(&["a", "b"]);
        assert!(eval(&Condition::Fact("a".into()), &f));
        assert!(!eval(&Condition::Fact("z".into()), &f));
        assert!(eval(&Condition::AllOf(vec!["a".into(), "b".into()]), &f));
        assert!(!eval(&Condition::AllOf(vec!["a".into(), "z".into()]), &f));
        assert!(eval(&Condition::AnyOf(vec!["a".into(), "z".into()]), &f));
        assert!(eval(&Condition::Not("z".into()), &f));
        assert!(!eval(&Condition::Not("a".into()), &f));
    }

    #[test]
    fn eval_empty_set_conventions() {
        let f = facts(&[]);
        assert!(eval(&Condition::AllOf(vec![]), &f));
        assert!(!eval(&Condition::AnyOf(vec![]), &f));
    }

    #[test]
    fn topic_visible_rules() {
        let avail = Topic {
            id: "t".into(),
            label: "L".into(),
            available: true,
            unlock_after: None,
        };
        assert!(topic_visible(&avail, &facts(&[])));

        let locked = Topic {
            id: "t".into(),
            label: "L".into(),
            available: false,
            unlock_after: Some(Condition::AllOf(vec!["a".into(), "b".into()])),
        };
        assert!(!topic_visible(&locked, &facts(&["a"])));
        assert!(topic_visible(&locked, &facts(&["a", "b"])));

        let permanent = Topic {
            id: "t".into(),
            label: "L".into(),
            available: false,
            unlock_after: None,
        };
        assert!(!topic_visible(&permanent, &facts(&["anything"])));
    }
}
