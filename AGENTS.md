# Skim Agent Guidelines

## Build/Test/Lint Commands
- Build: `cargo build [--release]`
- Run: `cargo run [--release]`
- Test (all): `cargo nextest`
- Test (single): `cargo nextest test_name`
- E2E tests: `cargo nextest --tests` (will need tmux under the hood)
- Lint: `cargo clippy`
- Format: `cargo fmt` (check only: `cargo fmt --check`)

## Code Style
- Format with 120 char line width (defined in .rustfmt.toml)
- Use standard Rust naming conventions (snake_case for functions/variables, CamelCase for types)
- Organize imports by standard library, external crates, then internal modules
- Prefer Option/Result types for error handling over panicking
- Use proper error propagation with `?` operator
- Document public API with rustdoc comments
- Use meaningful type annotations, especially for public functions
- Follow the existing structure for new modules (see src/engine/ or src/model/)
- Implement relevant traits (SkimItem, etc.) for new types when needed

## Project Structure
- Core functionality in `skim/src/`
- Common utilities in `skim-common/`
- Task automation in `xtask/`


## Testing

This application can be tested by :
- creating a new `tmux` session in the background (`tmux new-session -s <session name> -d`)
- creating a new named tmux window in that session : `tmux new-window -d -P -F '#I' -n <window name> -t <session name>` and configuring the pane naming using `tmux set-window-option -t <window name> pane-base-index 0`
- sending the command to run and input using `tmux send-keys -t <window name> <keys>`
- when ready, capturing the window using `tmux capture-pane -b <window name> -t <window name>.0` and then saving the capture to a file using `tmux save-buffer -b <window name> <output file>`