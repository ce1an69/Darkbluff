//! 对话 Markdown 解析。
//!
//! 设计见 docs/data-formats.md「对话数据」。对话文件按角色拆分，结构：
//!
//! ```markdown
//! ## whereabouts            <- 话题分隔符（H2）
//! ### [surface]             <- 世界版本分隔符（H3）
//! （正文，保留原始 Markdown）
//! ### [shadow]
//! （正文）
//! ```
//!
//! 解析策略：**fence-aware 的行扫描**。只有 H2（话题）与 H3（世界标记）是分节边界，
//! H4 及以上视为正文保留（设计要求正文标题须从 `####` 起）；code fence 内的 `##`/`###`
//! 不被当作标题。每个分节的正文按原始 Markdown 切片保存（不做事件重建），以直接作为
//! 对话快照的「渲染前原文」。

use std::collections::BTreeMap;

use crate::error::{AppError, Result};
use crate::world::World;

/// 单个话题的两个世界版本正文（surface / shadow 各可缺省，至少存在其一）。
#[derive(Debug, Clone, Default)]
struct DialogueEntry {
    surface: Option<String>,
    shadow: Option<String>,
}

/// 一个对话文件解析后的全部话题 → 世界版本 → 原始 Markdown 正文。
///
/// 使用 `BTreeMap` 以获得确定性的话题迭代顺序（便于稳定输出/测试）。
#[derive(Debug, Clone, Default)]
pub struct DialogueBook {
    by_topic: BTreeMap<String, DialogueEntry>,
}

impl DialogueBook {
    pub fn new() -> Self {
        Self::default()
    }

    fn set(&mut self, topic: String, world: World, body: String) {
        let entry = self.by_topic.entry(topic).or_default();
        match world {
            World::Surface => entry.surface = Some(body),
            World::Shadow => entry.shadow = Some(body),
        }
    }

    /// 该话题是否出现在本文件中（至少一个世界版本）。
    pub fn contains_topic(&self, topic: &str) -> bool {
        self.by_topic.contains_key(topic)
    }

    /// 所有话题 id（确定性顺序）。
    pub fn topics(&self) -> impl Iterator<Item = &str> {
        self.by_topic.keys().map(|s| s.as_str())
    }

    /// 取某话题在某世界版本的正文（缺失返回 `None`）。
    pub fn get(&self, topic: &str, world: World) -> Option<&str> {
        self.by_topic.get(topic).and_then(|e| match world {
            World::Surface => e.surface.as_deref(),
            World::Shadow => e.shadow.as_deref(),
        })
    }

    /// 该话题是否包含指定世界版本。
    pub fn has_world(&self, topic: &str, world: World) -> bool {
        self.get(topic, world).is_some()
    }

    /// 该话题是否为「单世界话题」（只存在一侧版本）。话题不存在返回 `None`。
    pub fn is_single_world(&self, topic: &str) -> Option<bool> {
        self.by_topic.get(topic).map(|e| e.surface.is_some() ^ e.shadow.is_some())
    }
}

/// 解析对话 Markdown 源文本。
pub fn parse_dialogue(src: &str) -> Result<DialogueBook> {
    let mut book = DialogueBook::new();
    let mut current_topic: Option<String> = None;
    let mut current_world: Option<World> = None;
    let mut buffer: Vec<&str> = Vec::new();
    let mut in_fence = false;

    // 将当前 (话题, 世界) 的正文缓冲写入 book，并清空缓冲。
    macro_rules! flush {
        () => {{
            if let (Some(topic), Some(world)) = (&current_topic, current_world) {
                let body = normalize_body(&buffer);
                book.set(topic.clone(), world, body);
            }
            buffer.clear();
        }};
    }

    for line in src.lines() {
        let is_fence = is_fence_line(line);

        if !in_fence {
            if let Some((level, text)) = parse_atx_heading(line) {
                match level {
                    2 => {
                        flush!();
                        if book.contains_topic(&text) {
                            return Err(AppError::Content(format!(
                                "对话文件中话题 id 重复: {text}"
                            )));
                        }
                        current_topic = Some(text);
                        current_world = None;
                        continue;
                    }
                    3 => {
                        if current_topic.is_none() {
                            return Err(AppError::Content(
                                "对话文件中世界标记出现在任何话题之前".into(),
                            ));
                        }
                        let world = parse_world_tag(&text)?;
                        flush!();
                        current_world = Some(world);
                        continue;
                    }
                    // H1 / H4+ 视为正文，落入下方追加
                    _ => {}
                }
            }
        }

        if is_fence {
            in_fence = !in_fence;
        }

        // 正文行：含 fence 行本身、H4+ 标题行、code fence 内的所有行
        if current_topic.is_some() && current_world.is_some() {
            buffer.push(line);
        }
    }
    flush!();
    Ok(book)
}

/// 解析 ATX 标题，返回 (层级 1-6, 标题文本)。非标题行返回 `None`。
fn parse_atx_heading(line: &str) -> Option<(usize, String)> {
    let no_indent = line.trim_start();
    if !no_indent.starts_with('#') {
        return None;
    }
    let hashes = no_indent.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = &no_indent[hashes..];
    let text = if after.is_empty() {
        String::new()
    } else if let Some(rest) = after.strip_prefix(' ') {
        rest.trim().to_string()
    } else {
        // 形如 `##x`：井号后无空格，按 CommonMark 不是标题
        return None;
    };
    Some((hashes, text))
}

/// 解析 H3 世界标记文本（如 `[surface]` / `[shadow]`）。
fn parse_world_tag(text: &str) -> Result<World> {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    match inner.trim().to_ascii_lowercase().as_str() {
        "surface" => Ok(World::Surface),
        "shadow" => Ok(World::Shadow),
        other => Err(AppError::Content(format!("未知的对话世界标记: [{other}]"))),
    }
}

/// 是否为 code fence 行（≥3 个反引号或波浪号）。
fn is_fence_line(line: &str) -> bool {
    let t = line.trim_start();
    match t.chars().next() {
        Some('`') => t.chars().take_while(|&c| c == '`').count() >= 3,
        Some('~') => t.chars().take_while(|&c| c == '~').count() >= 3,
        _ => false,
    }
}

/// 去除正文缓冲首尾的空行后用 `\n` 连接，保留内部结构与缩进。
fn normalize_body(lines: &[&str]) -> String {
    let mut start = 0;
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }
    let mut end = lines.len();
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_worlds() {
        let src = "## whereabouts\n\n### [surface]\n\n我在酒馆。\n\n### [shadow]\n\n我在家。\n";
        let book = parse_dialogue(src).unwrap();
        assert!(book.contains_topic("whereabouts"));
        assert_eq!(book.get("whereabouts", World::Surface), Some("我在酒馆。"));
        assert_eq!(book.get("whereabouts", World::Shadow), Some("我在家。"));
        assert_eq!(book.is_single_world("whereabouts"), Some(false));
    }

    #[test]
    fn single_world_topic() {
        let src = "## victim\n\n### [surface]\n\n只有表面。\n";
        let book = parse_dialogue(src).unwrap();
        assert!(book.has_world("victim", World::Surface));
        assert!(!book.has_world("victim", World::Shadow));
        assert_eq!(book.is_single_world("victim"), Some(true));
    }

    #[test]
    fn preserves_h4_subheaders_in_body() {
        let src = "## t\n\n### [surface]\n\n正文。\n\n#### 子标题\n\n更多。\n";
        let book = parse_dialogue(src).unwrap();
        let body = book.get("t", World::Surface).unwrap();
        assert!(body.contains("#### 子标题"));
        assert!(body.contains("正文。"));
    }

    #[test]
    fn code_fence_hashes_are_not_topics() {
        let src = "## t\n\n### [surface]\n\n```\n## not_a_topic\n### [shadow]\n```\n结尾。\n";
        let book = parse_dialogue(src).unwrap();
        // fence 内的 ##/### 不被识别为分节
        assert_eq!(book.topics().collect::<Vec<_>>(), vec!["t"]);
        let body = book.get("t", World::Surface).unwrap();
        assert!(body.contains("## not_a_topic"));
        assert!(body.contains("结尾。"));
        assert!(!book.has_world("t", World::Shadow));
    }

    #[test]
    fn frontmatter_and_preamble_ignored() {
        let src = "---\nid: wolf\n---\n\n前言应被忽略。\n\n## t\n\n### [surface]\n\n正文。\n";
        let book = parse_dialogue(src).unwrap();
        assert_eq!(book.topics().collect::<Vec<_>>(), vec!["t"]);
        let body = book.get("t", World::Surface).unwrap();
        assert!(!body.contains("前言"));
    }

    #[test]
    fn duplicate_topic_errors() {
        let src = "## t\n\n### [surface]\n\na\n\n## t\n\n### [surface]\n\nb\n";
        assert!(parse_dialogue(src).is_err());
    }

    #[test]
    fn world_before_topic_errors() {
        let src = "### [surface]\n\na\n";
        assert!(parse_dialogue(src).is_err());
    }

    #[test]
    fn unknown_world_tag_errors() {
        let src = "## t\n\n### [underworld]\n\na\n";
        assert!(parse_dialogue(src).is_err());
    }

    #[test]
    fn world_tag_case_insensitive() {
        let src = "## t\n\n### [SURFACE]\n\na\n";
        let book = parse_dialogue(src).unwrap();
        assert_eq!(book.get("t", World::Surface), Some("a"));
    }

    #[test]
    fn multiple_topics_ordered() {
        let src = "## b\n\n### [surface]\n\nx\n\n## a\n\n### [surface]\n\ny\n";
        let book = parse_dialogue(src).unwrap();
        // BTreeMap → 字母序
        assert_eq!(book.topics().collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
