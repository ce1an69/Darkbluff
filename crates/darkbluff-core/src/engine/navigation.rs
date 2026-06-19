//! 场景移动与视角切换。

use crate::engine::logic::move_options;
use crate::engine::outcome::{MenuKind, MenuOption, Message, Outcome, SessionState};
use crate::engine::state::Session;
use crate::world::World;

impl Session {
    pub(crate) fn do_gaze(&mut self) -> Outcome {
        self.save.current_world = self.save.current_world.toggle();
        self.hints.ever_gazed = true;
        self.persist();
        self.state = SessionState::Exploring;
        let (eye, action) = match self.save.current_world {
            World::Surface => ("右眼·表面", "睁开右眼"),
            World::Shadow => ("左眼·影子", "闭上右眼"),
        };
        let msgs = vec![format!("[{eye}] {action}，视野里的事物好像变得不一样了…")];
        self.then_narrative(Outcome::Message(Message::info(msgs)))
    }

    pub(crate) fn do_move(&mut self, dest: Option<String>) -> Outcome {
        match dest {
            None => {
                let opts = move_options(&self.engine, &self.save);
                if opts.is_empty() {
                    return Outcome::Message(Message::info(vec!["没有可以前往的地方。".into()]));
                }
                let options: Vec<MenuOption> = opts
                    .iter()
                    .map(|(id, name)| MenuOption {
                        id: id.clone(),
                        label: name.clone(),
                    })
                    .collect();
                self.set_menu(MenuKind::MoveDestination, options.clone());
                self.state = SessionState::ChoosingMove;
                Outcome::MenuRequested {
                    kind: MenuKind::MoveDestination,
                    prompt: "前往哪里？".into(),
                    options,
                }
            }
            Some(d) => {
                if d == "__leave" {
                    return self.attempt_leave();
                }
                let cur = self.save.current_scene.clone();
                let reachable = self.engine.get_reachable_scenes(&cur);
                if !reachable.iter().any(|s| *s == d) {
                    return Outcome::Message(Message::info(vec!["你现在无法前往那里。".into()]));
                }
                self.save.current_scene = d;
                self.persist();
                self.state = SessionState::Exploring;
                let base = Outcome::Message(Message::info(self.scene_description_messages()));
                self.then_narrative(base)
            }
        }
    }
}
