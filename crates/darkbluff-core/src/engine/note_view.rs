//! 笔记视图构建：遍历 `chapter_path` 收集对话/叙事/审判快照。

use crate::engine::outcome::{NoteDialogue, NoteJudgment, NoteNarrative, NoteView, NoteVoice};
use crate::engine::state::Session;

impl Session {
    pub(crate) fn build_note_view(&self) -> NoteView {
        let mut view = NoteView::default();
        for ch in &self.save.chapter_path {
            self.push_dialogue_notes(ch, &mut view);
            self.push_narrative_notes(ch, &mut view);
            self.push_judgment_notes(ch, &mut view);
            self.push_voice_notes(ch, &mut view);
        }
        view
    }

    fn push_dialogue_notes(&self, ch: &str, view: &mut NoteView) {
        let Some(list) = self.save.viewed_dialogues.get(ch) else {
            return;
        };
        for d in list {
            let char_name = self
                .engine
                .get_character(&d.character)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| d.character.clone());
            let topic_label = self
                .engine
                .get_topics(ch, &d.character)
                .iter()
                .find(|t| t.id == d.topic)
                .map(|t| t.label.clone())
                .unwrap_or_else(|| d.topic.clone());
            view.dialogues.push(NoteDialogue {
                chapter: ch.to_string(),
                character_id: d.character.clone(),
                character_name: char_name,
                topic_id: d.topic.clone(),
                topic_label,
                world: d.world,
                text: self.read_snapshot_or_missing(&d.snapshot),
            });
        }
    }

    fn push_narrative_notes(&self, ch: &str, view: &mut NoteView) {
        let title = self
            .engine
            .get_chapter(ch)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        if let Some(rel) = self.save.viewed_intros.get(ch) {
            view.narratives.push(NoteNarrative {
                chapter: ch.to_string(),
                title: title.clone(),
                is_outro: false,
                text: self.read_snapshot_or_missing(rel),
            });
        }
        if let Some(rel) = self.save.viewed_outros.get(ch) {
            view.narratives.push(NoteNarrative {
                chapter: ch.to_string(),
                title,
                is_outro: true,
                text: self.read_snapshot_or_missing(rel),
            });
        }
    }

    fn push_judgment_notes(&self, ch: &str, view: &mut NoteView) {
        let Some(list) = self.save.judgments_made.get(ch) else {
            return;
        };
        for j in list {
            let target_name = self
                .engine
                .get_judgments(ch)
                .iter()
                .find(|x| x.id == j.judgment)
                .and_then(|x| self.engine.get_character(&x.target))
                .map(|c| c.name.clone())
                .unwrap_or_default();
            view.judgments.push(NoteJudgment {
                chapter: ch.to_string(),
                judgment_id: j.judgment.clone(),
                target_name,
                text: self.read_snapshot_or_missing(&j.result_snapshot),
            });
        }
    }

    fn push_voice_notes(&self, ch: &str, view: &mut NoteView) {
        let Some(list) = self.save.viewed_narrative.get(ch) else {
            return;
        };
        for n in list {
            let label = self
                .engine
                .get_narrative_item(ch, &n.id)
                .map(|x| x.label.clone())
                .unwrap_or_else(|| {
                    if n.id == crate::content::LEAVE_ATTEMPT_TRIGGER {
                        "走不出去".into()
                    } else {
                        "旁白".into()
                    }
                });
            view.voices.push(NoteVoice {
                chapter: ch.to_string(),
                label,
                text: self.read_snapshot_or_missing(&n.snapshot),
            });
        }
    }

    pub(crate) fn read_snapshot_or_missing(&self, rel: &str) -> String {
        match self.store.snapshots().read(rel) {
            Ok(text) => text,
            Err(_) => "该记录快照缺失".into(),
        }
    }
}
