# Darkbluff

CLI/TUI 文字推理游戏。玩家扮演一只异色瞳的猫——右眼看表面世界(恒真),左眼看影子世界(恒假)——通过对比两个世界的信息进行逻辑推理,审判角色,走向不同结局。

## 剧情简介

这是一座被遗忘的工业城镇。屠宰场昼夜运转，铁腥味覆盖每一条巷子。表面世界维持着正常秩序——酒馆在营业，市场在交易，工人在棚屋间进出。切换到左眼，同一面墙开始蠕动，同一段对话底下翻出说话者正在否认的恐惧与欲望。

镇上没有绝对善恶。屠夫行会掌控经济命脉，首领的沉默本身就是一种立场。少数人拥有精神畸变——能感知自己正被注视，能控制心底的谎言暴露多少。其中一人几乎完全光滑，猫的左眼抓不住任何东西。

没有魔法，没有怪物。超自然仅存在于精神层面。但注视是有重量的——被异色瞳凝视的人会感到一种无法解释的不安。

七宗罪，七场审判。每场审判剥开镇子的一层，也剥开猫自己的一层。四条世界线在同一堆碎片上坍缩出四种形状——幸存者、参与者、钥匙、投影。没有哪条是真正的真相。真相是所有线索走完之后，由你拼出来的那个东西。

## 当前状态

核心引擎(content/save/engine/cli)与 TUI 渲染层已完成,**171 测试通过**。

TUI:圆角 Catppuccin 紫色主题;左侧 markdown 对话转录、右侧场景描述 + 在场 NPC,底部 Claude-Code 式斜杠指令输入(`/ask`、`/judge`... 输入即自动补全,候选与引擎菜单同源);界面文案英文,剧情内容随数据语言。

> 仓库暂未附带正式 `data/`,以下示例使用测试 fixture 数据——一个可完整通关的迷你剧本「失踪的屠夫」。

## 快速开始

```bash
# 内容校验(离线,不启动 TUI)
cargo run -- check --data-dir crates/darkbluff-core/tests/fixtures/data

# 进入游戏(TUI,终端 ≥ 86×24)
cargo run -- --data-dir crates/darkbluff-core/tests/fixtures/data

# 运行测试
cargo test
```

游戏内操作:标题菜单 `↑/↓` 选择、`Enter` 确认;探索态输入 `/` 触发指令补全,`Tab` 补全、`Enter` 提交;任意状态 `Ctrl+C` 存档退出。TUI 推荐输入 `/ask`、`/judge` 等斜杠形式;底层引擎仍接受裸命令 `ask / judge / move / gaze / note / map / help / quit`。

## 技术栈

Rust · ratatui · crossterm · unicode-width · serde/JSON · YAML(serde_yml) · clap · tracing · dirs · chrono

数据结构 YAML + Markdown,与代码完全分离。正式 `data/` 与发布模式 `include_dir!` 内嵌待实现;当前示例使用测试 fixture。

## 项目结构

Cargo workspace,三个 crate 严格单向依赖(`binary → {core,tui}`,`tui → core`):

```
crates/
├── darkbluff-core/   # 核心库:内容/存档/引擎(渲染无关,可无终端测试)
│   ├── src/{content,save,engine} + world.rs/error.rs
│   └── tests/        # 含 fixtures/data 测试数据集
├── darkbluff/        # 二进制:CLI 装配 + play/check 分发
└── darkbluff-tui/    # 渲染层:ratatui/crossterm,只依赖 core 公共契约
docs/                 # 设计文档
```

**依赖方向由 Cargo 强制**:core 不含 clap/ratatui;TUI 只通过 engine 门面驱动游戏流程。


## License

暂未指定。
