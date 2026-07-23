# Skim Agent Guidelines

## Build/Test/Lint Commands
- Build: `cargo build [--release]`
- Run: `cargo run [--release]`
- Test (all): `cargo nextest run`
- Test (single): `cargo nextest test_name`
- Integration/E2E tests: `cargo nextest run --tests` (drives `sk` through Zellij under the hood; needs `zellij` >= 0.44 and `bash` on `$PATH`)
- Memory leak detection: `cargo nextest run --profile valgrind`
- Thread leak/race detection:
  1. Build: `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly build --tests -Zbuild-std --target x86_64-unknown-linux-gnu`
  2. Run: `TSAN_OPTIONS="detect_deadlocks=1" cargo +nightly nextest run --profile tsan --target x86_64-unknown-linux-gnu`
- Lint: `cargo clippy`
- Format: `cargo +nightly fmt` (check only: `cargo +nightly fmt --check`)
- Fuzz (requires nightly + `cargo install cargo-fuzz`): `cargo +nightly fuzz run <target>` — see `fuzz/README.md` for target list

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

The end-to-end tests drive a real `sk` process through a terminal, using the
Zellij-backed harness in `tests/common/zellij.rs` (`ZellijController` + the
`sk_test!` DSL). It requires `zellij` (>= 0.44) and `bash` on `$PATH`. The
harness is cross-platform (Linux, macOS and Windows). The pure-harness tests in
`interactive.rs` run on all three platforms; `execute.rs`, `popup.rs` and
`listen.rs` stay `#![cfg(unix)]` for reasons unrelated to the multiplexer (they
install POSIX mock binaries / bind a unix socket), so they run on Linux and
macOS. Two harness details make the non-Linux runners work: the pane's shell is
resolved to an absolute `bash` path (the Zellij server's environment may lack
`bash` on `PATH`), and `wait_ready` nudges the client's terminal size until the
server gives the pane a non-zero geometry to render into. The harness drives
Zellij with:
- `zellij attach --create <session>` (spawned on an in-process PTY via
  `portable-pty`) to start a detached session; `SKIM_DEFAULT_OPTIONS` and friends
  are cleared on the spawned process.
- `zellij --session <session> action write <bytes...>` to inject keystrokes.
- `zellij --session <session> action dump-screen [--ansi]` to capture the pane.

When exploring manually you can reproduce the same flow with those commands; the
config the harness writes disables startup tips, pane frames and the kitty
keyboard protocol (so injected legacy escape sequences reach `sk`).

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
    @snap;                      // take a snapshot (cell text only)
    @snap_color;                // snapshot cell styling (fg/bg/modifier) instead
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
| DSL variant — Nth `@snap_color` | `{file}__{test}@color{NNN}.snap` | `ansi__ansi_flag_enabled@color002.snap` |

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
