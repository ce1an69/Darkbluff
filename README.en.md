# Darkbluff

A CLI/TUI mystery deduction game. Play as a cat with heterochromia — right eye sees the surface world (truth), left eye sees the shadow world (lies). Gather clues, judge characters, and piece together the truth.

## Status

Core engine (content/save/engine/CLI) and the TUI render layer are complete — **137 tests passing**.

The TUI features a rounded Catppuccin-purple theme, a markdown dialogue transcript on the left, a scene description + present NPCs panel on the right, and a Claude-Code-style slash command input at the bottom (`/ask`, `/judge`, ... with autocompletion sourced from the engine's own menus). UI chrome is in English; story content follows the data language.

> The repo does not yet ship a production `data/` directory — the examples below use the test fixture dataset, a fully playable mini-scenario ("The Missing Butcher").

## Quick Start

```bash
# Validate content data (offline, no TUI)
cargo run -- check --data-dir crates/darkbluff-core/tests/fixtures/data

# Run the game (TUI, terminal ≥ 86×24)
cargo run -- --data-dir crates/darkbluff-core/tests/fixtures/data

# Run tests
cargo test
```

In-game: title menu uses `↑/↓` to choose and `Enter` to confirm; in exploration, type `/` to trigger command autocompletion, `Tab` to complete, `Enter` to submit; `Ctrl+C` saves and quits from any state. Commands: `ask / judge / move / gaze / note / map / help / quit`.

## Tech Stack

Rust · ratatui · crossterm · unicode-width · serde/JSON · YAML (serde_yml) · clap · tracing · dirs · chrono

YAML + Markdown data-driven content, fully separated from code. The production `data/` directory and `include_dir!` embedded release mode are still planned; current examples use the test fixture dataset.

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
