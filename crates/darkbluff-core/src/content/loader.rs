//! 数据源抽象与文件加载助手。
//!
//! 设计见 docs/content-engine.md「内嵌/外置双模式」。通过 [`DataSource`] trait 屏蔽
//! 「文件系统（开发）」与「内嵌（发布，TODO）」两种来源的差异。所有文本在加载阶段
//! 预读进内存，[`crate::content::engine::ContentEngine`] 持有全部内容、不再依赖数据源。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::de::DeserializeOwned;

use crate::error::{AppError, Result};

/// 内容数据来源抽象（相对路径以 `data/` 为根，使用 `/` 分隔）。
pub trait DataSource {
    /// 列出某目录下的直接子条目名称（已排序，不含路径前缀）。目录不存在时返回空。
    fn list_dir(&self, rel_dir: &str) -> Result<Vec<String>>;
    /// 读取文件文本内容。
    fn read(&self, rel_path: &str) -> Result<String>;
    /// 路径是否存在（文件或目录）。
    fn exists(&self, rel_path: &str) -> bool;
}

/// 文件系统数据源（开发模式）：从磁盘 `data/` 目录读取。
pub struct FilesystemSource {
    root: PathBuf,
}

impl FilesystemSource {
    /// 以 `root` 作为 `data/` 根；要求该目录存在。
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if !root.is_dir() {
            return Err(AppError::Content(format!(
                "数据目录不存在或不是目录: {}",
                root.display()
            )));
        }
        Ok(Self { root })
    }

    fn join(&self, rel: &str) -> PathBuf {
        // '/' 在 PathBuf::join 中跨平台可用
        self.root.join(rel)
    }
}

impl DataSource for FilesystemSource {
    fn list_dir(&self, rel_dir: &str) -> Result<Vec<String>> {
        let dir = self.join(rel_dir);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy().into_owned();
            // 忽略隐藏文件
            if !name.starts_with('.') {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    fn read(&self, rel_path: &str) -> Result<String> {
        let path = self.join(rel_path);
        std::fs::read_to_string(&path).map_err(|e| {
            AppError::Content(format!("读取数据文件失败 {}: {e}", path.display()))
        })
    }

    fn exists(&self, rel_path: &str) -> bool {
        self.join(rel_path).exists()
    }
}

/// 内存数据源：仅供测试使用，以「相对路径 → 文本」映射驱动。
#[derive(Debug, Clone, Default)]
pub struct InMemorySource {
    files: HashMap<String, String>,
}

impl InMemorySource {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(mut self, rel: impl Into<String>, text: impl Into<String>) -> Self {
        self.files.insert(rel.into(), text.into());
        self
    }
}

impl DataSource for InMemorySource {
    fn list_dir(&self, rel_dir: &str) -> Result<Vec<String>> {
        let prefix = if rel_dir.is_empty() || rel_dir == "/" {
            String::new()
        } else {
            format!("{rel_dir}/")
        };
        let mut names = std::collections::BTreeSet::new();
        for path in self.files.keys() {
            if let Some(rest) = path.strip_prefix(&prefix) {
                if let Some(idx) = rest.find('/') {
                    names.insert(rest[..idx].to_string());
                } else {
                    names.insert(rest.to_string());
                }
            }
        }
        Ok(names.into_iter().collect())
    }

    fn read(&self, rel_path: &str) -> Result<String> {
        self.files
            .get(rel_path)
            .cloned()
            .ok_or_else(|| AppError::Content(format!("内存数据源缺少文件: {rel_path}")))
    }

    fn exists(&self, rel_path: &str) -> bool {
        // 文件本身，或作为某文件路径前缀的目录
        if self.files.contains_key(rel_path) {
            return true;
        }
        let prefix = format!("{rel_path}/");
        self.files.keys().any(|p| p.starts_with(&prefix))
    }
}

/// 读取并反序列化 YAML 文件。
pub fn read_yaml<T: DeserializeOwned + 'static>(src: &dyn DataSource, rel: &str) -> Result<T> {
    let text = src.read(rel)?;
    serde_yml::from_str(&text).map_err(AppError::from)
}

/// 去掉 `.yaml` 后缀。
pub fn strip_yaml_ext(name: &str) -> Option<&str> {
    name.strip_suffix(".yaml")
}

/// 去掉 `.md` 后缀。
pub fn strip_md_ext(name: &str) -> Option<&str> {
    name.strip_suffix(".md")
}

/// 解析章节场景覆盖文件名（`{scene}.surface.md` / `{scene}.shadow.md`）。
pub fn parse_scene_override_name(name: &str) -> Option<(String, crate::world::World)> {
    use crate::world::World;
    let (stem, world) = if let Some(s) = name.strip_suffix(".surface.md") {
        (s, World::Surface)
    } else if let Some(s) = name.strip_suffix(".shadow.md") {
        (s, World::Shadow)
    } else {
        return None;
    };
    Some((stem.to_string(), world))
}

// ----- 全量加载 -----

use std::collections::HashSet;

use crate::content::dialogue::{parse_dialogue, DialogueBook};
use crate::content::models::{Chapter, Character, Clue, Judgment, Scene};
use crate::world::World;

/// `ContentEngine::load` 的全部产物（已建好索引、可达连接与根节点）。
#[derive(Debug, Clone)]
pub(crate) struct LoadedContent {
    pub(crate) scenes: Vec<Scene>,
    pub(crate) scene_index: HashMap<String, usize>,
    pub(crate) characters: Vec<Character>,
    pub(crate) char_index: HashMap<String, usize>,
    pub(crate) chapters: Vec<Chapter>,
    pub(crate) chapter_index: HashMap<String, usize>,
    pub(crate) judgments: HashMap<String, Vec<Judgment>>,
    pub(crate) clues: HashMap<String, Vec<Clue>>,
    pub(crate) dialogues: HashMap<String, HashMap<String, DialogueBook>>,
    pub(crate) scene_text: HashMap<String, String>,
    pub(crate) chapter_scene_text: HashMap<(String, String, World), String>,
    pub(crate) intro_text: HashMap<String, String>,
    pub(crate) outro_text: HashMap<String, String>,
    pub(crate) result_text: HashMap<(String, String), String>,
    pub(crate) narrative_text: HashMap<(String, String), String>,
    pub(crate) exit_attempt_text: HashMap<String, String>,
    pub(crate) reachable: HashMap<String, Vec<String>>,
    pub(crate) root_chapter: Option<String>,
}

/// 扫描并加载全部内容，建立索引、可达连接与根节点。
pub(crate) fn load_all(src: &dyn DataSource) -> Result<LoadedContent> {
    let scenes = load_yaml_dir::<Scene>(src, "scenes")?;
    let characters = load_yaml_dir::<Character>(src, "characters")?;

    let mut scene_text: HashMap<String, String> = HashMap::new();
    for s in &scenes {
        for path in [&s.description.surface, &s.description.shadow] {
            if let Ok(text) = src.read(path) {
                scene_text.entry(path.clone()).or_insert(text);
            }
        }
    }
    let mut exit_attempt_text: HashMap<String, String> = HashMap::new();
    for s in &scenes {
        if let Some(ea) = &s.exit_attempt {
            if let Ok(text) = src.read(&ea.text) {
                exit_attempt_text.insert(s.id.clone(), text);
            }
        }
    }

    let mut chapters = Vec::new();
    let mut judgments: HashMap<String, Vec<Judgment>> = HashMap::new();
    let mut clues: HashMap<String, Vec<Clue>> = HashMap::new();
    let mut dialogues: HashMap<String, HashMap<String, DialogueBook>> = HashMap::new();
    let mut chapter_scene_text: HashMap<(String, String, World), String> = HashMap::new();
    let mut intro_text: HashMap<String, String> = HashMap::new();
    let mut outro_text: HashMap<String, String> = HashMap::new();
    let mut result_text: HashMap<(String, String), String> = HashMap::new();
    let mut narrative_text: HashMap<(String, String), String> = HashMap::new();

    for name in src.list_dir("chapters")? {
        let chap_yaml = format!("chapters/{name}/chapter.yaml");
        if !src.exists(&chap_yaml) {
            continue;
        }
        let ch: Chapter = read_yaml(src, &chap_yaml)?;
        let cid = ch.id.clone();
        let base = format!("chapters/{cid}");

        let jlist: Vec<Judgment> =
            load_opt_yaml(src, &format!("{base}/judgments.yaml")).unwrap_or_default();
        let clist: Vec<Clue> =
            load_opt_yaml(src, &format!("{base}/clues.yaml")).unwrap_or_default();

        let mut dmap = HashMap::new();
        for dn in src.list_dir(&format!("{base}/dialogues"))? {
            if let Some(char_id) = strip_md_ext(&dn) {
                let text = src.read(&format!("{base}/dialogues/{dn}"))?;
                dmap.insert(char_id.to_string(), parse_dialogue(&text)?);
            }
        }

        for sn in src.list_dir(&format!("{base}/scenes"))? {
            if let Some((scene, world)) = parse_scene_override_name(&sn) {
                if let Ok(text) = src.read(&format!("{base}/scenes/{sn}")) {
                    chapter_scene_text.insert((cid.clone(), scene, world), text);
                }
            }
        }

        if let Some(t) = read_opt(src, &ch.intro.as_ref().map(|p| format!("{base}/{p}"))) {
            intro_text.insert(cid.clone(), t);
        }
        if let Some(t) = read_opt(src, &ch.outro.as_ref().map(|p| format!("{base}/{p}"))) {
            outro_text.insert(cid.clone(), t);
        }
        for j in &jlist {
            if let Ok(t) = src.read(&format!("{base}/{}", j.result)) {
                result_text.insert((cid.clone(), j.id.clone()), t);
            }
        }
        for n in &ch.narrative {
            if let Ok(t) = src.read(&format!("{base}/{}", n.text)) {
                narrative_text.insert((cid.clone(), n.id.clone()), t);
            }
        }

        judgments.insert(cid.clone(), jlist);
        clues.insert(cid.clone(), clist);
        dialogues.insert(cid.clone(), dmap);
        chapters.push(ch);
    }

    Ok(LoadedContent {
        scene_index: build_first_index(scenes.iter().map(|s| &s.id)),
        char_index: build_first_index(characters.iter().map(|c| &c.id)),
        chapter_index: build_first_index(chapters.iter().map(|c| &c.id)),
        reachable: compute_reachable(&scenes),
        root_chapter: compute_root(&chapters),
        scenes,
        characters,
        chapters,
        judgments,
        clues,
        dialogues,
        scene_text,
        chapter_scene_text,
        intro_text,
        outro_text,
        result_text,
        narrative_text,
        exit_attempt_text,
    })
}

/// 读取某目录下全部 `.yaml` 文件并反序列化。
fn load_yaml_dir<T: DeserializeOwned + 'static>(src: &dyn DataSource, dir: &str) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for name in src.list_dir(dir)? {
        if strip_yaml_ext(&name).is_some() {
            out.push(read_yaml(src, &format!("{dir}/{name}"))?);
        }
    }
    Ok(out)
}

/// 可选 YAML 文件：存在则反序列化，缺失返回 `None`。
fn load_opt_yaml<T: DeserializeOwned + 'static>(src: &dyn DataSource, rel: &str) -> Option<T> {
    if src.exists(rel) {
        Some(read_yaml(src, rel).ok()?)
    } else {
        None
    }
}

/// 可选文本文件：`Some(rel)` 存在则读取，否则 `None`。
fn read_opt(src: &dyn DataSource, rel: &Option<String>) -> Option<String> {
    rel.as_ref().and_then(|p| src.read(p).ok())
}

/// 构建「首次出现位置」索引（重复 id 只记第一个，重复由校验层报告）。
fn build_first_index<'a, I>(ids: I) -> HashMap<String, usize>
where
    I: Iterator<Item = &'a String>,
{
    let mut idx = HashMap::new();
    for (i, id) in ids.enumerate() {
        idx.entry(id.clone()).or_insert(i);
    }
    idx
}

/// 计算场景可达连接：`connections` 双向（自动补反向）、`one_way_connections` 单向。
fn compute_reachable(scenes: &[Scene]) -> HashMap<String, Vec<String>> {
    let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
    for s in scenes {
        for c in &s.connections {
            edges.entry(s.id.clone()).or_default().insert(c.clone());
            edges.entry(c.clone()).or_default().insert(s.id.clone());
        }
        for c in &s.one_way_connections {
            edges.entry(s.id.clone()).or_default().insert(c.clone());
        }
    }
    edges
        .into_iter()
        .map(|(k, set)| {
            let mut v: Vec<String> = set.into_iter().collect();
            v.sort();
            (k, v)
        })
        .collect()
}

/// 计算唯一根节点（无入度的章节）。零个或多个均返回 `None`，由校验层报告。
fn compute_root(chapters: &[Chapter]) -> Option<String> {
    let mut has_incoming: HashSet<String> = HashSet::new();
    for ch in chapters {
        if let Some(next) = &ch.next {
            has_incoming.insert(next.default.clone());
            for b in &next.branches {
                has_incoming.insert(b.target.clone());
            }
        }
    }
    let roots: Vec<&Chapter> = chapters
        .iter()
        .filter(|c| !has_incoming.contains(&c.id))
        .collect();
    match roots.len() {
        1 => Some(roots[0].id.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_list_and_read() {
        let src = InMemorySource::new()
            .insert("chapters/c1/chapter.yaml", "id: c1\n")
            .insert("chapters/c1/dialogues/wolf.md", "## t\n\n### [surface]\n\nx\n");
        let names = src.list_dir("chapters").unwrap();
        assert_eq!(names, vec!["c1"]);
        let dlg = src.list_dir("chapters/c1/dialogues").unwrap();
        assert_eq!(dlg, vec!["wolf.md"]);
        assert!(src.exists("chapters/c1/chapter.yaml"));
        assert!(src.exists("chapters/c1/dialogues"));
        assert!(!src.exists("chapters/c2"));
        assert!(src.read("chapters/c1/dialogues/wolf.md").unwrap().contains("## t"));
    }

    #[test]
    fn strip_helpers() {
        assert_eq!(strip_yaml_ext("tavern.yaml"), Some("tavern"));
        assert_eq!(strip_md_ext("wolf.md"), Some("wolf"));
        assert!(strip_yaml_ext("x.txt").is_none());
    }

    #[test]
    fn parse_scene_override() {
        use crate::world::World;
        assert_eq!(
            parse_scene_override_name("tavern.surface.md"),
            Some(("tavern".into(), World::Surface))
        );
        assert_eq!(
            parse_scene_override_name("alley.shadow.md"),
            Some(("alley".into(), World::Shadow))
        );
        assert!(parse_scene_override_name("tavern.md").is_none());
    }

    #[test]
    fn filesystem_source_rejects_missing_root() {
        assert!(FilesystemSource::new("/no/such/dir/xyz").is_err());
    }
}
