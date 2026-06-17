//! 指令执行：ask / judge / move / map / gaze / note / quit。
//!
//! 这些都是 [`crate::engine::state::Session`] 的方法，按职责拆到此文件。各方法把外部输入
//! 错误转为可读 [`Outcome`](crate::engine::outcome::Outcome)，绝不 panic。

use crate::content::condition::topic_visible;
use crate::content::Judgment;
use crate::engine::condition::{build_factset, chapter_complete};
use crate::engine::logic::{ask_topic_options, move_options, unjudged_character_options};
use crate::engine::outcome::{AppState, MenuOption, Outcome};
use crate::engine::state::Session;
use crate::save::checkpoint;
use crate::save::schema::{Checkpoint, CheckpointKind, JudgmentMade, ViewedDialogue};
use crate::save::snapshot::{dialogue_snapshot_path, judgment_snapshot_path, outro_snapshot_path};
use crate::world::World;

impl Session {
    pub(crate) fn do_quit(&mut self) -> Outcome {
        if self.try_persist() {
            Outcome::Quit
        } else {
            Outcome::Show(vec!["保存失败，未退出。请检查磁盘空间后重试。".into()])
        }
    }

    pub(crate) fn do_gaze(&mut self) -> Outcome {
        self.save.current_world = self.save.current_world.toggle();
        self.hints.ever_gazed = true;
        self.persist();
        self.state = AppState::Exploring;
        let eye = match self.save.current_world {
            World::Surface => "右眼·表面",
            World::Shadow => "左眼·影子",
        };
        let mut msgs = vec![format!("[已切换到 {eye}]")];
        msgs.extend(self.scene_description_messages());
        Outcome::Show(msgs)
    }

    // ----- ask -----

    pub(crate) fn do_ask(&mut self, target: Option<String>, topic: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let scene = self.save.current_scene.clone();
        match target {
            None => {
                let chars = self.engine.get_characters_in_scene(&ch, &scene);
                if chars.is_empty() {
                    return Outcome::Show(vec!["这里没有任何可以询问的角色。".into()]);
                }
                let options: Vec<MenuOption> = chars
                    .iter()
                    .map(|c| MenuOption { id: c.id.clone(), label: c.name.clone() })
                    .collect();
                self.pending.menu = Some(options.clone());
                self.state = AppState::ChoosingAskCharacter;
                Outcome::Menu { title: "询问谁？".into(), options }
            }
            Some(t) => {
                let in_scene = self.engine.get_characters_in_scene(&ch, &scene).iter().any(|c| c.id == t);
                if !in_scene {
                    return Outcome::Show(vec!["这里没有这个角色。".into()]);
                }
                match topic {
                    None => {
                        let topics = ask_topic_options(&self.engine, &self.save, &t);
                        if topics.is_empty() {
                            return Outcome::Show(vec!["没有可以问的。".into()]);
                        }
                        let options: Vec<MenuOption> = topics
                            .iter()
                            .map(|(id, label)| MenuOption { id: id.clone(), label: label.clone() })
                            .collect();
                        self.pending.ask_character = Some(t);
                        self.pending.menu = Some(options.clone());
                        self.state = AppState::ChoosingAskTopic;
                        Outcome::Menu { title: "问什么？".into(), options }
                    }
                    Some(topic_id) => self.ask_topic(&t, &topic_id),
                }
            }
        }
    }

    pub(crate) fn ask_topic(&mut self, character: &str, topic: &str) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let world = self.save.current_world;
        let facts = build_factset(&self.save);

        let topic_def = self.engine.get_topics(&ch, character).iter().find(|t| t.id == topic).cloned();
        let Some(t) = topic_def else {
            return Outcome::Show(vec!["你还没有足够线索。".into()]);
        };
        if !topic_visible(&t, &facts) {
            return Outcome::Show(vec!["你还没有足够线索。".into()]);
        }
        let Some(body) = self.engine.get_dialogue(&ch, character, topic, world) else {
            return Outcome::Show(vec!["这个话题在这一侧无从问起。".into()]);
        };
        let body = body.to_string();

        let mut notes = Vec::new();
        self.record_dialogue_view(&ch, character, topic, world, &body, &mut notes);
        let collected_new = self.collect_clues(&ch, character, topic, world, &mut notes);
        self.save.discovered.add_topic(&ch, character, topic);

        self.persist();
        self.state = AppState::Exploring;

        if let Some(hint) = self.hint_after_ask(world, collected_new) {
            notes.push(hint);
        }

        let char_name = self
            .engine
            .get_character(character)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| character.into());
        Outcome::Dialogue {
            header: format!("{char_name} - {} [{}]", t.label, world.label()),
            body,
            notes,
        }
    }

    /// 写对话快照（去重）：首次查看时写入并记录索引。
    fn record_dialogue_view(
        &mut self,
        ch: &str,
        character: &str,
        topic: &str,
        world: World,
        body: &str,
        notes: &mut Vec<String>,
    ) {
        let already = self
            .save
            .viewed_dialogues
            .get(ch)
            .map(|v| v.iter().any(|d| d.character == character && d.topic == topic && d.world == world))
            .unwrap_or(false);
        if already {
            return;
        }
        let rel = dialogue_snapshot_path(ch, character, topic, world);
        if let Err(e) = self.store.snapshots().write(&rel, body) {
            notes.push(format!("（快照写入失败：{e}）"));
        }
        self.save.views_mut(ch).push(ViewedDialogue {
            character: character.into(),
            topic: topic.into(),
            world,
            snapshot: rel,
        });
        notes.push("[对话已记录到 note]".into());
    }

    /// 收集命中线索（去重），返回是否有新收集。
    fn collect_clues(
        &mut self,
        ch: &str,
        character: &str,
        topic: &str,
        world: World,
        notes: &mut Vec<String>,
    ) -> bool {
        let src_key = format!("{character}.{topic}");
        let mut collected_new = false;
        for clue in self.engine.get_clues(ch) {
            if clue.source == src_key && clue.world == world && !self.save.has_clue(ch, &clue.id) {
                self.save.clues_mut(ch).push(clue.id.clone());
                collected_new = true;
            }
        }
        if collected_new {
            notes.push("[相关线索已收集]".into());
        }
        collected_new
    }

    // ----- judge -----

    pub(crate) fn do_judge(&mut self, target: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        match target {
            None => {
                let unjudged = unjudged_character_options(&self.engine, &self.save);
                if unjudged.is_empty() {
                    return Outcome::Show(vec!["本章没有可审判的角色。".into()]);
                }
                let options: Vec<MenuOption> = unjudged
                    .iter()
                    .map(|(id, name)| MenuOption { id: id.clone(), label: name.clone() })
                    .collect();
                self.pending.menu = Some(options.clone());
                self.state = AppState::ChoosingJudgeCharacter;
                Outcome::Menu { title: "审判谁？".into(), options }
            }
            Some(t) => {
                let Some(j) = self.engine.get_judgment_for_character(&ch, &t).cloned() else {
                    return Outcome::Show(vec!["现在还无法审判他。".into()]);
                };
                if self.save.judged(&ch, &j.id) {
                    return Outcome::Show(vec![
                        "你已经审判过他了。（可用 map 回到审判前 checkpoint）".into(),
                    ]);
                }
                self.execute_judgment(&j)
            }
        }
    }

    fn execute_judgment(&mut self, j: &Judgment) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let now = self.store.clock().now_iso();

        // 审判前检查点 + 记录审判 + 写审判剧情快照
        checkpoint::create_before_judgment(&mut self.save, &ch, &j.id, &now);
        let result_text = self.engine.get_result_text(&ch, &j.id).unwrap_or("").to_string();
        let rel = judgment_snapshot_path(&ch, &j.id);
        if let Err(e) = self.store.snapshots().write(&rel, &result_text) {
            tracing::warn!("审判快照写入失败: chapter={ch}, judgment={}, error={e}", j.id);
        }
        self.save.judgments_mut(&ch).push(JudgmentMade {
            judgment: j.id.clone(),
            result_snapshot: rel,
        });

        let facts = build_factset(&self.save);
        let all_jids: Vec<String> = self.engine.get_judgments(&ch).iter().map(|x| x.id.clone()).collect();
        let complete = self
            .engine
            .get_chapter(&ch)
            .map(|c| chapter_complete(c, &facts, &all_jids))
            .unwrap_or(false);

        self.persist();
        let map_hint = self.hint_after_judge();

        if complete {
            let outcome = self.advance_after_judgment(Some(result_text));
            match (map_hint, outcome) {
                (Some(hint), Outcome::Show(mut lines)) => {
                    lines.push(hint);
                    Outcome::Show(lines)
                }
                (_, o) => o,
            }
        } else {
            self.state = AppState::Exploring;
            let mut msgs = vec![result_text, "[本章必要审判尚未全部完成，继续探索]".into()];
            if let Some(hint) = map_hint {
                msgs.push(hint);
            }
            Outcome::Show(msgs)
        }
    }

    pub(crate) fn advance_after_judgment(&mut self, prelude: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let Some(chapter) = self.engine.get_chapter(&ch).cloned() else {
            self.state = AppState::Exploring;
            return Outcome::Show(vec!["无法定位当前章节。".into()]);
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
                self.state = AppState::ShowingOutro;
                self.persist();
                let text = match prelude {
                    Some(p) if !p.is_empty() => format!("{p}\n\n{outro}"),
                    _ => outro,
                };
                return Outcome::Outro { text };
            } else {
                self.state = AppState::Ending;
                return self.ending_outcome();
            }
        }

        // 评估下一章
        let facts = build_factset(&self.save);
        let Some(next_id) = self.engine.get_next_chapter(&ch, &facts).map(|s| s.to_string()) else {
            self.state = AppState::Exploring;
            let mut msgs = prelude.into_iter().collect::<Vec<_>>();
            msgs.push("[无法推进到下一章节]".into());
            return Outcome::Show(msgs);
        };
        let outcome = self.enter_chapter(&next_id, true);
        let head_line = "[必要审判已完成，自动推进章节]";
        match (prelude, outcome) {
            (Some(p), Outcome::Intro { text }) if !p.is_empty() => {
                Outcome::Intro { text: format!("{p}\n\n{text}") }
            }
            (Some(p), Outcome::Show(mut lines)) => {
                let mut head: Vec<String> = vec![p, head_line.into()];
                head.append(&mut lines);
                Outcome::Show(head)
            }
            (None, Outcome::Show(mut lines)) => {
                lines.insert(0, head_line.into());
                Outcome::Show(lines)
            }
            (_, o) => o,
        }
    }

    // ----- move / map -----

    pub(crate) fn do_move(&mut self, dest: Option<String>) -> Outcome {
        match dest {
            None => {
                let opts = move_options(&self.engine, &self.save);
                if opts.is_empty() {
                    return Outcome::Show(vec!["没有可以前往的地方。".into()]);
                }
                let options: Vec<MenuOption> = opts
                    .iter()
                    .map(|(id, name)| MenuOption { id: id.clone(), label: name.clone() })
                    .collect();
                self.pending.menu = Some(options.clone());
                self.state = AppState::ChoosingMove;
                Outcome::Menu { title: "前往哪里？".into(), options }
            }
            Some(d) => {
                let cur = self.save.current_scene.clone();
                let reachable = self.engine.get_reachable_scenes(&cur);
                if !reachable.iter().any(|s| *s == d) {
                    return Outcome::Show(vec!["你现在无法前往那里。".into()]);
                }
                self.save.current_scene = d;
                self.persist();
                self.state = AppState::Exploring;
                Outcome::Show(self.scene_description_messages())
            }
        }
    }

    pub(crate) fn do_map(&mut self) -> Outcome {
        if self.save.checkpoints.is_empty() {
            return Outcome::Show(vec!["还没有可以回到的节点。".into()]);
        }
        let options: Vec<MenuOption> = self
            .save
            .checkpoints
            .iter()
            .map(|c| MenuOption { id: c.id.clone(), label: self.describe_checkpoint(c) })
            .collect();
        self.pending.menu = Some(options.clone());
        self.state = AppState::ChoosingCheckpoint;
        Outcome::Menu { title: "选择要回到的节点：".into(), options }
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

    pub(crate) fn execute_rollback_confirm(&mut self) -> Outcome {
        let id = match self.pending.confirm_rollback_id.clone() {
            Some(id) => id,
            None => {
                self.state = AppState::Exploring;
                return Outcome::Show(vec!["这个节点已经无法回到。".into()]);
            }
        };
        // 先取 kind（rollback 后该检查点会被移除）
        let kind = self.save.checkpoints.iter().find(|c| c.id == id).map(|c| c.kind);
        match checkpoint::map_checkpoint_rollback(&mut self.save, &id) {
            Ok(()) => {
                let _ = self.store.snapshots().cleanup_orphans(&self.save);
                self.persist();
                // 回到 chapter_start：若有 intro 重新展示（去重）；否则直接进场景
                if kind == Some(CheckpointKind::ChapterStart) {
                    let ch = self.save.current_chapter.clone();
                    if let Some(text) = self.engine.get_intro_text(&ch).map(|s| s.to_string()) {
                        self.pending.intro_needs_checkpoint = false;
                        self.state = AppState::ShowingIntro;
                        return Outcome::Intro { text };
                    }
                }
                self.state = AppState::Exploring;
                Outcome::Show(self.scene_description_messages())
            }
            Err(_) => {
                self.state = AppState::Exploring;
                Outcome::Show(vec!["这个节点已经无法回到。".into()])
            }
        }
    }

    pub(crate) fn do_note(&mut self) -> Outcome {
        Outcome::Note(self.build_note_view())
    }
}
