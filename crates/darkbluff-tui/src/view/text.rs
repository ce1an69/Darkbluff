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
