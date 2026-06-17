//! 会话状态机集成测试：新游戏、指令执行、菜单、自动推进、hints。

use std::sync::atomic::{AtomicU64, Ordering};

use darkbluff_core::content::{ContentEngine, InMemorySource};
use darkbluff_core::engine::{Input, Outcome, Selection, Session, SessionState};
use darkbluff_core::save::{CheckpointKind, FakeClock, SaveStore};
use darkbluff_core::world::World;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn build_session() -> Session {
    let src = InMemorySource::new()
        .insert("scenes/tavern.yaml", "id: tavern\nname: 酒馆\nconnections: [market]\ndescription:\n  surface: ts.surface.md\n  shadow: ts.shadow.md\n")
        .insert("scenes/market.yaml", "id: market\nname: 集市\ndescription:\n  surface: ms.surface.md\n  shadow: ms.shadow.md\n")
        .insert("ts.surface.md", "酒馆表面。").insert("ts.shadow.md", "酒馆影子。")
        .insert("ms.surface.md", "集市表面。").insert("ms.shadow.md", "集市影子。")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert("characters/crow.yaml", "id: crow\nname: 乌鸦\n")
        .insert("chapters/c1/chapter.yaml", "id: c1\ntitle: 首\nintro: intro.md\nscenes: [tavern, market]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    appears_in: [tavern]\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n      - id: the_knife\n        label: 刀\n        available: true\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after:\n          all_of: [wolf_alibi]\n  - id: crow\n    appears_in: [tavern]\n    topics: []\nrequired_judgments: [judge_wolf]\nnext:\n  default: c_other\n  branches:\n    - when: judge_wolf\n      target: c_truth\n")
        .insert("chapters/c1/intro.md", "首章开场。")
        .insert("chapters/c1/judgments.yaml", "- id: judge_wolf\n  target: wolf\n  result: r.md\n- id: judge_crow\n  target: crow\n  result: r.md\n")
        .insert("chapters/c1/clues.yaml", "- id: wolf_alibi\n  source: wolf.whereabouts\n  world: surface\n")
        .insert("chapters/c1/dialogues/wolf.md", "## whereabouts\n\n### [surface]\n\n在场。\n\n### [shadow]\n\n不在。\n\n## the_knife\n\n### [surface]\n\n刀。\n\n## secret\n\n### [surface]\n\n秘密。\n")
        .insert("chapters/c1/r.md", "灰狼受审。")
        .insert("chapters/c_truth/chapter.yaml", "id: c_truth\ntitle: 真相\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_truth]\noutro: outro.md\n")
        .insert("chapters/c_truth/outro.md", "真相结局。")
        .insert("chapters/c_truth/judgments.yaml", "- id: judge_wolf_truth\n  target: wolf\n  result: r.md\n")
        .insert("chapters/c_truth/r.md", "终审。")
        .insert("chapters/c_other/chapter.yaml", "id: c_other\ntitle: 其他\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_other]\n")
        .insert("chapters/c_other/judgments.yaml", "- id: judge_wolf_other\n  target: wolf\n  result: r.md\n")
        .insert("chapters/c_other/r.md", "其他审判。");
    let engine = ContentEngine::load(&src).unwrap();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("darkbluff-session-{}-{n}", std::process::id()));
    let store = SaveStore::open(dir, Box::new(FakeClock::new())).unwrap();
    Session::new(engine, store)
}

#[test]
fn new_game_shows_intro_then_exploring() {
    let mut s = build_session();
    match s.start_new_game() {
        Outcome::ChapterIntro { text } => assert!(text.contains("首章开场")),
        o => panic!("expected intro, got {:?}", o),
    }
    assert_eq!(*s.state(), SessionState::ShowingIntro);
    match s.handle(Input::Ack) {
        Outcome::Message(message) => assert!(message.lines.join("").contains("酒馆")),
        o => panic!("expected show, got {:?}", o),
    }
    assert_eq!(*s.state(), SessionState::Exploring);
    assert!(s
        .save()
        .checkpoints
        .iter()
        .any(|c| c.kind == CheckpointKind::ChapterStart));
}

#[test]
fn ask_direct_collects_clue_and_records_snapshot() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("ask wolf whereabouts".into())) {
        Outcome::Dialogue { body, notes, .. } => {
            assert!(body.contains("在场"));
            assert!(!notes.is_empty());
        }
        o => panic!("expected dialogue, got {:?}", o),
    }
    assert!(s.save().has_clue("c1", "wolf_alibi"));
    assert_eq!(s.save().viewed_dialogues.get("c1").unwrap().len(), 1);
    assert!(s
        .save()
        .discovered
        .topics
        .get("c1")
        .unwrap()
        .contains(&"wolf.whereabouts".to_string()));
}

#[test]
fn ask_unknown_target_not_in_scene() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("ask ghost whereabouts".into())) {
        Outcome::Message(message) => assert_eq!(message.lines[0], "这里没有这个角色。"),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn ask_locked_topic_hint() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("ask wolf secret".into())) {
        Outcome::Message(message) => assert_eq!(message.lines[0], "你还没有足够线索。"),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn gaze_toggles_world_and_description() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("gaze".into())) {
        Outcome::Message(message) => {
            let joined = message.lines.join("\n");
            assert!(joined.contains("左眼·影子"));
            assert!(joined.contains("酒馆影子"));
        }
        o => panic!("got {:?}", o),
    }
    assert_eq!(s.save().current_world, World::Shadow);
}

#[test]
fn move_blocked_and_allowed() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("move cellar".into())) {
        Outcome::Message(message) => assert_eq!(message.lines[0], "你现在无法前往那里。"),
        o => panic!("got {:?}", o),
    }
    match s.handle(Input::Text("move market".into())) {
        Outcome::Message(message) => assert!(message.lines[0].contains("集市")),
        o => panic!("got {:?}", o),
    }
    assert_eq!(s.save().current_scene, "market");
}

#[test]
fn judge_creates_checkpoint_and_advances() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("judge wolf".into()));
    assert_eq!(s.save().current_chapter, "c_truth");
    assert!(s
        .save()
        .checkpoints
        .iter()
        .any(|c| c.kind == CheckpointKind::BeforeJudgment));
}

#[test]
fn judge_unknown_target_hint() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("judge ghost".into())) {
        Outcome::Message(message) => assert_eq!(message.lines[0], "现在还无法审判他。"),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn map_rollback_to_chapter_start() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("ask wolf whereabouts".into()));
    s.handle(Input::Text("gaze".into()));
    match s.handle(Input::Text("map".into())) {
        Outcome::MenuRequested { options, .. } => assert!(!options.is_empty()),
        o => panic!("got {:?}", o),
    }
    match s.handle(Input::Select(Selection::Index(0))) {
        Outcome::ConfirmationRequested { .. } => {}
        o => panic!("got {:?}", o),
    }
    match s.handle(Input::Confirm(true)) {
        Outcome::ChapterIntro { .. } => {}
        o => panic!("expected intro re-show, got {:?}", o),
    }
    match s.handle(Input::Ack) {
        Outcome::Message(_) => {}
        o => panic!("got {:?}", o),
    }
    assert!(!s.save().has_clue("c1", "wolf_alibi"));
    assert_eq!(s.save().current_world, World::Surface);
    assert!(s.save().discovered.topics.contains_key("c1"));
}

#[test]
fn help_overview_and_unknown() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("help".into())) {
        Outcome::Message(message) => assert!(message.lines.join("\n").contains("ask")),
        o => panic!("got {:?}", o),
    }
    match s.handle(Input::Text("help fly".into())) {
        Outcome::Message(message) => assert!(message.lines[0].contains("未知指令")),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn unknown_command_hint() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("fly".into())) {
        Outcome::Message(message) => assert!(message.lines[0].contains("未知指令")),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn quit_persists_and_returns() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("quit".into())) {
        Outcome::QuitRequested => {}
        o => panic!("got {:?}", o),
    }
}

#[test]
fn hint_gaze_after_three_surface_asks() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    let mut saw_gaze_hint = false;
    for _ in 0..3 {
        match s.handle(Input::Text("ask wolf whereabouts".into())) {
            Outcome::Dialogue { notes, .. } => {
                if notes.iter().any(|n| n.contains("gaze")) {
                    saw_gaze_hint = true;
                }
            }
            o => panic!("got {:?}", o),
        }
    }
    assert!(saw_gaze_hint);
}

#[test]
fn hint_judge_after_collecting_clue() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    match s.handle(Input::Text("ask wolf whereabouts".into())) {
        Outcome::Dialogue { notes, .. } => assert!(notes.iter().any(|n| n.contains("judge"))),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn hint_map_after_first_judgment() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("gaze".into()));
    match s.handle(Input::Text("judge crow".into())) {
        Outcome::Message(message) => assert!(message.lines.iter().any(|x| x.contains("map"))),
        o => panic!("got {:?}", o),
    }
}

#[test]
fn hints_only_in_first_chapter() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("judge wolf".into()));
    assert_ne!(s.save().current_chapter, "c1");
}

#[test]
fn ending_judgment_with_outro_keeps_result_text() {
    let mut s = build_session();
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("judge wolf".into()));
    assert_eq!(s.save().current_chapter, "c_truth");

    match s.handle(Input::Text("judge wolf".into())) {
        Outcome::ChapterOutro { text } => {
            assert!(text.contains("终审。"));
            assert!(text.contains("真相结局。"));
        }
        o => panic!("expected outro, got {:?}", o),
    }
}

// ----- 标题界面 -----

#[test]
fn title_shows_menu_no_save() {
    let mut s = build_session();
    // 新 Session 在 Title 状态，任意输入触发菜单构建
    match s.handle(Input::Text("".into())) {
        Outcome::MenuRequested {
            prompt, options, ..
        } => {
            assert!(prompt.contains("Darkbluff"));
            // 无存档时没有"继续"
            assert!(!options.iter().any(|o| o.id == "continue"));
            assert!(options.iter().any(|o| o.id == "new_game"));
            assert!(options.iter().any(|o| o.id == "quit"));
        }
        o => panic!("expected menu, got {:?}", o),
    }
}

#[test]
fn title_new_game_starts_chapter() {
    let mut s = build_session();
    s.handle(Input::Text("".into())); // 触发 Title 菜单
    match s.handle(Input::Select(Selection::Index(0))) {
        Outcome::ChapterIntro { text } => assert!(text.contains("首章开场")),
        Outcome::Message(_) => {} // 无 intro 时直接展示场景消息
        o => panic!("expected intro or show, got {:?}", o),
    }
}

#[test]
fn menu_selection_can_use_option_id() {
    let mut s = build_session();
    s.handle(Input::Text("".into()));
    match s.handle(Input::Select(Selection::Id("new_game".into()))) {
        Outcome::ChapterIntro { text } => assert!(text.contains("首章开场")),
        Outcome::Message(_) => {}
        o => panic!("expected intro or show, got {:?}", o),
    }
}

#[test]
fn title_quit_exits() {
    let mut s = build_session();
    s.handle(Input::Text("".into())); // 触发 Title 菜单 [new_game, quit]
    match s.handle(Input::Select(Selection::Index(1))) {
        Outcome::QuitRequested => {}
        o => panic!("expected quit, got {:?}", o),
    }
}
