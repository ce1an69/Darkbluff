//! 会话的输入、产出与状态类型。
//!
//! 设计见 docs/architecture.md「状态机」。这些类型与 [`crate::engine::state::Session`]
//! 解耦：TUI 据 [`Outcome`] 渲染、据 [`Input`] 驱动；测试据此断言。

use crate::world::World;

/// 菜单选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuOption {
    pub id: String,
    pub label: String,
}

/// 笔记中的对话条目。
#[derive(Debug, Clone)]
pub struct NoteDialogue {
    pub chapter: String,
    pub character_id: String,
    pub character_name: String,
    pub topic_id: String,
    pub topic_label: String,
    pub world: World,
    pub text: String,
}

/// 笔记中的叙事条目（intro/outro）。
#[derive(Debug, Clone)]
pub struct NoteNarrative {
    pub chapter: String,
    pub title: String,
    pub is_outro: bool,
    pub text: String,
}

/// 笔记中的审判条目。
#[derive(Debug, Clone)]
pub struct NoteJudgment {
    pub chapter: String,
    pub judgment_id: String,
    pub target_name: String,
    pub text: String,
}

/// 笔记视图（当前 `chapter_path` 范围）。
#[derive(Debug, Clone, Default)]
pub struct NoteView {
    pub dialogues: Vec<NoteDialogue>,
    pub narratives: Vec<NoteNarrative>,
    pub judgments: Vec<NoteJudgment>,
}

/// 引擎产出。TUI 据此渲染；测试据此断言。
#[derive(Debug, Clone)]
pub enum Outcome {
    /// 展示若干文本行（提示 / 剧情等），随后回到当前交互态。
    Show(Vec<String>),
    /// 展示对话全文 + 备注。
    Dialogue {
        header: String,
        body: String,
        notes: Vec<String>,
    },
    /// 展示一个选择菜单。
    Menu {
        title: String,
        options: Vec<MenuOption>,
    },
    /// 破坏性操作二次确认。
    Confirm {
        prompt: String,
    },
    /// 章节开场文本（等待 Ack）。
    Intro {
        text: String,
    },
    /// 终章结局文本（等待 Ack）。
    Outro {
        text: String,
    },
    /// 笔记视图。
    Note(NoteView),
    /// 结局界面。
    Ending {
        title: String,
        found: usize,
        total: usize,
    },
    /// 回到标题界面。
    Title,
    /// 退出。
    Quit,
    /// 静默忽略（菜单态下的非法输入）。
    Ignored,
}

/// 输入。
#[derive(Debug, Clone)]
pub enum Input {
    /// 命令行文本（Exploring 态）。
    Text(String),
    /// 菜单编号选择。
    Pick(usize),
    /// 取消（Esc）。
    Cancel,
    /// 二次确认（true=确认）。
    Confirm(bool),
    /// 继续确认（intro/outro/ending，按任意键）。
    Ack,
}

/// 应用状态（与 architecture.md「状态机」对应；Title/ViewingNote 的渲染由 UI 层处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    Title,
    ShowingIntro,
    Exploring,
    ChoosingAskCharacter,
    ChoosingAskTopic,
    ChoosingJudgeCharacter,
    ChoosingMove,
    ChoosingCheckpoint,
    Confirming,
    ShowingOutro,
    Ending,
}
