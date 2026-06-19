//! 章节进入、自动推进与结局。
//!
//! 设计见 docs/data-formats.md「自动推进章节」、docs/narrative.md「终章结构」。
//! 这些是 [`crate::engine::state::Session`] 的方法，按职责拆到此文件。

use crate::engine::condition::{build_factset, chapter_complete};
use crate::engine::outcome::{Message, Outcome, SessionState};
use crate::engine::state::Session;
use crate::save::checkpoint;
use crate::save::snapshot::intro_snapshot_path;
use crate::world::World;

impl Session {
    /// 直接注入存档（继续游戏）。先做内容引用失效校验与回退，再恢复到 Exploring。
    pub fn continue_with(&mut self, save: crate::save::Save) -> Outcome {
        self.save = save;
        self.pending = Default::default();
        self.hints = Default::default();
        let warnings = crate::engine::logic::reconcile_save(&mut self.save, &self.engine);
        for w in &warnings {
            tracing::warn!("存档回退: {w}");
        }

        // 崩溃恢复：drain_or_advance 把「审判完成后的推进」挂到非持久化的 pending；若上次在
        // 「审判完成 + 心声展示中」退出，存档里审判已完成但章节未切换。检测到当前章必要审判
        // 已完成却仍停在 Exploring，补回被中断的推进，避免软锁（无指令可恢复）。
        if self.advance_was_interrupted() {
            return self.advance_after_judgment(None);
        }

        self.state = SessionState::Exploring;
        let has_warnings = !warnings.is_empty();
        let mut msgs = warnings;
        msgs.extend(self.scene_description_messages());
        let message = if has_warnings {
            Message::warning(msgs)
        } else {
            Message::info(msgs)
        };
        Outcome::Message(message)
    }

    /// 当前章必要审判是否已完成（供 continue_with 检测被中断的自动推进）。
    fn advance_was_interrupted(&self) -> bool {
        let ch = &self.save.current_chapter;
        let Some(chapter) = self.engine.get_chapter(ch) else {
            return false;
        };
        let facts = build_factset(&self.save);
        let all_jids: Vec<String> = self
            .engine
            .get_judgments(ch)
            .iter()
            .map(|j| j.id.clone())
            .collect();
        chapter_complete(chapter, &facts, &all_jids)
    }

    /// 开始新游戏：初始化首章。
    pub fn start_new_game(&mut self) -> Outcome {
        let first = match self.engine.first_chapter_id() {
            Some(c) => c.to_string(),
            None => {
                return Outcome::Message(Message::info(vec![
                    "找不到首章（内容校验未通过）。".into()
                ]))
            }
        };
        let Some(ch) = self.engine.get_chapter(&first) else {
            return Outcome::Message(Message::info(vec!["首章数据缺失。".into()]));
        };
        if ch.starting_scene.is_empty() {
            return Outcome::Message(Message::info(vec!["首章缺少 starting_scene。".into()]));
        }
        match self.store.new_game(&first, &ch.starting_scene) {
            Ok(save) => self.save = save,
            Err(e) => {
                return Outcome::Message(Message::info(vec![format!("无法初始化新游戏：{e}")]))
            }
        }
        self.pending = Default::default();
        self.hints = Default::default();
        self.enter_chapter(&first, false)
    }

    // ----- 章节进入 -----

    pub(crate) fn enter_chapter(&mut self, chapter_id: &str, new_entry: bool) -> Outcome {
        let Some(chapter) = self.engine.get_chapter(chapter_id).cloned() else {
            return Outcome::Message(Message::info(vec![format!("章节不存在：{chapter_id}")]));
        };
        if new_entry {
            if self.save.chapter_path.last().map(|s| s.as_str()) != Some(chapter_id) {
                self.save.chapter_path.push(chapter_id.into());
            }
            self.save.discovered.add_chapter(chapter_id);
        }
        self.save.current_chapter = chapter_id.into();
        self.save.current_scene = chapter.starting_scene.clone();
        self.save.current_world = World::Surface;

        if let Some(text) = self
            .engine
            .get_intro_text(chapter_id)
            .map(|s| s.to_string())
        {
            if !self.save.viewed_intros.contains_key(chapter_id) {
                let rel = intro_snapshot_path(chapter_id);
                match self.store.snapshots().write(&rel, &text) {
                    Ok(path) => {
                        self.save.viewed_intros.insert(chapter_id.into(), path);
                    }
                    Err(e) => {
                        tracing::warn!("开场快照写入失败: chapter={chapter_id}, error={e}");
                    }
                }
            }
            self.set_intro_pending(true);
            self.state = SessionState::ShowingIntro;
            self.persist();
            Outcome::ChapterIntro { text }
        } else {
            self.finalize_chapter_entry(true)
        }
    }

    pub(crate) fn ack_intro(&mut self) -> Outcome {
        let create = match self.pending.action {
            crate::engine::state::PendingAction::Intro { create_checkpoint } => create_checkpoint,
            _ => false,
        };
        self.pending.action = crate::engine::state::PendingAction::None;
        self.finalize_chapter_entry(create)
    }

    pub(crate) fn finalize_chapter_entry(&mut self, create_ckpt: bool) -> Outcome {
        if create_ckpt {
            let ch = self.save.current_chapter.clone();
            let scene = self.save.current_scene.clone();
            let world = self.save.current_world;
            let now = self.store.clock().now_iso();
            checkpoint::create_chapter_start(&mut self.save, &ch, &scene, world, &now);
        }
        self.state = SessionState::Exploring;
        self.persist();
        let base = Outcome::Message(Message::info(self.scene_description_messages()));
        self.then_narrative(base)
    }

    pub(crate) fn to_ending(&mut self) -> Outcome {
        self.state = SessionState::Ending;
        self.ending_outcome()
    }

    pub(crate) fn ending_outcome(&self) -> Outcome {
        let title = self
            .engine
            .get_chapter(&self.save.current_chapter)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        let total = self.engine.ending_chapter_ids().len();
        let found = self.save.discovered.endings.len();
        Outcome::EndingReached {
            title,
            found,
            total,
        }
    }
}
