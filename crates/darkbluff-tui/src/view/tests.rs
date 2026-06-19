//! 用 TestBackend 无头渲染视图，确认布局/面板/浮层/首页不 panic 且产出预期文本。

use super::{draw, MenuView, ViewState};
use crate::app::{NpcInfo, NpcTopic, SuggestKind, Suggestion, Suggestions};
use crate::input::CommandInput;
use crate::markdown::StyledLine;
use darkbluff_core::engine::{ConfirmationAction, MenuKind, MenuOption, SessionState};
use darkbluff_core::world::World;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::collections::VecDeque;
use unicode_width::UnicodeWidthChar;

/// 把渲染缓冲区拼成文本（跳过宽字符的占位 spacer，使 CJK 连续可搜）。
fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let w = buf.area.width as usize;
    let mut out = String::new();
    let mut skip_spacer = false;
    for (i, cell) in buf.content.iter().enumerate() {
        if (i + 1) % w == 0 {
            skip_spacer = false;
            out.push('\n');
            continue;
        }
        if skip_spacer {
            skip_spacer = false;
            continue;
        }
        out.push_str(cell.symbol());
        skip_spacer = cell
            .symbol()
            .chars()
            .next()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0) == 2)
            .unwrap_or(false);
    }
    out
}

fn sample_npcs() -> Vec<NpcInfo> {
    vec![NpcInfo {
        name: "灰狼".into(),
        id: "wolf".into(),
        topics: vec![
            NpcTopic { label: "昨晚的行踪".into(), available: true },
            NpcTopic { label: "隐藏的秘密".into(), available: false },
        ],
    }]
}

#[test]
fn renders_exploring_layout_npcs_and_palette() {
    let title = "失踪的屠夫".to_string();
    let scene_name = "酒馆".to_string();
    let scene_text = "昏黄的酒馆里飘着麦芽味，角落传来低语。".to_string();
    let transcript = VecDeque::from(vec![
        StyledLine { text: "灰狼 · 昨晚的行踪".into(), style: ratatui::style::Style::default() },
        StyledLine { text: "“我一直在这儿喝酒。”".into(), style: ratatui::style::Style::default() },
    ]);
    let npcs = sample_npcs();
    let input = CommandInput::default();
    let state = SessionState::Exploring;
    let suggestions = Suggestions {
        kind: SuggestKind::Command,
        items: vec![Suggestion {
            display: "/ask".into(),
            desc: "Question a character".into(),
            insert: "/ask ".into(),
        }],
        selected: 0,
    };
    let vs = ViewState {
        title: &title,
        scene_name: &scene_name,
        world: World::Surface,
        scene_text: &scene_text,
        npcs: &npcs,
        endings: (0, 2),
        state: &state,
        input: &input,
        transcript: &transcript,
        menu: None,
        confirmation: None,
        suggestions: Some(&suggestions),
        status: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
    };

    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());

    for needle in [
        "Transcript", "Scene", "PRESENT", "灰狼", "DarkBluff", "Surface", "/ask", "Endings",
        "昨晚的行踪",
    ] {
        assert!(text.contains(needle), "render missing {needle:?}:\n{text}");
    }
}

#[test]
fn renders_menu_and_confirmation_overlays() {
    let input = CommandInput::default();
    let state = SessionState::ChoosingAskCharacter;
    let options = vec![
        MenuOption { id: "wolf".into(), label: "灰狼".into() },
        MenuOption { id: "crow".into(), label: "乌鸦".into() },
    ];
    let menu = MenuView { kind: MenuKind::AskCharacter, options: &options, selected: 0 };
    let transcript = VecDeque::new();
    let empty = String::new();
    let vs = ViewState {
        title: &empty,
        scene_name: &empty,
        world: World::Surface,
        scene_text: &empty,
        npcs: &[],
        endings: (0, 0),
        state: &state,
        input: &input,
        transcript: &transcript,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        status: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    assert!(text.contains("Ask Character"), "menu title missing:\n{text}");
    assert!(text.contains("灰狼"), "menu option missing:\n{text}");

    // 确认浮层
    let action = ConfirmationAction::NewGame;
    let confirming = SessionState::Confirming;
    let vs2 = ViewState {
        title: &empty,
        scene_name: &empty,
        world: World::Surface,
        scene_text: &empty,
        npcs: &[],
        endings: (0, 0),
        state: &confirming,
        input: &input,
        transcript: &transcript,
        menu: None,
        confirmation: Some(&action),
        suggestions: None,
        status: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
    };
    term.draw(|f| draw(f, &vs2)).unwrap();
    let text2 = buffer_text(term.backend().buffer());
    assert!(text2.contains("Confirm"), "confirm title missing:\n{text2}");
    assert!(text2.contains("overwritten"), "english confirm prompt missing:\n{text2}");
}

#[test]
fn renders_home_screen() {
    let options = vec![
        MenuOption { id: "new_game".into(), label: "新游戏".into() },
        MenuOption { id: "quit".into(), label: "退出".into() },
    ];
    let menu = MenuView { kind: MenuKind::Title, options: &options, selected: 0 };
    let input = CommandInput::default();
    let state = SessionState::Title;
    let transcript = VecDeque::new();
    let empty = String::new();
    let vs = ViewState {
        title: &empty,
        scene_name: &empty,
        world: World::Surface,
        scene_text: &empty,
        npcs: &[],
        endings: (1, 2),
        state: &state,
        input: &input,
        transcript: &transcript,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        status: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    for needle in ["New Game", "Quit"] {
        assert!(text.contains(needle), "home missing {needle:?}:\n{text}");
    }
}

#[test]
fn renders_map_panel_tree_and_checkpoints() {
    use crate::view::{MapGroup, MapRow};
    let groups = vec![
        MapGroup {
            title: "失踪的屠夫".into(),
            ending: false,
            is_current: true,
            unseen_branches: 1,
            topic_progress: Some((2, 3)),
            checkpoints: vec![
                MapRow { flat_index: 0, label: "章节开始 · 酒馆".into() },
                MapRow { flat_index: 1, label: "审判前 · 酒馆".into() },
            ],
        },
        MapGroup {
            title: "酒馆真相".into(),
            ending: true,
            is_current: false,
            unseen_branches: 0,
            topic_progress: None,
            checkpoints: vec![],
        },
    ];
    let options = vec![
        MenuOption { id: "ckpt_c1_start".into(), label: "章节开始 · 酒馆".into() },
        MenuOption { id: "ckpt_before".into(), label: "审判前 · 酒馆".into() },
    ];
    let menu = MenuView { kind: MenuKind::Checkpoint, options: &options, selected: 0 };
    let input = CommandInput::default();
    let state = SessionState::ChoosingCheckpoint;
    let transcript = VecDeque::new();
    let empty = String::new();
    let vs = ViewState {
        title: &empty,
        scene_name: &empty,
        world: World::Surface,
        scene_text: &empty,
        npcs: &[],
        endings: (1, 2),
        state: &state,
        input: &input,
        transcript: &transcript,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        status: None,
        note: None,
        notice: None,
        map: Some(&groups),
        no_motion: false,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    for needle in ["Map", "失踪的屠夫", "酒馆真相", "★", "???", "话题 2/3", "章节开始"] {
        assert!(text.contains(needle), "map missing {needle:?}:\n{text}");
    }
}
