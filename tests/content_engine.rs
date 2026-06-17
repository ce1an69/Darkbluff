//! ContentEngine 查询接口的单元级集成测试（内存数据源）。

use std::collections::HashSet;

use darkbluff::content::{ContentEngine, InMemorySource};
use darkbluff::world::World;

fn sample_source() -> InMemorySource {
    InMemorySource::new()
        .insert(
            "scenes/tavern.yaml",
            "id: tavern\nname: 酒馆\nconnections: [market]\none_way_connections: [cellar]\ndescription:\n  surface: text/scenes/tavern.surface.md\n  shadow: text/scenes/tavern.shadow.md\n",
        )
        .insert(
            "scenes/market.yaml",
            "id: market\nname: 集市\ndescription:\n  surface: text/scenes/market.surface.md\n  shadow: text/scenes/market.shadow.md\n",
        )
        .insert(
            "scenes/alley.yaml",
            "id: alley\nname: 巷子\nconnections: [tavern]\ndescription:\n  surface: text/scenes/alley.surface.md\n  shadow: text/scenes/alley.shadow.md\n",
        )
        .insert(
            "scenes/cellar.yaml",
            "id: cellar\nname: 地窖\nconnections: [alley]\ndescription:\n  surface: text/scenes/cellar.surface.md\n  shadow: text/scenes/cellar.shadow.md\n",
        )
        .insert("text/scenes/tavern.surface.md", "酒馆表面。")
        .insert("text/scenes/tavern.shadow.md", "酒馆影子。")
        .insert("text/scenes/market.surface.md", "集市表面。")
        .insert("text/scenes/market.shadow.md", "集市影子。")
        .insert("text/scenes/alley.surface.md", "巷子表面。")
        .insert("text/scenes/alley.shadow.md", "巷子影子。")
        .insert("text/scenes/cellar.surface.md", "地窖表面。")
        .insert("text/scenes/cellar.shadow.md", "地窖影子。")
        .insert("characters/wolf.yaml", "id: wolf\nname: 灰狼\n")
        .insert("characters/crow.yaml", "id: crow\nname: 乌鸦\n")
        .insert(
            "chapters/c1/chapter.yaml",
            "id: c1\ntitle: 首\norder: 1\nscenes: [tavern, market]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    appears_in: [tavern]\n    topics:\n      - id: whereabouts\n        label: 行踪\n        available: true\n      - id: secret\n        label: 秘密\n        available: false\n        unlock_after:\n          all_of: [wolf_alibi]\n  - id: crow\n    topics:\n      - id: victim\n        label: 受害者\n        available: true\nrequired_judgments: [judge_wolf]\nnext:\n  default: c_other\n  branches:\n    - when: judge_wolf\n      target: c_truth\n",
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
            "## whereabouts\n\n### [surface]\n\n我在酒馆。\n\n### [shadow]\n\n我在家。\n\n## secret\n\n### [surface]\n\n秘密表面。\n",
        )
        .insert(
            "chapters/c1/dialogues/crow.md",
            "## victim\n\n### [surface]\n\n只有表面。\n",
        )
        .insert("chapters/c1/results/wolf.md", "灰狼受审。")
        .insert(
            "chapters/c1/scenes/tavern.surface.md",
            "酒馆（c1 覆盖的表面）。",
        )
        .insert(
            "chapters/c_truth/chapter.yaml",
            "id: c_truth\ntitle: 真相\norder: 2\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_truth]\noutro: outro.md\n",
        )
        .insert(
            "chapters/c_truth/judgments.yaml",
            "- id: judge_wolf_truth\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c_truth/results/wolf.md", "真相审判。")
        .insert("chapters/c_truth/outro.md", "真相结局。")
        .insert(
            "chapters/c_other/chapter.yaml",
            "id: c_other\ntitle: 其他\norder: 3\nending: true\nscenes: [tavern]\nstarting_scene: tavern\ncharacters:\n  - id: wolf\n    topics: []\nrequired_judgments: [judge_wolf_other]\n",
        )
        .insert(
            "chapters/c_other/judgments.yaml",
            "- id: judge_wolf_other\n  target: wolf\n  result: results/wolf.md\n",
        )
        .insert("chapters/c_other/results/wolf.md", "其他审判。")
}

fn engine() -> ContentEngine {
    ContentEngine::load(&sample_source()).expect("load")
}

fn facts(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn loads_chapters_scenes_characters() {
    let e = engine();
    assert!(e.chapter_exists("c1"));
    assert!(e.scene_exists("tavern"));
    assert!(e.character_exists("wolf"));
    assert_eq!(e.first_chapter_id(), Some("c1"));
    let mut endings = e.ending_chapter_ids();
    endings.sort();
    assert_eq!(endings, vec!["c_other", "c_truth"]);
}

#[test]
fn scene_description_override_then_global() {
    let e = engine();
    assert_eq!(
        e.get_scene_description("c1", "tavern", World::Surface),
        Some("酒馆（c1 覆盖的表面）。")
    );
    assert_eq!(
        e.get_scene_description("c1", "tavern", World::Shadow),
        Some("酒馆影子。")
    );
    assert_eq!(
        e.get_scene_description("c1", "market", World::Surface),
        Some("集市表面。")
    );
}

#[test]
fn characters_in_scene() {
    let e = engine();
    let in_tavern: Vec<&str> = e
        .get_characters_in_scene("c1", "tavern")
        .into_iter()
        .map(|c| c.id.as_str())
        .collect();
    assert!(in_tavern.contains(&"wolf"));
    assert!(in_tavern.contains(&"crow"));
    let in_market: Vec<&str> = e
        .get_characters_in_scene("c1", "market")
        .into_iter()
        .map(|c| c.id.as_str())
        .collect();
    assert!(!in_market.contains(&"wolf"));
    assert!(in_market.contains(&"crow"));
}

#[test]
fn topics_and_dialogue_and_single_world() {
    let e = engine();
    let topics = e.get_topics("c1", "wolf");
    assert_eq!(topics.len(), 2);
    assert_eq!(
        e.get_dialogue("c1", "wolf", "whereabouts", World::Surface),
        Some("我在酒馆。")
    );
    assert_eq!(
        e.get_dialogue("c1", "wolf", "whereabouts", World::Shadow),
        Some("我在家。")
    );
    // crow.victim 是单世界话题（仅 surface）
    assert_eq!(
        e.get_dialogue("c1", "crow", "victim", World::Surface),
        Some("只有表面。")
    );
    assert_eq!(e.get_dialogue("c1", "crow", "victim", World::Shadow), None);
}

#[test]
fn judgments_and_results() {
    let e = engine();
    let js = e.get_judgments("c1");
    assert_eq!(js.len(), 1);
    assert_eq!(js[0].target, "wolf");
    assert_eq!(
        e.get_judgment_for_character("c1", "wolf").map(|j| j.id.as_str()),
        Some("judge_wolf")
    );
    assert_eq!(e.get_result_text("c1", "judge_wolf"), Some("灰狼受审。"));
}

#[test]
fn clues() {
    let e = engine();
    let cl = e.get_clues("c1");
    assert_eq!(cl.len(), 1);
    assert_eq!(cl[0].source, "wolf.whereabouts");
}

#[test]
fn reachable_scenes_bidirectional_and_oneway() {
    let e = engine();
    let mut r = e.get_reachable_scenes("tavern");
    r.sort();
    assert_eq!(r, vec!["alley", "cellar", "market"]);
    let mut r2 = e.get_reachable_scenes("alley");
    r2.sort();
    assert_eq!(r2, vec!["cellar", "tavern"]);
    assert_eq!(e.get_reachable_scenes("cellar"), vec!["alley"]);
}

#[test]
fn next_chapter_branch_default_ending() {
    let e = engine();
    assert_eq!(
        e.get_next_chapter("c1", &facts(&["judge_wolf"])),
        Some("c_truth")
    );
    assert_eq!(e.get_next_chapter("c1", &facts(&[])), Some("c_other"));
    assert_eq!(e.get_next_chapter("c_truth", &facts(&[])), None);
}

#[test]
fn next_chapter_skips_invalid_branch_target() {
    let src = sample_source().insert(
        "chapters/c1/chapter.yaml",
        "id: c1\ntitle: 首\norder: 1\nscenes: [tavern]\nstarting_scene: tavern\nrequired_judgments: [judge_wolf]\nnext:\n  default: c_other\n  branches:\n    - when: judge_wolf\n      target: does_not_exist\n",
    );
    let e = ContentEngine::load(&src).expect("load");
    assert_eq!(
        e.get_next_chapter("c1", &facts(&["judge_wolf"])),
        Some("c_other")
    );
}

#[test]
fn intro_outro_text() {
    let e = engine();
    assert_eq!(e.get_intro_text("c1"), None);
    assert_eq!(e.get_outro_text("c_truth"), Some("真相结局。"));
    assert_eq!(e.get_outro_text("c1"), None);
}
