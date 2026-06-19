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

/// 按显示宽度截断（尾部加 …），用于输入框右侧瞬时状态。
pub(super) fn truncate_s(s: &str, max_w: usize) -> String {
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_w.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
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
}
