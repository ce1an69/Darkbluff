//! 内容引擎核心：加载全部内容到内存并建立索引，提供统一查询接口。
//!
//! 设计见 docs/content-engine.md「查询接口（概念）」。所有文本（场景描述、对话、
//! intro/outro/审判剧情）在 [`ContentEngine::load`] 时预读进内存，之后查询不再依赖
//! [`DataSource`]。加载与建索引的细节在 [`crate::content::loader`]；引用完整性校验由
//! [`crate::content::checker`] 单独负责。`load` 遇到缺失文件降级为 `None`，由校验层报告。

use std::collections::{HashMap, HashSet};

use crate::content::condition::eval;
use crate::content::dialogue::DialogueBook;
use crate::content::loader::{load_all, LoadedContent};
use crate::content::models::{Chapter, Character, Clue, Judgment, Scene, Topic};
use crate::error::Result;
use crate::world::World;

/// 章节元信息（轻量视图，供 map 面板 / check 输出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterMeta {
    pub id: String,
    pub title: String,
    pub order: i64,
    pub ending: bool,
}

/// 内容引擎：持有全部已加载内容与索引，提供查询接口。
#[derive(Debug, Clone)]
pub struct ContentEngine {
    scenes: Vec<Scene>,
    scene_index: HashMap<String, usize>,
    characters: Vec<Character>,
    char_index: HashMap<String, usize>,
    chapters: Vec<Chapter>,
    chapter_index: HashMap<String, usize>,
    judgments: HashMap<String, Vec<Judgment>>,
    clues: HashMap<String, Vec<Clue>>,
    dialogues: HashMap<String, HashMap<String, DialogueBook>>,
    /// 全局场景描述 Markdown 路径 → 文本。
    scene_text: HashMap<String, String>,
    /// 章节场景覆盖：(chapter, scene, world) → 文本。
    chapter_scene_text: HashMap<(String, String, World), String>,
    /// 章节开场文本：chapter → 文本。
    intro_text: HashMap<String, String>,
    /// 终章结局文本：chapter → 文本。
    outro_text: HashMap<String, String>,
    /// 审判剧情文本：(chapter, judgment_id) → 文本。
    result_text: HashMap<(String, String), String>,
    /// 场景可达连接（含自动补全的反向连接与 one_way）。
    reachable: HashMap<String, Vec<String>>,
    /// 首章（唯一根节点）；校验层保证有且仅有一个，否则为 None。
    root_chapter: Option<String>,
}

impl ContentEngine {
    /// 扫描并加载全部内容（加载细节见 [`crate::content::loader::load_all`]）。
    pub fn load(src: &dyn crate::content::loader::DataSource) -> Result<Self> {
        let lc: LoadedContent = load_all(src)?;
        let LoadedContent {
            scenes,
            scene_index,
            characters,
            char_index,
            chapters,
            chapter_index,
            judgments,
            clues,
            dialogues,
            scene_text,
            chapter_scene_text,
            intro_text,
            outro_text,
            result_text,
            reachable,
            root_chapter,
        } = lc;
        Ok(Self {
            scenes,
            scene_index,
            characters,
            char_index,
            chapters,
            chapter_index,
            judgments,
            clues,
            dialogues,
            scene_text,
            chapter_scene_text,
            intro_text,
            outro_text,
            result_text,
            reachable,
            root_chapter,
        })
    }

    // ----- 章节 -----

    pub fn get_chapter(&self, id: &str) -> Option<&Chapter> {
        self.chapter_index.get(id).map(|&i| &self.chapters[i])
    }
    pub fn chapter_exists(&self, id: &str) -> bool {
        self.chapter_index.contains_key(id)
    }
    pub fn chapter_ids(&self) -> impl Iterator<Item = &str> {
        self.chapters.iter().map(|c| c.id.as_str())
    }
    pub fn list_chapters(&self) -> Vec<ChapterMeta> {
        let mut v: Vec<ChapterMeta> = self
            .chapters
            .iter()
            .map(|c| ChapterMeta {
                id: c.id.clone(),
                title: c.title.clone(),
                order: c.order,
                ending: c.ending,
            })
            .collect();
        v.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        v
    }
    pub fn first_chapter_id(&self) -> Option<&str> {
        self.root_chapter.as_deref()
    }
    pub fn ending_chapter_ids(&self) -> Vec<&str> {
        self.chapters
            .iter()
            .filter(|c| c.ending)
            .map(|c| c.id.as_str())
            .collect()
    }

    // ----- 场景 -----

    pub fn get_scene(&self, id: &str) -> Option<&Scene> {
        self.scene_index.get(id).map(|&i| &self.scenes[i])
    }
    pub fn scene_exists(&self, id: &str) -> bool {
        self.scene_index.contains_key(id)
    }
    /// 全部全局场景 id（原始顺序，含重复，供校验查重）。
    pub fn scene_ids(&self) -> impl Iterator<Item = &str> {
        self.scenes.iter().map(|s| s.id.as_str())
    }

    /// 取场景描述：优先章节覆盖，回退全局默认。
    pub fn get_scene_description(&self, chapter: &str, scene: &str, world: World) -> Option<&str> {
        let key = (chapter.to_string(), scene.to_string(), world);
        if let Some(t) = self.chapter_scene_text.get(&key) {
            return Some(t);
        }
        let scene_def = self.get_scene(scene)?;
        let path = match world {
            World::Surface => &scene_def.description.surface,
            World::Shadow => &scene_def.description.shadow,
        };
        self.scene_text.get(path).map(|s| s.as_str())
    }

    /// 场景可达连接（含自动补全的反向连接与 one_way）。
    pub fn get_reachable_scenes(&self, scene: &str) -> Vec<&str> {
        self.reachable
            .get(scene)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    // ----- 角色 -----

    pub fn get_character(&self, id: &str) -> Option<&Character> {
        self.char_index.get(id).map(|&i| &self.characters[i])
    }
    pub fn character_exists(&self, id: &str) -> bool {
        self.char_index.contains_key(id)
    }
    /// 全部全局角色 id（原始顺序，含重复，供校验查重）。
    pub fn character_ids(&self) -> impl Iterator<Item = &str> {
        self.characters.iter().map(|c| c.id.as_str())
    }
    /// 当前场景在场角色（`appears_in` 含该场景，或省略=本章所有场景）。
    pub fn get_characters_in_scene(&self, chapter: &str, scene: &str) -> Vec<&Character> {
        let Some(ch) = self.get_chapter(chapter) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for cc in &ch.characters {
            let in_scene = cc
                .appears_in
                .as_ref()
                .map_or(true, |list| list.iter().any(|s| s == scene));
            if in_scene {
                if let Some(c) = self.get_character(&cc.id) {
                    out.push(c);
                }
            }
        }
        out
    }

    // ----- 话题 / 对话 -----

    /// 某角色本章的话题原始数据（可见性由引擎层用 FactSet 求值）。
    pub fn get_topics(&self, chapter: &str, character: &str) -> &[Topic] {
        let Some(ch) = self.get_chapter(chapter) else {
            return &[];
        };
        let Some(cc) = ch.characters.iter().find(|c| c.id == character) else {
            return &[];
        };
        &cc.topics
    }

    pub fn get_dialogue(
        &self,
        chapter: &str,
        character: &str,
        topic: &str,
        world: World,
    ) -> Option<&str> {
        self.dialogues
            .get(chapter)?
            .get(character)?
            .get(topic, world)
    }

    /// 话题在指定角色对话文件中是否存在任意世界版本。
    pub fn dialogue_topic_exists(&self, chapter: &str, character: &str, topic: &str) -> bool {
        let Some(book) = self.dialogues.get(chapter).and_then(|m| m.get(character)) else {
            return false;
        };
        book.contains_topic(topic)
    }

    // ----- 审判 -----

    pub fn get_judgments(&self, chapter: &str) -> &[Judgment] {
        self.judgments
            .get(chapter)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    /// 取审判对象（角色 id）对应的审判点（一章内一个角色一个审判点）。
    pub fn get_judgment_for_character(&self, chapter: &str, character: &str) -> Option<&Judgment> {
        self.get_judgments(chapter)
            .iter()
            .find(|j| j.target == character)
    }
    pub fn get_result_text(&self, chapter: &str, judgment_id: &str) -> Option<&str> {
        self.result_text
            .get(&(chapter.to_string(), judgment_id.to_string()))
            .map(|s| s.as_str())
    }

    // ----- 线索 -----

    pub fn get_clues(&self, chapter: &str) -> &[Clue] {
        self.clues
            .get(chapter)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    // ----- 叙事文本 -----

    pub fn get_intro_text(&self, chapter: &str) -> Option<&str> {
        self.intro_text.get(chapter).map(|s| s.as_str())
    }
    pub fn get_outro_text(&self, chapter: &str) -> Option<&str> {
        self.outro_text.get(chapter).map(|s| s.as_str())
    }

    // ----- 章节跳转 -----

    /// 解析非终章的下一章目标。终章返回 `None`。
    ///
    /// 运行时降级（见 content-engine.md）：branch target 失效时跳过该分支继续匹配；
    /// `default` 也失效时返回 `None`（由上层报错）。
    pub fn get_next_chapter(&self, chapter: &str, facts: &HashSet<String>) -> Option<&str> {
        let ch = self.get_chapter(chapter)?;
        if ch.ending {
            return None;
        }
        let next = ch.next.as_ref()?;
        for b in &next.branches {
            if eval(&b.when, facts) && self.chapter_exists(&b.target) {
                return Some(&b.target);
            }
        }
        if self.chapter_exists(&next.default) {
            Some(&next.default)
        } else {
            None
        }
    }

    /// 某章节声明的全部跳转目标（default + 各 branch target），供章节图/校验使用。
    pub fn next_targets(&self, chapter: &str) -> Vec<&str> {
        let Some(ch) = self.get_chapter(chapter) else {
            return Vec::new();
        };
        let Some(next) = &ch.next else {
            return Vec::new();
        };
        let mut v: Vec<&str> = vec![&next.default];
        for b in &next.branches {
            v.push(&b.target);
        }
        v
    }
}
