//! 启动校验：在不启动 TUI 的情况下校验 `data/` 内容的引用完整性。
//!
//! 设计见 docs/content-engine.md「启动校验」。校验器只依赖 [`ContentEngine`]：
//! `load` 阶段已把缺失文件降级为「文本为 `None`」，因此各类「路径存在」校验通过
//! 查询接口是否返回 `Some` 判定。返回 [`CheckReport`]（错误 / 警告 / 建议），
//! 可被 `darkbluff check` 直接使用。
//!
//! 实现按校验类别拆分：本模块含类型、调度、条件校验与章节图校验；
//! id 唯一性、场景与章节细查见 [`chapters`]。

mod chapters;

use std::collections::{HashMap, HashSet};

use crate::content::engine::ContentEngine;
use crate::content::models::Condition;

/// 校验问题严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 阻断：内容无法正确运行。
    Error,
    /// 非阻断但很可能有问题。
    Warning,
    /// 内容审校提示（作者守则类）。
    Advice,
}

impl Severity {
    pub fn is_error(self) -> bool {
        matches!(self, Severity::Error)
    }
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Advice => "advice",
        }
    }
}

/// 一条校验问题。
#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

/// 校验结果。
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    pub issues: Vec<Issue>,
}

impl CheckReport {
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity.is_error())
    }
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity.is_error())
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| !i.severity.is_error())
    }
}

/// 运行全部启动校验。
pub fn check(engine: &ContentEngine) -> CheckReport {
    let mut c = Checker::new(engine);
    c.run();
    CheckReport { issues: c.issues }
}

struct Checker<'a> {
    pub(crate) eng: &'a ContentEngine,
    pub(crate) issues: Vec<Issue>,
    /// 线索 id → 定义所在章节（首次出现）。
    pub(crate) clue_owner: HashMap<String, String>,
    /// 审判点 id → 定义所在章节（首次出现）。
    pub(crate) judgment_owner: HashMap<String, String>,
    /// 叙事触发器 id → 定义所在章节（首次出现）。
    pub(crate) narrative_owner: HashMap<String, String>,
    /// 所有场景作为他人连接/单向连接目标的集合（孤立判定用；单向入边也算可达）。
    pub(crate) referenced_targets: HashSet<String>,
}

impl<'a> Checker<'a> {
    fn new(eng: &'a ContentEngine) -> Self {
        let mut clue_owner = HashMap::new();
        let mut judgment_owner = HashMap::new();
        let mut narrative_owner = HashMap::new();
        for cid in eng.chapter_ids() {
            for clue in eng.get_clues(cid) {
                clue_owner
                    .entry(clue.id.clone())
                    .or_insert_with(|| cid.to_string());
            }
            for j in eng.get_judgments(cid) {
                judgment_owner
                    .entry(j.id.clone())
                    .or_insert_with(|| cid.to_string());
            }
            for n in eng.get_narrative(cid) {
                narrative_owner
                    .entry(n.id.clone())
                    .or_insert_with(|| cid.to_string());
            }
        }
        let mut referenced_targets: HashSet<String> = HashSet::new();
        for sid in eng.scene_ids() {
            if let Some(scene) = eng.get_scene(sid) {
                for t in scene
                    .connections
                    .iter()
                    .chain(scene.one_way_connections.iter())
                {
                    referenced_targets.insert(t.clone());
                }
            }
        }
        Self {
            eng,
            issues: Vec::new(),
            clue_owner,
            judgment_owner,
            narrative_owner,
            referenced_targets,
        }
    }

    fn run(&mut self) {
        self.check_id_uniqueness();
        self.check_scenes();
        self.check_chapters();
        self.check_conditions();
        self.check_graph();
    }

    pub(crate) fn err(&mut self, msg: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            message: msg.into(),
        });
    }
    pub(crate) fn warn(&mut self, msg: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            message: msg.into(),
        });
    }
    pub(crate) fn advice(&mut self, msg: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Advice,
            message: msg.into(),
        });
    }

    /// 条件表达式引用的 id 有效（线索或审判点），跨章线索给出建议。
    fn check_conditions(&mut self) {
        for cid in self.eng.chapter_ids().collect::<Vec<_>>() {
            let Some(ch) = self.eng.get_chapter(cid) else {
                continue;
            };
            for cc in &ch.characters {
                for t in &cc.topics {
                    if let Some(cond) = &t.unlock_after {
                        for id in cond_ids(cond) {
                            self.validate_condition_id(cid, id);
                        }
                    }
                }
            }
            if let Some(next) = &ch.next {
                for b in &next.branches {
                    for id in cond_ids(&b.when) {
                        self.validate_condition_id(cid, id);
                    }
                }
            }
        }
    }

    fn validate_condition_id(&mut self, current_chapter: &str, id: &str) {
        let is_clue = self.clue_owner.contains_key(id);
        let is_judg = self.judgment_owner.contains_key(id);
        let is_narr = self.narrative_owner.contains_key(id);
        if !is_clue && !is_judg && !is_narr {
            self.err(format!("章节 {current_chapter} 的条件引用了未知 id：{id}"));
            return;
        }
        if is_clue {
            if let Some(owner) = self.clue_owner.get(id) {
                if owner != current_chapter {
                    self.advice(format!(
                        "线索 {id} 定义在章节 {owner}，被章节 {current_chapter} 引用，请确认 {owner} 是 {current_chapter} 的前置章节"
                    ));
                }
            }
        }
    }

    /// 章节图：唯一根、DAG、可达性。
    fn check_graph(&mut self) {
        let chapters: Vec<String> = self.eng.chapter_ids().map(|s| s.to_string()).collect();
        let mut incoming: HashSet<String> = HashSet::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for c in &chapters {
            let mut targets = Vec::new();
            for t in self.eng.next_targets(c) {
                if self.eng.chapter_exists(t) {
                    incoming.insert(t.to_string());
                    targets.push(t.to_string());
                }
            }
            adj.insert(c.clone(), targets);
        }
        let roots: Vec<String> = chapters
            .iter()
            .filter(|c| !incoming.contains(*c))
            .cloned()
            .collect();
        match roots.len() {
            0 => self.err("没有任何根节点（所有章节都有入度），缺少首章"),
            1 => {}
            n => self.err(format!("存在 {n} 个根节点（无入度章节），首章必须唯一")),
        }

        // 环检测（DFS 三色标记）
        let mut color: HashMap<String, u8> = HashMap::new();
        for c in &chapters {
            if self.has_cycle(c, &adj, &mut color) {
                self.err(format!(
                    "章节图存在环（涉及节点 {c}），next 指针不得构成循环"
                ));
                break;
            }
        }

        // 可达性：从根遍历，未达章节 → 孤立 warning
        if let Some(root) = roots.first() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut stack = vec![root.clone()];
            while let Some(node) = stack.pop() {
                if visited.insert(node.clone()) {
                    if let Some(ns) = adj.get(&node) {
                        for n in ns {
                            stack.push(n.clone());
                        }
                    }
                }
            }
            for c in &chapters {
                if !visited.contains(c) {
                    self.warn(format!("章节 {c} 从首章不可达，疑似孤立章节"));
                }
            }
        }
    }

    fn has_cycle(
        &self,
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        color: &mut HashMap<String, u8>,
    ) -> bool {
        match color.get(node) {
            Some(2) => return false,
            Some(1) => return true,
            _ => {}
        }
        color.insert(node.to_string(), 1);
        if let Some(ns) = adj.get(node) {
            for n in ns {
                if self.has_cycle(n, adj, color) {
                    return true;
                }
            }
        }
        color.insert(node.to_string(), 2);
        false
    }
}

fn cond_ids(cond: &Condition) -> Vec<&str> {
    match cond {
        Condition::Fact(id) => vec![id.as_str()],
        Condition::AllOf(ids) | Condition::AnyOf(ids) => ids.iter().map(|s| s.as_str()).collect(),
        Condition::Not(id) => vec![id.as_str()],
    }
}
