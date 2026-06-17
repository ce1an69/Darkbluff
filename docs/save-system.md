# 存档系统

> 属于 DarkBluff 设计文档。主索引见 [design.md](design.md)。重玩/检查点的玩家侧行为见 [narrative.md](narrative.md)；存档引用的内容格式见 [data-formats.md](data-formats.md)。

单存档自动保存机制，记录玩家的权威事实状态、当前位置、对话快照索引和探索记忆。**存档采用「存储事实为权威」模型**：`collected_clues` / `viewed_dialogues` / `judgments_made` 直接存储；检查点记录这些数组在创建时刻的长度，回滚即按长度截断，不做事件回放。

存档不保存可由内容数据或权威事实推导出的派生状态（如已解锁话题列表）。

## 存档结构

```json
{
  "version": 1,
  "timestamp": "2026-06-16T12:00:00Z",
  "current_chapter": "the_missing_butcher",
  "current_scene": "tavern",
  "current_world": "surface",
  "collected_clues": {
    "the_missing_butcher": ["wolf_alibi", "wolf_alibi_shadow", "crow_testimony"]
  },
  "viewed_dialogues": {
    "the_missing_butcher": [
      {
        "character": "wolf",
        "topic": "whereabouts",
        "world": "surface",
        "snapshot": "snapshots/the_missing_butcher/wolf.whereabouts.surface.md"
      },
      {
        "character": "wolf",
        "topic": "whereabouts",
        "world": "shadow",
        "snapshot": "snapshots/the_missing_butcher/wolf.whereabouts.shadow.md"
      }
    ]
  },
  "judgments_made": {
    "the_missing_butcher": [
      { "judgment": "judge_wolf", "result_snapshot": "snapshots/the_missing_butcher/judge_wolf.md" },
      { "judgment": "judge_crow", "result_snapshot": "snapshots/the_missing_butcher/judge_crow.md" }
    ]
  },
  "viewed_intros": {
    "the_missing_butcher": "snapshots/the_missing_butcher/intro.md"
  },
  "viewed_outros": {},
  "chapter_path": ["the_missing_butcher"],
  "checkpoints": [
    {
      "id": "ckpt_the_missing_butcher_start",
      "chapter": "the_missing_butcher",
      "scene": "tavern",
      "world": "surface",
      "kind": "chapter_start",
      "timestamp": "2026-06-16T11:50:00Z",
      "state": { "clues_len": 0, "views_len": 0, "judgments_len": 0 }
    },
    {
      "id": "ckpt_before_judge_wolf_murderer",
      "chapter": "the_missing_butcher",
      "scene": "tavern",
      "world": "surface",
      "kind": "before_judgment",
      "timestamp": "2026-06-16T11:58:00Z",
      "state": { "clues_len": 3, "views_len": 4, "judgments_len": 0 }
    }
  ],
  "discovered": {
    "chapters": ["the_missing_butcher"],
    "endings": [],
    "topics": {
      "the_missing_butcher": ["wolf.whereabouts", "wolf.the_knife", "crow.victim"]
    }
  }
}
```

**字段说明**:

| 字段 | 说明 |
|------|------|
| `version` | 存档结构版本号，用于向后兼容迁移 |
| `timestamp` | 最后保存时间 |
| `current_chapter` | 当前所在章节 id |
| `current_scene` | 当前所在场景 id。进入新章节时由该章 `starting_scene` 解析得到 |
| `current_world` | 当前视角（surface/shadow）。进入新章节时重置为 `surface`（见 [data-formats.md](data-formats.md)「自动推进章节」） |
| `collected_clues` | 已收集的线索 id（按章节分组，权威存储） |
| `viewed_dialogues` | 已查看的对话索引和快照路径（按章节分组，权威存储，用于 note「对话」标签页展示） |
| `judgments_made` | 已审判的记录（按章节分组；`judgment` 为审判点 id，`result_snapshot` 为审判剧情快照路径、供 note「审判」标签页回顾）。条件表达式求值时合并 `chapter_path` 中当前及之前所有章节的事实（见 [content-engine.md](content-engine.md)「FactSet」） |
| `viewed_intros` | 已展示的章节开场文本快照路径（按章节分组，chapter_id → 相对路径），供 note「叙事」标签页回顾；章节无 `intro` 则无条目 |
| `viewed_outros` | 已展示的终章结局文本快照路径（按章节分组，chapter_id → 相对路径），供 note「叙事」标签页回顾；非终章或无 `outro` 的终章无条目 |
| `chapter_path` | 玩家经过的章节路径（用于当前流程的树状高亮） |
| `checkpoints` | 自动创建的检查点，记录位置 + 当前章节三个权威数组的长度 |
| `discovered` | **append-only** 的探索记忆：到过的章节、达成的结局、问过的话题。任何回滚都不截断此字段。三个子集均为 **set 语义**（追加时去重：重复到达同一章节/结局、或重复问同一话题，不产生重复条目）；其中 `chapters` 为**有序去重**（保留首次到达顺序），供 `map` 的章节树展示与探索进度排序。「已发现结局 X / Y」等计数基于去重后的集合。`chapters`/`endings` 序列化为 JSON 数组；`topics` 序列化为按章节分组的对象（chapter → 话题 id 数组），追加时去重 |

**字段命名约定**: 存档中所有内容引用均为 id（与内容文件中的 id 完全一致，无第二套标识）。

**`discovered` 更新时机**（所有更新均为 append，任何回滚都不截断）:

- 进入任何章节（新游戏首章 / 自动推进跳转 / `map` 回滚）→ 追加章节 id 到 `discovered.chapters`
- 自动推进进入 `ending: true` 的章节 → 追加到 `discovered.chapters`（注意：此时只记章节，不记结局）
- 在终章中完成最后一个必要审判时（即玩家结束终章体验）→ 追加该终章 id 到 `discovered.endings`
- 通过 `ask` 查看某话题 → `{character}.{topic}` 追加到 `discovered.topics[chapter]`（去重）

> **`discovered` 子集语义**: `chapters` / `endings` / `topics` 都是「玩家曾经体验过什么」的集合记忆，追加时去重，不参与任何跳转逻辑。其中 `discovered.topics` 记录玩家在任意周目或被回滚的尝试中**曾问过**的话题（按章节分组的 `{character}.{topic}` 列表），供 map 面板按角色聚合标注「话题 X/Y」（见 [architecture.md](architecture.md)「Map 面板」），为重玩提供目标感。它与 `viewed_dialogues`（当前流程的对话快照、可被回滚）职责不同。

## 检查点与回滚

- 检查点不保存完整状态快照，只记录位置（`scene`/`world`）+ 当前章节三个权威数组在创建时刻的长度（`clues_len` / `views_len` / `judgments_len`）
- 检查点种类：`chapter_start`（进入章节时）、`before_judgment`（玩家**执行 `judge` 审判时**；`judge` 无参数选角色时 Esc 取消不创建检查点）
- **当前章 checkpoint 回滚**（通过 `map` 选择当前章的 checkpoint）：
  - 恢复位置到检查点的 `scene`/`world`
  - 将当前章节的 `collected_clues` / `viewed_dialogues` / `judgments_made` 截断到对应长度
  - `chapter_path` 与 `discovered` 不变
- **跨章 checkpoint 回滚**（通过 `map` 选择早前章节的 checkpoint）：
  - 恢复位置到该检查点
  - 截断该章节的三个数组到对应长度
  - 丢弃 `chapter_path` 中该章之后的所有章节，并清除这些章节在三个数组中的全部条目
  - 丢弃该章之后所有章节对应的 `checkpoints`
  - **`discovered` 仍然不变**——已探索过的结局/审判永久保留

## 回滚实现

本节给出检查点管理的精确算法（对应 `save/checkpoint.rs`）。**核心不变量**：当前游戏状态由 `current_*` 字段 + 三个权威数组直接表示；`checkpoints` 列表只是「可回滚的目标历史」，不参与表达当前状态。

**检查点列表模型**: `checkpoints` 是按创建时间顺序追加的全局列表，每个检查点记录其所属章节（`chapter`）。进入章节建 `chapter_start`，执行 `judge` 审判前建 `before_judgment`。因此 `chapter_path` 与 `chapter_start` 检查点一一对应、顺序一致。

**基本操作 `rollback_to(ckpt)`** ——把当前状态恢复到某检查点记录的快照：

```rust
fn rollback_to(save, ckpt) {
    save.current_chapter = ckpt.chapter;
    save.current_scene   = ckpt.scene;
    save.current_world   = ckpt.world;
    let ch = ckpt.chapter;
    save.collected_clues[ch].truncate(ckpt.state.clues_len);
    save.viewed_dialogues[ch].truncate(ckpt.state.views_len);
    save.judgments_made[ch].truncate(ckpt.state.judgments_len);
}
```

**作废检查点移除规则**: 回滚会使「创建时间晚于回滚目标」的检查点全部作废，必须一并移除，否则后续 `map` 会展示已不存在的未来节点。统一用 `checkpoints.truncate(cutoff)` 截断列表。若目标是 `chapter_start`，保留该检查点（`cutoff = idx + 1`）；若目标是 `before_judgment`，移除该检查点及其后全部（`cutoff = idx`），因为它代表的审判已被撤销。

**`map` checkpoint 回滚**:

```rust
fn map_checkpoint_rollback(save, checkpoint_id) -> Result {
    let idx = save.checkpoints.index_where(|c| c.id == checkpoint_id)
        .ok_or("这个节点已经无法回到")?;
    let ckpt = save.checkpoints[idx].clone();
    rollback_to(save, &ckpt);

    // 若目标不在当前章，截断 chapter_path 并清理其后章节的权威事实与快照索引
    let pidx = save.chapter_path.index_of(ckpt.chapter);
    let dropped = save.chapter_path[pidx + 1..].to_vec();
    save.chapter_path.truncate(pidx + 1);
    for ch in &dropped {
        save.collected_clues.remove(ch);
        save.viewed_dialogues.remove(ch);
        save.judgments_made.remove(ch);
        save.viewed_intros.remove(ch);
        save.viewed_outros.remove(ch);
    }

    let cutoff = match ckpt.kind {
        ChapterStart => idx + 1,
        BeforeJudgment => idx,
    };
    save.checkpoints.truncate(cutoff);
    // discovered 不变
    Ok(())
}
```

**边界与一致性**:

- `map` 中若没有可选 checkpoint → 提示「还没有可以回到的节点」，**不改存档**
- `map` 选择的 checkpoint 已失效（内容/存档不一致或并发变更）→ 提示「这个节点已经无法回到」，刷新地图，**不改存档**
- `chapter_start` 检查点在进入章节时必定创建；任何当前流程中已展示的 `chapter_start` 都可作为 `map` 回滚目标
- 回滚截断 `viewed_dialogues` 后，对应磁盘快照文件可能成为孤儿（无害，保存时可选压缩清理，见「笔记系统与快照」）
- `discovered` 在任何回滚中都不截断——已体验的结局/问过的话题永久保留
- 任何回滚完成后立即自动保存（见「自动保存时机」）

## 笔记系统与快照

`note` 指令展示玩家实际看过的对话全文，用于回顾和推理。为了保证推理公平性，玩家看到过的对话会保存为快照，后续剧情文本更新不会改变既有笔记内容。

**行为定义**:

`note` 面板含四个标签页（标签页范围均为当前流程实际经过的 `chapter_path`，而非 append-only 的 `discovered.chapters`）：

- **对话**：玩家通过 `ask` 看过的对话全文（来源 `viewed_dialogues`）。记录按角色分组、含世界版本标识；同一章节内按 `viewed_dialogues` 数组顺序（首次查看时间序）展示，同一角色同一话题的两个世界版本各自独立、按查看顺序排列
- **叙事**：玩家进入各章时看过的 `intro` 文本（来源 `viewed_intros`）与完成终章最后一个必要审判后看过的 `outro` 文本（来源 `viewed_outros`）。按 `chapter_path` 顺序排列，每条标注「开场」或「结局」前缀；无 `intro`/`outro` 的章节不出现
- **审判**：玩家已审判的审判剧情文本（来源 `judgments_made[].result_snapshot`）。按章节与角色列出，点选展开审判剧情；缺快照时显示「该记录快照缺失」
- **章节树**：分支树（见 [architecture.md](architecture.md)「Map 面板」）

**交互流程**:

```
> note
[失踪的屠夫]  [酒馆真相]  [...]
─────────────────────────────────────────────────
灰狼 - 昨晚的行踪 [表面世界]
  灰狼靠在吧台边，用爪子拨弄着空酒杯。
  "昨晚？我一直在这儿喝酒，直到打烊。老板可以作证。"
  他的尾巴不安地扫过地面。

灰狼 - 昨晚的行踪 [影子世界]
  灰狼蜷缩在角落，眼神闪烁。
  "昨晚？我哪儿也没去，一直待在家里。"
  他的影子在墙上扭曲成不自然的形状。

乌鸦 - 关于受害者 [表面世界]
  ...
```

**实现逻辑**:

- 玩家通过 `ask` 查看对话时，引擎将当时实际渲染前的 Markdown 原文写入对话快照文件；`viewed_dialogues` 索引与快照相对路径保存在 `save.json`（按章节分组）
- 进入章节展示 `intro` 时，将开场文本写入快照 `snapshots/{chapter}/intro.md`，路径记入 `viewed_intros`（章节无 `intro` 则不写）
- 终章完成最后一个必要审判后展示 `outro` 时，将结局文本写入快照 `snapshots/{chapter}/outro.md`，路径记入 `viewed_outros`（终章无 `outro` 则不写）
- 玩家**执行 `judge` 审判**时（与建 `before_judgment` 检查点同一时机），将该审判点的 `result` 文本写入快照，路径记入 `judgments_made` 对应条目的 `result_snapshot`
- `note` 展示时优先读取快照文件，不重新读取当前剧情文本
- 如果快照文件缺失，界面显示"该记录快照缺失"，并保留索引信息供排查

**单存档目录结构**:

```
save/
├── save.json
├── save.json.bak                # 上次保存前的备份（见下文「存档健壮性」）
└── snapshots/
    └── the_missing_butcher/
        ├── intro.md                              # 章节开场快照（有 intro 时）
        ├── outro.md                              # 终章结局快照（终章有 outro 时）
        ├── wolf.whereabouts.surface.md           # 对话快照
        ├── wolf.whereabouts.shadow.md
        ├── judge_wolf.md                         # 审判剧情快照
        └── judge_crow.md
```

快照文件路径保存在 `save.json` 中，必须使用相对路径（**基准目录为存档根目录，即 `save/`**），避免跨机器或系统目录变化时失效。

**快照与回滚的同步**: `viewed_dialogues` 是存档的权威状态（存储事实模型）。回滚到检查点时，当前章节的 `viewed_dialogues` 列表会按检查点记录的长度截断；被截断的快照文件可能成为孤儿文件留在磁盘上。

**快照压缩（孤儿清理）**: 引擎在**`map` checkpoint 回滚完成后的自动保存时**扫描 `snapshots/` 目录，删除 `viewed_dialogues`、`viewed_intros`、`viewed_outros`、`judgments_made[].result_snapshot` 四类引用之外的孤儿快照文件。非回滚的正常保存不触发扫描（因为正常操作只增不删快照引用，不产生孤儿），避免不必要的 IO。压缩失败（如权限问题）不影响游戏运行，仅记录 warning。

**跨章回滚与 note 数据**: `map` 选择早前章节 checkpoint 会丢弃 `chapter_path` 中目标章之后所有章节的 `viewed_dialogues`、`viewed_intros`、`viewed_outros`、`judgments_made` 条目（见「检查点与回滚」），因此这些章节的 note 记录在回滚后**不可恢复**——这是单存档销毁性回滚的固有取舍。`discovered` 仍保留「你曾体验过该章节」的记忆，但对话/叙事/审判文本快照本身已被截断，note 标签页也随之收窄到当前 `chapter_path`。

## 派生状态（不进存档）

- 已解锁话题由 `collected_clues` 和 `chapter.yaml` 中的 `unlock_after` 条件表达式在加载时重新计算
- 当前流程的树状高亮由 `chapter_path` 推导
- 完整的「已发现分支树」由 append-only 的 `discovered` 推导（这是树面板的渲染来源）

## 加载规则

- 加载存档时直接用 id 匹配当前内容（id 与内容文件一致，无需映射层）
- 如果某个 id 无法解析（内容已被删除或改名），保留原始存档记录但在运行时跳过对应内容，并输出 warning

## 存档健壮性

- **原子写入**: 保存时先写入 `save.json.tmp`，再 rename 为 `save.json`，避免写入中断导致损坏
- **备份**: 覆盖前将当前 `save.json` 复制为 `save.json.bak`
- **损坏恢复**: 加载时若 `save.json` 解析失败，回退到 `save.json.bak`；若 `.bak` 也损坏，将损坏文件另存为 `save.json.corrupt-<timestamp>` 并以新存档启动，向玩家提示
- 对话快照由 `snapshots/` 目录独立保存，与回滚互不影响

## 自动保存时机

- 场景移动（`move`）后
- 收集到新对话/线索（`ask`）后
- 执行审判（`judge`）后
- 视角切换（`gaze`）后
- 展示章节开场（`intro`）后
- 展示终章结局文本（`outro`）后
- 创建检查点（审判确认时 / 章节开始）后
- 自动推进章节后
- `map` checkpoint 回滚完成后
- 退出（`quit`）时

## 兼容性策略

| 场景 | 处理方式 |
|------|----------|
| 新增章节/角色/话题 | 无影响，存档中未涉及的内容视为未触发 |
| 删除已存在的章节 | 若 `current_chapter` 指向已删除章节，回退到 `chapter_path` 中最后一个有效章节 |
| 重命名内容 id | **改名即破坏存档**——id 是存档外键，不可改名。若必须替换内容，使用新 id，旧 id 让其自然消失（旧存档引用按「跳过 + warning」处理） |
| 修改章节跳转逻辑 | `chapter_path` 历史保留，未来跳转按新逻辑执行 |
| 存档结构升级 | 按 `version` 字段执行迁移函数 |
| 未知字段 | 反序列化时忽略（向前兼容） |
| 缺失字段 | 使用默认值填充（向后兼容） |
| 快照文件缺失 | 保留索引，note 显示缺失提示，不读取最新剧情文本替代 |
| `save.json` 损坏 | 回退 `.bak`；均损坏则备份后以新存档启动（见「存档健壮性」） |

## 存档路径

```
~/.local/share/darkbluff/save/                    # Linux
~/Library/Application Support/darkbluff/save/      # macOS
%APPDATA%/darkbluff/save/                          # Windows
```

使用 `dirs` crate 获取跨平台数据目录。单存档由 `save.json`、`save.json.bak` 和同目录下的 `snapshots/` 组成。设置文件（动画开关等）独立存放，不混入 `save.json`。

**首次运行**: 若存档目录不存在，引擎首次写入时自动递归创建目录层级（等价于 `mkdir -p`）。目录创建失败（如权限不足）时向玩家提示错误并退出。

## 新游戏初始化

标题界面选择「新游戏」时（已有存档需二次确认）：

1. 清空 `snapshots/` 目录下所有快照文件
2. 删除 `save.json.bak`（旧备份不再需要）
3. 生成空存档并写入 `save.json`（初始章节为首章、`starting_scene`、`surface` 视角、所有数组为空）
4. 创建首章的 `chapter_start` 检查点
5. 将首章追加到 `discovered.chapters`
6. 进入首章（展示 `intro` 或直接进入场景）

## 设置文件

动画开关等玩家偏好持久化到独立的 `settings.json`（与 `save.json` 同目录），与存档解耦：换/删存档不影响偏好，偏好写入也不触发存档的检查点与原子写入流程。

```json
{
  "version": 1,
  "motion": "full"          // "full" | "reduced" | "off" —— 默认 / 缩短 / 关闭过渡动画
}
```

- **健壮性**: 与存档一致——原子写入 + `.bak` 备份；加载损坏时回退 `.bak`，均损坏则以默认值（`motion: full`）启动
- **兼容性**: 未知字段忽略、缺失字段使用默认值（向前/向后兼容）；结构升级按 `version` 迁移
- **覆盖优先级**: 命令行 `--no-motion` 临时覆盖（本次运行有效，不写回设置文件）；设置文件 > 内置默认值
