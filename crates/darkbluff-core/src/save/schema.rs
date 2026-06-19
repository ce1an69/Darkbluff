//! 存档数据模型。
//!
//! 设计见 docs/save-system.md「存档结构」。采用「存储事实为权威」模型：`collected_clues`
//! / `viewed_dialogues` / `judgments_made` 直接存储；`checkpoints` 只记录三个权威数组在
//! 创建时刻的长度，回滚按长度截断。存档层是纯字符串模型，不依赖 [`crate::content`]。
//!
//! 兼容性：`#[serde(default)]` 使缺失字段以默认值填充、未知字段被忽略（向前/向后兼容）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::world::World;

/// 当前存档结构版本号。
pub const CURRENT_VERSION: u32 = 1;

/// 单存档权威状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Save {
    pub version: u32,
    pub timestamp: String,
    pub current_chapter: String,
    pub current_scene: String,
    pub current_world: World,
    /// 已收集线索 id（按章节分组，权威存储）。
    pub collected_clues: HashMap<String, Vec<String>>,
    /// 已查看对话索引 + 快照路径（按章节分组，权威存储）。
    pub viewed_dialogues: HashMap<String, Vec<ViewedDialogue>>,
    /// 已审判记录（按章节分组）。
    pub judgments_made: HashMap<String, Vec<JudgmentMade>>,
    /// 已展示的章节开场文本快照路径（chapter → 相对路径）。
    pub viewed_intros: HashMap<String, String>,
    /// 已展示的终章结局文本快照路径（chapter → 相对路径）。
    pub viewed_outros: HashMap<String, String>,
    /// 已展示的叙事触发器（心声 / 记忆碎片）快照索引（按章节分组，展示记录；
    /// 不进检查点长度截断，跨章回滚随章节清理，与 viewed_intros 同构）。
    pub viewed_narrative: HashMap<String, Vec<NarrativeSeen>>,
    /// 当前流程经过的章节路径。
    pub chapter_path: Vec<String>,
    /// 自动创建的检查点（全局追加列表）。
    pub checkpoints: Vec<Checkpoint>,
    /// append-only 探索记忆，任何回滚都不截断。
    pub discovered: Discovered,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            timestamp: String::new(),
            current_chapter: String::new(),
            current_scene: String::new(),
            current_world: World::default(),
            collected_clues: HashMap::new(),
            viewed_dialogues: HashMap::new(),
            judgments_made: HashMap::new(),
            viewed_intros: HashMap::new(),
            viewed_outros: HashMap::new(),
            viewed_narrative: HashMap::new(),
            chapter_path: Vec::new(),
            checkpoints: Vec::new(),
            discovered: Discovered::default(),
        }
    }
}

impl Save {
    /// 新游戏初始化的空存档（不含 chapter_start 检查点，由调用方按时机创建）。
    pub fn new_game(first_chapter: &str, starting_scene: &str, timestamp: String) -> Self {
        let mut discovered = Discovered::default();
        discovered.add_chapter(first_chapter);
        Self {
            version: CURRENT_VERSION,
            timestamp,
            current_chapter: first_chapter.to_string(),
            current_scene: starting_scene.to_string(),
            current_world: World::Surface,
            chapter_path: vec![first_chapter.to_string()],
            discovered,
            ..Self::default()
        }
    }

    /// 取某章已收集线索的可变引用（缺失则插入空数组）。
    pub fn clues_mut(&mut self, chapter: &str) -> &mut Vec<String> {
        self.collected_clues.entry(chapter.to_string()).or_default()
    }
    /// 取某章已查看对话的可变引用。
    pub fn views_mut(&mut self, chapter: &str) -> &mut Vec<ViewedDialogue> {
        self.viewed_dialogues
            .entry(chapter.to_string())
            .or_default()
    }
    /// 取某章已审判记录的可变引用。
    pub fn judgments_mut(&mut self, chapter: &str) -> &mut Vec<JudgmentMade> {
        self.judgments_made.entry(chapter.to_string()).or_default()
    }

    /// 当前章节是否已收集某线索。
    pub fn has_clue(&self, chapter: &str, clue: &str) -> bool {
        self.collected_clues
            .get(chapter)
            .map_or(false, |v| v.iter().any(|c| c == clue))
    }
    /// 当前章节是否已审判某审判点。
    pub fn judged(&self, chapter: &str, judgment: &str) -> bool {
        self.judgments_made
            .get(chapter)
            .map_or(false, |v| v.iter().any(|j| j.judgment == judgment))
    }
}

/// 已查看对话条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewedDialogue {
    pub character: String,
    pub topic: String,
    pub world: World,
    /// 快照相对路径（基准为存档根目录 `save/`）。
    pub snapshot: String,
}

/// 已审判记录条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JudgmentMade {
    pub judgment: String,
    /// 审判剧情快照相对路径。
    pub result_snapshot: String,
}

/// 已展示的叙事触发器（心声 / 记忆碎片）记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrativeSeen {
    /// 触发器 id（同时进 `discovered.triggers`，发布冻结）。
    pub id: String,
    /// 快照相对路径（基准为存档根目录 `save/`）。
    pub snapshot: String,
}

/// 检查点种类。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    /// 进入章节时。
    ChapterStart,
    /// 玩家执行 `judge` 审判时。
    BeforeJudgment,
}

/// 检查点记录的三个权威数组长度。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CkptState {
    #[serde(default)]
    pub clues_len: usize,
    #[serde(default)]
    pub views_len: usize,
    #[serde(default)]
    pub judgments_len: usize,
}

/// 检查点。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    pub id: String,
    pub chapter: String,
    pub scene: String,
    pub world: World,
    pub kind: CheckpointKind,
    pub timestamp: String,
    pub state: CkptState,
}

impl Checkpoint {
    /// 基于本章当前权威数组长度生成检查点状态。
    pub fn state_of(save: &Save, chapter: &str) -> CkptState {
        CkptState {
            clues_len: save.collected_clues.get(chapter).map_or(0, Vec::len),
            views_len: save.viewed_dialogues.get(chapter).map_or(0, Vec::len),
            judgments_len: save.judgments_made.get(chapter).map_or(0, Vec::len),
        }
    }
}

/// append-only 探索记忆。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Discovered {
    /// 曾到过的章节（有序去重，保留首次到达顺序）。
    #[serde(default)]
    pub chapters: Vec<String>,
    /// 曾达成的结局（set 语义）。
    #[serde(default)]
    pub endings: Vec<String>,
    /// 曾问过的话题（chapter → `{character}.{topic}` 列表，去重）。
    #[serde(default)]
    pub topics: HashMap<String, Vec<String>>,
    /// 曾触发的叙事触发器 id（心声 / 碎片 / 走不出去），去重；任何回滚都不截断。
    #[serde(default)]
    pub triggers: Vec<String>,
}

impl Discovered {
    /// 追加章节（有序去重）。
    pub fn add_chapter(&mut self, chapter: &str) {
        Self::push_dedup(&mut self.chapters, chapter);
    }
    /// 追加结局（去重）。
    pub fn add_ending(&mut self, ending: &str) {
        Self::push_dedup(&mut self.endings, ending);
    }
    /// 追加话题（按章节分组去重）。
    pub fn add_topic(&mut self, chapter: &str, character: &str, topic: &str) {
        let entry = format!("{character}.{topic}");
        Self::push_dedup(self.topics.entry(chapter.to_string()).or_default(), &entry);
    }
    /// 追加已触发的叙事触发器 id（去重）。
    pub fn add_trigger(&mut self, id: &str) {
        Self::push_dedup(&mut self.triggers, id);
    }
    /// 某叙事触发器是否已触发过。
    pub fn triggered(&self, id: &str) -> bool {
        self.triggers.iter().any(|t| t == id)
    }

    fn push_dedup(vec: &mut Vec<String>, item: &str) {
        if !vec.iter().any(|v| v == item) {
            vec.push(item.to_string());
        }
    }
}

/// 在已存在的检查点列表中保证 id 唯一：若 `base` 冲突则追加 `_2` / `_3` …。
pub fn unique_checkpoint_id(base: &str, existing: &[Checkpoint]) -> String {
    let mut id = base.to_string();
    let mut n = 2;
    while existing.iter().any(|c| c.id == id) {
        id = format!("{base}_{n}");
        n += 1;
    }
    id
}

/// 检查点 id 基名：进入章节。
pub fn chapter_start_id_base(chapter: &str) -> String {
    format!("ckpt_{chapter}_start")
}
/// 检查点 id 基名：审判前。
pub fn before_judgment_id_base(judgment_id: &str) -> String {
    format!("ckpt_before_{judgment_id}")
}

/// 设置文件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub motion: Motion,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            motion: Motion::default(),
        }
    }
}

/// 动画偏好。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    Full,
    Reduced,
    Off,
}

impl Motion {
    /// 设置菜单 option id（唯一来源，供引擎构建菜单与渲染层回查）。
    pub fn menu_id(self) -> &'static str {
        match self {
            Motion::Full => "motion_full",
            Motion::Reduced => "motion_reduced",
            Motion::Off => "motion_off",
        }
    }

    /// 由设置菜单 option id 反查 Motion。
    pub fn from_menu_id(id: &str) -> Option<Motion> {
        match id {
            "motion_full" => Some(Motion::Full),
            "motion_reduced" => Some(Motion::Reduced),
            "motion_off" => Some(Motion::Off),
            _ => None,
        }
    }

    /// 中文标签（数据语言；随存档/设置面向玩家）。
    pub fn zh_label(self) -> &'static str {
        match self {
            Motion::Full => "动画：完整",
            Motion::Reduced => "动画：减少",
            Motion::Off => "动画：关闭",
        }
    }

    /// 英文标签（界面 chrome 语言）。
    pub fn en_label(self) -> &'static str {
        match self {
            Motion::Full => "Motion: Full",
            Motion::Reduced => "Motion: Reduced",
            Motion::Off => "Motion: Off",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_roundtrip() {
        let mut save = Save::new_game("c1", "tavern", "2026-06-16T12:00:00Z".into());
        save.clues_mut("c1").push("wolf_alibi".into());
        save.views_mut("c1").push(ViewedDialogue {
            character: "wolf".into(),
            topic: "whereabouts".into(),
            world: World::Shadow,
            snapshot: "snapshots/c1/wolf.whereabouts.shadow.md".into(),
        });
        save.judgments_mut("c1").push(JudgmentMade {
            judgment: "judge_wolf".into(),
            result_snapshot: "snapshots/c1/judge_wolf.md".into(),
        });
        save.discovered.add_topic("c1", "wolf", "whereabouts");

        let json = serde_json::to_string_pretty(&save).unwrap();
        let mut back: Save = serde_json::from_str(&json).unwrap();
        assert_eq!(back.current_chapter, "c1");
        assert_eq!(back.current_world, World::Surface);
        assert_eq!(back.clues_mut("c1").clone(), vec!["wolf_alibi".to_string()]);
        assert!(back.judged("c1", "judge_wolf"));
        assert!(back.has_clue("c1", "wolf_alibi"));
        assert_eq!(
            back.discovered.topics.get("c1").unwrap(),
            &vec!["wolf.whereabouts".to_string()]
        );
    }

    #[test]
    fn save_tolerates_missing_and_unknown_fields() {
        // 只有 version 与 current_chapter，其余缺失；含未知字段
        let json = r#"{"version":1,"current_chapter":"c2","unknown_field":42}"#;
        let save: Save = serde_json::from_str(json).unwrap();
        assert_eq!(save.current_chapter, "c2");
        assert_eq!(save.current_world, World::Surface); // 默认
        assert!(save.chapter_path.is_empty()); // 默认
        assert!(save.checkpoints.is_empty());
    }

    #[test]
    fn discovered_dedup() {
        let mut d = Discovered::default();
        d.add_chapter("c1");
        d.add_chapter("c2");
        d.add_chapter("c1"); // 去重，保持顺序
        assert_eq!(d.chapters, vec!["c1", "c2"]);
        d.add_ending("c_truth");
        d.add_ending("c_truth");
        assert_eq!(d.endings, vec!["c_truth"]);
        d.add_topic("c1", "wolf", "whereabouts");
        d.add_topic("c1", "wolf", "whereabouts"); // 去重
        assert_eq!(d.topics.get("c1").unwrap().len(), 1);
    }

    #[test]
    fn checkpoint_state_of() {
        let mut save = Save::new_game("c1", "s", "t".into());
        save.clues_mut("c1")
            .extend(["a", "b"].iter().map(|s| s.to_string()));
        save.views_mut("c1").push(ViewedDialogue {
            character: "w".into(),
            topic: "t".into(),
            world: World::Surface,
            snapshot: "x".into(),
        });
        let st = Checkpoint::state_of(&save, "c1");
        assert_eq!(st.clues_len, 2);
        assert_eq!(st.views_len, 1);
        assert_eq!(st.judgments_len, 0);
    }

    #[test]
    fn unique_checkpoint_id_handles_collision() {
        let mut existing = vec![Checkpoint {
            id: "ckpt_c1_start".into(),
            chapter: "c1".into(),
            scene: "s".into(),
            world: World::Surface,
            kind: CheckpointKind::ChapterStart,
            timestamp: "t".into(),
            state: CkptState::default(),
        }];
        let id1 = unique_checkpoint_id(&chapter_start_id_base("c1"), &existing);
        assert_eq!(id1, "ckpt_c1_start_2");
        existing.push(Checkpoint {
            id: id1,
            chapter: "c1".into(),
            scene: "s".into(),
            world: World::Surface,
            kind: CheckpointKind::ChapterStart,
            timestamp: "t2".into(),
            state: CkptState::default(),
        });
        let id2 = unique_checkpoint_id(&chapter_start_id_base("c1"), &existing);
        assert_eq!(id2, "ckpt_c1_start_3");
    }

    #[test]
    fn settings_default_full() {
        let s = Settings::default();
        assert_eq!(s.motion, Motion::Full);
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"full\""));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn motion_menu_id_roundtrip() {
        for motion in [Motion::Full, Motion::Reduced, Motion::Off] {
            assert_eq!(Motion::from_menu_id(motion.menu_id()), Some(motion));
        }
        // menu_id ↔ zh_label ↔ en_label 三者一一对应、互不串味。
        assert_eq!(Motion::Full.menu_id(), "motion_full");
        assert_eq!(Motion::Reduced.menu_id(), "motion_reduced");
        assert_eq!(Motion::Off.menu_id(), "motion_off");
        assert_eq!(Motion::Full.en_label(), "Motion: Full");
        assert_eq!(Motion::Off.zh_label(), "动画：关闭");
        assert_eq!(Motion::from_menu_id("bogus"), None);
    }
}
