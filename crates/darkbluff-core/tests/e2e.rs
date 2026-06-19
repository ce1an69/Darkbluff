//! 端到端集成测试：用 tests/fixtures/data 驱动完整游戏流程，验证最终存档状态、
//! 自动推进分支、线索解锁、检查点回滚与结局达成。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use darkbluff_core::content::{ContentEngine, FilesystemSource, check};
use darkbluff_core::engine::{Input, Outcome, Selection, Session, SessionState};
use darkbluff_core::save::{FakeClock, SaveStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}

fn temp_save_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("darkbluff-e2e-{tag}-{}-{n}", std::process::id()))
}

fn new_session(tag: &str) -> Session {
    let src = FilesystemSource::new(fixture_dir()).expect("fixture dir");
    let engine = ContentEngine::load(&src).expect("load");
    let store =
        SaveStore::open(temp_save_dir(tag), Box::new(FakeClock::new())).expect("open store");
    Session::new(engine, store)
}

#[test]
fn fixture_passes_check() {
    let src = FilesystemSource::new(fixture_dir()).expect("fixture dir");
    let engine = ContentEngine::load(&src).expect("load");
    let report = check(&engine);
    let errors: Vec<String> = report.errors().map(|i| i.message.clone()).collect();
    assert!(errors.is_empty(), "fixture has check errors: {errors:?}");
}

#[test]
fn new_game_intro_then_exploring() {
    let mut s = new_session("intro");
    assert!(matches!(s.start_new_game(), Outcome::ChapterIntro { .. }));
    assert_eq!(*s.state(), SessionState::ShowingIntro);
    match s.handle(Input::Ack) {
        Outcome::Message(message) => assert!(message.lines.join("").contains("酒馆")),
        o => panic!("expected show, got {:?}", o),
    }
    assert_eq!(*s.state(), SessionState::Exploring);
}

#[test]
fn ask_collects_clue_and_unlocks_secret() {
    let mut s = new_session("clue");
    s.start_new_game();
    s.handle(Input::Ack);

    // secret 初始不可问
    match s.handle(Input::Text("ask wolf secret".into())) {
        Outcome::Message(message) => assert_eq!(message.lines[0], "你还没有足够线索。"),
        o => panic!("got {:?}", o),
    }

    // 收集 wolf_alibi（surface）
    s.handle(Input::Text("ask wolf whereabouts".into()));
    assert!(s.save().has_clue("the_missing_butcher", "wolf_alibi"));
    // 收集 crow_testimony（surface）
    s.handle(Input::Text("ask crow victim".into()));
    assert!(s.save().has_clue("the_missing_butcher", "crow_testimony"));

    // 现在 secret 可问
    match s.handle(Input::Text("ask wolf secret".into())) {
        Outcome::Dialogue { body, .. } => assert!(body.contains("回不去了")),
        o => panic!("expected dialogue, got {:?}", o),
    }
}

#[test]
fn gaze_switches_world_and_collects_shadow_clue() {
    let mut s = new_session("gaze");
    s.start_new_game();
    s.handle(Input::Ack);
    s.handle(Input::Text("gaze".into()));
    // 影子侧问 whereabouts → 收集 wolf_alibi_shadow
    s.handle(Input::Text("ask wolf whereabouts".into()));
    assert!(
        s.save()
            .has_clue("the_missing_butcher", "wolf_alibi_shadow")
    );
}

#[test]
fn judging_crow_then_wolf_reaches_truth_ending() {
    let mut s = new_session("truth");
    s.start_new_game();
    s.handle(Input::Ack);

    // 先审 crow（非必要审判）→ 不推进
    s.handle(Input::Text("judge crow".into()));
    assert_eq!(s.save().current_chapter, "the_missing_butcher");
    assert!(s.save().judged("the_missing_butcher", "judge_crow"));

    // 再审 wolf → 必要审判完成 → 命中 all_of(both) → tavern_truth
    s.handle(Input::Text("judge wolf".into()));
    assert_eq!(s.save().current_chapter, "tavern_truth");
    assert!(
        s.save()
            .discovered
            .chapters
            .contains(&"tavern_truth".to_string())
    );

    // 终章审判 → 结局（有 outro）→ ShowingOutro
    s.handle(Input::Text("judge wolf".into()));
    assert_eq!(*s.state(), SessionState::ShowingOutro);
    assert!(
        s.save()
            .discovered
            .endings
            .contains(&"tavern_truth".to_string())
    );

    // 确认结局 → Ending
    match s.handle(Input::Ack) {
        Outcome::EndingReached { found, total, .. } => {
            assert_eq!(found, 1);
            assert_eq!(total, 2);
        }
        o => panic!("expected ending, got {:?}", o),
    }
    assert_eq!(*s.state(), SessionState::Ending);
}

#[test]
fn judging_wolf_only_reaches_deceit_ending() {
    let mut s = new_session("deceit");
    s.start_new_game();
    s.handle(Input::Ack);

    s.handle(Input::Text("judge wolf".into()));
    // 必要审判（judge_wolf）完成 → any_of(judge_wolf) → tavern_deceit
    assert_eq!(s.save().current_chapter, "tavern_deceit");
}

#[test]
fn map_rollback_before_judgment_undoes_judgment() {
    let mut s = new_session("rollback");
    s.start_new_game();
    s.handle(Input::Ack);
    // 审判 crow（非必要审判）→ 不推进，留在本章，产生 before_judgment 检查点
    s.handle(Input::Text("judge crow".into()));
    assert!(s.save().judged("the_missing_butcher", "judge_crow"));
    assert_eq!(s.save().current_chapter, "the_missing_butcher");

    // map → 菜单（chapter_start + before_judgment 两个检查点）
    match s.handle(Input::Text("map".into())) {
        Outcome::MenuRequested { options, .. } => assert_eq!(options.len(), 2),
        o => panic!("got {:?}", o),
    }
    // 列表顺序：[chapter_start, before_judgment]，故 before_judgment = Index(1)
    match s.handle(Input::Select(Selection::Index(1))) {
        Outcome::ConfirmationRequested { .. } => {}
        o => panic!("got {:?}", o),
    }
    match s.handle(Input::Confirm(true)) {
        Outcome::Message(_) => {}
        o => panic!("got {:?}", o),
    }
    // 审判已被撤销
    assert!(!s.save().judged("the_missing_butcher", "judge_crow"));
    // before_judgment 检查点已移除（审判撤销），仅剩 chapter_start
    assert_eq!(s.save().checkpoints.len(), 1);
    // discovered 仍保留该章记忆
    assert!(
        s.save()
            .discovered
            .chapters
            .contains(&"the_missing_butcher".to_string())
    );
}

#[test]
fn persistence_survives_reload() {
    // 在一个 session 收集线索后，重新打开存档目录加载，验证事实持久化
    let dir = temp_save_dir("persist");
    let src = FilesystemSource::new(fixture_dir()).unwrap();
    let engine = ContentEngine::load(&src).unwrap();
    {
        let store = SaveStore::open(dir.clone(), Box::new(FakeClock::new())).unwrap();
        let mut s = Session::new(engine.clone(), store);
        s.start_new_game();
        s.handle(Input::Ack);
        s.handle(Input::Text("ask wolf whereabouts".into()));
        s.handle(Input::Text("quit".into()));
    }
    // 重新加载
    let store = SaveStore::open(dir, Box::new(FakeClock::new())).unwrap();
    match store.load().unwrap() {
        darkbluff_core::save::LoadResult::Save(loaded, _) => {
            assert!(loaded.has_clue("the_missing_butcher", "wolf_alibi"));
            assert_eq!(loaded.current_chapter, "the_missing_butcher");
        }
        other => panic!("expected save, got {:?}", other),
    }
}
