//! 用 TestBackend 无头渲染视图，确认布局/面板/浮层/首页不 panic 且产出预期文本。

use super::{MenuView, ViewState, draw};
use crate::app::{AnimationView, NpcInfo, NpcTopic, SuggestKind, Suggestion, Suggestions};
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

#[test]
fn renders_animation_indicator() {
    let title = "失踪的屠夫".to_string();
    let scene_name = "酒馆".to_string();
    let scene_text = "昏黄的酒馆。".to_string();
    let transcript = VecDeque::new();
    let input = CommandInput::default();
    let state = SessionState::Exploring;
    let animation = AnimationView {
        label: "视角",
        progress: 0.2,
    };
    let vs = ViewState {
        title: &title,
        scene_name: &scene_name,
        world: World::Surface,
        scene_text: &scene_text,
        npcs: &[],
        endings: (0, 2),
        state: &state,
        input: &input,
        transcript: &transcript,
        offset: 0,
        menu: None,
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: Some(animation),
        typewriter: None,
    };

    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    assert!(
        text.contains("视角"),
        "animation indicator missing:\n{text}"
    );
}

fn sample_npcs() -> Vec<NpcInfo> {
    vec![NpcInfo {
        name: "灰狼".into(),
        id: "wolf".into(),
        topics: vec![
            NpcTopic {
                label: "昨晚的行踪".into(),
                available: true,
            },
            NpcTopic {
                label: "隐藏的秘密".into(),
                available: false,
            },
        ],
    }]
}

#[test]
fn renders_exploring_layout_npcs_and_palette() {
    let title = "失踪的屠夫".to_string();
    let scene_name = "酒馆".to_string();
    let scene_text = "昏黄的酒馆里飘着麦芽味，角落传来低语。".to_string();
    let transcript = VecDeque::from(vec![
        StyledLine {
            text: "灰狼 · 昨晚的行踪".into(),
            style: ratatui::style::Style::default(),
        },
        StyledLine {
            text: "“我一直在这儿喝酒。”".into(),
            style: ratatui::style::Style::default(),
        },
    ]);
    let npcs = sample_npcs();
    let input = CommandInput::default();
    let state = SessionState::Exploring;
    let suggestions = Suggestions {
        kind: SuggestKind::Command,
        items: vec![Suggestion {
            display: "/ask".into(),
            desc: "询问在场角色".into(),
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
        offset: 0,
        menu: None,
        confirmation: None,
        suggestions: Some(&suggestions),
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };

    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());

    for needle in [
        "对话记录",
        "场景",
        "在场",
        "灰狼",
        "DarkBluff",
        "表面",
        "/ask",
        "结局",
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
        MenuOption {
            id: "wolf".into(),
            label: "灰狼".into(),
        },
        MenuOption {
            id: "crow".into(),
            label: "乌鸦".into(),
        },
    ];
    let menu = MenuView {
        kind: MenuKind::AskCharacter,
        options: &options,
        selected: 0,
    };
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
        offset: 0,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    assert!(
        text.contains("询问角色"),
        "menu title missing:\n{text}"
    );
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
        offset: 0,
        menu: None,
        confirmation: Some(&action),
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    term.draw(|f| draw(f, &vs2)).unwrap();
    let text2 = buffer_text(term.backend().buffer());
    assert!(text2.contains("确认"), "confirm title missing:\n{text2}");
    assert!(
        text2.contains("覆盖"),
        "confirm prompt missing:\n{text2}"
    );
}

#[test]
fn renders_home_screen() {
    let options = vec![
        MenuOption {
            id: "new_game".into(),
            label: "新游戏".into(),
        },
        MenuOption {
            id: "quit".into(),
            label: "退出".into(),
        },
    ];
    let menu = MenuView {
        kind: MenuKind::Title,
        options: &options,
        selected: 0,
    };
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
        offset: 0,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    for needle in ["新游戏", "退出"] {
        assert!(text.contains(needle), "home missing {needle:?}:\n{text}");
    }
}

#[test]
fn renders_settings_current_value() {
    // 设置菜单为维度行：单行 id="motion"，label 含当前值（由 core 拼装）。
    let options = vec![MenuOption {
        id: "motion".into(),
        label: "动画：减少".into(),
    }];
    let menu = MenuView {
        kind: MenuKind::Settings,
        options: &options,
        selected: 0,
    };
    let input = CommandInput::default();
    let state = SessionState::ChoosingSettings;
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
        offset: 0,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());

    assert!(
        text.contains("动画：减少"),
        "settings current value missing:\n{text}"
    );
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
                MapRow {
                    flat_index: 0,
                    label: "章节开始 · 酒馆".into(),
                },
                MapRow {
                    flat_index: 1,
                    label: "审判前 · 酒馆".into(),
                },
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
        MenuOption {
            id: "ckpt_c1_start".into(),
            label: "章节开始 · 酒馆".into(),
        },
        MenuOption {
            id: "ckpt_before".into(),
            label: "审判前 · 酒馆".into(),
        },
    ];
    let menu = MenuView {
        kind: MenuKind::Checkpoint,
        options: &options,
        selected: 0,
    };
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
        offset: 0,
        menu: Some(menu),
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: Some(&groups),
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    for needle in [
        "地图",
        "失踪的屠夫",
        "酒馆真相",
        "★",
        "???",
        "话题 2/3",
        "章节开始",
    ] {
        assert!(text.contains(needle), "map missing {needle:?}:\n{text}");
    }
}

#[test]
fn draw_transcript_typewriter_reveals_only_revealed() {
    use crate::app::TypewriterView;
    // 单行 5 汉字（10 列）；打字机只揭 4 列 → 仅 "一二" 可见，"三四五" 不应泄漏。
    let transcript = VecDeque::from([StyledLine {
        text: "一二三四五".into(),
        style: ratatui::style::Style::default(),
    }]);
    let input = CommandInput::default();
    let state = SessionState::Exploring;
    let typewriter = TypewriterView { lines: 1, skip: 0, revealed: 4 };
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
        offset: 0,
        menu: None,
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: Some(typewriter),
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    assert!(text.contains("一二"), "revealed part missing:\n{text}");
    assert!(!text.contains("三四五"), "unrevealed part leaked:\n{text}");
}

#[test]
fn draw_transcript_typewriter_keeps_visible_when_block_taller_than_panel() {
    use crate::app::TypewriterView;
    // 20 行叙事（每行 "AAAA"，4 列）覆盖整个 transcript；面板可见行数 < 20。
    // typewriter revealed 只够前两行。修复前 reveal 被屏幕外的覆盖区前部吃光、可见区全空；
    // 修复后 reveal 只分给可见行，可见区顶部仍有内容。
    let transcript: VecDeque<StyledLine> = (0..20)
        .map(|_| StyledLine {
            text: "AAAA".into(),
            style: ratatui::style::Style::default(),
        })
        .collect();
    let input = CommandInput::default();
    let state = SessionState::Exploring;
    let typewriter = TypewriterView { lines: 20, skip: 0, revealed: 8 };
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
        offset: 0,
        menu: None,
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: Some(typewriter),
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    let text = buffer_text(term.backend().buffer());
    assert!(
        text.contains("AAAA"),
        "visible rows empty under typewriter (desync regression):\n{text}"
    );
}

/// 用给定 offset 渲染转录并返回合成文本（视图层信任 offset，钳制由 app 层负责）。
fn render_transcript_with_offset(transcript: &VecDeque<StyledLine>, offset: usize) -> String {
    let input = CommandInput::default();
    let state = SessionState::Exploring;
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
        transcript,
        offset,
        menu: None,
        confirmation: None,
        suggestions: None,
        note: None,
        notice: None,
        map: None,
        no_motion: false,
        animation: None,
        typewriter: None,
    };
    let backend = TestBackend::new(100, 28);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| draw(f, &vs)).unwrap();
    buffer_text(term.backend().buffer())
}

#[test]
fn draw_transcript_renders_window_for_offset() {
    // 40 行（每行单视觉行），面板可见高约 18 → max offset = 22。
    let transcript: VecDeque<StyledLine> = (0..40)
        .map(|i| StyledLine {
            text: format!("L{i:02}"),
            style: ratatui::style::Style::default(),
        })
        .collect();

    // offset=0 → 贴底：末 18 行 L22..=L39 可见，L21 不可见。
    let text = render_transcript_with_offset(&transcript, 0);
    assert!(text.contains("L39"), "tail missing at offset 0:\n{text}");
    assert!(text.contains("L22"), "window top missing at offset 0:\n{text}");
    assert!(!text.contains("L21"), "above window leaked at offset 0:\n{text}");

    // offset=10 → 窗 [12,30)：L12/L29 可见，L11/L30/L39 不可见。
    let text = render_transcript_with_offset(&transcript, 10);
    assert!(text.contains("L12"), "window top missing:\n{text}");
    assert!(text.contains("L29"), "window bottom missing:\n{text}");
    assert!(!text.contains("L11"), "above window leaked:\n{text}");
    assert!(!text.contains("L30"), "below window leaked:\n{text}");
    assert!(!text.contains("L39"), "tail leaked while scrolled:\n{text}");

    // offset=max(22) → 顶部 [0,18)：L00 可见，L18 不可见。
    let text = render_transcript_with_offset(&transcript, 22);
    assert!(text.contains("L00"), "top missing at max offset:\n{text}");
    assert!(!text.contains("L18"), "below top window leaked:\n{text}");
}
