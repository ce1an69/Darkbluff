//! 对话 / 叙事快照读写。
//!
//! 设计见 docs/save-system.md「笔记系统与快照」。快照存「渲染前 Markdown 原文」，保证
//! 推理公平性（不随后续剧情漂移）。快照存于存档根目录下的 `snapshots/` 子树，路径在
//! `save.json` 中以相对路径（基准为存档根）记录。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::save::atomic::atomic_write_bytes;
use crate::save::schema::Save;
use crate::world::World;

/// 快照存储：以存档根目录为基准读写快照文件。
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    /// 写入快照（原子），返回相对路径。父目录自动创建。
    pub fn write(&self, rel: &str, text: &str) -> Result<String> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write_bytes(&path, text.as_bytes())?;
        Ok(rel.to_string())
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        fs::read_to_string(self.root.join(rel)).map_err(Into::into)
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    /// 删除快照（best-effort）。
    pub fn delete(&self, rel: &str) -> Result<()> {
        fs::remove_file(self.root.join(rel)).map_err(Into::into)
    }

    /// 列出 `snapshots/` 下所有快照的相对路径（已排序，`/` 分隔）。
    pub fn list_all(&self) -> Result<Vec<String>> {
        let dir = self.snapshots_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        walk(&self.root, &dir, &mut out)?;
        out.sort();
        Ok(out)
    }

    /// 清理孤儿快照：删除 `snapshots/` 中未被存档四类引用的文件，返回删除数。
    /// 清理失败（如权限问题）仅跳过该文件，不中断。
    pub fn cleanup_orphans(&self, save: &Save) -> Result<usize> {
        let referenced = referenced_snapshot_paths(save);
        let mut removed = 0;
        for rel in self.list_all()? {
            if !referenced.contains(&rel) {
                if fs::remove_file(self.root.join(&rel)).is_ok() {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            walk(root, &p, out)?;
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// 收集存档当前引用的全部快照相对路径（对话 / intro / outro / 审判）。
fn referenced_snapshot_paths(save: &Save) -> HashSet<String> {
    let mut set = HashSet::new();
    for list in save.viewed_dialogues.values() {
        for v in list {
            set.insert(v.snapshot.clone());
        }
    }
    for v in save.viewed_intros.values() {
        set.insert(v.clone());
    }
    for v in save.viewed_outros.values() {
        set.insert(v.clone());
    }
    for list in save.judgments_made.values() {
        for j in list {
            set.insert(j.result_snapshot.clone());
        }
    }
    for list in save.viewed_narrative.values() {
        for n in list {
            set.insert(n.snapshot.clone());
        }
    }
    set
}

// ----- 路径约定 -----

pub fn dialogue_snapshot_path(chapter: &str, character: &str, topic: &str, world: World) -> String {
    format!(
        "snapshots/{chapter}/{character}.{topic}.{}.md",
        world.as_str()
    )
}

pub fn intro_snapshot_path(chapter: &str) -> String {
    format!("snapshots/{chapter}/intro.md")
}

pub fn outro_snapshot_path(chapter: &str) -> String {
    format!("snapshots/{chapter}/outro.md")
}

pub fn judgment_snapshot_path(chapter: &str, judgment_id: &str) -> String {
    format!("snapshots/{chapter}/{judgment_id}.md")
}

pub fn narrative_snapshot_path(chapter: &str, trigger_id: &str) -> String {
    format!("snapshots/{chapter}/narrative.{trigger_id}.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::schema::{JudgmentMade, Save, ViewedDialogue};

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "darkbluff-snap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_read_exists() {
        let root = temp_root();
        let store = SnapshotStore::new(&root);
        let rel = dialogue_snapshot_path("c1", "wolf", "whereabouts", World::Surface);
        let back = store.write(&rel, "原文").unwrap();
        assert_eq!(back, rel);
        assert!(store.exists(&rel));
        assert_eq!(store.read(&rel).unwrap(), "原文");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cleanup_orphans_removes_unreferenced() {
        let root = temp_root();
        let store = SnapshotStore::new(&root);
        // 写两个快照：一个被引用，一个孤儿
        let referenced = dialogue_snapshot_path("c1", "wolf", "whereabouts", World::Surface);
        let orphan = dialogue_snapshot_path("c1", "wolf", "secret", World::Surface);
        store.write(&referenced, "a").unwrap();
        store.write(&orphan, "b").unwrap();

        let mut save = Save::new_game("c1", "s", "t".into());
        save.views_mut("c1").push(ViewedDialogue {
            character: "wolf".into(),
            topic: "whereabouts".into(),
            world: World::Surface,
            snapshot: referenced.clone(),
        });

        let removed = store.cleanup_orphans(&save).unwrap();
        assert_eq!(removed, 1);
        assert!(store.exists(&referenced));
        assert!(!store.exists(&orphan));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn referenced_includes_judgment_and_intro() {
        let mut save = Save::new_game("c1", "s", "t".into());
        save.judgments_mut("c1").push(JudgmentMade {
            judgment: "judge_wolf".into(),
            result_snapshot: "snapshots/c1/judge_wolf.md".into(),
        });
        save.viewed_intros
            .insert("c1".into(), "snapshots/c1/intro.md".into());
        let set = referenced_snapshot_paths(&save);
        assert!(set.contains("snapshots/c1/judge_wolf.md"));
        assert!(set.contains("snapshots/c1/intro.md"));
    }

    #[test]
    fn path_helpers() {
        assert_eq!(
            dialogue_snapshot_path("c1", "wolf", "whereabouts", World::Shadow),
            "snapshots/c1/wolf.whereabouts.shadow.md"
        );
        assert_eq!(intro_snapshot_path("c1"), "snapshots/c1/intro.md");
        assert_eq!(
            judgment_snapshot_path("c1", "judge_wolf"),
            "snapshots/c1/judge_wolf.md"
        );
    }
}
