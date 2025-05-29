# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building
- `cargo build` - Build development binary
- `cargo build --release` - Build optimized binary
- `cargo install` - Install sk binary to cargo bin

### Testing
- `cargo test` - Run unit tests for all workspace crates
- `cargo test -p skim -p skim-common -p skim-tuikit` - Run unit tests for core crates
- `cargo e2e` - Run end-to-end tests (requires tmux and release build)
- Full e2e test sequence:
  ```bash
  cargo build --release
  tmux new-session -d
  cargo e2e -j8
  ```

### Code Quality
- `cargo clippy` - Run linter
- `cargo fmt --all -- --check` - Check code formatting
- `cargo fmt` - Auto-format code

### Documentation Generation
- `cargo xtask mangen` - Generate man page (outputs to man/man1/sk.1)
- `cargo xtask compgen` - Generate shell completions (outputs to shell/ directory)

## Architecture Overview

Skim is a modular fuzzy finder with a multi-crate workspace architecture:

### Workspace Crates
- **`skim/`** - Main library and CLI binary with fuzzy finding logic
- **`skim-tuikit/`** - Independent terminal UI toolkit (can be used standalone)
- **`skim-common/`** - Shared utilities (currently just SpinLock)
- **`e2e/`** - End-to-end test suite
- **`xtask/`** - Build automation tasks

### Core Architecture Patterns

**Event-Driven MVC:**
- **Model** (`skim/src/model/`): Central state manager and event coordinator
- **Components**: Query, Selection, Header, Matcher, Reader, Previewer
- **Communication**: Crossbeam channels for thread-safe message passing

**Plugin System:**
- **Matching Engines** (`skim/src/engine/`): Fuzzy, exact, regex, and/or combinators
- **Item Sources**: Trait-based readers for stdin, commands, custom data
- **Preview System**: Configurable preview command execution

**Concurrent Pipeline:**
- Separate threads for input reading, matching, and UI rendering
- Lock-free data structures using SpinLock from skim-common
- Parallel processing with rayon for performance

### Key Integration Points

**Custom Item Types:**
- Implement `SkimItem` trait with downcast support for custom data
- Use `SkimItemReader` to convert BufRead sources to item streams

**Library Usage:**
- Main entry point: `Skim::run_with(&options, Some(items))`
- Configure via `SkimOptionsBuilder`
- Receive results through `SkimOutput::selected_items`

**TUI Extension:**
- `skim-tuikit` provides widget system for custom terminal applications
- Exported as `skim::tuikit` from main crate

## API Stability
**This is a published library crate - avoid making breaking changes to the public API.** Be especially careful with:
- Public trait definitions (`SkimItem`, `MatchEngine`, etc.)
- Public struct fields and method signatures
- Default behavior changes that could affect existing users
- Changes to `SkimOptions` and `SkimOptionsBuilder`
- Exported modules and re-exports

## Environment Variables
- `SKIM_DEFAULT_COMMAND` - Default command for file listing (used when no input provided)
- `LC_ALL=en_US.UTF-8` - Required for proper Unicode handling in tests
- `TERM=xterm-256color` - Terminal type for consistent behavior

## File Organization
- Main CLI binary: `skim/src/bin/`
- Core algorithms: `skim/src/engine/` (matching), `skim/src/reader.rs` (input)
- UI components: `skim/src/` (query, selection, header, previewer)
- Examples: `skim/examples/` for library usage patterns