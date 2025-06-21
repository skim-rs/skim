# Skim Agent Guidelines

## Build/Test/Lint Commands
- Build: `cargo build [--release]`
- Run: `cargo run [--release]`
- Test (all): `cargo test`
- Test (single): `cargo test test_name` or `cargo test -- test_name`
- E2E tests: `cargo test -p e2e`
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
- UI toolkit in `skim-tuikit/`
- Common utilities in `skim-common/`
- End-to-end tests in `e2e/`
- Task automation in `xtask/`