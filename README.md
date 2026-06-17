# Darkbluff

CLI/TUI 文字推理游戏。玩家扮演一只异色瞳的猫——右眼看表面世界(恒真),左眼看影子世界(恒假)——通过对比两个世界的信息进行逻辑推理,审判角色,走向不同结局。

## 当前状态

核心引擎(content/save/engine/cli)与基础 TUI 已接入,**133 测试通过**。

## 快速开始

```bash
# 内容校验
cargo run -- check --data-dir crates/darkbluff-core/tests/fixtures/data

# 进入游戏（当前示例使用测试 fixture 数据）
cargo run -- --data-dir crates/darkbluff-core/tests/fixtures/data

# 运行测试
cargo test
```

## 技术栈

Rust · ratatui · crossterm · serde/JSON · YAML(serde_yml) · clap · tracing · dirs · chrono

数据结构 YAML + Markdown,与代码完全分离。正式 `data/` 与发布模式 `include_dir` 内嵌待实现；当前示例使用测试 fixture。

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
