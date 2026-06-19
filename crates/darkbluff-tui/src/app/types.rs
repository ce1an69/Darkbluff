//! TUI 视图与控制器共用的数据类型。

use std::path::PathBuf;

use darkbluff_core::engine::NoteView;

#[derive(Debug, Clone, Default)]
pub struct TuiOptions {
    pub no_motion: bool,
    pub save_dir: Option<PathBuf>,
}

/// 右侧场景面板里单个 NPC 的展示数据。
#[derive(Debug, Clone)]
pub struct NpcInfo {
    pub name: String,
    pub id: String,
    pub topics: Vec<NpcTopic>,
}

#[derive(Debug, Clone)]
pub struct NpcTopic {
    pub label: String,
    pub available: bool,
}

/// 输入框右侧的瞬时状态（错误/提示/引导）。下一次按键即清除。
#[derive(Debug, Clone)]
pub struct StatusLine {
    pub kind: StatusKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy)]
pub enum StatusKind {
    Info,
    Warn,
    Error,
    Hint,
}

/// 斜杠补全浮层。
#[derive(Debug, Clone)]
pub struct Suggestions {
    pub kind: SuggestKind,
    pub items: Vec<Suggestion>,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestKind {
    Command,
    Character,
    Scene,
    Topic,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub display: String,
    pub desc: String,
    /// 选中后替换行尾 token 的文本（已含尾随空格）。
    pub insert: String,
}

impl Suggestions {
    /// 空候选返回 None（浮层不显示）。
    pub(super) fn new(kind: SuggestKind, items: Vec<Suggestion>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self {
                kind,
                items,
                selected: 0,
            })
        }
    }
}

/// 笔记独立面板：持有引擎产出的 [`NoteView`] 与当前标签页。
#[derive(Debug, Clone)]
pub struct NotePanel {
    pub view: NoteView,
    pub tab: NoteTab,
}

/// 笔记标签页（叙事 / 对话 / 审判 / 心声）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTab {
    Narrative,
    Dialogue,
    Judgment,
    Voice,
}

impl NoteTab {
    pub const ALL: [NoteTab; 4] = [
        NoteTab::Narrative,
        NoteTab::Dialogue,
        NoteTab::Judgment,
        NoteTab::Voice,
    ];
    pub fn label(self) -> &'static str {
        match self {
            NoteTab::Narrative => "叙事",
            NoteTab::Dialogue => "对话",
            NoteTab::Judgment => "审判",
            NoteTab::Voice => "心声",
        }
    }
    pub fn from_digit(d: char) -> Option<Self> {
        match d {
            '1' => Some(NoteTab::Narrative),
            '2' => Some(NoteTab::Dialogue),
            '3' => Some(NoteTab::Judgment),
            '4' => Some(NoteTab::Voice),
            _ => None,
        }
    }
}

/// 顶部通知条（存档恢复 / 内容失效等 warning，瞬时展示至下一次按键）。
#[derive(Debug, Clone)]
pub struct Notice {
    pub warn: bool,
    pub text: String,
}
