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

/// 选择菜单的领域类型。渲染层可据此决定展示方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Title,
    Settings,
    AskCharacter,
    AskTopic,
    JudgeCharacter,
    MoveDestination,
    Checkpoint,
}

/// 需要二次确认的领域动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationAction {
    NewGame,
    Rollback { checkpoint_id: String },
}

/// 文本消息级别。渲染层可据此选择样式或通知策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// 一组领域消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub level: MessageLevel,
    pub lines: Vec<String>,
}

impl Message {
    pub fn info(lines: Vec<String>) -> Self {
        Self {
            level: MessageLevel::Info,
            lines,
        }
    }

    pub fn warning(lines: Vec<String>) -> Self {
        Self {
            level: MessageLevel::Warning,
            lines,
        }
    }

    pub fn error(lines: Vec<String>) -> Self {
        Self {
            level: MessageLevel::Error,
            lines,
        }
    }
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

/// 笔记中的心声 / 碎片条目。
#[derive(Debug, Clone)]
pub struct NoteVoice {
    pub chapter: String,
    pub label: String,
    pub text: String,
}

/// 笔记视图（当前 `chapter_path` 范围）。
#[derive(Debug, Clone, Default)]
pub struct NoteView {
    pub dialogues: Vec<NoteDialogue>,
    pub narratives: Vec<NoteNarrative>,
    pub judgments: Vec<NoteJudgment>,
    pub voices: Vec<NoteVoice>,
}

/// 引擎产出。渲染层（TUI/GUI）据此自行决定展示方式；测试据此断言。
#[derive(Debug, Clone)]
pub enum Outcome {
    /// 一组领域消息（提示 / 剧情等）。
    Message(Message),
    /// 展示对话全文 + 备注。
    Dialogue {
        header: String,
        body: String,
        notes: Vec<String>,
    },
    /// 请求渲染层打开选择菜单。
    MenuRequested {
        kind: MenuKind,
        prompt: String,
        options: Vec<MenuOption>,
    },
    /// 请求渲染层进行二次确认。
    ConfirmationRequested {
        action: ConfirmationAction,
        prompt: String,
    },
    /// 章节开场文本（等待 Ack）。
    ChapterIntro { text: String },
    /// 终章结局文本（等待 Ack）。
    ChapterOutro { text: String },
    /// 场景描述（进入 / 切换 / 回溯场景时展示）：由渲染层正常 markdown 渲染，不阻塞、不引用包裹。
    SceneDescription { text: String },
    /// 叙事触发器文本（心声 / 记忆碎片 / 旁白，等待 Ack）。
    Narrative { label: String, text: String },
    /// 笔记视图。
    Notes(NoteView),
    /// 结局界面。
    EndingReached {
        title: String,
        found: usize,
        total: usize,
    },
    /// 退出。
    QuitRequested,
    /// 静默忽略（菜单态下的非法输入）。
    Ignored,
}

/// 菜单选择。TUI 可按序号选择，GUI 可直接按 option id 选择。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Index(usize),
    Id(String),
}

/// 输入。
#[derive(Debug, Clone)]
pub enum Input {
    /// 命令行文本（Exploring 态）。
    Text(String),
    /// 菜单选择。
    Select(Selection),
    /// 取消（Esc）。
    Cancel,
    /// 二次确认（true=确认）。
    Confirm(bool),
    /// 继续确认（intro/outro/ending，按任意键）。
    Ack,
    /// 退出：任意状态都走标准退出路径（持久化失败则留在游戏内提示），
    /// 供渲染层的退出快捷键（如 Ctrl+C）使用，避免绕过引擎直接结束。
    Quit,
    /// 强制退出：best-effort 持久化后无条件 [`Outcome::QuitRequested`]，
    /// 供 SIGTERM 等不可忽略的信号使用，保证进程始终可被终止（即使磁盘满存档失败）。
    ForceQuit,
}

/// 会话状态（与 architecture.md「状态机」对应；渲染层可据此选择可接受输入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Title,
    ChoosingSettings,
    ShowingIntro,
    Exploring,
    ChoosingAskCharacter,
    ChoosingAskTopic,
    ChoosingJudgeCharacter,
    ChoosingMove,
    ChoosingCheckpoint,
    Confirming,
    ShowingNarrative,
    ShowingOutro,
    Ending,
}
