//! 叙事触发器（心声 / 碎片）与「走不出去」集成测试。
//!
//! 用 InMemorySource 构造最小数据集（不改 fixtures），驱动 Session 验证：
//! 进入章节 drain、审判后碎片在推进前触发、走不出去首次/重复、narrative id 解锁话题。

use std::sync::atomic::{AtomicU64, Ordering};

use darkbluff_core::content::{ContentEngine, InMemorySource, check};
use darkbluff_core::engine::{Input, Outcome, Session, SessionState};
use darkbluff_core::save::{FakeClock, SaveStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn src() -> InMemorySource {
    InMemorySource::new()
        .insert(
            "scenes/alley.yaml",
            "id: alley\nname: 后巷\nconnections: [tavern]\nexit_attempt:\n  text: leave.md\ndescription:\n  surface: alley.surface.md\n  shadow: alley.shadow.md\n",
        )
        .insert(
            "scenes/tavern.yaml",
            "id: tavern\nname: 酒馆\nconnections: [alley]\ndescription:\n  surface: tavern.surface.md\n  shadow: tavern.shadow.md\n",
        )
        .insert("alley.surface.md", "后巷表面。")
        .insert("alley.shadow.md", "后巷影子。")
        .insert("tavern.surface.md", "酒馆表面。")
        .insert("tavern.shadow.md", "酒馆影子。")
        .insert("leave.md", "路还在，往前走，回过神来又站在巷口。")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首章\nscenes: [alley, tavern]\nstarting_scene: alley\ncharacters:\n  - id: wolf\n    appears_in: [tavern]\n    topics:\n      - id: hello\n        label: 打招呼\n        available: true\n      - id: secret\n        label: 隐藏\n        available: false\n        unlock_after: voice_open\nnarrative:\n  - id: voice_open\n    label: 心声\n    text: voice_open.md\n  - id: fragment_after_judge\n    label: 记忆碎片\n    when: judge_wolf\n    text: fragment.md\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c1/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: r.md\n",
        )
        .insert(
            "chapters/c1/dialogues/wolf.md",
            "## hello\n\n### [surface]\n\n你好。\n\n## secret\n\n### [surface]\n\n秘密。\n",
        )
        .insert("chapters/c1/r.md", "审判剧情。")
        .insert("chapters/c1/voice_open.md", "（脑中响起一个声音。）")
        .insert("chapters/c1/fragment.md", "（一闪而过的画面。）")
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 终章\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_final]\n",
        )
        .insert(
            "chapters/c2/judgments.yaml",
            "- id: judge_wolf_final\n  target: wolf\n  result: r.md\n",
        )
        .insert("chapters/c2/r.md", "终审。")
}

fn new_session(tag: &str) -> Session {
    let engine = ContentEngine::load(&src()).expect("load");
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "darkbluff-trigger-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let store = SaveStore::open(dir, Box::new(FakeClock::new())).expect("open store");
    Session::new(engine, store)
}

#[test]
fn dataset_is_valid() {
    let engine = ContentEngine::load(&src()).expect("load");
    let errors: Vec<String> = check(&engine).errors().map(|i| i.message.clone()).collect();
    assert!(
        errors.is_empty(),
        "trigger dataset has check errors: {errors:?}"
    );
}

#[test]
fn narrative_drains_on_chapter_enter() {
    let mut s = new_session("enter");
    // 进入首章 → finalize → drain voice_open（when 省略=立即触发）→ ShowingNarrative。
    assert!(matches!(s.start_new_game(), Outcome::Narrative { .. }));
    assert_eq!(*s.state(), SessionState::ShowingNarrative);
    assert!(s.save().discovered.triggered("voice_open"));
    assert!(
        s.save()
            .viewed_narrative
            .get("c1")
            .unwrap()
            .iter()
            .any(|n| n.id == "voice_open")
    );
    // Ack 心声 → 无更多 → 回探索态（场景描述）。
    assert!(matches!(s.handle(Input::Ack), Outcome::Message(_)));
    assert_eq!(*s.state(), SessionState::Exploring);
    assert_eq!(s.save().current_scene, "alley");
}

#[test]
fn fragment_drains_after_judgment_then_advances() {
    let mut s = new_session("fragment");
    s.start_new_game(); // ShowingNarrative（voice_open）
    s.handle(Input::Ack); // Exploring alley
    s.handle(Input::Text("move tavern".into())); // 进入酒馆（wolf 在场）
    assert_eq!(s.save().current_scene, "tavern");
    // 审判 wolf（required）→ complete → drain_or_advance：先触发碎片，再推进。
    s.handle(Input::Text("judge wolf".into()));
    assert_eq!(*s.state(), SessionState::ShowingNarrative);
    assert_eq!(s.save().current_chapter, "c1"); // 尚未推进
    assert!(s.save().discovered.triggered("fragment_after_judge"));
    // Ack 碎片 → 执行被推迟的推进 → 进入终章。
    let outcome = s.handle(Input::Ack);
    assert_eq!(s.save().current_chapter, "c2");
    // 推进产出：c2 无 intro → 场景描述 Message（或叙事 drain）。
    let _ = outcome;
}

#[test]
fn leave_attempt_first_then_repeat() {
    let mut s = new_session("leave");
    s.start_new_game();
    s.handle(Input::Ack); // Exploring alley
    // 首次走不出去 → 展示完整旁白，不移动。
    assert!(matches!(
        s.handle(Input::Text("move __leave".into())),
        Outcome::Narrative { .. }
    ));
    assert_eq!(*s.state(), SessionState::ShowingNarrative);
    assert_eq!(s.save().current_scene, "alley"); // 未移动
    s.handle(Input::Ack); // Exploring alley
    // 再次尝试 → 已触发，简短提示，仍不移动。
    assert!(matches!(
        s.handle(Input::Text("move __leave".into())),
        Outcome::Message(_)
    ));
    assert_eq!(*s.state(), SessionState::Exploring);
    assert_eq!(s.save().current_scene, "alley");
    assert!(s.save().discovered.triggered("leave_attempt"));
}

#[test]
fn narrative_id_unlocks_topic() {
    let mut s = new_session("unlock");
    s.start_new_game(); // voice_open 已 drain → 进入 factset
    s.handle(Input::Ack); // Exploring alley
    s.handle(Input::Text("move tavern".into())); // wolf 在场
    // secret 的 unlock_after: voice_open —— 心声触发后解锁。
    assert!(matches!(
        s.handle(Input::Text("ask wolf secret".into())),
        Outcome::Dialogue { body, .. } if body.contains("秘密")
    ));
}

#[test]
fn reload_recovers_interrupted_advance() {
    use darkbluff_core::save::LoadResult;
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "darkbluff-trigger-recover-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    // 第一段：审判完成触发「审判后碎片」，进入 ShowingNarrative 后在 ack 前 quit
    // （模拟崩溃/退出中断了被推迟的章节推进）。
    {
        let engine = ContentEngine::load(&src()).expect("load");
        let store = SaveStore::open(dir.clone(), Box::new(FakeClock::new())).expect("open");
        let mut s = Session::new(engine, store);
        s.start_new_game(); // voice_open 心声
        s.handle(Input::Ack); // Exploring alley
        s.handle(Input::Text("move tavern".into())); // wolf 在场
        s.handle(Input::Text("judge wolf".into())); // 碎片 + 推迟推进
        assert_eq!(*s.state(), SessionState::ShowingNarrative);
        assert_eq!(s.save().current_chapter, "c1"); // 推进被推迟，尚未切换
        s.handle(Input::Quit); // 持久化并退出（ack 前）
    }

    // 第二段：重新加载。存档里 c1 的必要审判已完成却停在 c1 —— continue_with 应补回推进。
    let engine = ContentEngine::load(&src()).expect("load");
    let store = SaveStore::open(dir, Box::new(FakeClock::new())).expect("open");
    let save = match store.load().expect("load save") {
        LoadResult::Save(s, _) => s,
        other => panic!("expected save, got {other:?}"),
    };
    let mut s = Session::new(engine, store);
    s.continue_with(save);
    assert_eq!(s.save().current_chapter, "c2", "被中断的自动推进应被恢复");
}
