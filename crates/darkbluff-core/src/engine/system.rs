//! 系统类指令：退出与笔记入口。

use crate::engine::outcome::{Message, Outcome};
use crate::engine::state::Session;

impl Session {
    pub(crate) fn do_quit(&mut self) -> Outcome {
        if self.try_persist() {
            Outcome::QuitRequested
        } else {
            Outcome::Message(Message::info(vec![
                "保存失败，未退出。请检查磁盘空间后重试。".into(),
            ]))
        }
    }

    pub(crate) fn do_note(&mut self) -> Outcome {
        Outcome::Notes(self.build_note_view())
    }
}
