//! `judge` 指令与审判后的章节推进。

use crate::content::Judgment;
use crate::engine::condition::{build_factset, chapter_complete};
use crate::engine::logic::unjudged_character_options;
use crate::engine::outcome::{MenuKind, MenuOption, Message, Outcome, SessionState};
use crate::engine::state::Session;
use crate::save::checkpoint;
use crate::save::schema::JudgmentMade;
use crate::save::snapshot::{judgment_snapshot_path, outro_snapshot_path};

impl Session {
    pub(crate) fn do_judge(&mut self, target: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        match target {
            None => {
                let unjudged = unjudged_character_options(&self.engine, &self.save);
                if unjudged.is_empty() {
                    return Outcome::Message(Message::info(vec!["本章没有可审判的角色。".into()]));
                }
                let options: Vec<MenuOption> = unjudged
                    .iter()
                    .map(|(id, name)| MenuOption {
                        id: id.clone(),
                        label: name.clone(),
                    })
                    .collect();
                self.set_menu(MenuKind::JudgeCharacter, options.clone());
                self.state = SessionState::ChoosingJudgeCharacter;
                Outcome::MenuRequested {
                    kind: MenuKind::JudgeCharacter,
                    prompt: "审判谁？".into(),
                    options,
                }
            }
            Some(t) => {
                let Some(j) = self.engine.get_judgment_for_character(&ch, &t).cloned() else {
                    return Outcome::Message(Message::info(vec!["现在还无法审判他。".into()]));
                };
                if self.save.judged(&ch, &j.id) {
                    return Outcome::Message(Message::info(vec![
                        "你已经审判过他了。（可用 map 回到审判前 checkpoint）".into(),
                    ]));
                }
                self.execute_judgment(&j)
            }
        }
    }

    fn execute_judgment(&mut self, j: &Judgment) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let now = self.store.clock().now_iso();

        checkpoint::create_before_judgment(&mut self.save, &ch, &j.id, &now);
        let result_text = self
            .engine
            .get_result_text(&ch, &j.id)
            .unwrap_or("")
            .to_string();
        let rel = judgment_snapshot_path(&ch, &j.id);
        if let Err(e) = self.store.snapshots().write(&rel, &result_text) {
            tracing::warn!(
                "审判快照写入失败: chapter={ch}, judgment={}, error={e}",
                j.id
            );
        }
        self.save.judgments_mut(&ch).push(JudgmentMade {
            judgment: j.id.clone(),
            result_snapshot: rel,
        });

        let facts = build_factset(&self.save);
        let all_jids: Vec<String> = self
            .engine
            .get_judgments(&ch)
            .iter()
            .map(|x| x.id.clone())
            .collect();
        let complete = self
            .engine
            .get_chapter(&ch)
            .map(|c| chapter_complete(c, &facts, &all_jids))
            .unwrap_or(false);

        self.persist();
        let map_hint = self.hint_after_judge();

        if complete {
            self.drain_or_advance(Some(result_text), map_hint)
        } else {
            self.state = SessionState::Exploring;
            let mut msgs = vec![result_text, "[本章必要审判尚未全部完成，继续探索]".into()];
            if let Some(hint) = map_hint {
                msgs.push(hint);
            }
            self.then_narrative(Outcome::Message(Message::info(msgs)))
        }
    }

    /// 推进章节并把 `map_hint` 拼到产出的 Message（若有）。
    pub(crate) fn advance_with_hint(
        &mut self,
        prelude: Option<String>,
        map_hint: Option<String>,
    ) -> Outcome {
        let outcome = self.advance_after_judgment(prelude);
        match map_hint {
            Some(hint) => match outcome {
                Outcome::Message(mut message) => {
                    message.lines.push(hint);
                    Outcome::Message(message)
                }
                other => other,
            },
            None => outcome,
        }
    }

    pub(crate) fn advance_after_judgment(&mut self, prelude: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let Some(chapter) = self.engine.get_chapter(&ch).cloned() else {
            self.state = SessionState::Exploring;
            return Outcome::Message(Message::info(vec!["无法定位当前章节。".into()]));
        };

        if chapter.ending {
            self.save.discovered.add_ending(&ch);
            self.persist();
            if let Some(outro) = self.engine.get_outro_text(&ch).map(|s| s.to_string()) {
                if !self.save.viewed_outros.contains_key(&ch) {
                    let rel = outro_snapshot_path(&ch);
                    match self.store.snapshots().write(&rel, &outro) {
                        Ok(path) => {
                            self.save.viewed_outros.insert(ch.clone(), path);
                        }
                        Err(e) => {
                            tracing::warn!("结局快照写入失败: chapter={ch}, error={e}");
                        }
                    }
                }
                self.state = SessionState::ShowingOutro;
                self.persist();
                let text = match prelude {
                    Some(p) if !p.is_empty() => format!("{p}\n\n{outro}"),
                    _ => outro,
                };
                return Outcome::ChapterOutro { text };
            } else {
                self.state = SessionState::Ending;
                return self.ending_outcome();
            }
        }

        let facts = build_factset(&self.save);
        let Some(next_id) = self
            .engine
            .get_next_chapter(&ch, &facts)
            .map(|s| s.to_string())
        else {
            self.state = SessionState::Exploring;
            let mut msgs = prelude.into_iter().collect::<Vec<_>>();
            msgs.push("[无法推进到下一章节]".into());
            return Outcome::Message(Message::info(msgs));
        };
        let outcome = self.enter_chapter(&next_id, true);
        let head_line = "[必要审判已完成，自动推进章节]";
        match (prelude, outcome) {
            (Some(p), Outcome::ChapterIntro { text }) if !p.is_empty() => Outcome::ChapterIntro {
                text: format!("{p}\n\n{text}"),
            },
            (Some(p), Outcome::Message(mut message)) => {
                let mut head: Vec<String> = vec![p, head_line.into()];
                head.append(&mut message.lines);
                message.lines = head;
                Outcome::Message(message)
            }
            (None, Outcome::Message(mut message)) => {
                message.lines.insert(0, head_line.into());
                Outcome::Message(message)
            }
            (_, o) => o,
        }
    }
}
