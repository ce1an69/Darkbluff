//! `ask` 指令：角色/话题菜单、对话快照与线索收集。

use crate::content::condition::topic_visible;
use crate::engine::condition::build_factset;
use crate::engine::logic::ask_topic_options;
use crate::engine::outcome::{MenuKind, MenuOption, Message, Outcome, SessionState};
use crate::engine::state::Session;
use crate::save::schema::ViewedDialogue;
use crate::save::snapshot::dialogue_snapshot_path;
use crate::world::World;

impl Session {
    pub(crate) fn do_ask(&mut self, target: Option<String>, topic: Option<String>) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let scene = self.save.current_scene.clone();
        match target {
            None => {
                let chars = self.engine.get_characters_in_scene(&ch, &scene);
                if chars.is_empty() {
                    return Outcome::Message(Message::info(vec![
                        "这里没有任何可以询问的角色。".into()
                    ]));
                }
                let options: Vec<MenuOption> = chars
                    .iter()
                    .map(|c| MenuOption {
                        id: c.id.clone(),
                        label: c.name.clone(),
                    })
                    .collect();
                self.set_menu(MenuKind::AskCharacter, options.clone());
                self.state = SessionState::ChoosingAskCharacter;
                Outcome::MenuRequested {
                    kind: MenuKind::AskCharacter,
                    prompt: "询问谁？".into(),
                    options,
                }
            }
            Some(t) => {
                let in_scene = self
                    .engine
                    .get_characters_in_scene(&ch, &scene)
                    .iter()
                    .any(|c| c.id == t);
                if !in_scene {
                    return Outcome::Message(Message::info(vec!["这里没有这个角色。".into()]));
                }
                match topic {
                    None => {
                        let topics = ask_topic_options(&self.engine, &self.save, &t);
                        if topics.is_empty() {
                            return Outcome::Message(Message::info(vec!["没有可以问的。".into()]));
                        }
                        let options: Vec<MenuOption> = topics
                            .iter()
                            .map(|(id, label)| MenuOption {
                                id: id.clone(),
                                label: label.clone(),
                            })
                            .collect();
                        self.set_ask_topic_menu(t, options.clone());
                        self.state = SessionState::ChoosingAskTopic;
                        Outcome::MenuRequested {
                            kind: MenuKind::AskTopic,
                            prompt: "问什么？".into(),
                            options,
                        }
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

        let topic_def = self
            .engine
            .get_topics(&ch, character)
            .iter()
            .find(|t| t.id == topic)
            .cloned();
        let Some(t) = topic_def else {
            return Outcome::Message(Message::info(vec!["你还没有足够线索。".into()]));
        };
        if !topic_visible(&t, &facts) {
            return Outcome::Message(Message::info(vec!["你还没有足够线索。".into()]));
        }
        let Some(body) = self.engine.get_dialogue(&ch, character, topic, world) else {
            return Outcome::Message(Message::info(vec!["这个话题在这一侧无从问起。".into()]));
        };
        let body = body.to_string();

        let mut notes = Vec::new();
        self.record_dialogue_view(&ch, character, topic, world, &body, &mut notes);
        let collected_new = self.collect_clues(&ch, character, topic, world, &mut notes);
        self.save.discovered.add_topic(&ch, character, topic);

        self.persist();
        self.state = SessionState::Exploring;

        if let Some(hint) = self.hint_after_ask(world, collected_new) {
            notes.push(hint);
        }

        let char_name = self
            .engine
            .get_character(character)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| character.into());
        self.then_narrative(Outcome::Dialogue {
            header: format!("{char_name} - {} [{}]", t.label, world.label()),
            body,
            notes,
        })
    }

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
            .map(|v| {
                v.iter()
                    .any(|d| d.character == character && d.topic == topic && d.world == world)
            })
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
}
