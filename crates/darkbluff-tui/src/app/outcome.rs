//! Outcome → 转录 / 菜单 / 状态 的应用。

use darkbluff_core::engine::{
    ConfirmationAction, MenuKind, MenuOption, Message, MessageLevel, NoteView, Outcome,
};
use ratatui::style::{Modifier, Style};

use crate::markdown::{self, StyledLine};
use crate::theme;

use super::{ActiveMenu, App, NotePanel, NoteTab, Notice};

/// 转录最多保留的行数（FIFO 滚动）。
const MAX_TRANSCRIPT: usize = 512;

impl App {
    /// 把一帧引擎产出映射到转录/菜单/状态。
    pub(super) fn process_outcome(&mut self, outcome: Outcome) {
        let narrative = outcome.is_narrative();
        // 哪些产出会推新内容进转录且应「回到底部」：先判好（match 会消费 outcome）。
        // 场景描述是背景信息（同时显示在场景面板），不打断玩家当前的回看位置。
        let snaps = outcome_snaps_to_bottom(&outcome);
        let before = self.transcript_pushes;
        self.pending_tw_skip = 0;
        match outcome {
            Outcome::Dialogue {
                header,
                body,
                notes,
            } => self.apply_dialogue(header, body, notes),
            Outcome::ChapterIntro { text } => self.apply_chapter_card("▌ 开场", &text),
            Outcome::ChapterOutro { text } => self.apply_chapter_card("▌ 尾声", &text),
            Outcome::SceneDescription { text } => self.apply_scene_description(&text),
            Outcome::Narrative { label, text } => self.apply_narrative(label, &text),
            Outcome::Notes(notes) => self.apply_notes(notes),
            Outcome::EndingReached {
                title,
                found,
                total,
            } => self.apply_ending(title, found, total),
            Outcome::Message(message) => self.apply_message(message),
            Outcome::MenuRequested { kind, options, .. } => self.apply_menu(kind, options),
            Outcome::ConfirmationRequested { action, .. } => self.apply_confirmation(action),
            Outcome::QuitRequested => self.running = false,
            Outcome::Ignored => {}
        }
        if snaps {
            self.transcript_offset = 0;
        }
        if narrative && !self.motion.is_off() {
            let added = self.transcript_pushes.saturating_sub(before);
            if added > 0 {
                let skip = std::mem::take(&mut self.pending_tw_skip);
                self.start_typewriter(added, skip);
            }
        }
    }

    fn apply_dialogue(&mut self, header: String, body: String, notes: Vec<String>) {
        self.push_blank();
        self.push_line(header, header_style(theme::MAUVE));
        self.pending_tw_skip = 2; // blank + header 瞬显
        self.push_md(&body);
        if !notes.is_empty() {
            self.push_md(&quote_block(&notes.join("  ·  ")));
        }
    }

    fn apply_chapter_card(&mut self, title: &str, text: &str) {
        self.push_blank();
        self.push_line(title.into(), header_style(theme::LAVENDER));
        self.pending_tw_skip = 2; // blank + title 瞬显
        self.push_md(text);
    }

    /// 场景描述：正常 markdown 渲染进转录（标题 + 正文），不引用包裹、不阻塞。
    fn apply_scene_description(&mut self, text: &str) {
        self.push_blank();
        self.pending_tw_skip = 1; // blank 瞬显
        self.push_md(text);
    }

    /// 心声 / 记忆碎片 / 旁白（走不出去）：整段以引用块呈现，label 融入首行。
    fn apply_narrative(&mut self, label: String, text: &str) {
        self.push_blank();
        self.pending_tw_skip = 1; // blank 瞬显
        self.push_md(&narrative_quote(&label, text));
    }

    fn apply_notes(&mut self, notes: NoteView) {
        // 默认聚焦首个有内容的标签；全空则回叙事。
        let tab = [
            NoteTab::Narrative,
            NoteTab::Dialogue,
            NoteTab::Judgment,
            NoteTab::Voice,
        ]
        .into_iter()
        .find(|t| match *t {
            NoteTab::Narrative => !notes.narratives.is_empty(),
            NoteTab::Dialogue => !notes.dialogues.is_empty(),
            NoteTab::Judgment => !notes.judgments.is_empty(),
            NoteTab::Voice => !notes.voices.is_empty(),
        })
        .unwrap_or(NoteTab::Narrative);
        self.note_panel = Some(NotePanel { view: notes, tab });
    }

    fn apply_ending(&mut self, title: String, found: usize, total: usize) {
        self.push_blank();
        self.push_line(format!("✦ 结局 · {title}"), header_style(theme::MAUVE));
        self.push_line(
            format!("已发现结局  {found}/{total}"),
            Style::default().fg(theme::SUBTEXT0),
        );
    }

    fn apply_message(&mut self, message: Message) {
        // warning 级额外在顶部通知条提示（存档恢复 / 内容失效等）。
        if matches!(message.level, MessageLevel::Warning) {
            if let Some(first) = message.lines.first() {
                self.notice = Some(Notice {
                    warn: true,
                    text: first.clone(),
                });
            }
        }
        // 全部进转录：多行（help / 拼接警告）正常 markdown 渲染；单行瞬时反馈（gaze 动作 / 提示）
        // 包成引用块，错误级保留红色以示严重。
        if message.lines.len() > 1 {
            self.push_blank();
            self.push_md(&message.lines.join("\n"));
        } else {
            let line = message.lines.into_iter().next().unwrap_or_default();
            if matches!(message.level, MessageLevel::Error) {
                self.push_line(format!("│ {line}"), Style::default().fg(theme::RED));
            } else {
                self.push_md(&quote_block(&line));
            }
        }
    }

    fn apply_menu(&mut self, kind: MenuKind, options: Vec<MenuOption>) {
        // 进入菜单光标默认在第 0 行；设置菜单的当前值已写进 label，无需反查。
        let selected = 0;
        self.confirmation = None;
        self.menu = Some(ActiveMenu {
            kind,
            options,
            selected,
        });
    }

    fn apply_confirmation(&mut self, action: ConfirmationAction) {
        self.menu = None;
        self.confirmation = Some(action);
    }

    // ----- 转录 / 状态 小助手（也为控制器层复用） -----

    pub(super) fn push_line(&mut self, text: String, style: Style) {
        self.transcript.push_back(StyledLine { text, style });
        while self.transcript.len() > MAX_TRANSCRIPT {
            self.transcript.pop_front();
        }
        self.transcript_pushes += 1;
    }

    pub(super) fn push_md(&mut self, body: &str) {
        for sl in markdown::render(body) {
            self.push_line(sl.text, sl.style);
        }
    }

    pub(super) fn push_blank(&mut self) {
        self.push_line(String::new(), Style::default());
    }
}

/// 哪些产出会把新内容推进转录并应「回到底部」。
/// 场景描述属背景信息（同时显示在场景面板），不打断玩家当前的回看位置。
fn outcome_snaps_to_bottom(outcome: &Outcome) -> bool {
    matches!(
        outcome,
        Outcome::Dialogue { .. }
            | Outcome::ChapterIntro { .. }
            | Outcome::ChapterOutro { .. }
            | Outcome::Narrative { .. }
            | Outcome::EndingReached { .. }
            | Outcome::Message(..)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrative_quote_strips_markdown_heading() {
        // 心声文本里的 `# 标题` 经引用渲染后不应残留字面 `#`（regression: 走不出去场景）。
        let q = narrative_quote("旁白", "# 走不出去\n\n你朝桥对面走。");
        let rendered: Vec<String> =
            crate::markdown::render(&q).into_iter().map(|s| s.text).collect();
        assert_eq!(rendered[0], "│ 〔旁白〕走不出去");
        assert!(rendered.iter().any(|l| l.contains("你朝桥对面走。")));
    }

    #[test]
    fn snaps_to_bottom_excludes_scene_description() {
        let snap = [
            Outcome::Dialogue {
                header: "h".into(),
                body: "b".into(),
                notes: vec![],
            },
            Outcome::ChapterIntro { text: "t".into() },
            Outcome::ChapterOutro { text: "t".into() },
            Outcome::Narrative {
                label: "l".into(),
                text: "t".into(),
            },
            Outcome::EndingReached {
                title: "t".into(),
                found: 0,
                total: 1,
            },
            Outcome::Message(Message::info(vec!["x".into()])),
        ];
        let no_snap = [
            Outcome::SceneDescription { text: "t".into() },
            Outcome::MenuRequested {
                kind: MenuKind::Title,
                prompt: "p".into(),
                options: vec![],
            },
            Outcome::ConfirmationRequested {
                action: ConfirmationAction::NewGame,
                prompt: "p".into(),
            },
            Outcome::Ignored,
        ];
        for o in snap {
            assert!(outcome_snaps_to_bottom(&o), "{o:?} 应回底");
        }
        for o in no_snap {
            assert!(!outcome_snaps_to_bottom(&o), "{o:?} 不应回底");
        }
    }
}

/// 转录里的小标题样式（彩色 + 粗体）。
fn header_style(color: ratatui::style::Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// 去掉行首 markdown 标题/列表前缀，避免引用块里残留字面 `#`/`•`。
fn strip_md_prefix(line: &str) -> &str {
    line.trim_start_matches("### ")
        .trim_start_matches("## ")
        .trim_start_matches("# ")
        .trim_start_matches("- ")
        .trim_start_matches("* ")
}

/// 把任意文本包成 markdown 引用块（每行加 `> ` 前缀），供 [`App::push_md`] 渲染成竖线引用。
fn quote_block(text: &str) -> String {
    text.lines()
        .map(|l| format!("> {}", strip_md_prefix(l)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 心声引用：label 融入首行，正文每行加引用前缀；标题/列表标记一并剥除。
fn narrative_quote(label: &str, text: &str) -> String {
    let mut lines = text.lines();
    let head = strip_md_prefix(lines.next().unwrap_or(""));
    let mut out = format!("> 〔{label}〕{head}");
    for l in lines {
        out.push_str("\n> ");
        out.push_str(strip_md_prefix(l));
    }
    out
}
