//! 极简 Markdown → 带样式行的渲染。
//!
//! 仅处理对话/剧情文本里实际出现的子集：标题（`#`/`##`/`###`）、无序列表（`- `/`* `）、
//! 段落。每行一个 [`StyledLine`]（行内统一样式），便于视图层按显示宽度折行。
//! 行内 `**bold**` 等inline 标记不解析（数据里几乎不用），保持实现极简。

use ratatui::style::{Modifier, Style};

use crate::theme;

/// 一行带统一样式的文本。
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub text: String,
    pub style: Style,
}

/// 把一段（可能多行）markdown 文本渲染为若干 [`StyledLine`]，空行丢弃。
pub fn render(text: &str) -> Vec<StyledLine> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(h) = trim_heading(line) {
            out.push(StyledLine {
                text: h.to_string(),
                style: Style::default()
                    .fg(theme::LAVENDER)
                    .add_modifier(Modifier::BOLD),
            });
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            out.push(StyledLine {
                text: format!("• {}", item.trim()),
                style: Style::default().fg(theme::SUBTEXT0),
            });
        } else {
            out.push(StyledLine {
                text: line.to_string(),
                style: Style::default().fg(theme::TEXT),
            });
        }
    }
    out
}

fn trim_heading(line: &str) -> Option<&str> {
    for prefix in ["### ", "## ", "# "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

    fn texts(out: &[StyledLine]) -> Vec<&str> {
        out.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn render_styles_headings_lists_and_paragraphs() {
        let out = render("# Title\n\nbody\n\n- item");
        assert_eq!(texts(&out), vec!["Title", "body", "• item"]);
        assert_eq!(
            out[0].style,
            Style::default().fg(theme::LAVENDER).add_modifier(Modifier::BOLD)
        );
        assert_eq!(out[1].style, Style::default().fg(theme::TEXT));
        assert_eq!(out[2].style, Style::default().fg(theme::SUBTEXT0));
    }

    #[test]
    fn render_skips_blank_lines() {
        assert_eq!(texts(&render("\n\na\n\n\nb\n")), vec!["a", "b"]);
    }

    #[test]
    fn render_strips_all_heading_levels() {
        for input in ["# h", "## h", "### h"] {
            assert_eq!(render(input)[0].text, "h");
        }
    }
}
