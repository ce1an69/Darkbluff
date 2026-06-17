# 概念清单 (Glossary)

> 属于 DarkBluff 设计文档。主索引见 [design.md](design.md)。本页汇总全部核心概念（英文命名 + 解释），便于实现时查词典、新人入门。命名遵循文档约定：代码/数据侧用 snake_case，类型/状态用 PascalCase。🆕 标记近期新增或明确的概念。

## 1. 世界观与推理机制

| 命名 | 解释 |
|------|------|
| `surface` (Surface World) | 表面世界，角色说真话，信息恒为真命题 |
| `shadow` (Shadow World) | 影子世界，角色无意识说谎，信息恒为假命题（取反即真相） |
| `World` | 视角枚举，取值 `surface` / `shadow` |
| `gaze` | 切换左右眼（影子/表面视角）的动作 |
| Binary Truth Model | 二元真值模型：表面恒真、影子恒假，贯穿全游戏，难度不靠破坏它提升 |
| `current_world` | 当前视角，进入新章节时默认 `surface` |
| single-world topic | 单世界话题：对话只存在 `[surface]` 或 `[shadow]` 一个版本 |
| Shadow text author guideline | 影子文本作者守则：影子文本须是其逻辑否定有意义且无歧义的命题 |

## 2. 内容实体 (Content Entities)

> 每个实体都有唯一 `id`（展示引用 + 存档外键 + 条件标记，发布冻结），`name` 承担中文展示。

| 命名 | 解释 |
|------|------|
| `Chapter` | 章节，一个独立案件（15–30 分钟），流程推进的单位 |
| `Scene` | 场景，玩家可移动到达的地点，含 `connections`（默认双向）/ `one_way_connections`（单向）与 surface/shadow 描述 |
| `Character` | 角色，可被 `ask`/`judge`；仅含 `id` 与中文展示名 `name` |
| `Topic` | 话题，对话主题，有 `available` 与可选 `unlock_after` 可见性 |
| `Dialogue` | 对话文本（Markdown），按角色拆分，由「章节+角色+话题」定位（无独立 id） |
| `Clue` | 线索，纯逻辑层机制；由 `ask` 触发对话时收集，用于解锁话题 |
| `Judgment` | 审判点，`target` 指向某角色（章级操作，不要求角色在场），触发该角色一段固定 `result` 审判剧情；一章内一个角色一个审判点 |
| `appears_in` | 角色本章出场的场景列表；`ask` 校验目标角色在场（`judge` 为章级操作，不校验 `appears_in`） |

## 3. 标识符 (Identifiers)

| 命名 | 解释 |
|------|------|
| `id` | 每个实体唯一的可读标识（snake_case），**三重职责**：展示引用、存档外键、条件标记 |
| 全局唯一性 | `chapter`/`scene`/`character`/`clue`/`judgment` 的 id 全局唯一；`topic` id 仅需「章节 × 角色」内唯一 |
| 发布即冻结 | id 一旦发布不得改名/复用，改名会破坏旧存档与条件引用 |
| `name` | 中文展示名，与英文 `id` 分离 |

## 4. 章节结构与跳转 (Chapter & Navigation)

| 命名 | 解释 |
|------|------|
| `scenes` / `starting_scene` | 本章场景列表 / 进入本章的初始场景 |
| `required_judgments` | 自动推进前必须完成的审判点 id；显式空数组不合法 |
| `next.default` / `next.branches` | 默认跳转目标 / 分支跳转（按序匹配，首条命中生效） |
| `when` / `target` | 分支条件表达式 / 命中跳转目标 |
| `ending` | 终章标记，`true` 时无 `next`，完成最后一个必要审判时记达成结局 |
| Chapter Tree | 由所有章节 `next` 指针构建的章节树；必须是**有向无环图** |
| 自动推进章节 | `judge` 后若本章 `required_judgments` 已全部完成，系统自动按审判结果跳转下一章或进入结局 |
| 🆕 首章 (root chapter) | 章节树的**唯一根节点**（无入度），新游戏从此开始；启动校验有且仅有一个根 |
| 🆕 `intro` | 可选的章节开场/过场叙事文本（Markdown）；进入本章时先展示，玩家确认后再进 `starting_scene`；展示时写入快照供 `note`「叙事」标签页回顾 |
| 🆕 `outro` | 可选的终章结局收尾文本（Markdown），仅 `ending: true` 有效；完成终章最后一个必要审判后展示，确认后进入 `Ending` 状态；展示时写入快照供 `note`「叙事」标签页回顾 |

## 5. 条件表达式与 FactSet

| 命名 | 解释 |
|------|------|
| Condition Expression | 扁平条件表达式，直接引用 id，用于话题解锁与章节跳转 |
| `all_of` / `any_of` / `not` | 全部满足 / 任一满足 / 不满足；**不允许嵌套** |
| `FactSet` | 事实集合 = 已收集线索 id ∪ 已审判审判点 id，合并当前及之前所有章节，供条件求值 |

## 6. 存档状态字段 (Save State)

> 单存档自动保存，采用「存储事实为权威」模型——事实直接存储，检查点只记数组长度。

| 命名 | 解释 |
|------|------|
| `version` / `timestamp` | 存档结构版本号 / 最后保存时间 |
| `current_chapter` / `current_scene` / `current_world` | 当前章节 / 场景 / 视角 |
| `collected_clues` | 已收集线索 id（按章节分组，权威存储） |
| `viewed_dialogues` | 已查看对话索引 + 快照路径（按章节分组） |
| 🆕 `viewed_intros` | 已展示的章节开场文本快照路径（按章节分组） |
| 🆕 `viewed_outros` | 已展示的终章结局文本快照路径（按章节分组） |
| `judgments_made` | 已审判记录（`judgment` + `result_snapshot`，按章节分组） |
| `chapter_path` | 当前流程经过的章节路径（树状高亮 + `map` 跨章回滚范围来源） |
| `checkpoints` | 自动检查点，记录位置 + 当前章节三数组长度 |

## 7. 检查点与回滚 (Checkpoint & Rollback)

| 命名 | 解释 |
|------|------|
| `Checkpoint` | 检查点，只记位置 + 当前章节三数组长度，不存完整快照 |
| `Checkpoint.kind` | `chapter_start`（进入章节）/ `before_judgment`（玩家**执行 `judge` 审判时**；无参数选角色 Esc 取消不创建） |
| `Checkpoint.state` | `clues_len` / `views_len` / `judgments_len`，回滚按长度截断 |
| `map` checkpoint 回滚 | 通过 `map` 选择已经历过的 `chapter_start` 或 `before_judgment` checkpoint，回到该节点 |
| 当前章回滚 | 选择当前章的 checkpoint，截断当前章 checkpoint 之后的事实与检查点 |
| 跨章回滚 | 选择早前章节 checkpoint，销毁性丢弃回滚点之后的当前流程进度与检查点，`discovered` 保留 |

## 8. 探索记忆 (Discovered)

| 命名 | 解释 |
|------|------|
| `discovered` | append-only 探索记忆，任何回滚都不截断；set 语义（去重） |
| `discovered.chapters` / `.endings` / `.topics` | 曾到过的章节 / 曾达成的结局 / 曾问过的话题（按章节分组） |
| 🆕 有序去重 | `chapters` 保留首次到达顺序，供 `map` 章节树展示与探索进度排序 |
| 🆕 话题进度标注 | map 面板按角色聚合显示「话题 X/Y」：X = `discovered.topics` 中该角色本章已问话题数，Y = 该角色本章可问话题总数（不含永久不可问）；只给计数不给未问 label（剧透安全） |

## 9. 笔记与快照 (Notes & Snapshots)

| 命名 | 解释 |
|------|------|
| `note` | 查看玩家实际见过的文本记录（对话/叙事/审判剧情），含章节树标签页 |
| `Snapshot` | 对话快照，首次 `ask` 时写入渲染前 Markdown 原文，保证推理公平性 |
| Snapshot immutability | 快照不随后续剧情漂移；缺失则提示，不用最新文本替代 |
| 🆕 intro / outro / 审判快照 | 章节开场、终章结局与审判剧情文本的快照（进入章节 / 完成终章最后一个必要审判 / 执行审判时写入），供 note「叙事」「审判」标签页回顾 |

## 10. 指令 (Commands)

| 命名 | 解释 |
|------|------|
| `ask` | 从在场角色收集信息；无话题弹话题菜单；Esc 取消回到 Exploring |
| 🆕 `ask` 无参数 | 两步菜单：当前场景在场角色 → 该角色可问话题 |
| `judge` | 审判某角色（章级操作，不要求角色在场），触发固定审判剧情；无选项；完成必要审判后自动推进 |
| 🆕 `judge` 无参数 | 列出本章**未审判的角色**，选择后触发其审判剧情 |
| `move` / `gaze` | 移动（无参数列可达连接）/ 切换视角；场景描述面板自动刷新 |
| `map` | 打开章节树 / checkpoint 地图，可选择已经历过的 checkpoint 回滚 |
| `note` / `help` / `quit` | 查看笔记 / 查看指令用法 / 保存退出 |

## 11. 内容引擎与校验 (Content Engine)

| 命名 | 解释 |
|------|------|
| `ContentEngine` | 无状态的加载/查询层，只做加载和查询 |
| `darkbluff check` | 离线校验命令，不启动 TUI |
| Startup validation | 启动校验：引用完整性、id 全局唯一、条件扁平、章节图无环、可达性 |
| 🆕 首章校验 | 有且仅有一个根节点（无入度），该根即首章 |
| 🆕 `get_characters_in_scene` | 查询某场景在场角色（供 `ask` 无参数菜单） |
| 🆕 `get_topics` | 查询某角色话题原始数据（可见性由引擎层用 FactSet 求值） |
| Scene override / `DataSource` / `include_dir` | 章节覆盖 / 内嵌·外置双模式 |
| 🆕 `connections` / `one_way_connections` | 场景连接：`connections` 默认双向（引擎自动补反向），`one_way_connections` 为单向 |
| 🆕 场景描述完整性 | 启动校验强制每个被章节引用的场景同时有 surface 和 shadow 描述 |
| 🆕 `serde_yml` | YAML 解析库，`serde_yaml` 的活跃 fork |

## 12. 应用状态机与运行时 (App State Machine & Runtime)

| 命名 | 解释 |
|------|------|
| App state | `Title` / `ShowingIntro` / `Exploring` / `ChoosingTopic` / `ViewingNote` / `ViewingMap` / `Confirming` / `Transitioning` / `ShowingOutro` / `Ending` |
| 🆕 `ShowingIntro` | 章节开场/过场文本（`intro`）展示中，确认后进入 `Exploring` |
| 🆕 `ShowingOutro` | 终章结局文本（`outro`）展示中，确认后进入 `Ending`；无 `outro` 则跳过 |
| 🆕 `ChoosingCharacter` | `ask`/`judge` 无参数第一步：选场景在场角色（ask）或本章未审判角色（judge） |
| 🆕 `ViewingMap` | 章节树 / checkpoint 地图浏览中，可选择已经历过的 checkpoint 回滚 |
| `settings.json` / `motion` | 独立设置文件 / 动画偏好（`full`/`reduced`/`off`） |
| Atomic write | 原子写入（`.tmp`→rename）+ `.bak` 备份 + 损坏回退 |
| CLI | `darkbluff`(=play) / `play` / `check` 子命令；`--no-motion`、`--data-dir` flag |
| 🆕 Esc 取消 | 所有选择菜单支持 Esc 取消回到 Exploring，不产生副作用 |
| 🆕 UI Warning | 存档损坏恢复、内容引用失效等场景在 TUI 中以通知条形式向玩家展示 |
| 🆕 快照压缩 | `map` 回滚后自动保存时清理 `viewed_dialogues`/`viewed_intros`/`viewed_outros`/`judgments_made` 未引用的孤儿快照文件 |
| 🆕 新游戏初始化 | 清空 snapshots + 删除 .bak + 生成空存档 + 创建首章检查点 |
