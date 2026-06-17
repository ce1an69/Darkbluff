# Darkbluff

A CLI/TUI mystery deduction game. Play as a cat with heterochromia — right eye sees the surface world (truth), left eye sees the shadow world (lies). Gather clues, judge characters, and piece together the truth.

## Status

Core engine (content/save/engine/CLI) and a basic TUI are wired — **133 tests passing**.

## Quick Start

```bash
# Validate content data
cargo run -- check --data-dir crates/darkbluff-core/tests/fixtures/data

# Run the game (current example uses the test fixture dataset)
cargo run -- --data-dir crates/darkbluff-core/tests/fixtures/data

# Run tests
cargo test
```

## Tech Stack

Rust · ratatui · crossterm · serde/JSON · YAML (serde_yml) · clap · tracing · dirs · chrono

YAML + Markdown data-driven content, fully separated from code. The production `data/` directory and `include_dir` embedded mode are still planned; current examples use the test fixture dataset.

## Project Layout

Cargo workspace with three crates in a strict one-way DAG (`binary → {core,tui}`, `tui → core`):

```
crates/
├── darkbluff-core/   # Core lib: content/save/engine (render-agnostic, testable headless)
│   ├── src/{content,save,engine} + world.rs/error.rs
│   └── tests/        # includes fixtures/data test dataset
├── darkbluff/        # Binary: CLI wiring + play/check dispatch
└── darkbluff-tui/    # Render layer: ratatui/crossterm, depends only on core's public contract
docs/                 # Design documents
```

The dependency direction is enforced by Cargo: core has no clap/ratatui, and TUI drives gameplay only through the engine facade.


## License

Not specified yet.
