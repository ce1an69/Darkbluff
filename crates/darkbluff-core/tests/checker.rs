//! 启动校验（checker）的集成测试。

use darkbluff_core::content::{check, CheckReport, ContentEngine, InMemorySource, Severity};

/// 一个最小且合法的数据集，期望 0 error。
fn valid_source() -> InMemorySource {
    InMemorySource::new()
        .insert(
            "scenes/s.yaml",
            "id: s\nname: 场景\ndescription:\n  surface: text/scenes/s.surface.md\n  shadow: text/scenes/s.shadow.md\n",
        )
        .insert("text/scenes/s.surface.md", "表面。")
        .insert("text/scenes/s.shadow.md", "影子。")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c1/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert(
            "chapters/c1/clues.yaml",
            "- id: wolf_alibi\n  source: wolf.whereabouts\n  world: surface\n",
        )
        .insert(
            "chapters/c1/dialogues/wolf.md",
            "## whereabouts\n\n### [surface]\n\n我在场。\n\n### [shadow]\n\n我不在。\n",
        )
        .insert("chapters/c1/results/wolf.md", "灰狼受审。")
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 终\nending: true\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_final]\noutro: outro.md\n",
        )
        .insert(
            "chapters/c2/judgments.yaml",
            "- id: judge_wolf_final\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c2/results/wolf.md", "终章审判。")
        .insert("chapters/c2/outro.md", "结局。")
}

fn check_source(src: &InMemorySource) -> CheckReport {
    let eng = ContentEngine::load(src).expect("load");
    check(&eng)
}

fn err_msgs(report: &CheckReport) -> Vec<String> {
    report.errors().map(|i| i.message.clone()).collect()
}

#[test]
fn valid_dataset_has_no_errors() {
    let r = check_source(&valid_source());
    assert!(!r.has_errors(), "unexpected errors: {:?}", err_msgs(&r));
}

#[test]
fn detects_topic_missing_in_dialogue() {
    let src = valid_source().insert(
        "chapters/c1/chapter.yaml",
        "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: ghost\n        label: 幽灵话题\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("ghost") && m.contains("对话文件")));
}

#[test]
fn detects_invalid_next_target() {
    let src = valid_source().insert(
        "chapters/c1/chapter.yaml",
        "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: nowhere\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("跳转目标不存在")));
}

#[test]
fn detects_judgment_target_not_in_characters() {
    let src = valid_source().insert(
        "chapters/c1/judgments.yaml",
        "- id: judge_wolf\n  target: ghost\n  result: results/wolf.md\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("target") && m.contains("ghost")));
}

#[test]
fn detects_no_judgments() {
    let src = valid_source().insert("chapters/c1/judgments.yaml", "[]\n");
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("至少定义一个审判点")));
}

#[test]
fn detects_clue_world_mismatch_real() {
    let src = valid_source()
        .insert("characters/crow.yaml", "id: crow\nname: 乌鸦\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n  - id: crow\n    appears_in: [s]\n    topics:\n      - id: victim\n        label: 受害者\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c1/clues.yaml",
            "- id: wolf_alibi\n  source: wolf.whereabouts\n  world: surface\n- id: bad\n  source: crow.victim\n  world: shadow\n",
        )
        .insert(
            "chapters/c1/dialogues/crow.md",
            "## victim\n\n### [surface]\n\n受害者。\n",
        );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("bad") && m.contains("world")));
}

#[test]
fn detects_unknown_condition_id() {
    let src = valid_source().insert(
        "chapters/c1/chapter.yaml",
        "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after: no_such_clue\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("未知 id") && m.contains("no_such_clue")));
}

#[test]
fn detects_cycle() {
    let src = valid_source()
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 终\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\nnext:\n  default: c1\n",
        );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("环")));
}

#[test]
fn detects_multiple_roots() {
    let src = valid_source()
        .insert("characters/fox.yaml", "id: fox\nname: 狐\n")
        .insert(
            "chapters/c3/chapter.yaml",
            "id: c3\ntitle: 孤立终\nending: true\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_fox]\n",
        )
        .insert(
            "chapters/c3/judgments.yaml",
            "- id: judge_fox\n  target: fox\n  result: results/fox.md\n",
        )
        .insert("chapters/c3/results/fox.md", "狐受审。");
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("根节点")));
}

#[test]
fn detects_ending_with_next() {
    let src = valid_source().insert(
        "chapters/c2/chapter.yaml",
        "id: c2\ntitle: 终\nending: true\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\nnext:\n  default: c1\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("终章") && m.contains("next")));
}

#[test]
fn detects_non_ending_without_next() {
    let src = valid_source().insert(
        "chapters/c2/chapter.yaml",
        "id: c2\ntitle: 终\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("必须提供 next")));
}

#[test]
fn detects_outro_on_non_ending() {
    let src = valid_source().insert(
        "chapters/c1/chapter.yaml",
        "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\noutro: outro.md\n",
    );
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("outro")));
}

#[test]
fn detects_missing_scene_description() {
    let src = InMemorySource::new()
        .insert(
            "scenes/s.yaml",
            "id: s\nname: 场景\ndescription:\n  surface: text/scenes/s.surface.md\n  shadow: text/scenes/s.shadow.md\n",
        )
        .insert("text/scenes/s.surface.md", "表面。")
        // 故意不提供 shadow 文件
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c1/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert(
            "chapters/c1/clues.yaml",
            "- id: wolf_alibi\n  source: wolf.whereabouts\n  world: surface\n",
        )
        .insert(
            "chapters/c1/dialogues/wolf.md",
            "## whereabouts\n\n### [surface]\n\n我在场。\n",
        )
        .insert("chapters/c1/results/wolf.md", "灰狼受审。")
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 终\nending: true\nscenes: [s]\nstarting_scene: s\nrequired_judgments: [judge_wolf]\noutro: outro.md\n",
        )
        .insert(
            "chapters/c2/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c2/results/wolf.md", "终章审判。")
        .insert("chapters/c2/outro.md", "结局。");
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("缺少 shadow 描述")));
}

#[test]
fn detects_dead_end_oneway() {
    let src = InMemorySource::new()
        .insert(
            "scenes/tavern.yaml",
            "id: tavern\nname: 酒馆\nconnections: [market]\none_way_connections: [cellar]\ndescription:\n  surface: ts.surface.md\n  shadow: ts.shadow.md\n",
        )
        .insert(
            "scenes/market.yaml",
            "id: market\nname: 集市\ndescription:\n  surface: ms.surface.md\n  shadow: ms.shadow.md\n",
        )
        .insert(
            "scenes/cellar.yaml",
            "id: cellar\nname: 地窖\ndescription:\n  surface: cs.surface.md\n  shadow: cs.shadow.md\n",
        )
        .insert("ts.surface.md", "a").insert("ts.shadow.md", "b")
        .insert("ms.surface.md", "a").insert("ms.shadow.md", "b")
        .insert("cs.surface.md", "a").insert("cs.shadow.md", "b")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [tavern, market]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics:\n      - id: t\n        label: 话题\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert(
            "chapters/c1/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c1/dialogues/wolf.md", "## t\n\n### [surface]\n\nx\n")
        .insert("chapters/c1/results/wolf.md", "审判。")
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 终\nending: true\nscenes: [tavern]\nstarting_scene: tavern\nrequired_judgments: [judge_wolf]\n",
        )
        .insert(
            "chapters/c2/judgments.yaml",
            "- id: judge_wolf\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c2/results/wolf.md", "终审。");
    let r = check_source(&src);
    assert!(err_msgs(&r).iter().any(|m| m.contains("死胡同")));
}

#[test]
fn cross_chapter_clue_is_advice_not_error() {
    let src = InMemorySource::new()
        .insert(
            "scenes/s.yaml",
            "id: s\nname: 场景\ndescription:\n  surface: s.surface.md\n  shadow: s.shadow.md\n",
        )
        .insert("s.surface.md", "a").insert("s.shadow.md", "b")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert("characters/crow.yaml", "id: crow\nname: 乌鸦\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: wolf\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c2\n",
        )
        .insert("chapters/c1/judgments.yaml", "- id: judge_wolf\n  target: wolf\n  result: r.md\n")
        .insert("chapters/c1/clues.yaml", "- id: shared\n  source: wolf.whereabouts\n  world: surface\n")
        .insert("chapters/c1/dialogues/wolf.md", "## whereabouts\n\n### [surface]\n\nx\n### [shadow]\n\ny\n")
        .insert("chapters/c1/r.md", "审判。")
        .insert(
            "chapters/c2/chapter.yaml",
            "id: c2\ntitle: 二\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: crow\n    topics:\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after: shared\nrequired_judgments: [judge_crow]\nnext:\n  default: c3\n",
        )
        .insert("chapters/c2/judgments.yaml", "- id: judge_crow\n  target: crow\n  result: r.md\n")
        .insert("chapters/c2/dialogues/crow.md", "## secret\n\n### [surface]\n\nz\n### [shadow]\n\nw\n")
        .insert("chapters/c2/r.md", "审判。")
        .insert(
            "chapters/c3/chapter.yaml",
            "id: c3\ntitle: 终\nending: true\nscenes: [s]\nstarting_scene: s\ncharacters:\n  - id: crow\n    topics: []\nrequired_judgments: [judge_crow_final]\n",
        )
        .insert("chapters/c3/judgments.yaml", "- id: judge_crow_final\n  target: crow\n  result: r.md\n")
        .insert("chapters/c3/r.md", "终审。");
    let r = check_source(&src);
    assert!(!r.has_errors(), "errors: {:?}", err_msgs(&r));
    assert!(r
        .issues
        .iter()
        .any(|i| i.message.contains("shared") && i.severity == Severity::Advice));
}
