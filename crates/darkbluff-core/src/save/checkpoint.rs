//! 检查点创建与回滚。
//!
//! 设计见 docs/save-system.md「检查点与回滚」「回滚实现」。核心不变量：当前游戏状态由
//! `current_*` 字段 + 三个权威数组直接表示；`checkpoints` 列表只是「可回滚的目标历史」，
//! 不参与表达当前状态。回滚按检查点记录的数组长度截断，`discovered` 永不截断。

use crate::error::{AppError, Result};
use crate::save::schema::{
    before_judgment_id_base, chapter_start_id_base, unique_checkpoint_id, Checkpoint,
    CheckpointKind, Save,
};
use crate::world::World;

/// 创建并追加 `chapter_start` 检查点（进入章节时），返回其 id。
pub fn create_chapter_start(
    save: &mut Save,
    chapter: &str,
    scene: &str,
    world: World,
    now: &str,
) -> String {
    let id = unique_checkpoint_id(&chapter_start_id_base(chapter), &save.checkpoints);
    push_checkpoint(
        save,
        id.clone(),
        chapter,
        scene,
        world,
        CheckpointKind::ChapterStart,
        now,
    );
    id
}

/// 创建并追加 `before_judgment` 检查点（执行 `judge` 审判时），返回其 id。
/// 记录的是审判前的当前位置与本章三数组长度。
pub fn create_before_judgment(
    save: &mut Save,
    chapter: &str,
    judgment_id: &str,
    now: &str,
) -> String {
    let id = unique_checkpoint_id(&before_judgment_id_base(judgment_id), &save.checkpoints);
    let scene = save.current_scene.clone();
    let world = save.current_world;
    push_checkpoint(
        save,
        id.clone(),
        chapter,
        &scene,
        world,
        CheckpointKind::BeforeJudgment,
        now,
    );
    id
}

fn push_checkpoint(
    save: &mut Save,
    id: String,
    chapter: &str,
    scene: &str,
    world: World,
    kind: CheckpointKind,
    now: &str,
) {
    let state = Checkpoint::state_of(save, chapter);
    save.checkpoints.push(Checkpoint {
        id,
        chapter: chapter.to_string(),
        scene: scene.to_string(),
        world,
        kind,
        timestamp: now.to_string(),
        state,
    });
}

/// 基本回滚：把当前状态恢复到某检查点记录的快照（位置 + 本章三数组截断）。
pub fn rollback_to(save: &mut Save, ckpt: &Checkpoint) {
    save.current_chapter = ckpt.chapter.clone();
    save.current_scene = ckpt.scene.clone();
    save.current_world = ckpt.world;
    let ch = &ckpt.chapter;
    if let Some(v) = save.collected_clues.get_mut(ch) {
        v.truncate(ckpt.state.clues_len);
    }
    if let Some(v) = save.viewed_dialogues.get_mut(ch) {
        v.truncate(ckpt.state.views_len);
    }
    if let Some(v) = save.judgments_made.get_mut(ch) {
        v.truncate(ckpt.state.judgments_len);
    }
}

/// `map` checkpoint 回滚：回到指定检查点，丢弃其后的当前流程进度，`discovered` 保留。
///
/// 失败（checkpoint 不存在）返回错误，由调用方转为「这个节点已经无法回到」提示。
pub fn map_checkpoint_rollback(save: &mut Save, checkpoint_id: &str) -> Result<()> {
    let idx = save
        .checkpoints
        .iter()
        .position(|c| c.id == checkpoint_id)
        .ok_or_else(|| AppError::Save("这个节点已经无法回到".into()))?;
    let ckpt = save.checkpoints[idx].clone();

    rollback_to(save, &ckpt);

    // 跨章：截断 chapter_path 并清理其后章节的权威事实与快照索引
    if let Some(pidx) = save.chapter_path.iter().position(|c| c == &ckpt.chapter) {
        let dropped: Vec<String> = save.chapter_path[pidx + 1..].to_vec();
        save.chapter_path.truncate(pidx + 1);
        for ch in &dropped {
            save.collected_clues.remove(ch);
            save.viewed_dialogues.remove(ch);
            save.judgments_made.remove(ch);
            save.viewed_intros.remove(ch);
            save.viewed_outros.remove(ch);
            save.viewed_narrative.remove(ch);
        }
    }

    // 作废检查点移除：chapter_start 保留自身，before_judgment 连同自身移除（审判已撤销）
    let cutoff = match ckpt.kind {
        CheckpointKind::ChapterStart => idx + 1,
        CheckpointKind::BeforeJudgment => idx,
    };
    save.checkpoints.truncate(cutoff);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::schema::{JudgmentMade, ViewedDialogue};

    fn mk_save() -> Save {
        // c1 → c2 流程：c1 有线索/对话/审判，已推进到 c2
        let mut save = Save::new_game("c1", "tavern", "t0".into());
        create_chapter_start(&mut save, "c1", "tavern", World::Surface, "t1");
        save.clues_mut("c1").extend(["a", "b"].iter().map(|s| s.to_string()));
        save.views_mut("c1").push(ViewedDialogue {
            character: "wolf".into(),
            topic: "whereabouts".into(),
            world: World::Surface,
            snapshot: "snapshots/c1/wolf.whereabouts.surface.md".into(),
        });
        let bj = create_before_judgment(&mut save, "c1", "judge_wolf", "t2");
        save.judgments_mut("c1").push(JudgmentMade {
            judgment: "judge_wolf".into(),
            result_snapshot: "snapshots/c1/judge_wolf.md".into(),
        });
        // 推进到 c2
        save.chapter_path.push("c2".into());
        save.current_chapter = "c2".into();
        save.current_scene = "market".into();
        save.current_world = World::Shadow;
        save.discovered.add_chapter("c2");
        create_chapter_start(&mut save, "c2", "market", World::Shadow, "t3");
        let _ = bj;
        save
    }

    #[test]
    fn rollback_to_before_judgment_current_chapter_keeps_chapter_start() {
        let mut save = mk_save();
        // 回到审判前检查点（c1 的 before_judgment）
        let bj_id = save
            .checkpoints
            .iter()
            .find(|c| c.kind == CheckpointKind::BeforeJudgment && c.chapter == "c1")
            .map(|c| c.id.clone())
            .unwrap();
        map_checkpoint_rollback(&mut save, &bj_id).unwrap();

        // 当前章回到 c1，审判被撤销（judgments 截断为 0），线索/对话保留
        assert_eq!(save.current_chapter, "c1");
        assert_eq!(save.judgments_made.get("c1").map(|v| v.len()).unwrap_or(0), 0);
        assert_eq!(save.collected_clues.get("c1").unwrap().len(), 2);
        assert_eq!(save.viewed_dialogues.get("c1").unwrap().len(), 1);
        // chapter_path：跨章回滚会截断 c2
        assert_eq!(save.chapter_path, vec!["c1"]);
        // before_judgment 检查点自身已被移除
        assert!(save.checkpoints.iter().all(|c| c.kind != CheckpointKind::BeforeJudgment));
    }

    #[test]
    fn rollback_to_chapter_start_resets_arrays() {
        let mut save = mk_save();
        let start_id = save
            .checkpoints
            .iter()
            .find(|c| c.kind == CheckpointKind::ChapterStart && c.chapter == "c1")
            .map(|c| c.id.clone())
            .unwrap();
        map_checkpoint_rollback(&mut save, &start_id).unwrap();
        // chapter_start 检查点保留，其创建时三数组长度为 0 → 全部截断
        assert_eq!(save.collected_clues.get("c1").map(|v| v.len()).unwrap_or(0), 0);
        assert_eq!(save.judgments_made.get("c1").map(|v| v.len()).unwrap_or(0), 0);
        // chapter_start 自身仍在
        assert!(save
            .checkpoints
            .iter()
            .any(|c| c.kind == CheckpointKind::ChapterStart && c.chapter == "c1"));
    }

    #[test]
    fn discovered_survives_rollback() {
        let mut save = mk_save();
        assert!(save.discovered.chapters.contains(&"c2".to_string()));
        let start_id = save
            .checkpoints
            .iter()
            .find(|c| c.kind == CheckpointKind::ChapterStart && c.chapter == "c1")
            .map(|c| c.id.clone())
            .unwrap();
        map_checkpoint_rollback(&mut save, &start_id).unwrap();
        // discovered 保留 c2 记忆
        assert!(save.discovered.chapters.contains(&"c2".to_string()));
    }

    #[test]
    fn rollback_unknown_checkpoint_errors() {
        let mut save = mk_save();
        assert!(map_checkpoint_rollback(&mut save, "nope").is_err());
    }

    #[test]
    fn checkpoint_ids_unique_on_reentry() {
        // 模拟重新进入 c1：第二个 chapter_start 应得到唯一 id
        let mut save = Save::new_game("c1", "s", "t".into());
        let id1 = create_chapter_start(&mut save, "c1", "s", World::Surface, "t1");
        let id2 = create_chapter_start(&mut save, "c1", "s", World::Surface, "t2");
        assert_ne!(id1, id2);
        assert_eq!(id1, "ckpt_c1_start");
        assert_eq!(id2, "ckpt_c1_start_2");
    }
}
