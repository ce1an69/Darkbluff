//! 内容数据模型 —— 与 `data/` 下 YAML 文件直接对应的反序列化结构。
//!
//! 设计见 docs/data-formats.md。这些是「原始模型」：仅承载 YAML 字段，不做引用解析
//! 或索引；后者由 [`crate::content::engine::ContentEngine`] 在加载时完成。
//!
//! 关键点：
//! - [`Condition`] 采用扁平结构（裸 id / `all_of` / `any_of` / `not`），嵌套由类型本身拒绝。
//! - `id` 在此只是字符串，全局唯一性由启动校验保证。

use serde::Deserialize;

use crate::world::World;

/// 扁平条件表达式。v1 不支持任意嵌套：`all_of`/`any_of` 列表项只能是裸 id 字符串，
/// `not` 后只能跟单个 id。嵌套结构会在反序列化阶段被拒绝。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// 裸 id：该事实（线索/审判点 id）存在即成立。
    Fact(String),
    /// 列表中所有 id 都存在。
    AllOf(Vec<String>),
    /// 列表中任一 id 存在。
    AnyOf(Vec<String>),
    /// 该 id 不存在。
    Not(String),
}

/// 反序列化中间态：裸字符串或映射（`all_of`/`any_of`/`not` 三选一）。
#[derive(Deserialize)]
#[serde(untagged)]
enum RawCondition {
    Bare(String),
    Struct {
        #[serde(default)]
        all_of: Option<Vec<String>>,
        #[serde(default)]
        any_of: Option<Vec<String>>,
        #[serde(default)]
        not: Option<String>,
    },
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        match RawCondition::deserialize(deserializer)? {
            RawCondition::Bare(s) => Ok(Condition::Fact(s)),
            RawCondition::Struct {
                all_of,
                any_of,
                not,
            } => {
                let present = [all_of.is_some(), any_of.is_some(), not.is_some()]
                    .iter()
                    .filter(|&&b| b)
                    .count();
                if present != 1 {
                    return Err(D::Error::custom(
                        "条件表达式须为裸 id、all_of、any_of 或 not 之一，不得混用或嵌套",
                    ));
                }
                match (all_of, any_of, not) {
                    (Some(v), None, None) => Ok(Condition::AllOf(v)),
                    (None, Some(v), None) => Ok(Condition::AnyOf(v)),
                    (None, None, Some(s)) => Ok(Condition::Not(s)),
                    // present == 1 保证三选一，其余分支不可达
                    _ => return Err(D::Error::custom("条件表达式须为裸 id、all_of、any_of 或 not 之一")),
                }
            }
        }
    }
}

/// 对话主题。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Topic {
    /// 话题 id（snake_case），仅需在「章节 × 角色」内唯一。
    pub id: String,
    /// 中文展示名。
    pub label: String,
    /// `true` = 默认可问；`false` 配合 `unlock_after` 解锁，或无 `unlock_after` 表示永久不可问。
    pub available: bool,
    /// 解锁条件（仅 `available: false` 时有意义）。
    pub unlock_after: Option<Condition>,
}

/// 章节内的角色条目。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChapterCharacter {
    /// 引用的全局角色 id。
    pub id: String,
    /// 该角色本章出现的场景 id 列表；省略（None）= 本章所有场景。
    pub appears_in: Option<Vec<String>>,
    /// 该角色本章可问的话题。
    pub topics: Vec<Topic>,
}

/// 全局场景的描述文本引用（surface / shadow 两侧各一）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SceneDescription {
    /// 表面世界描述（相对 `data/` 的 Markdown 路径）。
    pub surface: String,
    /// 影子世界描述（相对 `data/` 的 Markdown 路径）。
    pub shadow: String,
}

/// 全局场景定义（`data/scenes/*.yaml`）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    /// 双向连接（引擎加载时自动补反向）。
    #[serde(default)]
    pub connections: Vec<String>,
    /// 单向连接（不补反向）。
    #[serde(default)]
    pub one_way_connections: Vec<String>,
    pub description: SceneDescription,
}

/// 全局角色定义（`data/characters/*.yaml`）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Character {
    pub id: String,
    pub name: String,
}

/// 章节跳转分支。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Branch {
    /// 命中条件（扁平条件表达式）。
    pub when: Condition,
    /// 命中后跳转目标章节 id。
    pub target: String,
}

/// 章节跳转配置。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NextConfig {
    /// 无分支命中时的默认目标。
    pub default: String,
    /// 按序匹配、首条命中生效的分支列表。
    #[serde(default)]
    pub branches: Vec<Branch>,
}

/// 章节元数据（`data/chapters/{id}/chapter.yaml`）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    /// 开发辅助排序字段，不影响运行时行为。
    #[serde(default)]
    pub order: i64,
    /// 终章标记；`true` 时无 `next`，完成最后一个必要审判时记达成结局。
    #[serde(default)]
    pub ending: bool,
    /// 可选章节开场/过场文本（相对章节目录的 Markdown 路径）。
    pub intro: Option<String>,
    /// 可选终章结局收尾文本（仅 `ending: true` 有效）。
    pub outro: Option<String>,
    pub scenes: Vec<String>,
    pub starting_scene: String,
    #[serde(default)]
    pub characters: Vec<ChapterCharacter>,
    /// 自动推进前必须完成的审判点 id；省略（None）= 本章全部审判。
    /// 显式空数组 `[]` 不合法，由启动校验拦截。
    pub required_judgments: Option<Vec<String>>,
    /// 非终章必须提供；终章必须为 None。由启动校验保证。
    pub next: Option<NextConfig>,
}

/// 审判点（`data/chapters/{id}/judgments.yaml` 中的单项）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Judgment {
    /// 审判点 id（全局唯一，同时作为条件标记）。
    pub id: String,
    /// 被审判角色 id（章级操作，不要求在场）。
    pub target: String,
    /// 审判剧情文本（相对章节目录的 Markdown 路径）。
    pub result: String,
}

/// 线索（`data/chapters/{id}/clues.yaml` 中的单项）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Clue {
    /// 线索 id（全局唯一），用于话题解锁条件。
    pub id: String,
    /// 来源对话，格式 `{character}.{topic}`。
    pub source: String,
    /// 触发该线索的世界版本。
    pub world: World,
}

/// 将 `{character}.{topic}` 形式的来源字符串解析为 `(character_id, topic_id)`。
/// 缺少点号或存在多余点号时返回 `None`。
pub fn parse_dialogue_source(source: &str) -> Option<(&str, &str)> {
    let mut it = source.split('.');
    let ch = it.next()?;
    let topic = it.next()?;
    if it.next().is_some() || ch.is_empty() || topic.is_empty() {
        return None;
    }
    Some((ch, topic))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(yaml: &str) -> Condition {
        serde_yml::from_str(yaml).expect("condition parses")
    }

    #[test]
    fn condition_bare_id() {
        assert_eq!(cond("wolf_alibi"), Condition::Fact("wolf_alibi".into()));
    }

    #[test]
    fn condition_all_of() {
        let c = cond("all_of:\n  - wolf_alibi\n  - crow_testimony\n");
        assert_eq!(
            c,
            Condition::AllOf(vec!["wolf_alibi".into(), "crow_testimony".into()])
        );
    }

    #[test]
    fn condition_any_of() {
        let c = cond("any_of:\n  - judge_wolf\n  - judge_crow\n");
        assert_eq!(
            c,
            Condition::AnyOf(vec!["judge_wolf".into(), "judge_crow".into()])
        );
    }

    #[test]
    fn condition_not() {
        assert_eq!(cond("not: judge_wolf"), Condition::Not("judge_wolf".into()));
    }

    #[test]
    fn condition_rejects_mixing() {
        assert!(serde_yml::from_str::<Condition>(
            "all_of:\n  - a\nany_of:\n  - b\n"
        )
        .is_err());
    }

    #[test]
    fn condition_rejects_empty_map() {
        assert!(serde_yml::from_str::<Condition>("{}").is_err());
    }

    #[test]
    fn condition_rejects_nested() {
        // all_of 的列表项只能是字符串；嵌套映射会被类型拒绝
        assert!(serde_yml::from_str::<Condition>(
            "all_of:\n  - not: a\n"
        )
        .is_err());
    }

    #[test]
    fn parse_dialogue_source_ok() {
        assert_eq!(
            parse_dialogue_source("wolf.whereabouts"),
            Some(("wolf", "whereabouts"))
        );
    }

    #[test]
    fn parse_dialogue_source_bad() {
        assert_eq!(parse_dialogue_source("nope"), None);
        assert_eq!(parse_dialogue_source(".topic"), None);
        assert_eq!(parse_dialogue_source("wolf."), None);
        assert_eq!(parse_dialogue_source("a.b.c"), None);
    }

    #[test]
    fn chapter_parses_full() {
        let yaml = r#"
id: the_missing_butcher
title: "失踪的屠夫"
order: 1
scenes: [tavern, market]
starting_scene: tavern
characters:
  - id: wolf
    appears_in: [tavern]
    topics:
      - id: whereabouts
        label: "昨晚的行踪"
        available: true
      - id: secret
        label: "隐藏的秘密"
        available: false
        unlock_after:
          all_of:
            - wolf_alibi
required_judgments: [judge_wolf]
next:
  default: tavern_uncertain
  branches:
    - when:
        all_of: [judge_wolf]
      target: tavern_truth
"#;
        let ch: Chapter = serde_yml::from_str(yaml).unwrap();
        assert_eq!(ch.id, "the_missing_butcher");
        assert_eq!(ch.ending, false);
        assert_eq!(ch.order, 1);
        assert_eq!(ch.starting_scene, "tavern");
        assert_eq!(ch.required_judgments.as_deref(), Some(&["judge_wolf".to_string()][..]));
        let wolf = &ch.characters[0];
        assert_eq!(wolf.appears_in.as_deref(), Some(&["tavern".to_string()][..]));
        assert_eq!(wolf.topics.len(), 2);
        assert!(wolf.topics[0].available);
        assert!(!wolf.topics[1].available);
        let next = ch.next.unwrap();
        assert_eq!(next.default, "tavern_uncertain");
        assert_eq!(next.branches.len(), 1);
        assert_eq!(next.branches[0].target, "tavern_truth");
    }

    #[test]
    fn chapter_required_judgments_omitted_is_none() {
        let yaml = "id: c\ntitle: t\nscenes: [s]\nstarting_scene: s\nnext:\n  default: d\n";
        let ch: Chapter = serde_yml::from_str(yaml).unwrap();
        assert!(ch.required_judgments.is_none());
    }

    #[test]
    fn scene_parses_with_defaults() {
        let yaml = r#"
id: tavern
name: "锈桶酒馆"
description:
  surface: text/scenes/tavern.surface.md
  shadow: text/scenes/tavern.shadow.md
"#;
        let s: Scene = serde_yml::from_str(yaml).unwrap();
        assert!(s.connections.is_empty());
        assert!(s.one_way_connections.is_empty());
        assert_eq!(s.description.surface, "text/scenes/tavern.surface.md");
    }

    #[test]
    fn ending_chapter_has_no_next() {
        let yaml = r#"
id: tavern_truth
title: "酒馆真相"
order: 3
ending: true
scenes: [tavern]
starting_scene: tavern
outro: outro.md
required_judgments: [judge_wolf]
"#;
        let ch: Chapter = serde_yml::from_str(yaml).unwrap();
        assert!(ch.ending);
        assert!(ch.next.is_none());
        assert_eq!(ch.outro.as_deref(), Some("outro.md"));
    }
}
