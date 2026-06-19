//! Catppuccin Mocha 配色（偏紫）与主题化组件助手。
//!
//! 所有面板统一圆角 + MANTLE 底色；聚焦面板（输入框）边框提亮为主色 MAUVE。
//! 视角：Surface=天蓝、Shadow=紫，对比清晰且整体偏紫。

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType};

use darkbluff_core::world::World;

// --- Catppuccin Mocha（背景层）---
pub const CRUST: Color = Color::Rgb(17, 17, 27); // #11111b  画面最底
pub const MANTLE: Color = Color::Rgb(24, 24, 37); // #181825  面板底

// --- 表面/线条 ---
pub const SURFACE1: Color = Color::Rgb(69, 71, 90); // #45475a  选中高亮
pub const OVERLAY0: Color = Color::Rgb(108, 112, 134); // #6c7086  普通边框
pub const OVERLAY1: Color = Color::Rgb(124, 127, 147); // #7c7f93
pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200); // #a6adc8  次要文本
pub const SUBTEXT1: Color = Color::Rgb(181, 189, 220); // #b5bddc
pub const TEXT: Color = Color::Rgb(205, 214, 244); // #cdd6f4  正文

// --- 强调色（紫为主）---
pub const MAUVE: Color = Color::Rgb(203, 166, 247); // #cba6f7  主色
pub const LAVENDER: Color = Color::Rgb(180, 190, 254); // #b4befe
pub const PINK: Color = Color::Rgb(245, 194, 231); // #f5c2e7
pub const RED: Color = Color::Rgb(243, 139, 168); // #f38ba8
pub const YELLOW: Color = Color::Rgb(249, 226, 175); // #f9e2af
pub const SKY: Color = Color::Rgb(137, 220, 235); // #89dceb
pub const BLUE: Color = Color::Rgb(137, 180, 250); // #89b4fa

/// 暗紫：非聚焦面板的边框色（整体偏紫，但弱于聚焦态的 MAUVE）。
pub const BORDER: Color = Color::Rgb(130, 100, 180); // #8264b4

/// 视角主色：Surface=天蓝，Shadow=紫。
pub fn world_color(world: World) -> Color {
    match world {
        World::Surface => SKY,
        World::Shadow => MAUVE,
    }
}

/// 视角英文标签。
pub fn world_label(world: World) -> &'static str {
    match world {
        World::Surface => "Surface",
        World::Shadow => "Shadow",
    }
}

/// 圆角主题面板。`focus=true` 时边框提亮为主色（用于输入框等聚焦元素）。
pub fn panel<'a>(title: Option<&str>, focus: bool) -> Block<'a> {
    let border = if focus { MAUVE } else { BORDER };
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(MANTLE));
    if let Some(t) = title {
        block = block
            .title(format!(" {t} "))
            .title_style(Style::default().fg(border).add_modifier(Modifier::BOLD));
    }
    block
}
