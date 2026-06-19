//! Outcome → 转录 / 菜单 / 状态 的应用。

use darkbluff_core::engine::{
    ConfirmationAction, MenuKind, MenuOption, Message, MessageLevel, NoteView, Outcome,
};
use ratatui::style::{Modifier, Style};

use crate::markdown::{self, StyledLine};
use crate::theme;

use super::types::{StatusKind, StatusLine};
use super::{ActiveMenu, App, NotePanel, NoteTab, Notice};

/// 转录最多保留的行数（FIFO 滚动）。
const MAX_TRANSCRIPT: usize = 512;

impl App {
    /// 把一帧引擎产出映射到转录/菜单/状态。
    pub(super) fn process_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Dialogue { header, body, notes } => self.apply_dialogue(header, body, notes),
            Outcome::ChapterIntro { text } => self.apply_chapter_card("▌ Intro", &text),
            Outcome::ChapterOutro { text } => self.apply_chapter_card("▌ Outro", &text),
            Outcome::Narrative { label, text } => self.apply_narrative(label, &text),
            Outcome::Notes(notes) => self.apply_notes(notes),
            Outcome::EndingReached { title, found, total } => {
                self.apply_ending(title, found, total)
            }
            Outcome::Message(message) => self.apply_message(message),
            Outcome::MenuRequested { kind, options, .. } => self.apply_menu(kind, options),
            Outcome::ConfirmationRequested { action, .. } => self.apply_confirmation(action),
            Outcome::QuitRequested => self.running = false,
            Outcome::Ignored => {}
        }
    }

    fn apply_dialogue(&mut self, header: String, body: String, notes: Vec<String>) {
        self.push_blank();
        self.push_line(header, header_style(theme::MAUVE));
        self.push_md(&body);
        if !notes.is_empty() {
            self.set_status(StatusKind::Hint, notes.join("  ·  "));
        }
    }

    fn apply_chapter_card(&mut self, title: &str, text: &str) {
        self.push_blank();
        self.push_line(title.into(), header_style(theme::LAVENDER));
        self.push_md(text);
        self.set_status(StatusKind::Info, "Press Enter to continue".into());
    }

    /// 心声 / 记忆碎片 / 旁白（走不出去）：PINK 前缀 + 正文，区别于对话与过场。
    fn apply_narrative(&mut self, label: String, text: &str) {
        self.push_blank();
        self.push_line(format!("▌ {label}"), header_style(theme::PINK));
        self.push_md(text);
        self.set_status(StatusKind::Info, "Press Enter to continue".into());
    }

    fn apply_notes(&mut self, notes: NoteView) {
        // 默认聚焦首个有内容的标签；全空则回叙事。
        let tab = [NoteTab::Narrative, NoteTab::Dialogue, NoteTab::Judgment, NoteTab::Voice]
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
        self.push_line(format!("✦ Ending · {title}"), header_style(theme::MAUVE));
        self.push_line(
            format!("Endings discovered  {found}/{total}"),
            Style::default().fg(theme::SUBTEXT0),
        );
        self.set_status(StatusKind::Info, "Press Enter to return".into());
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
        // 多行（如 help）进转录可读可滚；单行瞬时反馈进输入框右侧状态。
        if message.lines.len() > 1 {
            self.push_blank();
            let style = Style::default().fg(theme::SUBTEXT0);
            for line in message.lines {
                self.push_line(line, style);
            }
        } else {
            let kind = match message.level {
                MessageLevel::Info => StatusKind::Info,
                MessageLevel::Warning => StatusKind::Warn,
                MessageLevel::Error => StatusKind::Error,
            };
            self.set_status(kind, message.lines.into_iter().next().unwrap_or_default());
        }
    }

    fn apply_menu(&mut self, kind: MenuKind, options: Vec<MenuOption>) {
        self.confirmation = None;
        self.menu = Some(ActiveMenu {
            kind,
            options,
            selected: 0,
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
    }

    pub(super) fn push_md(&mut self, body: &str) {
        for sl in markdown::render(body) {
            self.push_line(sl.text, sl.style);
        }
    }

    pub(super) fn push_blank(&mut self) {
        self.push_line(String::new(), Style::default());
    }

    pub(super) fn set_status(&mut self, kind: StatusKind, text: String) {
        self.status = Some(StatusLine { kind, text });
    }
}

/// 转录里的小标题样式（彩色 + 粗体）。
fn header_style(color: ratatui::style::Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}
