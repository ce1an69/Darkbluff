# Darkbluff

A CLI/TUI mystery deduction game. Play as a cat with heterochromia — right eye sees the surface world (truth), left eye sees the shadow world (lies). Gather clues, judge characters, and piece together the truth.

## Story

A forgotten industrial town. The slaughterhouse runs day and night; the smell of iron and blood covers every alley. The surface world maintains a veneer of order — the tavern is open, the market is trading, workers shuffle between shacks. Switch to the left eye: the same wall begins to writhe, and beneath every conversation churns the fear and desire the speaker is denying.

There is no absolute good or evil here. The Butcher's Guild controls the economy; the Guildmaster's silence is itself a position. A few possess psychic aberrations — they can sense when they are being watched, can limit how much of their inner lies leak out. One among them is almost perfectly smooth; the cat's left eye cannot get a grip on anything.

No magic. No monsters. The supernatural exists only in the mind. But a gaze has weight — those caught in the cat's heterochromatic stare feel an unease they cannot explain.

Seven sins. Seven judgments. Each judgment peels back a layer of the town, and a layer of the cat. Four worldlines collapse from the same fragments into four shapes — Survivor, Participant, Key, Projection. None of them is the truth. The truth is what you piece together after walking every path.

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
