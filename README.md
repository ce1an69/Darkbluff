# Darkbluff

CLI/TUI 文字推理游戏。玩家扮演一只异色瞳的猫——右眼看表面世界(恒真),左眼看影子世界(恒假)——通过对比两个世界的信息进行逻辑推理,审判角色,走向不同结局。

## 当前状态

核心引擎(content/save/engine/cli)已完成,**128 测试通过**。TUI 渲染层待实现。

## 快速开始

```bash
# 内容校验
cargo run -- check --data-dir tests/fixtures/data

# 进入游戏（TUI 尚未实现）
cargo run

# 运行测试
cargo test
```

## 技术栈

Rust · serde/JSON · YAML(serde_yml) · pulldown-cmark · clap · tracing · dirs · chrono

数据结构 YAML + Markdown,与代码完全分离。发布模式将通过 `include_dir` 内嵌数据(待实现)。

## 项目结构

```
src/
├── content/   # 内容引擎（模型/加载/校验/查询,无状态）
├── save/      # 存档系统（原子写/检查点回滚/快照/迁移）
├── engine/    # 游戏引擎（条件求值/指令解析/状态机/审判推进）
├── cli.rs     # CLI（check 实装）
└── log.rs     # 日志（check→stderr,play→文件）
docs/          # 设计文档
tests/fixtures/data/  # 测试数据集
```

## License

暂未指定。
