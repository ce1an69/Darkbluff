# 技术架构

> 属于 DarkBluff 设计文档。主索引见 [design.md](design.md)。

## 技术栈

| 类别          | 选择                             | 说明                                                                                            |
| ------------- | -------------------------------- | ----------------------------------------------------------------------------------------------- |
| 语言          | Rust                             | 类型安全，单二进制分发，性能优秀                                                                |
| TUI 框架      | ratatui                          | 即用模式渲染，双缓冲 diff，布局系统成熟                                                         |
| 终端后端      | crossterm                        | 跨平台终端交互                                                                                  |
| 事件循环      | crossterm event + 按需 tick      | 常态事件驱动，仅动画播放期间定时重绘                                                            |
| 动画          | tachyonfx                        | 场景切换过渡特效（淡入淡出、径向扩散、颜色偏移等）                                              |
| Markdown 渲染 | tui-markdown                     | pulldown-cmark 解析，ratatui 原生输出                                                           |
| 树状图        | tui-tree-widget                  | 章节分支树展示                                                                                  |
| 输入组件      | tui-prompts                      | 文本输入 + 自动补全 + 选择菜单                                                                  |
| YAML 解析     | serde + serde_yml                | 数据文件序列化/反序列化。`serde_yml` 是 `serde_yaml`（已停止维护）的活跃 fork，API 基本兼容 |
| JSON 存档     | serde_json                       | 存档读写                                                                                        |
| 对话文本解析  | fence-aware 行扫描           | 按 Markdown 标题层级（`##`/`###`）识别话题和世界版本；code fence 内的标题不被误识别            |
| 内容内嵌      | include_dir                      | 计划用于发布模式；当前未实现                                                                    |
| 存档路径      | dirs                             | 获取跨平台应用数据目录                                                                          |

## 项目结构

```
darkbluff/                          # Cargo workspace
├── Cargo.toml                      # workspace 清单（members + 共享 package 字段）
├── crates/
│   ├── darkbluff-core/             # 核心库：渲染无关、无终端可测
│   │   ├── src/
│   │   │   ├── engine/             # 游戏引擎层（对渲染层的门面）
│   │   │   │   ├── state.rs        #   游戏状态机 + Session
│   │   │   │   ├── outcome.rs      #   UI 无关的 Input / Outcome / SessionState 契约
│   │   │   │   ├── commands.rs     #   指令解析（ask/judge/move/gaze/map/note/help/quit）
│   │   │   │   ├── ask.rs          #   ask 指令流程
│   │   │   │   ├── judge.rs        #   judge 指令与章节推进
│   │   │   │   ├── navigation.rs   #   move / gaze
│   │   │   │   ├── map.rs          #   checkpoint 菜单与回滚
│   │   │   │   ├── system.rs       #   标题界面 / note / quit
│   │   │   │   ├── chapter_flow.rs #   章节进入、intro/outro、结局
│   │   │   │   ├── note_view.rs    #   笔记视图组装
│   │   │   │   ├── logic.rs        #   纯领域查询与存档 reconcile
│   │   │   │   └── condition.rs    #   条件表达式求值（扁平 all_of/any_of/not）
│   │   │   ├── content/            # 内容引擎（无状态加载/查询层）
│   │   │   │   ├── engine.rs       #   内容引擎核心（自动发现、查询接口）
│   │   │   │   ├── loader.rs       #   YAML/Markdown 文件加载器（DataSource 抽象）
│   │   │   │   ├── models.rs       #   数据模型（Scene/Character/Chapter 等）
│   │   │   │   ├── dialogue.rs     #   对话 fence-aware 行扫描解析器
│   │   │   │   ├── condition.rs    #   条件求值（eval/topic_visible）
│   │   │   │   └── checker/        #   内容完整性校验（darkbluff check）
│   │   │   ├── save/               # 存档系统
│   │   │   │   ├── store.rs        #   SaveStore 编排（加载/保存/新游戏/设置）
│   │   │   │   ├── schema.rs       #   存档数据结构定义
│   │   │   │   ├── checkpoint.rs   #   检查点创建与回滚（数组长度截断）
│   │   │   │   ├── snapshot.rs     #   笔记快照读写 + 孤儿清理
│   │   │   │   ├── atomic.rs       #   原子写入 + .bak 备份 + 损坏恢复
│   │   │   │   ├── clock.rs        #   时钟抽象（注入，便于测试）
│   │   │   │   └── migration.rs    #   存档版本迁移逻辑
│   │   │   ├── world.rs            # 叶子：视角枚举（surface/shadow）
│   │   │   └── error.rs            # 叶子：跨层错误 AppError / Result
│   │   └── tests/
│   │       ├── *.rs                # e2e / session / checker / content_engine 集成测试
│   │       └── fixtures/data/      # 测试内容数据集
│   ├── darkbluff/                  # 二进制：装配 + CLI
│   │   └── src/
│   │       ├── main.rs             # 入口（mod cli / mod log）
│   │       ├── cli.rs              # CLI 参数（play/check/--no-motion/--data-dir）
│   │       └── log.rs              # 日志初始化（check→stderr，play→文件）
│   └── darkbluff-tui/              # 渲染层（当前为空壳；实装时仅依赖 engine 门面）
│       └── src/lib.rs
├── data/                           # 正式游戏内容数据（TODO；当前仅 crates/darkbluff-core/tests/fixtures/data 可用）
│   ├── scenes/  characters/  chapters/  text/
└── docs/                           # 设计文档（本目录）
```

依赖方向由 Cargo 强制（单向、无环）：`darkbluff`（bin）→ `darkbluff-core` ← `darkbluff-tui`。
core 不含 clap/ratatui；`darkbluff check` 无需编译 TUI 重依赖。`engine` 内部处理模块为私有实现细节，渲染层的游戏流程入口应只使用 `darkbluff_core::engine` 暴露的 `Session` / `Input` / `Outcome` / `SessionState` 等门面类型。`content` / `save` 仍作为 core 的公共作者工具与存档 API 暴露给二进制、测试和后续工具；TUI/GUI 不应依赖这些模块来驱动游戏流程。


## 架构分层

UI 层是独立的 `darkbluff-tui` crate（当前为空壳）；引擎层 / 存档层 / 内容引擎均在 `darkbluff-core` crate 内（编译器强制单向依赖 `darkbluff` → `darkbluff-core` ← `darkbluff-tui`）。

```
┌─────────────────────────────────────────┐
│  UI 层 (darkbluff-tui crate)            │
│  面板布局、视角指示器、Markdown 渲染、  │
│  动画、输入交互                          │
├─────────────────────────────────────────┤
│  引擎层 (darkbluff-core::engine)        │
│  指令处理、游戏状态机、审判逻辑、       │
│  自动推进章节、条件求值                 │
├─────────────────────────────────────────┤
│  存档层 (darkbluff-core::save)          │
│  存档读写、检查点回滚、原子写入与备份、 │
│  版本迁移、兼容性处理                   │
├─────────────────────────────────────────┤
│  内容引擎 (darkbluff-core::content)     │
│  自动发现、校验、统一查询接口           │
├─────────────────────────────────────────┤
│  游戏数据 (data/)                       │
│  场景、角色、章节、对话文本             │
└─────────────────────────────────────────┘
```

## 技术约束

- 第三方 ratatui 生态 crate（tachyonfx、tui-markdown、tui-tree-widget、tui-prompts）需确认对同一 ratatui 主版本的兼容性；**`tui-markdown` 相对小众，需在实现前做 POC 验证**——确认其维护状态、渲染能力（中文排版、嵌套列表、代码块）是否满足需求。若不满足则回退到基于 pulldown-cmark 自行渲染 `Text`/`Paragraph` widget 的方案（复杂度可控，仅需处理标题/段落/加粗/斜体/列表等基础元素）
- `include_dir` 内嵌模式尚未实现；实现后，内嵌的 `data/` 目录变更需要重新编译
- crossterm 事件读取需在独立线程或 async task 中进行，避免阻塞渲染
- **终端最小尺寸**: 需定义最小终端尺寸（如 80×24），低于此尺寸时显示提示并降级布局，避免面板错乱
- **场景描述完整性**: 启动校验保证每个被章节引用的场景同时有 surface 和 shadow 描述，运行时不存在视角缺失的降级场景（见 [content-engine.md](content-engine.md)）
- **存档写入安全**: 必须遵循 [save-system.md](save-system.md)「存档健壮性」的原子写入 + `.bak` 备份策略，任何崩溃都不应导致存档永久损坏
- **信号处理**: 注册 `SIGINT`/`SIGTERM` handler（Ctrl+C、终端关闭），确保：(1) 退出 raw mode、恢复终端状态（crossterm `disable_raw_mode` + `LeaveAlternateScreen`）；(2) 在退出前触发一次自动保存（best-effort，若保存失败不阻塞退出）。使用 `ctrlc` crate 或 crossterm 自带信号事件

## 错误处理与日志

- **错误策略**: 用 `thiserror` 定义领域错误枚举（内容错误 / 存档错误 / 指令错误 / IO 错误），跨层以 `Result<T, AppError>` 传播，保留错误类型，避免 `anyhow` 在库层吞类型
- **panic 边界**: 仅在「本应由启动校验保证合法、却仍违反不变量」的程序员错误处 panic（附清晰消息）；一切外部输入（玩家指令、存档文件、内容数据）的错误必须以 `Result` 返回并转为 UI 提示，**绝不 panic**（见 [commands.md](commands.md)「运行时错误处理」）
- **panic hook**: 设置自定义 panic hook，在 panic 前恢复终端状态（退出 raw mode、恢复光标、离开 alternate screen），避免用户终端被锁死
- **日志**: 使用 `tracing` + `tracing-appender`，默认级别 `warn`，通过 `RUST_LOG` 调节。`play` 模式日志输出到**日志文件**（应用数据目录下的 `darkbluff/darkbluff.log`，使用 `dirs` crate 获取路径），**不输出到 stderr**——ratatui 使用 alternate screen 后，stderr 输出会干扰渲染。`darkbluff check`（不启动 TUI）时日志输出到 stderr。当前实现使用 tracing-appender 按天滚动；按大小轮转（如 5MB，保留最近 2 个文件）为后续 TODO

## 渲染与无障碍

- **CJK 宽字符**: 中文为全角字符，显示宽度为 2 个终端单元格。布局、换行、面板宽度必须基于 `unicode-width` 的**显示宽度**计算，而非字符数或字节数，否则面板错位、文本被错误截断
- **视角区分不依赖颜色**: 表面/影子视角以顶部文字指示器（`👁 右眼·表面` / `👁 左眼·影子`）为权威标识，颜色与动画仅作辅助。确保色盲玩家、单色终端下仍能明确区分两个世界
- **动画可关闭**: `--no-motion` 或设置项关闭/缩短过渡动画（见 [save-system.md](save-system.md)「设置文件」），兼顾低性能终端与前庭敏感玩家

## 动画与事件循环

- **常态事件驱动**: 无动画时只响应输入、不轮询重绘（节省 CPU）
- **动画期间 tick 重绘**: 动画播放时启用定时 tick 重绘，结束后回到事件驱动
- **动画期间输入**: 动画期间的玩家输入**排队缓存**（不丢弃），结束后按序处理；过渡动画通常很短可忽略，较长的动画需支持按键跳过

## CLI 与运行模式

```
darkbluff                        # 默认进入标题界面（等价于 play）
darkbluff play                   # 显式进入游戏（标题界面）
darkbluff check                  # 离线校验 data/ 内容，不启动 TUI（CI / 内容审校用）
darkbluff --no-motion            # 本次运行关闭过渡动画（临时，不写设置文件；TUI 实装后生效）
darkbluff --data-dir <path>      # 指定内容数据目录（当前开发模式可用）
```

- `check` 与 `play` 为子命令，`--no-motion` / `--data-dir` 为全局 flag
- 当前 `play` 会先校验内容，然后打印“TUI 尚未实现”并退出；正式渲染循环后续接入 `darkbluff-tui`
- 当前仓库尚未包含正式根目录 `data/`，开发与测试使用 `crates/darkbluff-core/tests/fixtures/data`
- 发布模式的 `include_dir!` 内嵌数据尚未实现；实现后可再决定 `--data-dir` 是否仅在开发 feature 下生效

## 状态机

`engine::Session` 维护 UI 无关的显式状态机。渲染层只通过 `Input` 驱动会话、根据 `Outcome` 渲染结果，并可读取 `SessionState` 判断当前可接受输入。引擎层不依赖 ratatui/crossterm；TUI 与未来 GUI 应共享同一套会话 API。

| 状态 | 说明 |
|------|------|
| `Title` | 标题菜单（新游戏 / 继续 / 退出） |
| `ShowingIntro` | 章节开场/过场文本（`intro`）展示中，玩家确认后进入 `Exploring`；无 `intro` 则跳过 |
| `Exploring` | 章节内自由探索（ask / move / gaze / map / note / help / quit 可用） |
| `ChoosingAskCharacter` | `ask` 无参数时选择当前场景在场角色 |
| `ChoosingAskTopic` | `ask` 第二步：选择该角色可问话题 |
| `ChoosingJudgeCharacter` | `judge` 无参数时选择本章未审判角色 |
| `ChoosingMove` | `move` 无参数时选择当前可达场景 |
| `ChoosingCheckpoint` | `map` 后选择已经历过的 checkpoint |
| `Confirming` | 破坏性操作二次确认（map checkpoint 回滚 / 覆盖存档） |
| `ShowingOutro` | 终章结局文本（`outro`）展示中，玩家确认后进入 `Ending`；无 `outro` 则跳过直接进入 `Ending` |
| `Ending` | 结局界面：终章 `title` 作为结局名 +「已发现结局 X/Y」+ 返回标题 |

对外输入输出类型：

- `Input::Text` 仅在 `Exploring` 态承载玩家命令文本。
- `Input::Select(Selection::Index | Selection::Id)` 用于所有菜单选择；TUI 可按索引，GUI 可直接传 `MenuOption.id`。
- `Input::Confirm(bool)` 用于二次确认，`Input::Cancel` 用于取消菜单/确认，`Input::Ack` 用于继续 intro/outro/ending。
- `Outcome::Message(Message)` 返回带级别的领域消息；`MessageLevel` 只表达 info/warning/error 语义，不绑定具体 UI 样式。
- `Outcome::MenuRequested { kind, prompt, options }` 与 `Outcome::ConfirmationRequested { action, prompt }` 只描述领域意图，不描述控件形态。
- `Outcome::ChapterIntro` / `ChapterOutro` / `Dialogue` / `Notes` / `EndingReached` / `QuitRequested` 分别表达叙事、对话、笔记、结局与退出意图。

## 设计取舍

### `map` 的回滚粒度

`map` 回到某个 checkpoint 会连带撤销该 checkpoint 之后的所有探索（`move`/`gaze`/`ask` 的位置与事实变更）。这不是疏忽，而是**检查点模型的固有约束**：

- 检查点只记录「三个权威数组在创建时刻的长度」，回滚即按长度截断
- 若要支持「只撤销审判、保留后续探索」，需要事件回放或更细粒度的快照，显著增加存档复杂度和正确性风险
- 在实际游戏中，玩家通常通过 `map` 主动选择明确的回滚点，因此粒度可接受
- 若后续玩测反馈频繁遇到此问题，可考虑引入「审判撤销标记 + 重选」的轻量方案，但 v1 暂不支持

## Help 文本

`help` 指令的帮助文本**硬编码在 `engine/commands.rs` 中**，以中文书写：

- `help`（无参数）：列出所有指令名称 + 一句话用途说明
- `help [指令]`：展示该指令的详细用法（语法、参数、示例）
- 帮助文本不从数据文件加载——它属于引擎固有知识，不随内容变更

## 性能预期

### 预期内容规模

- **章节数**: 10–30 个（含分支章节与终章）
- **角色数**: 10–20 个全局角色
- **场景数**: 10–30 个全局场景
- **对话文件**: 每章 3–8 个角色 × 1 个 .md，单文件 1–5 KB
- **总数据量**: 约 500 KB–2 MB（纯文本，无媒体资源）

### 资源估算

- **内存**: 全量加载所有内容数据到内存，预期占用 < 10 MB，完全可行
- **启动时间**: YAML 解析 + 对话行扫描解析 + 启动校验，预期 < 500ms（即使含 30 个章节）
- **二进制体积**: `include_dir` 内嵌后，预期增加 1–3 MB（可接受，仍为小型 CLI 工具）
- **编译时间**: `include_dir!` 在编译期扫描 `data/`，数据量在上述规模内对编译时间影响可忽略

> 若未来内容规模超出预期（如 100+ 章节），可考虑按需加载（lazy loading）替代全量加载，但 v1 无此需求。

## UI 级 Warning

除 stderr 日志外，以下场景需要在 TUI 中向玩家展示可见的 warning 通知：

| 场景 | UI 展示方式 |
|------|-------------|
| 存档加载时发现引用的内容已删除 | 加载完成后在场景面板底部短暂显示通知条（如"部分记录因内容更新而失效"），5 秒后自动消失或按任意键关闭 |
| `save.json` 损坏回退到 `.bak` | 显示通知条"存档文件损坏，已从备份恢复" |
| `.bak` 也损坏，以新存档启动 | 显示通知条"存档损坏无法恢复，已开始新游戏" |
| 快照文件缺失 | 在 note 面板对应条目处直接显示"该记录快照缺失"（已定义） |

非紧急 warning（如孤立章节检测）仅输出到日志文件，不在 TUI 中显示。

## 测试策略

| 层级 | 测试类型 | 覆盖范围 |
|------|----------|----------|
| 条件求值 (`condition.rs`) | 单测 | `Fact`/`AllOf`/`AnyOf`/`Not` 各变体、空集边界、未知 id |
| 内容引擎 (`content/`) | 单测 + 集成 | 校验逻辑覆盖所有校验点；给定测试 `data/` 目录验证加载结果 |
| 存档回滚 (`save/`) | 单测 | `map` checkpoint 回滚的状态正确性、边界（无 checkpoint、当前章/跨章、`chapter_start`/`before_judgment` 等） |
| 指令解析 (`commands.rs`) | 单测 | 各指令合法/非法输入、大小写、多空白、补全候选 |
| 端到端 | 集成 | 给定指令序列，验证最终存档状态与 `discovered` 内容（snapshot 测试） |
| 内容数据 | CI check | `darkbluff check` 集成到 CI，每次内容变更自动校验 |

测试 `data/` 使用专用的 `tests/fixtures/` 目录，与正式游戏内容解耦。

## 开发工作流

- **开发模式**（默认 feature）：从文件系统读取 `data/` 目录，修改数据文件后**重启游戏**即生效（不支持运行中热重载——状态机持有内容引用，热重载需处理引用失效，v1 不做）
- **内容迭代循环**：编辑 YAML/MD → `darkbluff check`（秒级校验）→ 重启 `darkbluff play` 验证效果
- **发布模式**（TODO）：`include_dir!` 内嵌数据，任何数据变更需重新编译
- **`--data-dir <path>`**：当前用于覆盖默认 `data/` 路径，方便测试不同内容集

## 新手引导

首章承担教程职责——通过 `intro` 文本和对话内容自然引入 `gaze`/`ask`/`judge` 操作，不设独立教程模式。引擎层额外提供**上下文提示机制**：

- 若玩家在表面世界连续多次 `ask` 而未 `gaze`（首章前 3 次对话后），场景面板底部显示一次性提示"试试 `gaze` 切换到影子世界，看看会有什么不同"
- 若玩家收集了足够线索但未尝试 `judge`，提示"也许可以对某人做出 `judge` 了"
- 若玩家达成非预期走向后回到游戏内，提示"可以用 `map` 回到之前的检查点，尝试另一条路"
- 上述 gaze/judge/map 提示只在首章触发、每种只显示一次、不阻断操作

这些触发条件硬编码在引擎中（首章特权），不需要数据文件定义。后续章节不再显示引导提示。

## Map 面板

`map_panel.rs` 负责渲染章节分支树与 checkpoint 地图，数据来源和交互：

- **数据来源**: 内容引擎提供完整章节图结构（所有章节及其 `next` 关系）与各章角色话题定义（`get_topics`）；`discovered.chapters` 标记已到过的节点，`discovered.topics` 提供话题的跨周目收集计数；存档 `checkpoints` 提供可回滚节点
- **渲染规则**:
  - 已到过的章节：显示章节 `title`（中文），正常样式
  - 未到过但可见的分支（已到过章节的直接 `next` 目标）：显示为 `???`（给予分支存在感但不剧透）
  - 完全不可见的章节（距已探索区域 > 1 跳）：不渲染
  - 当前所在章节：高亮标记（如 `>` 前缀或反色）
  - 已达成的终章：追加结局标记（如 `★`）
  - 已经历 checkpoint：在对应章节下列出 `chapter_start` 与 `before_judgment` 节点，标注创建时间或审判目标
  - **已到过章节的话题进度**: 在该章节点下按角色聚合显示「话题 X/Y」（X = `discovered.topics[chapter]` 中该角色已问过的话题数，Y = 该角色本章可问话题总数——不含永久不可问的话题）。仅给计数、不给未问话题的 label，避免剧透；无话题的章节不展开
- **交互**: 通过 `map` 进入后可选择已经历过的 checkpoint；选择后进入 `Confirming` 二次确认，Esc 取消。通过 `note` 进入的章节树标签页为只读展示
- **窄终端降级**: 终端宽度不足时隐藏侧边 map，仅在 `map` / `note` 界面以列表形式展示
