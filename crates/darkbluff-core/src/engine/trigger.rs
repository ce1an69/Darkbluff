//! 叙事触发器（心声 / 记忆碎片）与「走不出去」。
//!
//! 设计见 docs/narrative/rules.md、docs/data-formats.md「叙事触发器」。
//!
//! - **状态驱动**：引擎在状态变更点（进入章节 / ask / judge / gaze / move）用 FactSet
//!   求值本章 `narrative` 的 `when`，命中且未触发者写入快照、记入 `discovered.triggers`
//!   与 `viewed_narrative`，产出 [`Outcome::Narrative`] 进入 `ShowingNarrative`；
//!   Ack 后继续 drain，序列结束执行 `follow_up`（回放暂存的 Outcome，或被推迟的章节推进）。
//! - **走不出去**：由场景 `exit_attempt` 承载，`move` 伪出口 `__leave` 触发
//!   （见 [`crate::engine::navigation`]），首次展示完整文本，之后简短提示，不移动。

use crate::content::LEAVE_ATTEMPT_TRIGGER;
use crate::content::condition::eval;
use crate::engine::condition::build_factset;
use crate::engine::outcome::{Message, Outcome, SessionState};
use crate::engine::state::{NarrativeFollowUp, PendingAction, Session};
use crate::save::schema::NarrativeSeen;
use crate::save::snapshot::narrative_snapshot_path;

impl Session {
    /// 取下一个待触发的叙事触发器：写快照、记 `viewed_narrative` / `discovered.triggers`，
    /// 返回其展示 [`Outcome::Narrative`]；无则 `None`。
    pub(crate) fn pop_narrative(&mut self) -> Option<Outcome> {
        let ch = self.save.current_chapter.clone();
        let facts = build_factset(&self.save);
        let pick = self
            .engine
            .get_narrative(&ch)
            .iter()
            .find(|n| {
                !self.save.discovered.triggered(&n.id)
                    && n.when.as_ref().map_or(true, |c| eval(c, &facts))
            })
            .cloned()?;
        let text = self
            .engine
            .get_narrative_text(&ch, &pick.id)
            .unwrap_or("")
            .to_string();
        let rel = narrative_snapshot_path(&ch, &pick.id);
        if let Err(e) = self.store.snapshots().write(&rel, &text) {
            tracing::warn!("叙事快照写入失败: chapter={ch}, id={}, error={e}", pick.id);
        }
        self.save
            .viewed_narrative
            .entry(ch.clone())
            .or_default()
            .push(NarrativeSeen {
                id: pick.id.clone(),
                snapshot: rel,
            });
        self.save.discovered.add_trigger(&pick.id);
        self.persist();
        Some(Outcome::Narrative {
            label: pick.label.clone(),
            text,
        })
    }

    /// 在基础 [`Outcome`] 之后接上叙事触发器：有待触发的心声则改为展示心声并把
    /// `base` 暂存为 `follow_up`（心声序列结束后回放），否则原样返回 `base`。
    pub(crate) fn then_narrative(&mut self, base: Outcome) -> Outcome {
        if let Some(narr) = self.pop_narrative() {
            self.pending.action = PendingAction::Narrative {
                follow_up: Some(NarrativeFollowUp::Outcome(base)),
            };
            self.state = SessionState::ShowingNarrative;
            narr
        } else {
            base
        }
    }

    /// 审判已完成时：先 drain 本章心声（碎片常挂在审判后），有心声则展示完毕再推进；
    /// 无心声则立即推进。承接 [`crate::engine::judge`] 的 complete 分支，确保
    /// 「审判后碎片」在章节切换前得以触发。
    pub(crate) fn drain_or_advance(
        &mut self,
        prelude: Option<String>,
        map_hint: Option<String>,
    ) -> Outcome {
        if let Some(narr) = self.pop_narrative() {
            self.pending.action = PendingAction::Narrative {
                follow_up: Some(NarrativeFollowUp::AdvanceAfterJudgment { prelude, map_hint }),
            };
            self.state = SessionState::ShowingNarrative;
            narr
        } else {
            self.advance_with_hint(prelude, map_hint)
        }
    }

    /// 确认当前心声：继续 drain 下一个；序列结束后执行 `follow_up`
    /// （回放 Outcome / 推进章节 / 回探索态）。
    pub(crate) fn ack_narrative(&mut self) -> Outcome {
        let follow_up = match self.pending.action.clone() {
            PendingAction::Narrative { follow_up } => follow_up,
            _ => None,
        };
        // 心声可能成串：继续 drain，保留原 follow_up。
        if let Some(narr) = self.pop_narrative() {
            self.pending.action = PendingAction::Narrative { follow_up };
            self.state = SessionState::ShowingNarrative;
            return narr;
        }
        self.pending.action = PendingAction::None;
        match follow_up {
            Some(NarrativeFollowUp::Outcome(base)) => {
                self.state = SessionState::Exploring;
                base
            }
            Some(NarrativeFollowUp::AdvanceAfterJudgment { prelude, map_hint }) => {
                self.advance_with_hint(prelude, map_hint)
            }
            None => {
                self.state = SessionState::Exploring;
                Outcome::SceneDescription {
                    text: self.scene_description_text(),
                }
            }
        }
    }

    /// 「走不出去」：首次展示完整 `exit_attempt` 文本（记 [`LEAVE_ATTEMPT_TRIGGER`]），
    /// 之后简短提示；不移动，留在当前场景。
    pub(crate) fn attempt_leave(&mut self) -> Outcome {
        let ch = self.save.current_chapter.clone();
        let scene = self.save.current_scene.clone();
        if self.save.discovered.triggered(LEAVE_ATTEMPT_TRIGGER) {
            self.state = SessionState::Exploring;
            return Outcome::Message(Message::info(vec![
                "你试着朝镇外走，脚步却把你带回原处。".into(),
            ]));
        }
        let text = self
            .engine
            .scene_exit_attempt_text(&scene)
            .unwrap_or("")
            .to_string();
        let rel = narrative_snapshot_path(&ch, LEAVE_ATTEMPT_TRIGGER);
        if let Err(e) = self.store.snapshots().write(&rel, &text) {
            tracing::warn!("走不出去快照写入失败: chapter={ch}, error={e}");
        }
        self.save
            .viewed_narrative
            .entry(ch.clone())
            .or_default()
            .push(NarrativeSeen {
                id: LEAVE_ATTEMPT_TRIGGER.into(),
                snapshot: rel,
            });
        self.save.discovered.add_trigger(LEAVE_ATTEMPT_TRIGGER);
        self.persist();
        // follow_up 用 Ignored：走不出去后玩家原地未动，ack 时无需重发场景描述。
        self.pending.action = PendingAction::Narrative {
            follow_up: Some(NarrativeFollowUp::Outcome(Outcome::Ignored)),
        };
        self.state = SessionState::ShowingNarrative;
        Outcome::Narrative {
            label: "旁白".into(),
            text,
        }
    }
}
