# Skim Agent Guidelines

## Build/Test/Lint Commands
- Build: `cargo build [--release]`
- Run: `cargo run [--release]`
- Test (all): `cargo nextest run`
- Test (single): `cargo nextest test_name`
- Integration/E2E tests: `cargo nextest --tests` (will need tmux under the hood)
- Memory leak detection: `cargo nextest run --profile valgrind`
- Thread leak/race detection:
  1. Build: `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly build --tests -Zbuild-std --target x86_64-unknown-linux-gnu`
  2. Run: `TSAN_OPTIONS="detect_deadlocks=1" cargo +nightly nextest run --profile tsan --target x86_64-unknown-linux-gnu`
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

## Architecture Documentation
- `ARCHITECTURE.md` documents the full architecture: data flow, operating modes, subsystems, threading model, and public API.
- **Update `ARCHITECTURE.md` whenever you make structural changes**, including:
  - Adding, removing, or renaming modules, structs, or traits
  - Changing the data flow between subsystems (reader → pool → matcher → TUI)
  - Adding new operating modes or modifying existing ones
  - Changing the threading model or synchronization primitives
  - Adding or removing public API surface (`SkimItem`, `SkimOptions`, `SkimOutput`, etc.)
  - Changing the event/action system or key binding infrastructure
- Keep call-site line numbers in the cross-reference table up to date when the referenced functions move.


## Testing

This application can be tested by :
- creating a new `tmux` session in the background (`tmux new-session -s <session name> -d`). Make sure to clear the `SKIM_DEFAULT_OPTIONS` env var.
- creating a new named tmux window in that session : `tmux new-window -d -P -F '#I' -n <window name> -t <session name>` and configuring the pane naming using `tmux set-window-option -t <window name> pane-base-index 0`
- sending the command to run and input using `tmux send-keys -t <window name> <keys>`
- when ready, capturing the window using `tmux capture-pane -b <window name> -t <window name>.0` and then saving the capture to a file using `tmux save-buffer -b <window name> <output file>`

## Insta Snapshot Tests

Most TUI behaviour is covered by insta snapshot tests in `tests/`. The
infrastructure lives in `tests/common/insta.rs` and is exposed through two
macros: `snap!` and `insta_test!`.

### `insta_test!` — writing tests

**Simple variant** (single snapshot, no interaction):
```rust
insta_test!(my_test, ["item1", "item2"], &["--opt1", "opt2"]);
insta_test!(my_test, @cmd "printf 'a\nb'", &["--ansi"]);
insta_test!(my_test, @interactive, &["-i", "--cmd", "echo {q}"]);
```

**DSL variant** (multiple snapshots with interaction between them):
```rust
insta_test!(my_test, ["a", "b", "c"], &["--multi"], {
    @snap;                      // take a snapshot
    @key Up;                    // send a named key (Enter, Down, Tab, …)
    @char 'f';                  // send a single character
    @type "foo";                // type a string
    @ctrl 'w';                  // Ctrl+key
    @alt 'b';                   // Alt+key
    @shift Tab;                 // Shift+key
    @action Last;               // send an Action variant (no args)
    @action Down(1);            // send an Action variant (with args)
    @snap;                      // take another snapshot
    @assert(|h| condition);     // boolean assertion (does not snapshot)
    @exited 0;                  // assert the app exited with this code
});
```

### Snapshot file naming

| Macro form | File pattern | Example |
|---|---|---|
| Simple variant | `{file}__{test}.snap` | `options__opt_wrap.snap` |
| DSL variant — Nth `@snap` | `{file}__{test}@{NNN}.snap` | `options__opt_cycle@002.snap` |

DSL snapshots use a zero-padded three-digit suffix (`@001`, `@002`, …) so that
`cargo insta review` presents them in the order they were taken.

### Snapshot front-matter

Every snapshot includes a `description` field in its YAML front-matter. For a
DSL test the description shows the input, options, and the DSL commands that
ran **since the previous `@snap`**, making it easy to understand what state
each screenshot captures:

```
description: "input: items [\"a\", \"b\", \"c\"]\noptions: --multi\nafter:\n  @key Up\n  @shift Tab"
```

The `expression` field is intentionally omitted (`omit_expression = true`) to
keep the files free of internal implementation details.

### Snapshot workflow

Generate / update snapshots:
```sh
# Generate all missing snapshots and accept them immediately:
INSTA_UPDATE=always cargo nextest run

# Generate missing snapshots as .snap.new files for manual review:
INSTA_UPDATE=new cargo nextest run
cargo insta review          # accept / reject interactively
```

When **adding new tests** that produce snapshots:
1. Write the test with `@snap` markers.
2. Run `INSTA_UPDATE=always cargo nextest run --test <file> <test_name>` to
   generate the initial snapshot files.
3. Inspect the generated `.snap` files to verify the rendered output is correct.
4. Commit both the test and its snapshot files.

When **changing rendering logic** that affects many existing snapshots:
1. Delete the affected `.snap` files:
   `find tests/snapshots -name "prefix__*.snap" -delete`
2. Regenerate: `INSTA_UPDATE=always cargo nextest run`
3. Review the diff with `git diff tests/snapshots/` before committing.
