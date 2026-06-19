//! 系统类指令：退出与笔记入口。

use crate::engine::outcome::{Message, Outcome};
use crate::engine::state::Session;

impl Session {
    /// 退出。`force = true`（SIGTERM 等）时 best-effort 持久化后无条件退出；
    /// `force = false`（Ctrl+C 等）时存档失败则留在游戏内提示，给玩家抢救机会。
    pub(crate) fn do_quit(&mut self, force: bool) -> Outcome {
        let saved = self.try_persist();
        if force || saved {
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
