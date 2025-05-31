# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Skim is a command-line fuzzy finder written in Rust, providing the `sk` executable as an alternative to tools like `fzf`. It features fuzzy matching, interactive mode, multi-selection, preview windows, and extensive shell integration.

## Development Commands

### Building and Testing
```bash
# Build the project
cargo build --release

# Run unit tests across all crates
cargo test -p skim -p skim-common -p skim-tuikit

# Run end-to-end tests (requires tmux)
cargo e2e -j8

# Code quality checks
cargo clippy
cargo fmt --all -- --check
```

### Development Tasks
```bash
# Generate man pages
cargo xtask mangen

# Generate shell completions
cargo xtask compgen
```

## Architecture

This is a multi-crate Cargo workspace:

- **`skim/`**: Main crate containing the core library and `sk` binary
- **`skim-tuikit/`**: Custom TUI framework for terminal rendering and events
- **`skim-common/`**: Shared utilities across crates
- **`e2e/`**: End-to-end testing framework using tmux automation
- **`xtask/`**: Development task runner for generating man pages and completions

### Core Library Architecture

The main library (`skim/src/lib.rs`) uses an event-driven architecture with these key traits:
- **`SkimItem`**: Items that can be searched, matched, and previewed
- **`MatchEngine`**: Different matching algorithms (fuzzy, exact, regex, etc.)
- **`Selector`**: Controls item pre-selection in multi-selection mode

Key modules:
- `engine/`: Matching algorithm implementations
- `input.rs`: Keyboard input and key binding handling
- `previewer.rs`: Preview window functionality  
- `reader.rs`: Input source handling (commands, pipes, files)
- `options.rs`: Configuration and CLI option parsing
- `theme.rs`: Color schemes and theming

### Communication Pattern

Uses crossbeam channels for event communication between components. The architecture follows a Model-View pattern where `Model` handles business logic and terminal rendering is delegated to tuikit.

## Testing Strategy

- **Unit tests**: Standard Rust tests in each crate
- **E2E tests**: Automated tmux-based tests that simulate real terminal usage
- **CI platforms**: Tests run on both Linux (musl) and macOS
- **Code quality**: Clippy linting and rustfmt formatting enforced

## Key Files for Development

- `skim/src/bin/main.rs`: Main binary entry point
- `skim/src/options.rs`: CLI argument parsing and configuration
- `skim/src/input.rs`: Key binding and input handling logic
- `e2e/tests/`: End-to-end test scenarios using tmux automation
- `xtask/src/main.rs`: Development task automation