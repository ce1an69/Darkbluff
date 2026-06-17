//! 首章新手引导提示（每种只显示一次、仅首章触发、不阻断操作）。
//!
//! 设计见 docs/architecture.md「新手引导」。Hints 存在于 [`crate::engine::state::Session`]
//! 的运行时状态中、不进存档，每次新游戏重置。

use crate::engine::logic::unjudged_character_options;
use crate::engine::state::Session;
use crate::world::World;

/// 首章新手引导提示的运行时状态（不进存档；每种只显示一次；仅首章触发）。
#[derive(Debug, Clone, Default)]
pub struct Hints {
    pub(crate) surface_asks: u32,
    pub(crate) ever_gazed: bool,
    pub(crate) ever_judged: bool,
    pub(crate) gaze_shown: bool,
    pub(crate) judge_shown: bool,
    pub(crate) map_shown: bool,
}

impl Session {
    /// ask 后的首章引导：连续 3 次 surface 询问未 gaze / 收集到线索却未 judge。
    pub(crate) fn hint_after_ask(&mut self, world: World, collected_new: bool) -> Option<String> {
        if !self.in_first_chapter() {
            return None;
        }
        if world == World::Surface {
            self.hints.surface_asks += 1;
        }
        let mut out: Option<String> = None;
        // gaze 提示：连续 3 次 surface ask 且从未 gaze
        if !self.hints.ever_gazed && !self.hints.gaze_shown && self.hints.surface_asks >= 3 {
            self.hints.gaze_shown = true;
            out = Some("试试 gaze 切换到影子世界，看看会有什么不同。".into());
        }
        // judge 提示：刚收集了线索且从未审判且仍有可审判角色
        if collected_new
            && !self.hints.ever_judged
            && !self.hints.judge_shown
            && !unjudged_character_options(&self.engine, &self.save).is_empty()
        {
            self.hints.judge_shown = true;
            let msg = "也许可以对某人做出 judge 了。".to_string();
            out = Some(match out {
                Some(prev) => format!("{prev}\n{msg}"),
                None => msg,
            });
        }
        out
    }

    /// 审判后的首章引导：可用 map 回到之前的检查点。
    pub(crate) fn hint_after_judge(&mut self) -> Option<String> {
        if !self.in_first_chapter() {
            return None;
        }
        self.hints.ever_judged = true;
        if !self.hints.map_shown {
            self.hints.map_shown = true;
            Some("可以用 map 回到之前的检查点，尝试另一条路。".into())
        } else {
            None
        }
    }
}
