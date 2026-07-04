# Fuzzing

This directory contains [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
(libFuzzer) targets for skim's hand-written, untrusted-input-facing parsers:
text that flows in from stdin, `--ansi` sequences, `--nth`/`--with-nth` field
specs, the search query syntax, and `--bind` key maps. These are exactly the
places where skim does manual byte/char-index bookkeeping on attacker- or
data-controlled strings, which is the most panic-prone code in the project.

## Targets

| Target            | Exercises                                                                 |
|-------------------|----------------------------------------------------------------------------|
| `ansi_strip`      | `helper::item::strip_ansi` — ANSI escape stripping & byte/char index map  |
| `field_extract`   | `field::{FieldRange, get_string_by_field, parse_matching_fields, parse_transform_fields}` — `--nth`/`--with-nth` |
| `fuzzy_match`     | `fuzzy_matcher::{skim, fzy, clangd}` — the fuzzy matching algorithms       |
| `query_match`     | `Matcher::create_engine_factory` + `DefaultSkimItem` — the full query → engine → match pipeline (exact/regex/AND-OR/fuzzy, with ANSI) |
| `keymap_parse`    | `binds::KeyMap` — the `--bind` key-map parser                              |

Each target asserts more than "doesn't panic" where a cheap invariant is
available (e.g. reported match indices must be valid char indices into the
matched text, index mappings must stay monotonic and land on char
boundaries).

## Running

Install `cargo-fuzz` (requires a nightly toolchain):

```sh
cargo install cargo-fuzz
```

Run a target:

```sh
cargo +nightly fuzz run ansi_strip
```

Run for a bounded time (useful in CI or for a quick check):

```sh
cargo +nightly fuzz run query_match -- -max_total_time=60
```

## Reproducing a crash

`cargo fuzz run` writes failing inputs to `fuzz/artifacts/<target>/`. Replay one with:

```sh
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<hash>
```

## Adding a target

Add a new `fuzz_targets/<name>.rs`, register it in `fuzz/Cargo.toml`'s
`[[bin]]` list, and prefer asserting a real invariant of the function under
test (bounds, monotonicity, round-tripping) rather than only catching panics.
