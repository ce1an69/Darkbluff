# Darkbluff

A CLI/TUI mystery deduction game. Play as a cat with heterochromia — right eye sees the surface world (truth), left eye sees the shadow world (lies). Gather clues, judge characters, and piece together the truth.

## Status

Core engine (content/save/engine/CLI) implemented — **133 tests passing**. TUI layer not yet built.

## Quick Start

```bash
# Validate content data
cargo run -- check --data-dir crates/darkbluff-core/tests/fixtures/data

# Run the game (TUI not yet available)
cargo run

# Run tests
cargo test
```

## Tech Stack

Rust · serde/JSON · YAML (serde_yml) · pulldown-cmark · clap · tracing · dirs · chrono

YAML + Markdown data-driven content, fully separated from code. The production `data/` directory and `include_dir` embedded mode are still planned; current examples use the test fixture dataset.

## Project Layout

Cargo workspace with three crates in a strict one-way DAG (`binary → core ← tui`):

```
crates/
├── darkbluff-core/   # Core lib: content/save/engine (render-agnostic, testable headless)
│   ├── src/{content,save,engine} + world.rs/error.rs
│   └── tests/        # includes fixtures/data test dataset
├── darkbluff/        # Binary: CLI wiring + play/check dispatch
└── darkbluff-tui/    # Render layer (stub; will depend only on core's public contract)
docs/                 # Design documents
```

The dependency direction is enforced by Cargo: core has no clap/ratatui, and `darkbluff check` compiles without TUI heavy deps.


## License

Not specified yet.
