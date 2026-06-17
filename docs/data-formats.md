# 数据格式

> 属于 DarkBluff 设计文档。主索引见 [design.md](design.md)。内容引擎如何加载/校验这些数据见 [content-engine.md](content-engine.md)。

所有叙事内容均为手写，结构化数据使用 YAML，大段叙事文本使用 Markdown，YAML 通过路径字符串引用 Markdown 文件，引擎运行时加载。

## 目录结构

```
data/
├── scenes/                          # 全局场景定义
│   ├── tavern.yaml
│   └── market.yaml
├── characters/                      # 全局角色定义
│   ├── wolf.yaml
│   └── crow.yaml
├── text/                            # 全局场景描述文本
│   └── scenes/
│       ├── tavern.surface.md
│       ├── tavern.shadow.md
│       └── ...
└── chapters/                        # 按章节组织（语义化 ID）
    ├── the_missing_butcher/
    │   ├── chapter.yaml             # 章节元数据 + 话题列表 + 解锁条件 + 跳转逻辑
    │   ├── judgments.yaml           # 审判逻辑
    │   ├── clues.yaml              # 线索定义
    │   ├── dialogues/              # 对话文本（按角色拆分）
    │   │   ├── wolf.md
    │   │   └── crow.md
    │   ├── results/                # 审判剧情文本
    │   │   ├── wolf_trial.md
    │   │   └── crow_trial.md
    │   └── scenes/                 # 章节场景覆盖（可选）
    │       ├── tavern.surface.md
    │       └── tavern.shadow.md
    ├── tavern_truth/
    │   └── ...
    └── tavern_deceit/
        └── ...
```

## ID 与路径约定

- `id` 是每个内容实体唯一的可读标识（snake_case），**同时承担展示引用、存档外键、条件标记三重职责**——本游戏只使用这一套 id，不再设独立的稳定 id
- **全局唯一性**：`chapter` / `scene` / `character` / `clue` / `judgment` 的 id 必须**全局唯一**；`topic` id 例外，只需在「章节 × 角色」范围内唯一（它总随角色一起引用，如 `wolf.whereabouts`）
- **发布即冻结**：因为 id 是存档外键与条件标记，内容一旦发布，id **不得改名或复用**——改名会使旧存档与条件表达式中的引用失效。若需实质性地替换某内容，应使用新 id，让旧 id 自然消失（旧存档引用时按「跳过 + warning」处理，见 [save-system.md](save-system.md)「兼容性策略」）
- 中文展示名由 `name` 字段承担，与 `id` 分离：`id` 是英文 snake_case 的稳定标识，`name` 是面向玩家的中文展示
- 全局内容中的路径以 `data/` 为基准；章节内部路径以对应章节目录为基准，例如 `results/wolf_is_murderer.md`
- **审判点的 id 同时作为条件标记**（章节跳转 `when`、话题解锁 `unlock_after` 引用它），不再有独立的 `condition` 字段；建议 id 带角色上下文以保证可读（如 `judge_wolf`）

## 条件表达式

话题解锁和章节跳转统一使用条件表达式。**条件表达式直接引用 id**——线索的 id（如 `wolf_alibi`）和审判点的 id（如 `judge_wolf`，同时承担条件标记职责）。

由于条件表达式与存档事实都使用同一套 id，求值时直接匹配，**无需任何映射**。引擎启动时必须校验所有条件引用的 id 有效。

支持的形式（**v1 仅支持扁平结构，不支持任意嵌套**）。即 `all_of`/`any_of` 的列表项只能是纯 id 字符串；`not` 后面也只能跟单个 id。不支持 `all_of` 内嵌套 `any_of` 或 `not` 内嵌套 `all_of` 等组合：

```yaml
# 单一条件
wolf_alibi

# 所有条件都满足
all_of:
  - wolf_alibi
  - crow_testimony

# 任一条件满足
any_of:
  - judge_wolf
  - judge_crow

# 条件不满足
not: judge_wolf
```

## 场景定义

全局场景定义存放于 `data/scenes/`，包含场景的结构化信息与默认描述文本引用。

`data/scenes/tavern.yaml`:

```yaml
id: tavern
name: "锈桶酒馆"
connections:
  - market
  - alley
# one_way_connections:          # 可选：单向连接（不自动补反向）
#   - cellar
description:
  surface: text/scenes/tavern.surface.md
  shadow: text/scenes/tavern.shadow.md
```

**章节覆盖机制**: 场景有全局默认描述，特定章节可在 `chapters/{chapter_id}/scenes/` 下提供覆盖版本。引擎加载时优先使用章节覆盖，不存在时回退到全局默认。

**场景连接**: `connections` 声明当前场景可直接移动到的相邻场景。连接**默认双向**——A 的 `connections` 含 B，引擎自动补 B → A 的反向连接，无需在 B 中重复声明。若需要单向通行（如剧情上的单向入口），在 `one_way_connections` 中声明：

```yaml
connections:
  - market          # market ↔ tavern 双向
  - alley            # alley ↔ tavern 双向
one_way_connections:
  - cellar           # tavern → cellar 单向，cellar 不自动可回 tavern
```

**单向死胡同约束**: `one_way_connections` 的目标必须有独立出口（自身声明了 `connections` 或 `one_way_connections`），否则玩家进入后无法离开、构成死锁——启动校验会以 error 拦截（见 [content-engine.md](content-engine.md)）。

`move` 时校验目标在当前场景的可达连接中（含引擎补全的反向连接，见 [commands.md](commands.md) 运行时错误表）。引擎启动校验 `connections` 和 `one_way_connections` 引用的场景 ID 有效（见 [content-engine.md](content-engine.md)）。

## 角色定义

全局角色定义存放于 `data/characters/`，仅包含角色的基础信息（`id` 为英文 snake_case 稳定标识，`name` 为中文展示名）。

`data/characters/wolf.yaml`:

```yaml
id: wolf
name: "灰狼"
```

## 章节元数据

每个章节有一个 `chapter.yaml`，定义本章引用的场景、角色、话题列表、解锁条件、必要审判、分支跳转逻辑，以及是否为终章。章节使用语义化 ID，同一叙事顺序可能存在多个分支版本。

`data/chapters/the_missing_butcher/chapter.yaml`:

```yaml
id: the_missing_butcher
title: "失踪的屠夫"
order: 1
ending: false                     # 终章标记；true 表示该章为结局章，无 next
intro: intro.md                   # 可选：章节开场/过场文本（Markdown，相对章节目录）；进入本章时先展示
scenes:
  - tavern
  - market
  - alley
starting_scene: tavern
characters:
  - id: wolf
    appears_in: [tavern]        # 该角色本章出现的场景；省略=本章所有场景
    topics:
      - id: whereabouts
        label: "昨晚的行踪"
        available: true
      - id: the_knife
        label: "那把刀"
        available: true
      - id: secret
        label: "隐藏的秘密"
        available: false
        unlock_after:
          all_of:
            - wolf_alibi
            - crow_testimony
  - id: crow
    topics:
      - id: victim
        label: "关于受害者"
        available: true
required_judgments: [judge_wolf, judge_crow]   # 自动推进前必须完成的审判 id；缺省=本章全部审判
next:
  default: tavern_uncertain
  branches:
    - when:
        all_of:
          - judge_wolf
          - judge_crow
      target: tavern_truth
    - when:
        any_of:
          - judge_wolf
      target: tavern_deceit
```

**终章示例** (`data/chapters/tavern_truth/chapter.yaml`):

```yaml
id: tavern_truth
title: "酒馆真相"
order: 3
ending: true                      # 终章标记
intro: intro.md                   # 可选：终章也可有开场
outro: outro.md                   # 可选：结局收尾文本（仅终章有效）；必要审判完成后展示
scenes:
  - tavern
starting_scene: tavern
characters:
  - id: wolf
    topics:
      - id: final_words
        label: "最后的话"
        available: true
required_judgments: [judge_wolf]    # 终章也必须通过必要审判结束
# 终章无 next 字段
```

**字段说明**:

- `order` — 开发辅助字段，用于 `darkbluff check` 输出和作者浏览时的排序参考；不影响游戏运行时行为，不作为章节树的唯一依据。玩家侧排序使用 `discovered.chapters` 的首次到达顺序
- `starting_scene` — 玩家进入本章时的初始场景（scene `id`）
- `characters[].appears_in` — 该角色本章出现的场景 `id` 列表；省略时默认出现在本章所有场景。`ask` 校验目标角色必须在当前场景（否则提示「这里没有这个角色。」）；`judge` 是章级操作，不校验 `appears_in`，只要求角色在本章 `characters` 中声明即可
- 话题可见性 — `available: true` 表示默认可问；`available: false` 配合 `unlock_after` 表示满足条件后解锁；`available: false` 且**无** `unlock_after` 表示该话题本章**永久不可问**（合法用法，用于剧情上有意封禁的话题）
- `required_judgments` — 自动推进前必须完成的审判（审判点 id）。省略时默认要求本章所有审判都已完成；显式空数组 `[]` 不合法。v1 不支持无审判推进章节，因此每章必须至少有一个审判点，且必要审判集合不能为空
- `intro` — 可选的章节开场/过场叙事文本（Markdown 文件相对路径，如 `intro.md`）。进入本章时（新游戏首章 / 自动推进跳转 / `map` 回滚）先展示开场，玩家确认后再进入 `starting_scene`；省略则直接进入场景。展示时引擎写入开场快照供 `note`「叙事」标签页回顾（见 [save-system.md](save-system.md)）
- `outro` — 可选的终章结局收尾叙事文本（Markdown 文件相对路径，如 `outro.md`），**仅对 `ending: true` 的终章有效**。终章完成最后一个必要审判后先展示 outro，玩家确认后进入结局界面（`Ending` 状态）；省略则直接进入结局界面。展示时写入快照供 `note`「叙事」标签页回顾。非终章定义 `outro` 为启动校验错误
- `ending` — `true` 表示终章；终章没有 `next`，完成最后一个必要审判时记为达成结局
- **`next` 必要性**: 非终章（`ending: false` 或未声明 `ending`）**必须提供 `next` 字段**（至少含 `default`）。终章**不得提供 `next` 字段**。引擎启动校验此约束
- `next.default` — 默认跳转目标
- `next.branches` — 根据条件表达式跳转到不同章节
- `when` — 条件表达式，引用审判点 id 或线索 id（同时作为条件标记）
- 分支按列表顺序匹配，第一条满足条件的分支生效；没有任何分支满足时走 `next.default`
- 章节树由所有章节的 `next` 指针构建，`order` 只用于同层节点排序
- **首章 = 章节树的唯一根节点**：即没有任何章节的 `next` 指向它的章节。新游戏从首章开始；引擎启动校验**有且仅有一个根节点**（见 [content-engine.md](content-engine.md)）
- 章节图**不允许成环**：`next` 指针不得构成循环（否则 `chapter_path` / `discovered.chapters` 会无限增长）。引擎启动校验章节图为有向无环图（见 [content-engine.md](content-engine.md)）

**自动推进章节**:

玩家执行 `judge` 后，若本章 `required_judgments` 尚未全部完成，回到 `Exploring`，不推进章节。若本章 `required_judgments` 已全部完成，立即触发自动推进：

1. **若本章 `ending: true`**：
   - a. 将本章记入 `discovered.endings`
   - b. 若有 `outro` → 展示结局文本，写入快照（记入 `viewed_outros`），玩家确认
   - c. 进入结局界面（`Ending` 状态：终章 `title` 作为结局名 +「已发现结局 X/Y」+「返回标题」）
   - **不评估 `next`**
2. 否则按 `next.branches` 顺序匹配已审判的审判点集合，命中则跳转对应 `target`，否则走 `next.default`
3. 追加目标章节到 `chapter_path` 与 append-only 的 `discovered.chapters`
4. 进入新章节：若有 `intro` 先展示开场并写入快照（记入 `viewed_intros`），玩家确认后进入 `starting_scene`，**此时**创建 `chapter_start` 检查点（默认 `surface` 视角）。新游戏首章同理。`map` 回到 `chapter_start` 时重新展示 `intro`（若有），但快照按首次查看规则去重；开场快照可在 note「叙事」回顾（见 [save-system.md](save-system.md)）

**终章的两阶段生命周期**：通过前一章自动推进**进入**终章时执行 step 2-4（此时只记 `discovered.chapters`，不记 `discovered.endings`）。终章和普通章节一样可以包含场景探索与审判。玩家在终章内完成最后一个必要审判时才记入 `discovered.endings`、展示 `outro`（若有）并进入结局界面——即终章不是进入时自动结束。

## 对话数据

对话文本按角色拆分为 Markdown 文件，存放于 `chapters/{chapter_id}/dialogues/`。

`data/chapters/the_missing_butcher/dialogues/wolf.md`:

```markdown
## whereabouts

### [surface]

灰狼靠在吧台边，用爪子拨弄着空酒杯。

"昨晚？我一直在这儿喝酒，直到打烊。老板可以作证。"

他的尾巴不安地扫过地面。

### [shadow]

灰狼蜷缩在角落，眼神闪烁。

"昨晚？我哪儿也没去，一直待在家里。"

他的影子在墙上扭曲成不自然的形状。

## the_knife

### [surface]

...

### [shadow]

...
```

**解析约定**:

- `## {topic_id}` — 话题分隔符，对应 chapter.yaml 中定义的话题 ID
- `### [surface]` / `### [shadow]` — 世界版本分隔符
- 两个标记之间的所有内容为该话题该世界的文本
- **单世界话题**: 一个话题允许只出现 `[surface]` 或只出现 `[shadow]` 一个版本（至少一个）。在缺失的那个世界 `ask` 该话题时，提示「这个话题在这一侧无从问起。」
- 对话解析器使用 fence-aware 行扫描解析标题层级（code fence 内的 `##`/`###` 不被当作标题），不使用纯字符串切割
- 正文可以正常使用 Markdown 格式；若正文需要标题，必须从 `####` 层级开始，避免与话题/世界分隔层级冲突
- 可选 frontmatter 用于文件级元数据；未知字段由引擎忽略
- 对话文件**不单独拥有作为外键的 id**：对话由「章节 + 角色 + 话题」三元组定位（topic id 仅在章节 × 角色内唯一），不进入存档外键或条件标记
- 引擎启动时校验：YAML 中引用的话题 ID 必须在对应 .md 文件中存在

## 审判逻辑

审判逻辑按章节定义，每个审判点是一次对某角色的**审判**——触发该角色一段固定的审判剧情，并记下「审判过该角色」。`judge` 的 `target` 是玩家输入的角色 ID。**一个角色在一章内只有一段审判**（一个审判点）。

`data/chapters/the_missing_butcher/judgments.yaml`:

```yaml
- id: judge_wolf                  # 审判点 id（条件标记：审判过 = 触发）
  target: wolf                    # 审判对象
  result: results/wolf_trial.md   # 审判剧情文本
- id: judge_crow
  target: crow
  result: results/crow_trial.md
```

**字段说明**:

- `id` — 审判点 id，全局唯一；同时作为条件标记（章节跳转 `when`、话题解锁 `unlock_after` 引用它），发布后冻结不得改名
- `target` — 被审判的角色 id；一章内一个角色只能被审判一次（一个 `target` 对应一个审判点），引擎启动校验本章 `target` 不重复
- `result` — 审判剧情文本（Markdown 相对路径），`judge` 触发时展示

**`target` 语义与解析**: 玩家通过 `judge [target]` 触发审判，`target` 为本章声明的角色 ID（`judge` 是**章级操作**，不要求目标角色在当前场景在场）。`judge wolf` 直接触发灰狼的审判剧情。引擎启动时校验每个审判的 `target` 是本章 `characters` 中声明的角色。

**无选项**: 审判没有选项可选——`judge` 一个角色就是触发该角色的审判剧情，剧情是固定的。不同章节走向由「**审判了哪些角色**」决定（见下文「审判与跳转的关系」），而非选项选择。

**审判与跳转的关系**:

- 每个审判点的 id 同时作为条件标记（不再有独立的 `condition` 字段或选项 id）
- 审判点 id 会进入存档和章节跳转条件，**发布后不得改名**（改名会破坏旧存档与条件引用）
- 一个章节可以存在多个审判点（对应多个可审判角色），审判了哪些角色共同决定自动推进时的后续章节
- 章节的 `next.branches` 使用条件表达式匹配「已审判的审判点 id 集合」
- 审判逻辑负责触发审判剧情、记录「审判过该角色」，并在必要审判完成后触发自动推进；章节跳转条件由 `chapter.yaml` 统一管理
- 默认规则：同一个审判点在一次章节流程中只能触发一次；触发后进入 `judgments_made`，想撤销需通过 `map` 回到审判前或章节开头 checkpoint

**文件命名建议**: 审判剧情文件推荐以对应审判点 id 命名（如 `judge_wolf` → `results/wolf_trial.md`），保持一致可读性。引擎不强制此命名，以 `result` 字段的路径为准。

**审判与检查点**: 玩家执行 `judge [target]` 时，引擎自动创建 `before_judgment` 检查点（见 [save-system.md](save-system.md)），保证 `map` 能精确回到审判前。`judge` 无参数时的角色选择菜单按 Esc 取消不会创建检查点。

**审判剧情快照**: 同一时机，引擎将该审判点的 `result` 文本写入快照、路径记入存档 `judgments_made` 对应条目的 `result_snapshot`，供 `note`「审判」标签页回顾（见 [save-system.md](save-system.md)）。

**审判顺序**: 同一章节的多个审判点之间**无顺序依赖**，玩家可以任意顺序审判。

## 线索系统

线索系统是纯逻辑层的机制，不负责展示内容。`clues.yaml` 定义"哪些对话被触发后算获得了线索"，线索 id 用于话题解锁条件（`unlock_after`）。

`data/chapters/the_missing_butcher/clues.yaml`:

```yaml
- id: wolf_alibi
  source: wolf.whereabouts
  world: surface

- id: wolf_alibi_shadow
  source: wolf.whereabouts
  world: shadow

- id: crow_testimony
  source: crow.victim
  world: surface
```

**说明**:

- `source` — 格式为 `{character_id}.{topic_id}`，表示该线索由哪个对话触发
- `source` 必须严格为两段，且只能包含一个点号；如 `wolf.whereabouts.extra` 视为非法
- `world` — 表示在哪个世界版本中触发；必须与 `source` 对应对话实际存在的世界一致（如对话只有 `[surface]` 版本，`world` 只能为 `surface`）。引擎启动校验一致性（见 [content-engine.md](content-engine.md)）
- 当玩家通过 `ask` 查看对应对话时，自动收集该线索
- 线索 id（如 `wolf_alibi`）可被 `chapter.yaml` 中的 `unlock_after` 条件表达式引用，实现话题解锁
- 存档中保存线索的 id；由于 id 发布后冻结，存档不会因内容迭代而失效
