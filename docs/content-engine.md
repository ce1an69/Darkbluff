# 内容引擎

> 属于 DarkBluff 设计文档。主索引见 [design.md](design.md)。引擎加载的数据格式见 [data-formats.md](data-formats.md)；指令执行中的运行时错误处理见 [commands.md](commands.md)。

内容引擎是一个无状态的加载/查询层，负责将 `data/` 目录下的所有游戏数据加载到内存，并提供统一的查询接口。

## 核心职责

1. **自动发现**: 扫描 `data/chapters/` 下所有子目录，每个含 `chapter.yaml` 的目录注册为一个章节
2. **启动校验**: 验证所有引用关系的完整性
   - 话题 ID 在对应 `.md` 文件中存在
   - 章节跳转目标有效（`next.default` 与 `next.branches[].target` 都指向已注册章节）
   - 场景/角色 ID 在全局定义中存在
   - 审判 `target` 是本章 `characters` 中声明的角色（`judge` 是章级操作，不校验 `appears_in`），且本章 `target` 不重复（一个角色一段审判）
   - 每个章节必须至少定义一个审判点；v1 不支持无审判推进章节
   - `required_judgments` 引用的审判点 id 在本章 `judgments.yaml` 中定义，且显式配置时不能为空；省略时默认本章全部审判
   - 线索 `source` 引用的角色和话题有效，且 `world` 与该对话实际存在的世界版本一致（单世界话题的线索 `world` 必须匹配）
   - 所有外键类 id（chapter / scene / character / clue / judgment）**全局唯一**且非空
   - 条件表达式引用的线索和审判点 id 有效，且结构扁平（无嵌套）
   - 条件表达式引用的线索 id，若定义在与当前章节不同的章节中，输出 **advisory warning**"线索 `{id}` 定义在章节 `{source_chapter}` 中，被章节 `{current_chapter}` 的 `unlock_after` 引用，请确认前者是后者的前置章节"（非阻断错误，仅提示内容作者复查）
   - 章节图为**有向无环图**（`next` 指针不得构成环）
   - 非终章（`ending` 为 `false` 或缺省）**必须提供 `next` 字段**（至少含 `default`）；终章**不得提供 `next` 字段**
   - **`outro` 仅终章有效**: 非终章定义了 `outro` 字段为 error；终章的 `outro` 引用的文件路径必须存在
   - **有且仅有一个根节点**（无入度，即没有任何章节的 `next` 指向它），该根节点即首章；新游戏从此开始。零个或多个根节点均为错误
   - 所有非根章节应从根（首章）可达；不可达的章节输出 warning（孤立章节）
   - `results/`、场景描述和对话文件路径存在
   - 每个影子文本块有对应的表面对照（内容审校提示，见 [gameplay.md](gameplay.md)「作者守则」）
   - **场景描述完整性**: 每个被章节引用的场景，在该章必须同时有 surface 和 shadow 描述（章节覆盖或全局默认均可）。缺任一版本为错误——运行时不存在视角缺失
   - **场景连接有效性**: `connections` 和 `one_way_connections` 引用的场景 ID 必须在全局场景定义中存在
   - **场景孤立检测**: 被章节引用的场景若最终无任何可达连接（既无 `connections`，也不在任何其他场景的 `connections`/`one_way_connections` 中——**单向入边也算可达**，避免把单向死胡同目标误判为孤立）且本章有多个场景，输出 warning（孤立场景，玩家无法到达）
   - **默认双向连接补全**: `connections` 中的连接默认双向，引擎在加载时自动补全反向连接；`one_way_connections` 中的连接为单向，不补反向
   - **单向死胡同检测**: `one_way_connections` 指向的目标场景，若自身既无 `connections` 也无 `one_way_connections`（即无任何出口），输出 **error**——玩家单向进入后无法离开，构成死锁陷阱（孤立检测只管「到不了」，死胡同是更隐蔽的「到了回不来」）。作者须确保单向目标至少有一个独立出口，或改为双向 `connections`
3. **id 索引**: 建立 id → 实体的索引，支持存档加载（按 id 直接匹配）与内容查询
4. **场景覆盖透明化**: 查询场景描述时，内部处理覆盖逻辑，调用方无需关心来源
5. **Markdown AST 解析**: 使用 Markdown AST 解析对话层级，并缓存解析结果
6. **内嵌/外置双模式**: 通过 feature flag 切换
   - 开发模式：从文件系统读取 `data/` 目录
   - 发布模式：通过 `include_dir!` 将数据内嵌到二进制文件
7. **独立校验命令**: 校验能力必须可被 `darkbluff check` 调用，便于在 CI 或内容编辑流程中提前发现错误

## FactSet

条件表达式求值与章节跳转需要一个「事实集合」作为输入：

> **`FactSet`** = 玩家已收集的线索 id 集合 ∪ 已审判的审判点 id 集合。

它由存档的 `collected_clues` 与 `judgments_made`（见 [save-system.md](save-system.md)）在当前章节及之前章节范围内合并得到，传给 `get_next_chapter` 等查询以评估条件。

## 条件求值实现

本节给出条件表达式的数据结构、FactSet 构造与求值的精确算法（对应 `engine/condition.rs`）。

**Condition 数据结构** ——把 YAML 条件解析为枚举：

```rust
enum Condition {
    Fact(String),        // 裸 id：单一事实存在
    AllOf(Vec<String>),  // 全部满足
    AnyOf(Vec<String>),  // 任一满足
    Not(String),         // 单个 id 的否定
}
```

解析规则：YAML 中裸字符串（如 `wolf_alibi`）→ `Fact`；`{all_of: [...]}` → `AllOf`；`{any_of: [...]}` → `AnyOf`；`{not: id}` → `Not`。结构必须扁平（启动校验保证无嵌套）。

**FactSet 构造** ——从存档合并 `chapter_path` 中所有章节（首章到当前章，含当前章）的事实：

```rust
fn build_factset(save) -> HashSet<String> {
    let mut facts = HashSet::new();
    for ch in &save.chapter_path {                  // 有序，末尾即当前章
        if let Some(clues) = save.collected_clues.get(ch) {
            facts.extend(clues.iter().cloned());
        }
        if let Some(judgs) = save.judgments_made.get(ch) {
            for j in judgs { facts.insert(j.judgment.clone()); }
        }
    }
    facts
}
```

范围说明：`judge` 后自动推进章节时，当前章的审判已完成并纳入（`chapter_path` 含当前章），因此跳转条件能反映本章选择；话题解锁用同一 FactSet。

**求值**：

```rust
fn eval(cond: &Condition, facts: &HashSet<String>) -> bool {
    match cond {
        Fact(id)    => facts.contains(id),
        AllOf(ids)  => ids.iter().all(|id| facts.contains(id)),
        AnyOf(ids)  => ids.iter().any(|id| facts.contains(id)),
        Not(id)     => !facts.contains(id),
    }
}
// 约定：空 AllOf = true（空真），空 AnyOf = false
```

**两处应用**：

```rust
// a. 话题可见性（展示菜单时实时求值；派生状态，不存档）
fn topic_visible(topic: &Topic, facts: &FactSet) -> bool {
    if topic.available { true }
    else { topic.unlock_after.as_ref().map_or(false, |c| eval(c, facts)) }
    // available:false 且无 unlock_after → false（永久不可问）
}

// b. 章节跳转（必要审判完成后自动推进，按序匹配，首条命中生效，否则走 default）
fn next_chapter(chapter: &Chapter, facts: &FactSet) -> &str {
    for branch in &chapter.next.branches {
        if eval(&branch.when, facts) { return branch.target; }
    }
    chapter.next.default
}
```

**约定与边界**:

- **空集约定**：空 `all_of` 恒真、空 `any_of` 恒假（数学约定，便于表达「无条件触发」）
- **未知 id**（内容已删除）：`facts.contains` 返回 `false`，条件不满足——安全降级。启动校验应在发布前拦截所有未知 id 引用
- **派生性**：话题可见性与跳转目标都不进存档，每次需要时用 FactSet 实时求值；FactSet 可在章节切换 / 事实变更时缓存并失效重建（规模小，O(n)）
- **跨章线索**：线索 id 全局唯一，FactSet 合并所有到过章节，因此某章 `unlock_after` 可引用前面章节收集的线索

## 查询接口（概念）

```rust
impl ContentEngine {
    /// 初始化：扫描并加载所有内容，校验引用完整性
    fn load(source: DataSource) -> Result<Self>;

    /// 章节查询
    fn get_chapter(id: &str) -> Option<&Chapter>;
    fn list_chapters() -> Vec<&ChapterMeta>;
    fn get_chapter_tree() -> &ChapterTree;
    fn get_next_chapter(chapter_id: &str, facts: &FactSet) -> Option<&str>;  // 返回目标章节 id

    /// 场景查询（自动处理章节覆盖）
    fn get_scene(id: &str) -> Option<&Scene>;
    fn get_scene_description(chapter_id: &str, scene_id: &str, world: World) -> Option<&str>;

    /// 角色查询
    fn get_character(id: &str) -> Option<&Character>;
    /// 场景在场角色（本章 characters[].appears_in 含该场景的角色，供 `ask` 无参数菜单）
    fn get_characters_in_scene(chapter_id: &str, scene_id: &str) -> Vec<&Character>;

    /// 话题查询（返回原始话题数据含 available/unlock_after；可见性由引擎层用 FactSet 求值）
    /// 菜单构建时引擎层还需按当前 world 过滤掉该世界无版本的 single-world topic
    fn get_topics(chapter_id: &str, character_id: &str) -> &[Topic];

    /// 对话查询
    fn get_dialogue(chapter_id: &str, character_id: &str, topic_id: &str, world: World) -> Option<&str>;

    /// 审判查询（target 角色可由此推导，供 `judge` 无参数列出本章未审判的角色）
    fn get_judgments(chapter_id: &str) -> &[Judgment];

    /// 线索查询
    fn get_clues(chapter_id: &str) -> &[Clue];

    /// 场景可达连接（含引擎自动补全的反向连接和 one_way_connections）
    fn get_reachable_scenes(scene_id: &str) -> Vec<&str>;
}
```

## 运行时降级策略

启动校验在正常流程中应当捕获所有引用问题，但在内容热更新或数据文件意外变更等极端场景下，运行时也需安全降级：

| 场景 | 降级策略 |
|------|----------|
| 自动推进计算出的目标章节不存在（branch target 失效） | 跳过该分支，继续匹配下一条 branch；所有 branch 均失效时走 `next.default` |
| `next.default` 指向的章节也不存在 | 报错提示"无法推进到下一章节"，保持当前章节不变，记录 error 日志 |
| 存档引用的当前章节不存在 | 回退到 `chapter_path` 中最后一个有效章节（见 [save-system.md](save-system.md)「兼容性策略」） |
