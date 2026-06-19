//! 引擎层的菜单候选查询与存档 reconcile 助手。
//!
//! - `ask_topic_options` / `unjudged_character_options` / `move_options`：为各菜单提供
//!   当前上下文下的 (id, label) 候选（只读）。设计见 docs/commands.md「输入辅助」。
//! - `reconcile_save`：继续游戏时校正存档中失效的内容引用（会修改存档并返回 warning）。

use crate::content::ContentEngine;
use crate::content::condition::topic_visible;
use crate::engine::condition::build_factset;
use crate::save::Save;
use crate::world::World;

/// `(id, label)` 对。
pub type Option = (String, String);

/// 存档内容引用失效校验与回退（设计见 docs/save-system.md「兼容性策略」）。
///
/// 当内容更新后某些 id 被删除/改名，旧存档仍引用它们。本函数在不丢失可用记忆的前提下
/// 做最小修复，返回需向玩家展示的 warning 文案：
/// - `current_chapter` 失效 → 回退到 `chapter_path` 中最后一个有效章节。
/// - `chapter_path` 中的失效章节被丢弃。
/// - `current_scene` 不在当前章节场景中 → 重置到 `starting_scene`（surface 视角）。
/// - 引用失效章节的检查点被移除（避免 `map` 展示无法回到的节点）。
///
/// 不会截断 `discovered`（探索记忆永久保留）。
pub fn reconcile_save(save: &mut Save, engine: &ContentEngine) -> Vec<String> {
    let mut warnings = Vec::new();

    // chapter_path：保留存在的章节，保持顺序
    let original_path = save.chapter_path.clone();
    save.chapter_path.retain(|c| engine.chapter_exists(c));
    if save.chapter_path != original_path {
        warnings.push("部分已访问的章节因内容更新而失效，已忽略。".into());
    }

    // current_chapter 回退
    if !engine.chapter_exists(&save.current_chapter) {
        if let Some(last) = save.chapter_path.last().cloned() {
            save.current_chapter = last;
            save.current_world = World::Surface;
        } else if save.chapter_path.is_empty() {
            warnings.push("存档中没有可达章节，内容可能已大幅更新。".into());
        }
        warnings.push("当前章节已失效，已回退到最近的有效章节。".into());
    }

    // current_scene / current_world 重置
    if let Some(ch) = engine.get_chapter(&save.current_chapter) {
        if !ch.scenes.iter().any(|s| s == &save.current_scene) {
            save.current_scene = ch.starting_scene.clone();
            save.current_world = World::Surface;
            warnings.push("当前场景已失效，已重置到章节起始场景。".into());
        }
    }

    // 检查点：丢弃引用失效章节的检查点
    let before = save.checkpoints.len();
    save.checkpoints
        .retain(|c| engine.chapter_exists(&c.chapter));
    if save.checkpoints.len() != before {
        warnings.push("部分检查点因内容更新而失效，已从地图中移除。".into());
    }

    warnings
}

/// 某角色在当前视角下可问的话题：已解锁（`available` 或 `unlock_after` 命中）且在当前
/// `current_world` 下有对话版本。单世界话题在缺失的一侧不出现。
pub fn ask_topic_options(engine: &ContentEngine, save: &Save, character: &str) -> Vec<Option> {
    let ch = &save.current_chapter;
    let facts = build_factset(save);
    engine
        .get_topics(ch, character)
        .iter()
        .filter(|t| topic_visible(t, &facts))
        .filter(|t| {
            engine
                .get_dialogue(ch, character, &t.id, save.current_world)
                .is_some()
        })
        .map(|t| (t.id.clone(), t.label.clone()))
        .collect()
}

/// 本章尚未审判的角色（有审判点且该审判点未触发）。
pub fn unjudged_character_options(engine: &ContentEngine, save: &Save) -> Vec<Option> {
    let ch = &save.current_chapter;
    engine
        .get_judgments(ch)
        .iter()
        .filter(|j| !save.judged(ch, &j.id))
        .filter_map(|j| {
            engine
                .get_character(&j.target)
                .map(|c| (j.target.clone(), c.name.clone()))
        })
        .collect()
}

/// 当前场景可达的目的地场景；边缘场景额外追加「试着离开镇子」伪出口（承载「走不出去」）。
pub fn move_options(engine: &ContentEngine, save: &Save) -> Vec<Option> {
    let mut opts: Vec<Option> = engine
        .get_reachable_scenes(&save.current_scene)
        .into_iter()
        .filter_map(|sid| {
            engine
                .get_scene(sid)
                .map(|s| (sid.to_string(), s.name.clone()))
        })
        .collect();
    if engine.scene_has_exit_attempt(&save.current_scene) {
        opts.push(("__leave".into(), "试着离开镇子".into()));
    }
    opts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::InMemorySource;

    // 复用 content::engine 测试中的小型数据集构造（这里内联一份精简版）
    fn build() -> (ContentEngine, Save) {
        let src = InMemorySource::new()
            .insert("scenes/tavern.yaml", "id: tavern\nname: 酒馆\nconnections: [market]\ndescription:\n  surface: ts.surface.md\n  shadow: ts.shadow.md\n")
            .insert("scenes/market.yaml", "id: market\nname: 集市\ndescription:\n  surface: ms.surface.md\n  shadow: ms.shadow.md\n")
            .insert("ts.surface.md", "酒馆表面。").insert("ts.shadow.md", "酒馆影子。")
            .insert("ms.surface.md", "集市表面。").insert("ms.shadow.md", "集市影子。")
            .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
            .insert("chapters/c1/chapter.yaml", "id: c1\ntitle: 首\nscenes: [tavern, market]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    appears_in: [tavern]\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after:\n          all_of: [wolf_alibi]\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n")
            .insert("chapters/c1/judgments.yaml", "- id: judge_wolf\n  target: wolf\n  result: r.md\n")
            .insert("chapters/c1/clues.yaml", "- id: wolf_alibi\n  source: wolf.whereabouts\n  world: surface\n")
            .insert("chapters/c1/dialogues/wolf.md", "## whereabouts\n\n### [surface]\n\n在场。\n\n### [shadow]\n\n不在。\n\n## secret\n\n### [surface]\n\n秘密。\n")
            .insert("chapters/c1/r.md", "审判。")
            .insert("chapters/c2/chapter.yaml", "id: c2\ntitle: 终\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_final]\n")
            .insert("chapters/c2/judgments.yaml", "- id: judge_wolf_final\n  target: wolf\n  result: r.md\n")
            .insert("chapters/c2/r.md", "终审。");
        let engine = ContentEngine::load(&src).unwrap();
        let save = Save::new_game("c1", "tavern", "t".into());
        (engine, save)
    }

    #[test]
    fn ask_topics_filters_unlocked_and_world() {
        let (e, mut s) = build();
        // 默认 surface，无 wolf_alibi → 只有 whereabouts
        let opts = ask_topic_options(&e, &s, "wolf");
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].0, "whereabouts");
        // 收集线索后 secret 解锁
        s.clues_mut("c1").push("wolf_alibi".into());
        let opts2 = ask_topic_options(&e, &s, "wolf");
        assert_eq!(opts2.len(), 2);
    }

    #[test]
    fn unjudged_lists_wolf() {
        let (e, s) = build();
        let opts = unjudged_character_options(&e, &s);
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].0, "wolf");
    }

    #[test]
    fn move_options_from_tavern() {
        let (e, s) = build();
        let opts = move_options(&e, &s);
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].0, "market");
    }

    #[test]
    fn reconcile_falls_back_current_chapter() {
        let (e, _) = build();
        // 模拟旧存档：current_chapter 指向已删除的章节，chapter_path 含 c1 + 已删章节
        let mut save = Save::new_game("c1", "tavern", "t".into());
        save.chapter_path = vec!["c1".into(), "deleted_chapter".into()];
        save.current_chapter = "deleted_chapter".into();
        let warns = reconcile_save(&mut save, &e);
        assert_eq!(save.current_chapter, "c1");
        assert_eq!(save.chapter_path, vec!["c1".to_string()]);
        assert!(warns.iter().any(|w| w.contains("当前章节已失效")));
    }

    #[test]
    fn reconcile_resets_invalid_scene() {
        let (e, _) = build();
        let mut save = Save::new_game("c1", "tavern", "t".into());
        save.current_scene = "gone_scene".into();
        let warns = reconcile_save(&mut save, &e);
        assert_eq!(save.current_scene, "tavern");
        assert_eq!(save.current_world, World::Surface);
        assert!(warns.iter().any(|w| w.contains("当前场景已失效")));
    }

    #[test]
    fn reconcile_drops_invalid_checkpoints() {
        let (e, _) = build();
        use crate::save::schema::{Checkpoint, CheckpointKind, CkptState};
        let mut save = Save::new_game("c1", "tavern", "t".into());
        save.checkpoints.push(Checkpoint {
            id: "bad".into(),
            chapter: "deleted".into(),
            scene: "x".into(),
            world: World::Surface,
            kind: CheckpointKind::ChapterStart,
            timestamp: "t".into(),
            state: CkptState::default(),
        });
        save.checkpoints.push(Checkpoint {
            id: "good".into(),
            chapter: "c1".into(),
            scene: "tavern".into(),
            world: World::Surface,
            kind: CheckpointKind::ChapterStart,
            timestamp: "t".into(),
            state: CkptState::default(),
        });
        let warns = reconcile_save(&mut save, &e);
        assert_eq!(save.checkpoints.len(), 1);
        assert_eq!(save.checkpoints[0].id, "good");
        assert!(warns.iter().any(|w| w.contains("检查点")));
    }

    #[test]
    fn reconcile_keeps_discovered_intact() {
        let (e, _) = build();
        let mut save = Save::new_game("c1", "tavern", "t".into());
        save.discovered.add_ending("c_truth");
        save.discovered.add_topic("c1", "wolf", "whereabouts");
        reconcile_save(&mut save, &e);
        assert!(save.discovered.endings.contains(&"c_truth".to_string()));
        assert!(save.discovered.topics.contains_key("c1"));
    }
}
