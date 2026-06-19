//! TUI 视图与控制器共用的数据类型。

use std::path::PathBuf;

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
