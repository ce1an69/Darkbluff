//! 校验：id 全局唯一性、场景连接/死胡同、章节细查（元数据/场景/角色话题/审判/线索）。

use std::collections::{HashMap, HashSet};

use crate::content::models::{Chapter, parse_dialogue_source};
use crate::world::World;

use super::Checker;

/// 引擎运行时保留的 id 命名空间：`__` 前缀（move 伪出口 `__leave`）与走不出去触发器
/// [`LEAVE_ATTEMPT_TRIGGER`](crate::content::LEAVE_ATTEMPT_TRIGGER)。作者内容不得声明，
/// 否则与内部哨兵碰撞（场景不可达 / 触发器互相遮蔽）。
fn is_reserved_id(id: &str) -> bool {
    id.starts_with("__") || id == crate::content::LEAVE_ATTEMPT_TRIGGER
}

impl<'a> Checker<'a> {
    /// 全局外键 id 唯一且非空（scene / character / chapter / clue / judgment）。
    pub(crate) fn check_id_uniqueness(&mut self) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for id in self.eng.scene_ids() {
            self.count_id(&mut counts, id, "场景");
        }
        for id in self.eng.character_ids() {
            self.count_id(&mut counts, id, "角色");
        }
        for id in self.eng.chapter_ids() {
            self.count_id(&mut counts, id, "章节");
        }
        for cid in self.eng.chapter_ids().collect::<Vec<_>>() {
            for clue in self.eng.get_clues(cid) {
                self.count_id(&mut counts, &clue.id, "线索");
            }
            for j in self.eng.get_judgments(cid) {
                self.count_id(&mut counts, &j.id, "审判点");
            }
            for n in self.eng.get_narrative(cid) {
                self.count_id(&mut counts, &n.id, "叙事触发器");
            }
        }
        for (id, n) in &counts {
            if *n > 1 {
                self.err(format!("id 全局重复：{id} 出现 {n} 次"));
            }
        }
    }

    fn count_id(&mut self, counts: &mut HashMap<String, usize>, id: &str, label: &str) {
        if id.is_empty() {
            self.err(format!("存在空的 {label} id"));
        }
        if is_reserved_id(id) {
            self.err(format!(
                "{label} id \"{id}\" 占用了引擎保留命名空间（\"__\" 前缀或走不出去触发器 \"leave_attempt\"），禁止作者使用"
            ));
        }
        *counts.entry(id.to_string()).or_insert(0) += 1;
    }

    pub(crate) fn check_scenes(&mut self) {
        for sid in self.eng.scene_ids() {
            self.check_scene_connections(sid);
            self.check_scene_exit_attempt(sid);
        }
        for sid in self.eng.scene_ids() {
            self.check_dead_end(sid);
        }
    }

    /// exit_attempt 文本文件存在（声明了 exit_attempt 的场景）。
    fn check_scene_exit_attempt(&mut self, scene_id: &str) {
        let Some(scene) = self.eng.get_scene(scene_id) else {
            return;
        };
        if scene.exit_attempt.is_some() && self.eng.scene_exit_attempt_text(scene_id).is_none() {
            self.err(format!("场景 {scene_id} 的 exit_attempt 文件缺失"));
        }
    }

    fn check_scene_connections(&mut self, scene_id: &str) {
        let Some(scene) = self.eng.get_scene(scene_id) else {
            return;
        };
        for c in scene
            .connections
            .iter()
            .chain(scene.one_way_connections.iter())
        {
            if !self.eng.scene_exists(c) {
                self.err(format!("场景 {scene_id} 的连接引用了未定义的场景：{c}"));
            }
        }
    }

    /// 单向死胡同：one_way 目标自身无任何出口 → error。
    fn check_dead_end(&mut self, scene_id: &str) {
        let Some(scene) = self.eng.get_scene(scene_id) else {
            return;
        };
        for t in &scene.one_way_connections {
            let Some(target) = self.eng.get_scene(t) else {
                continue;
            };
            if target.connections.is_empty() && target.one_way_connections.is_empty() {
                self.err(format!(
                    "场景 {scene_id} 的单向连接 {t} 无任何出口，构成死胡同陷阱"
                ));
            }
        }
    }

    pub(crate) fn check_chapters(&mut self) {
        for cid in self.eng.chapter_ids().collect::<Vec<_>>() {
            self.check_one_chapter(cid);
        }
    }

    fn check_one_chapter(&mut self, cid: &str) {
        let Some(ch) = self.eng.get_chapter(cid) else {
            return;
        };
        self.check_chapter_metadata(cid, ch);
        self.check_chapter_scenes(cid, ch);
        self.check_chapter_characters(cid, ch);
        self.check_chapter_judgments(cid, ch);
        self.check_chapter_clues(cid, ch);
        self.check_chapter_narrative(cid, ch);
    }

    /// 叙事触发器：label 非空、when 条件 id 有效、文本存在（id 唯一性已在
    /// `check_id_uniqueness` 全局校验）。
    fn check_chapter_narrative(&mut self, cid: &str, ch: &Chapter) {
        for n in &ch.narrative {
            if n.label.trim().is_empty() {
                self.warn(format!("章节 {cid} 叙事触发器 {} 缺少 label", n.id));
            }
            if let Some(cond) = &n.when {
                for id in super::cond_ids(cond) {
                    self.validate_condition_id(cid, id);
                }
            }
            if self.eng.get_narrative_text(cid, &n.id).is_none() {
                self.err(format!("章节 {cid} 叙事触发器 {} 的 text 文件缺失", n.id));
            }
        }
    }

    /// 标题 / next 必要性 / outro / intro / starting_scene。
    fn check_chapter_metadata(&mut self, cid: &str, ch: &Chapter) {
        if ch.title.trim().is_empty() {
            self.warn(format!("章节 {cid} 缺少标题"));
        }
        if ch.ending {
            if ch.next.is_some() {
                self.err(format!("终章 {cid} 不得定义 next"));
            }
        } else if ch.next.is_none() {
            self.err(format!("非终章 {cid} 必须提供 next"));
        }
        if !ch.ending && ch.outro.is_some() {
            self.err(format!("非终章 {cid} 定义了 outro（仅终章有效）"));
        }
        if ch.ending && ch.outro.is_some() && self.eng.get_outro_text(cid).is_none() {
            self.err(format!("终章 {cid} 的 outro 文件缺失"));
        }
        if ch.intro.is_some() && self.eng.get_intro_text(cid).is_none() {
            self.err(format!("章节 {cid} 的 intro 文件缺失"));
        }
        if !ch.scenes.iter().any(|s| s == &ch.starting_scene) {
            self.err(format!(
                "章节 {cid} 的 starting_scene {} 不在 scenes 列表中",
                ch.starting_scene
            ));
        }
    }

    /// 场景引用有效 + 描述完整性 + 孤立。
    fn check_chapter_scenes(&mut self, cid: &str, ch: &Chapter) {
        for s in &ch.scenes {
            if !self.eng.scene_exists(s) {
                self.err(format!("章节 {cid} 引用了未定义的场景：{s}"));
                continue;
            }
            if self
                .eng
                .get_scene_description(cid, s, World::Surface)
                .is_none()
            {
                self.err(format!("章节 {cid} 场景 {s} 缺少 surface 描述"));
            }
            if self
                .eng
                .get_scene_description(cid, s, World::Shadow)
                .is_none()
            {
                self.err(format!("章节 {cid} 场景 {s} 缺少 shadow 描述"));
            }
        }
        if ch.scenes.len() > 1 {
            for s in &ch.scenes {
                let reachable = self.eng.get_reachable_scenes(s);
                if reachable.is_empty() && !self.referenced_targets.contains(s) {
                    self.warn(format!("章节 {cid} 场景 {s} 无任何可达连接，疑似孤立场景"));
                }
            }
        }
    }

    /// 角色引用 / 话题存在 / 影子对照建议。
    fn check_chapter_characters(&mut self, cid: &str, ch: &Chapter) {
        let mut char_ids: HashSet<String> = HashSet::new();
        for cc in &ch.characters {
            if !self.eng.character_exists(&cc.id) {
                self.err(format!("章节 {cid} 引用了未定义的角色：{}", cc.id));
            }
            if !char_ids.insert(cc.id.clone()) {
                self.err(format!("章节 {cid} 角色重复声明：{}", cc.id));
            }
            self.check_appears_in(cid, &cc.id, cc.appears_in.as_ref(), ch);
            let mut topic_seen: HashSet<String> = HashSet::new();
            for t in &cc.topics {
                if !topic_seen.insert(t.id.clone()) {
                    self.err(format!(
                        "章节 {cid} 角色 {} 的话题 id 重复：{}",
                        cc.id, t.id
                    ));
                }
                if !self.eng.dialogue_topic_exists(cid, &cc.id, &t.id) {
                    self.err(format!(
                        "章节 {cid} 角色 {} 的话题 {} 在对话文件中不存在",
                        cc.id, t.id
                    ));
                }
                let has_surf = self
                    .eng
                    .get_dialogue(cid, &cc.id, &t.id, World::Surface)
                    .is_some();
                let has_shadow = self
                    .eng
                    .get_dialogue(cid, &cc.id, &t.id, World::Shadow)
                    .is_some();
                if has_shadow && !has_surf {
                    self.advice(format!(
                        "章节 {cid} 角色 {} 话题 {} 仅有影子版本，缺少表面对照",
                        cc.id, t.id
                    ));
                }
            }
        }
    }

    fn check_appears_in(
        &mut self,
        cid: &str,
        character: &str,
        appears_in: Option<&Vec<String>>,
        ch: &Chapter,
    ) {
        let Some(list) = appears_in else {
            return;
        };
        for s in list {
            if !ch.scenes.iter().any(|x| x == s) {
                self.warn(format!(
                    "章节 {cid} 角色 {character} 的 appears_in 引用了非本章场景：{s}"
                ));
            }
            if !self.eng.scene_exists(s) {
                self.err(format!(
                    "章节 {cid} 角色 {character} 的 appears_in 引用了未定义场景：{s}"
                ));
            }
        }
    }

    /// 审判点 / required_judgments / 跳转目标。
    fn check_chapter_judgments(&mut self, cid: &str, ch: &Chapter) {
        let judgments = self.eng.get_judgments(cid);
        if judgments.is_empty() {
            self.err(format!("章节 {cid} 必须至少定义一个审判点"));
        }
        let mut targets: HashSet<String> = HashSet::new();
        let mut judgment_ids: HashSet<String> = HashSet::new();
        for j in judgments {
            if !judgment_ids.insert(j.id.clone()) {
                self.err(format!("章节 {cid} 审判点 id 重复：{}", j.id));
            }
            if !ch.characters.iter().any(|c| c.id == j.target) {
                self.err(format!(
                    "章节 {cid} 审判 {} 的 target {} 未在本章 characters 中声明",
                    j.id, j.target
                ));
            }
            if !targets.insert(j.target.clone()) {
                self.err(format!(
                    "章节 {cid} 角色 {} 被多个审判点指向（一章内一个角色一个审判点）",
                    j.target
                ));
            }
            if self.eng.get_result_text(cid, &j.id).is_none() {
                self.err(format!("章节 {cid} 审判 {} 的 result 文件缺失", j.id));
            }
        }
        self.check_required_judgments(cid, judgments, &ch.required_judgments);
        if !ch.ending {
            for t in self.eng.next_targets(cid) {
                if !self.eng.chapter_exists(t) {
                    self.err(format!("章节 {cid} 的跳转目标不存在：{t}"));
                }
            }
        }
    }

    fn check_required_judgments(
        &mut self,
        cid: &str,
        judgments: &[crate::content::models::Judgment],
        required: &Option<Vec<String>>,
    ) {
        match required {
            None => {}
            Some(req) if req.is_empty() => {
                self.err(format!(
                    "章节 {cid} 的 required_judgments 显式为空数组（不合法）"
                ));
            }
            Some(req) => {
                let valid: HashSet<&str> = judgments.iter().map(|j| j.id.as_str()).collect();
                for r in req {
                    if !valid.contains(r.as_str()) {
                        self.err(format!(
                            "章节 {cid} 的 required_judgments 引用了未知审判点：{r}"
                        ));
                    }
                }
            }
        }
    }

    /// 线索 source / world 一致性。
    fn check_chapter_clues(&mut self, cid: &str, ch: &Chapter) {
        for clue in self.eng.get_clues(cid) {
            let Some((cchar, ctopic)) = parse_dialogue_source(&clue.source) else {
                self.err(format!(
                    "章节 {cid} 线索 {} 的 source 格式非法：{}",
                    clue.id, clue.source
                ));
                continue;
            };
            if !ch.characters.iter().any(|c| c.id == cchar) {
                self.err(format!(
                    "章节 {cid} 线索 {} 的 source 引用了未知角色：{cchar}",
                    clue.id
                ));
            }
            if !self.eng.dialogue_topic_exists(cid, cchar, ctopic) {
                self.err(format!(
                    "章节 {cid} 线索 {} 的 source 引用了不存在的话题：{}.{ctopic}",
                    clue.id, cchar
                ));
            }
            if self
                .eng
                .get_dialogue(cid, cchar, ctopic, clue.world)
                .is_none()
            {
                self.err(format!(
                    "章节 {cid} 线索 {} 的 world({}) 与对话 {}.{ctopic} 实际存在的世界不一致",
                    clue.id,
                    clue.world.label(),
                    cchar
                ));
            }
        }
    }
}
