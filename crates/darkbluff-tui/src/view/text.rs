//! 文本宽度工具：折行与截断（按显示宽度，兼容 CJK）。

use unicode_width::UnicodeWidthChar;

/// 按显示宽度折行（按字符宽度累加，超宽即换行）。
pub(super) fn wrap_by_width(s: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_w && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            w = 0;
        }
        cur.push(ch);
        w += cw;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// 按显示宽度截取核心：`ellipsis=true` 时预留 1 列并在溢出处补 `…`（CJK 安全）。
fn truncate_core(s: &str, max_w: usize, ellipsis: bool) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    let limit = if ellipsis {
        max_w.saturating_sub(1)
    } else {
        max_w
    };
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > limit {
            if ellipsis {
                out.push('…');
            }
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

/// 按显示宽度截断（尾部加 …），用于浮层等需要省略号的截断。
pub(super) fn truncate_s(s: &str, max_w: usize) -> String {
    truncate_core(s, max_w, true)
}

/// 按显示宽度截取前 `max_w` 列（不加省略号，CJK 安全），用于打字机逐字揭示。
pub(super) fn truncate_by_width(s: &str, max_w: usize) -> String {
    truncate_core(s, max_w, false)
}

/// 该文本在 `max_w` 列宽下折成多少视觉行（仅计数、不分配），用于打字机定位可见窗口。
pub(super) fn count_visual_lines(s: &str, max_w: usize) -> usize {
    if max_w == 0 || s.is_empty() {
        return 1;
    }
    let mut lines = 1usize;
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_w && w > 0 {
            lines += 1;
            w = 0;
        }
        w += cw;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_ascii_into_chunks() {
        assert_eq!(
            wrap_by_width("abcdef", 3),
            vec!["abc".to_string(), "def".to_string()]
        );
    }

    #[test]
    fn wrap_cjk_by_display_width() {
        // 每个汉字占 2 列：宽 4 容纳 2 字
        assert_eq!(
            wrap_by_width("一二三", 4),
            vec!["一二".to_string(), "三".to_string()]
        );
    }

    #[test]
    fn wrap_zero_width_passthrough() {
        assert_eq!(wrap_by_width("abc", 0), vec!["abc".to_string()]);
    }

    #[test]
    fn truncate_appends_ellipsis() {
        assert_eq!(truncate_s("abcdef", 4), "abc…");
    }

    #[test]
    fn truncate_keeps_short_input() {
        assert_eq!(truncate_s("ab", 10), "ab");
    }

    #[test]
    fn truncate_by_width_cjk_no_ellipsis() {
        // 汉字占 2 列：宽 5 取前 2 字（4 列），下一字会越界故停；不加省略号。
        assert_eq!(truncate_by_width("一二三四", 5), "一二");
        assert_eq!(truncate_by_width("abc", 10), "abc");
        // 宽 3 只容下 1 个汉字（2 列），第 2 字会到 4 列越界。
        assert_eq!(truncate_by_width("一二", 3), "一");
    }

    #[test]
    fn count_visual_lines_wraps_by_display_width() {
        assert_eq!(count_visual_lines("abcdef", 3), 2);
        // 汉字 2 列：宽 4 容 2 字，第 3 字换行 → 2 行。
        assert_eq!(count_visual_lines("一二三", 4), 2);
        assert_eq!(count_visual_lines("", 10), 1);
        assert_eq!(count_visual_lines("abc", 0), 1);
    }
}
