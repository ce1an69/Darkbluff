# Darkbluff

A CLI/TUI mystery deduction game. Play as a cat with heterochromia — right eye sees the surface world (truth), left eye sees the shadow world (lies). Gather clues, judge characters, and piece together the truth.

## Status

Core engine (content/save/engine/CLI) implemented — **128 tests passing**. TUI layer not yet built.

## Quick Start

```bash
# Validate content data
cargo run -- check --data-dir tests/fixtures/data

# Run the game (TUI not yet available)
cargo run

# Run tests
cargo test
```

## Tech Stack

Rust · serde/JSON · YAML (serde_yml) · pulldown-cmark · clap · tracing · dirs · chrono

YAML + Markdown data-driven content, fully separated from code. `include_dir` embedded mode planned for release.

## Project Layout

```
src/
├── content/   # Content engine (models/loader/checker/queries, stateless)
├── save/      # Save system (atomic writes/checkpoint rollback/snapshots/migration)
├── engine/    # Game engine (condition eval/command parsing/state machine/judgment flow)
├── cli.rs     # CLI (check subcommand)
└── log.rs     # Logging (check→stderr, play→file)
docs/          # Design documents
tests/fixtures/data/  # Test dataset
```

## License

Not specified yet.
