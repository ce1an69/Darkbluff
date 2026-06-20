//! `map` 指令与 checkpoint 回滚。

use crate::engine::outcome::{MenuKind, MenuOption, Message, Outcome, SessionState};
use crate::engine::state::Session;
use crate::save::checkpoint;
use crate::save::schema::{Checkpoint, CheckpointKind};

impl Session {
    pub(crate) fn do_map(&mut self) -> Outcome {
        if self.save.checkpoints.is_empty() {
            return Outcome::Message(Message::info(vec!["还没有可以回到的节点。".into()]));
        }
        let options: Vec<MenuOption> = self
            .save
            .checkpoints
            .iter()
            .map(|c| MenuOption {
                id: c.id.clone(),
                label: self.describe_checkpoint(c),
            })
            .collect();
        self.set_menu(MenuKind::Checkpoint, options.clone());
        self.state = SessionState::ChoosingCheckpoint;
        Outcome::MenuRequested {
            kind: MenuKind::Checkpoint,
            prompt: "选择要回到的节点：".into(),
            options,
        }
    }

    fn describe_checkpoint(&self, c: &Checkpoint) -> String {
        let chap_title = self
            .engine
            .get_chapter(&c.chapter)
            .map(|ch| ch.title.clone())
            .unwrap_or_else(|| c.chapter.clone());
        let kind = match c.kind {
            CheckpointKind::ChapterStart => "章节开始",
            CheckpointKind::BeforeJudgment => "审判前",
        };
        let scene_name = self
            .engine
            .get_scene(&c.scene)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| c.scene.clone());
        format!("{chap_title} · {kind} · {scene_name}")
    }

    pub(crate) fn execute_rollback_confirm(&mut self, id: &str) -> Outcome {
        let kind = self
            .save
            .checkpoints
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.kind);
        match checkpoint::map_checkpoint_rollback(&mut self.save, id) {
            Ok(()) => {
                let _ = self.store.snapshots().cleanup_orphans(&self.save);
                self.persist();
                if kind == Some(CheckpointKind::ChapterStart) {
                    let ch = self.save.current_chapter.clone();
                    if let Some(text) = self.engine.get_intro_text(&ch).map(|s| s.to_string()) {
                        self.set_intro_pending(false);
                        self.state = SessionState::ShowingIntro;
                        return Outcome::ChapterIntro { text };
                    }
                }
                self.state = SessionState::Exploring;
                Outcome::SceneDescription {
                    text: self.scene_description_text(),
                }
            }
            Err(_) => {
                self.state = SessionState::Exploring;
                Outcome::Message(Message::info(vec!["这个节点已经无法回到。".into()]))
            }
        }
    }
}
