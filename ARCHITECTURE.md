# Skim Architecture

## Table of Contents

1. [High-Level Overview](#high-level-overview)
2. [Workspace & Crate Layout](#workspace--crate-layout)
3. [Entry Points](#entry-points)
4. [Core Data Flow](#core-data-flow)
5. [Operating Modes](#operating-modes)
   - [Normal Interactive Mode](#normal-interactive-mode)
   - [Filter Mode (`--filter`)](#filter-mode---filter)
   - [Interactive / Command Mode (`--interactive`)](#interactive--command-mode---interactive)
   - [Select-1 / Exit-0 / Sync Modes](#select-1--exit-0--sync-modes)
   - [ANSI Mode (`--ansi`)](#ansi-mode---ansi)
   - [Popup Mode (`--popup` / `--tmux`)](#popup-mode---popup----tmux)
6. [Item Ingestion Pipeline](#item-ingestion-pipeline)
7. [The Matching Subsystem](#the-matching-subsystem)
   - [Match Engine Hierarchy](#match-engine-hierarchy)
   - [Fuzzy Algorithms](#fuzzy-algorithms)
   - [Parallel Matching](#parallel-matching)
   - [Ranking & Sorting](#ranking--sorting)
8. [TUI Subsystem](#tui-subsystem)
   - [Backend & Terminal Setup](#backend--terminal-setup)
   - [Event Loop](#event-loop)
   - [App State](#app-state)
   - [Widget System](#widget-system)
   - [Layout Engine](#layout-engine)
9. [Individual Widgets](#individual-widgets)
   - [Input Widget](#input-widget)
   - [ItemList Widget](#itemlist-widget)
   - [ItemRenderer](#itemrenderer)
   - [Preview Widget](#preview-widget)
   - [Header Widget](#header-widget)
   - [StatusLine / Info](#statusline--info)
10. [Key Bindings & Action System](#key-bindings--action-system)
11. [Preview System](#preview-system)
12. [Output & Result Collection](#output--result-collection)
13. [IPC / Listen Socket](#ipc--listen-socket)
14. [Theming](#theming)
15. [History](#history)
16. [Pre-Selection](#pre-selection)
17. [Threading Model](#threading-model)
18. [Important Call Sites (Cross-Reference)](#important-call-sites-cross-reference)
19. [Public Library API](#public-library-api)

---

## High-Level Overview

Skim (`sk`) is a terminal fuzzy-finder written in Rust, equivalent in spirit to `fzf`. It can operate as a standalone CLI binary or as an embedded library crate. At runtime it orchestrates four concurrent activities:

```
stdin / command
      │
      ▼
 ┌──────────┐   batched items   ┌──────────────┐
 │  Reader  │──────────────────▶│  ItemPool    │
 └──────────┘                   │  (Arc<…>)    │
                                └──────┬───────┘
                                       │ take()
                                       ▼
                                ┌──────────────┐
                                │   Matcher    │◀── query string
                                │  (parallel)  │
                                └──────┬───────┘
                                       │ ProcessedItems
                                       ▼
                   ┌──────────────────────────────────┐
                   │              TUI                  │
                   │  ┌────────┐  ┌────────┐           │
                   │  │ Input  │  │Preview │           │
                   │  ├────────┤  ├────────┤           │
                   │  │ItemList│  │ Header │           │
                   │  └────────┘  └────────┘           │
                   └──────────────────────────────────┘
                                       │
                                       ▼
                                  SkimOutput
```

The **Reader** pulls raw text from stdin or a shell command and converts it into `Arc<dyn SkimItem>` batches, depositing them into the shared `ItemPool`.  
The **Matcher** picks items up from the pool, evaluates every item against the current query string using a configured engine, and writes ranked `MatchedItem` results into `ProcessedItems`.  
The **TUI** renders four composable widgets (Input, ItemList, Preview, Header), drives a `crossterm`-based event loop, and converts user keystrokes into typed `Action` values that are dispatched back to the `App` state machine.

---

## Workspace & Crate Layout

```
skim/                  ← workspace root
├── src/               ← single `skim` crate (lib + binary)
│   ├── bin/
│   │   └── main.rs    ← `sk` binary entry point
│   ├── lib.rs         ← library root; re-exports public types
│   ├── skim.rs        ← Skim<Backend> orchestrator
│   ├── options.rs     ← SkimOptions (all CLI / library options)
│   ├── output.rs      ← SkimOutput (returned to callers)
│   ├── reader.rs      ← Reader + ReaderControl + CommandCollector trait
│   ├── matcher.rs     ← Matcher + MatcherControl (parallel worker dispatcher)
│   ├── item.rs        ← ItemPool, MatchedItem, Rank, RankBuilder
│   ├── skim_item.rs   ← SkimItem trait
│   ├── binds.rs       ← KeyMap, parse_key, parse_action_chain
│   ├── theme.rs       ← ColorTheme, named palettes
│   ├── thread_pool.rs ← ThreadPool + parallel_work_queue
│   ├── field.rs       ← field range parsing (--nth / --with-nth)
│   ├── spinlock.rs    ← lightweight SpinLock<T>
│   ├── util.rs        ← printf helper, misc utilities
│   ├── popup/         ← tmux & zellij popup integration
│   │   ├── mod.rs           ← SkimPopup trait, run_with(), check_env(), SkimPopupOutput
│   │   ├── tmux.rs          ← TmuxPopup (builds/runs tmux display-popup)
│   │   └── zellij.rs        ← ZellijPopup (builds/runs zellij action new-floating-pane)
│   ├── prelude.rs     ← convenience re-exports
│   ├── manpage.rs     ← man-page generation (cli feature)
│   ├── shell.rs       ← shell completion generation (cli feature)
│   ├── engine/        ← match engine implementations
│   │   ├── mod.rs
│   │   ├── factory.rs       ← ExactOrFuzzyEngineFactory, AndOrEngineFactory, RegexEngineFactory
│   │   ├── andor.rs         ← AndEngine, OrEngine
│   │   ├── exact.rs         ← ExactEngine (prefix/postfix/inverse/exact string)
│   │   ├── fuzzy.rs         ← FuzzyEngine + FuzzyAlgorithm enum
│   │   ├── all.rs           ← MatchAllEngine (match-all / empty query)
│   │   ├── normalized.rs    ← NormalizedEngine (Unicode normalization wrapper)
│   │   ├── regexp.rs        ← RegexEngine (regex-mode)
│   │   ├── split.rs         ← SplitMatchEngine (--split-match)
│   │   └── util.rs          ← normalization helpers
│   ├── fuzzy_matcher/  ← raw fuzzy scoring algorithms
│   │   ├── mod.rs           ← FuzzyMatcher trait, MatchIndices type alias
│   │   ├── skim.rs          ← SkimMatcherV2
│   │   ├── clangd.rs        ← ClangdMatcher
│   │   ├── fzy.rs           ← FzyMatcher
│   │   ├── frizbee.rs       ← FrizbeeMatcher (typo-resistant)
│   │   └── arinae/          ← ArinaeMatcher (default; Smith-Waterman based)
│   ├── helper/         ← higher-level item helpers
│   │   ├── mod.rs
│   │   ├── item.rs          ← DefaultSkimItem (ANSI parsing, field transforms)
│   │   ├── item_reader.rs   ← SkimItemReader + SkimItemReaderOption (stdin/cmd → items)
│   │   ├── selector.rs      ← DefaultSkimSelector (pre-selection)
│   │   └── macros.rs        ← helper macros
│   └── tui/            ← terminal UI
│       ├── mod.rs            ← Size, Direction, BorderType, re-exports
│       ├── app.rs            ← App struct + render + event dispatch (central state machine)
│       ├── backend.rs        ← Tui<B> (ratatui terminal wrapper + crossterm event pump)
│       ├── event.rs          ← Event enum, Action enum, ActionCallback, parse_action
│       ├── widget.rs         ← SkimWidget trait + SkimRender result type
│       ├── input.rs          ← Input widget (query box + cursor + status info)
│       ├── item_list.rs      ← ItemList widget (scrollable match result list)
│       ├── item_renderer.rs  ← ItemRenderer (per-item ANSI/highlight rendering)
│       ├── preview.rs        ← Preview widget (PTY or plain text preview pane)
│       ├── header.rs         ← Header widget (--header / --header-lines)
│       ├── statusline.rs     ← InfoDisplay enum (status bar mode)
│       ├── layout.rs         ← LayoutTemplate + AppLayout (pre-computed areas)
│       ├── options.rs        ← TuiLayout enum, PreviewLayout struct
│       └── util.rs           ← cursor helpers, style merging
├── tests/             ← integration & snapshot tests
│   ├── common/
│   │   └── insta.rs   ← snap! / insta_test! macros for TUI snapshot testing
│   ├── snapshots/     ← committed .snap files
│   ├── ansi.rs        ← ANSI rendering tests
│   ├── options.rs     ← option coverage tests
│   ├── preview.rs     ← preview pane tests
│   └── …
├── benches/           ← criterion benchmarks
└── Cargo.toml
```

The single crate exports:
- A **library** (`lib`): all types under `skim::*`, suitable for embedding.
- A **binary** (`sk`, requires feature `cli`): the `clap`-based CLI.

The `cli` feature gates `clap`, `clap_complete`, `shlex`, `env_logger`, and `clap_mangen`.

---

## Entry Points

### Binary (`src/bin/main.rs`)

```
main()
  │
  ├─ SkimOptions::from_env()          ← parses argv via clap (feature=cli)
  ├─ opts.build()                     ← applies defaults, loads history files
  │
  ├─ if opts.shell → generate_completions()   ← early exit
  ├─ if opts.man  → manpage::generate()       ← early exit
  ├─ if opts.remote → IPC relay mode          ← early exit
  │
  ├─ sk_main(opts)
  │     ├─ SkimItemReader::new(reader_opts)   ← configure stdin reader
  │     ├─ opts.cmd_collector = cmd_collector
  │     │
  │     ├─ if --popup/--tmux && check_env() → popup::run_with(&opts)
  │     │
  │     └─ else:
  │           ├─ if stdin not a TTY (piped) → cmd_collector.of_bufread(stdin)
  │           └─ Skim::run_with(opts, rx_item?)
  │
  └─ print output / write history / exit
```

### Library (`src/lib.rs` + `src/skim.rs`)

Two public entry points exist on `Skim`:

| Method | Use case |
|---|---|
| `Skim::run_with(options, source)` | Takes a `SkimItemReceiver` channel (or `None` to use the configured command collector). The canonical entry point. |
| `Skim::run_items(options, items)` | Convenience wrapper: accepts any `IntoIterator<Item: SkimItem>`, batches them through a bounded channel, and calls `run_with`. |

Both return `Result<SkimOutput>`.

---

## Core Data Flow

### Initialisation sequence

```
Skim::run_with(options, source)
  │
  ├─ Skim::init(options, source)
  │     ├─ parse height (Size enum)
  │     ├─ ColorTheme::init_from_options(&options)
  │     ├─ Reader::from_options(&options).source(source)
  │     ├─ resolve cmd / expand initial_cmd (interactive mode)
  │     └─ App::from_options(options, theme, cmd)
  │           ├─ Input::from_options(…)
  │           ├─ Preview::from_options(…)
  │           ├─ Header::from_options(…)
  │           ├─ ItemList::from_options(…)
  │           ├─ ItemPool::from_options(…)
  │           ├─ Matcher::from_options(…)
  │           └─ LayoutTemplate::from_options(…)
  │
  ├─ Skim::start()
  │     ├─ reader.collect(item_pool, initial_cmd)  ← spawns reader thread(s)
  │     └─ app.restart_matcher(force=true)          ← kicks off first match pass
  │
  ├─ Skim::should_enter() → decides whether to open TUI
  │     (handles filter / select-1 / exit-0 / sync blocking)
  │
  ├─ if should_enter:
  │     ├─ Skim::init_tui()  → Tui::new_with_height(height)
  │     ├─ Skim::enter()     → tui.enter() [raw mode + mouse + event task]
  │     └─ Skim::run()       → async event loop (tick())
  │
  └─ Skim::output()         ← collect results + kill reader
```

### Steady-state loop (`Skim::tick()`)

Each call to `tick()` runs a `tokio::select!` on four concurrent futures:

| Branch | Source | Action |
|---|---|---|
| `tui.next()` | crossterm keyboard/mouse/resize/paste events | Dispatch to `app.handle_event()` |
| `matcher_interval.tick()` | 10 ms periodic timer | `app.restart_matcher(false)` |
| `items_available.notified()` | `Notify` set by `ItemPool::append` | `app.restart_matcher(false)` |
| `listener.accept()` | IPC socket (when `--listen`) | Parse RON-encoded `Action`, push to event queue |

---

## Operating Modes

### Normal Interactive Mode

The default mode. The TUI is shown in full. Items arrive from stdin or a command, are matched against the live query, and displayed in the list. The user navigates with keyboard/mouse and presses Enter to accept.

**Key files:** `src/skim.rs`, `src/tui/app.rs`, `src/tui/backend.rs`

### Filter Mode (`--filter`)

When `--filter <query>` is set, skim never opens the TUI.

`Skim::should_enter()` enters a busy-wait loop:
```
loop {
    if matcher.stopped() && reader.is_done() && pool.num_not_taken() == 0 {
        break;
    }
    sleep(1ms);
    app.restart_matcher(false);
}
```
Then `app.item_list.items` is populated from `processed_items` and `output()` is called immediately. The matched items are printed to stdout by the binary, one per line (or null-delimited with `--print0`).

In filter mode the `FuzzyEngine` is built with `filter_mode = true`, which uses `fuzzy_match_range` instead of `fuzzy_indices` to skip the per-character index computation and run faster.

**Key files:** `src/skim.rs` (`should_enter()`), `src/engine/fuzzy.rs` (`filter_mode` fast path), `src/bin/main.rs` (output loop)

### Interactive / Command Mode (`--interactive`)

When `--interactive` is set together with `--cmd <template>`, the query box controls a shell command rather than a fuzzy filter. Every change to the input re-expands the template and issues a `Reload` event.

Template placeholders:
- `{}` — the current query
- `{q}` — alias for `{}`
- `{n}` — ordinal of the current item

`App::expand_cmd()` handles placeholder expansion. On `Action::ToggleInteractive`, the mode flips between the query controlling the fuzzy filter and the query driving the command.

In interactive mode, the initial command is expanded against the initial query:
```rust
// src/skim.rs Skim::init()
let initial_cmd = if app.options.interactive && app.options.cmd.is_some() {
    app.expand_cmd(&cmd, true)   // true = initial call
} else { cmd.clone() };
```

A `Reload(new_cmd)` event is handled at the `Skim::tick()` level (not `App::handle_event()`), because it must kill the reader and restart cleanly:
```rust
// src/skim.rs tick()
if let Event::Reload(new_cmd) = &evt {
    self.handle_reload(&new_cmd.clone());
}
```

`handle_reload()`:
1. Kills `reader_control` (waits for all reader threads to stop)
2. Clears `ItemPool`
3. Clears `ItemList` (unless `no_clear_if_empty`)
4. Calls `app.restart_matcher(force=true)`
5. Starts a new `reader.collect(…)`

**Key files:** `src/skim.rs` (`handle_reload`, `tick`), `src/tui/app.rs` (`expand_cmd`, `handle_action` → `RefreshCmd`)

### Select-1 / Exit-0 / Sync Modes

All three are handled in `Skim::should_enter()` before opening the TUI:

| Option | Meaning | Behaviour |
|---|---|---|
| `--select-1` | Auto-accept if exactly one match | Waits until ≥ 2 matches or reader/matcher done; returns without TUI if exactly 1 match |
| `--exit-0` | Exit immediately if no matches | Waits until ≥ 1 match or done; returns without TUI if 0 matches |
| `--sync` | Block until all items processed | Waits until `num_matched == usize::MAX` (effectively waits for full scan) |

### ANSI Mode (`--ansi`)

When `--ansi` is set, `SkimItemReaderOption::from_options` sets `use_ansi_color = true`. Each input line then creates a `DefaultSkimItem` with:

1. **Escape detection**: `DefaultSkimItem::contains_ansi_escape()` checks for `\x1b`.
2. **Stripping**: `strip_ansi()` returns `(stripped_text, ansi_info)` where `ansi_info` is a `Vec<(byte_pos, char_pos)>` mapping from stripped to original coordinates.
3. **Matching**: `text()` returns the stripped text; the matcher works on plain text.
4. **Display**: `display()` reconstructs styled `ratatui::text::Line` using `ansi_to_tui::IntoText`, then overlays match-highlight spans on top.

The coordinate mapping is critical: match indices come back in terms of stripped text positions, but the highlight must be applied to the original ANSI-containing string. `DefaultSkimItem::display()` applies a char-index offset conversion using `ansi_info`.

Without `--ansi`, any ANSI escape codes are passed through to `text()` and displayed as literal characters. If the raw input happens to contain escape sequences (but `--ansi` is not set), `escape_ansi()` is called to make them visible.

**Key files:** `src/helper/item.rs` (`DefaultSkimItem::new`, `strip_ansi`, `display`)

### Popup Mode (`--popup` / `--tmux`)

When `--popup [direction[,size[,size]]]` (alias `--tmux`) is set, the binary calls `check_and_run_popup()`, which checks `popup::check_env()` and, if true, delegates to `popup::run_with()` instead of `Skim::run_with()`.

**`check_env()`** returns `true` only when:
- `$_SKIM_POPUP` is **not** set in the environment (prevents the child process from recursing back into popup mode), and
- at least one supported multiplexer is detected: tmux (`$TMUX` set) or Zellij (`$ZELLIJ` set).

The popup flow:
1. Creates a temp directory for IPC (`/tmp/sk-popup-XXXXXXXX/`).
2. If stdin is piped, creates a named FIFO (`tmp_stdin`) and spawns a thread to relay stdin into it incrementally so the child can stream-read.
3. Reconstructs the `sk` command line from `std::env::args()`, stripping `--popup`/`--tmux` and `--output-format`, then appending `--print-query --print-cmd --print-header --print-current --print-score`.
4. Forwards all `SKIM_*`, `RUST*`, and `PATH` environment variables to the child via the multiplexer's `-e` flag, **plus `_SKIM_POPUP=1`** to prevent re-entry.
5. Launches the popup via the appropriate backend:
   - **tmux**: `tmux display-popup -E … sh -c <cmd> > stdout_file`
   - **Zellij**: `zellij action new-floating-pane … -- sh -c <cmd> > stdout_file`
6. Waits for the popup process to exit.
7. Parses the structured stdout file (`query\ncmd\nheader\ncurrent_item\nitem1\nscore1\n…`) into a synthetic `SkimOutput`.

The internal `SkimPopup` trait abstracts the two multiplexer backends:

```rust
trait SkimPopup {
    fn from_options(options: &SkimOptions) -> Box<dyn SkimPopup>;
    fn add_env(&mut self, key: &str, value: &str);
    fn run_and_wait(&mut self, command: &str) -> std::io::Result<ExitStatus>;
}
```

`TmuxPopup` and `ZellijPopup` each implement this trait. The active backend is selected at runtime: Zellij takes priority if both are available.

The child `sk` process runs fully independently inside the popup. The parent reads back a synthetic `SkimOutput` from the captured file. Because `_SKIM_POPUP=1` is set in the child's environment, `check_env()` returns `false` in the child, so it runs as a normal interactive skim session regardless of what `SKIM_DEFAULT_OPTIONS` contains.

**Key files:** `src/popup/mod.rs` (`run_with`, `check_env`), `src/popup/tmux.rs` (`TmuxPopup`), `src/popup/zellij.rs` (`ZellijPopup`), `src/bin/main.rs` (`check_and_run_popup`)

---

## Item Ingestion Pipeline

```
Source (stdin bytes or child process stdout)
  │
  │  SkimItemReader::of_bufread() or CommandCollector::invoke()
  │
  ├── Simple path (no ANSI, no --nth, no --with-nth):
  │     raw_bufread()
  │       ├─ Thread 1: I/O reader — reads 256 KB chunks, splits at line boundaries,
  │       │             assigns monotonic sequence numbers, sends to MPMC channel
  │       ├─ Thread N: workers — receive chunks, validate UTF-8, create DefaultSkimItem
  │       │             (no metadata allocation, just Box<str>), send (seq, items) pairs
  │       └─ Thread 1: reorder — collects (seq, items), emits in order through SkimItemReceiver
  │
  └── Complex path (ANSI | --nth | --with-nth):
        read_lines_into_items()  (single-threaded)
          ├─ reads line by line via read_until(line_ending)
          ├─ creates DefaultSkimItem::new(line, ansi, trans_fields, matching_fields, delimiter)
          └─ buffers up to 1024 items before sending to SkimItemReceiver

SkimItemReceiver channel
  │
  │  Reader::collect()
  │  collect_items() spawns a thread that polls the channel every 1ms
  │
  └── ItemPool::append(items)
        ├─ respects --tac (reverse order)
        ├─ respects --header-lines (reserves first N items)
        ├─ notifies items_available (Notify) to wake matcher
        └─ increments atomic length counter
```

### `DefaultSkimItem` construction matrix

| `with_nth` | `ansi` | `text` field | `orig_text` | `stripped_text` |
|---|---|---|---|---|
| false | false | original line | None | None |
| false | true  | original line | None | stripped (+ `ansi_info`) |
| true  | false | transformed   | original | None |
| true  | true  | transformed   | original | stripped (+ `ansi_info`) |

Fields `/0` bytes are stripped from `text` (used for display/matching) but preserved in `orig_text` (used for output).

---

## The Matching Subsystem

### Match Engine Hierarchy

Engines are composable through the factory pattern. Starting from `Matcher::create_engine_factory_with_builder()`:

```
options
  │
  ├── if regex mode:
  │     RegexEngineFactory
  │       └─ if normalize: NormalizedEngineFactory(RegexEngineFactory)
  │
  └── else (fuzzy/exact mode):
        ExactOrFuzzyEngineFactory
          └─ if split_match: SplitMatchEngineFactory(ExactOrFuzzyEngineFactory)
               └─ AndOrEngineFactory(SplitMatchEngineFactory | ExactOrFuzzyEngineFactory)
                    └─ if normalize: NormalizedEngineFactory(AndOrEngineFactory)
```

When `create_engine_with_case(query, case)` is called at match time, the factory chain parses the query string and builds a concrete engine tree:

```
query: "'abc def | ghi ^xyz"
  │
  AndOrEngineFactory.parse_andor()
    │
    ├─ "'abc"  → ExactOrFuzzyEngineFactory
    │              → ExactEngine (prefix=false, postfix=false, case-insensitive "abc")
    │
    ├─ "def | ghi" → OrEngine
    │                  ├─ FuzzyEngine("def")
    │                  └─ FuzzyEngine("ghi")
    │
    └─ "^xyz"  → ExactEngine(prefix=true, "xyz")
```

Query prefix semantics handled by `ExactOrFuzzyEngineFactory::create_engine_with_case()`:

| Prefix/suffix | Engine type |
|---|---|
| `'abc` | force ExactEngine (toggle from default) |
| `!abc` | ExactEngine with `inverse = true` |
| `^abc` | ExactEngine with `prefix = true` |
| `abc$` | ExactEngine with `postfix = true` |
| `!^abc` | ExactEngine inverse+prefix |
| `!^abc$` | ExactEngine inverse+prefix+postfix (exact string, inverted) |
| plain `abc` | FuzzyEngine (or ExactEngine if `--exact`) |
| empty / `!` | MatchAllEngine |

### Fuzzy Algorithms

All algorithms implement the `FuzzyMatcher` trait with two methods:
- `fuzzy_indices(choice, pattern) → Option<(score, Vec<usize>)>` — full match with per-character highlights
- `fuzzy_match_range(choice, pattern) → Option<(score, begin, end)>` — fast path without highlight indices (used in filter mode)

| Algorithm | Flag | Notes |
|---|---|---|
| `Arinae` | `--algorithm arinae` (default) | Smith-Waterman with affine gaps; typo-resistant; picks last occurrence on ties when `--last-match` |
| `SkimV2` | `--algorithm skim_v2` | Skim's classic dynamic-programming scorer |
| `Clangd` | `--algorithm clangd` | Clangd-style subsequence scoring |
| `Fzy` | `--algorithm fzy` | Port of the `fzy` C algorithm; supports `--typos` |
| `Frizbee` | `--algorithm frizbee` | Edit-distance based; explicitly typo-tolerant (x86_64 and aarch64 only) |

Typo tolerance is configured via `Typos`:
- `Typos::Disabled` — no tolerance (default)
- `Typos::Smart` — adaptive: `query.len() / 4` typos allowed
- `Typos::Fixed(n)` — exactly n typos

### Parallel Matching

`Matcher::run()` dispatches work across the thread pool using `thread_pool::parallel_work_queue()`:

```
Matcher::run(query, item_pool, thread_pool, …)
  │
  ├─ create matcher_engine from factory (synchronous)
  ├─ take items from pool synchronously (avoids race with restart)
  │
  └─ thread_pool.spawn(coordinator closure)
        │
        ├─ shares items as Arc<[Arc<dyn SkimItem>]>
        │
        ├─ parallel_work_queue(pool, num_workers, items, CHUNK_SIZE=512, …)
        │     │
        │     ├─ Worker threads (num_cpus - 1):
        │     │    ├─ atomically grab next chunk
        │     │    ├─ for each item: matcher_engine.match_item(item)
        │     │    ├─ flush processed/matched counters (Relaxed atomic)
        │     │    └─ accumulate into worker-local Vec<MatchedItem>
        │     │         └─ sort_unstable() on worker thread (parallel sort)
        │     │
        │     └─ coordinator:
        │          └─ merge_worker_results(worker_results, no_sort, …)
        │               ├─ concatenate k sorted runs
        │               ├─ sort() (stable; driftsort detects k runs → O(n log k))
        │               └─ write into SpinLock<Option<ProcessedItems>>
        │
        └─ stopped.store(true)
```

Interruption is cooperative: each chunk checks `interrupt.load(Relaxed)` before processing. `MatcherControl::kill()` sets `interrupt = true`; `MatcherControl::drop()` also calls `kill()`.

### Ranking & Sorting

`MatchedItem` implements `Ord` through a lazy sort key computed by `Rank::sort_key(criteria)`.

`Rank` fields:
| Field | Description |
|---|---|
| `score` | Raw match score (higher = better) |
| `begin` | First matched character index |
| `end` | Last matched character index |
| `length` | Total item text length in bytes |
| `index` | Ordinal position in the input stream |
| `path_name_offset` | Byte offset after last `/` or `\` (for path-name tiebreak) |

`RankCriteria` variants (configurable via `--tiebreak`): `Score`, `NegScore`, `Begin`, `NegBegin`, `End`, `NegEnd`, `Length`, `NegLength`, `Index`, `NegIndex`, `PathName`, `NegPathName`.

`MergeStrategy` (in `item_list.rs`):

| Strategy | When used |
|---|---|
| `Replace` | Fresh match pass (query changed, full re-sort) |
| `SortedMerge` | New items arrived during a running match (merge-insert) |
| `Append` | `--no-sort` mode |

---

## TUI Subsystem

### Backend & Terminal Setup

`Tui<B>` (in `src/tui/backend.rs`) wraps `ratatui::Terminal<B>` and owns:
- A `tokio::sync::mpsc` channel (`event_tx` / `event_rx`) of capacity 1 M for events.
- A `JoinHandle` for a background Tokio task that reads `crossterm::event::EventStream` and sends `Event` values.
- A `CancellationToken` to stop the background task.
- A `is_fullscreen` flag that determines the `ratatui::Viewport`.

**Viewport selection** (`Tui::new_with_height_and_backend()`):
- `Size::Percent(100)` → `Viewport::Fullscreen` (enters alternate screen).
- Any other size → `Viewport::Fixed(Rect)` at the current cursor position; scrolls the terminal if needed to make room.

The default backend is `CrosstermBackend<BufWriter<Stderr>>`. Skim always draws to **stderr** so stdout remains clean for piped output.

**Terminal lifecycle:**
```
Tui::enter()
  ├─ enable_raw_mode()
  ├─ execute!(EnableMouseCapture, EnableBracketedPaste)
  ├─ if fullscreen: execute!(EnterAlternateScreen, cursor::Hide)
  └─ Tui::start()  ← spawns event pump task

Tui::exit()
  ├─ Tui::stop() / cancel()
  ├─ cleanup_terminal()
  │    ├─ execute!(DisableMouseCapture, DisableBracketedPaste, LeaveAlternateScreen, Show)
  │    └─ disable_raw_mode()
  └─ if inline: clear() + reset cursor to top of drawing area
```

A panic hook is installed once (`PANIC_HOOK_SET: Once`) to ensure `cleanup_terminal()` runs even on panics.

### Event Loop

The crossterm event pump (background Tokio task in `Tui::start()`):

```
loop {
    select! {
        cancelled → break
        crossterm_event →
            Key(press)  → send Event::Key(key)
            Paste(text) → send Event::Paste(text)
            Mouse(m)    → send Event::Mouse(m)
            Resize(c,r) → send Event::Resize + Event::Render
            Error(e)    → send Event::Error(e)
        tick (1/12 s)  → send Event::Heartbeat
    }
}
```

The main loop (`Skim::run()`) calls `tick()` in a loop, which `select!`s on the same channel plus the matcher interval and IPC listener.

Frame rate is capped at 30 fps (`FRAME_TIME_MS = 1000/30`). `App::handle_event(Heartbeat)` checks `needs_render` (an `AtomicBool` set by the matcher when new results arrive) and emits `Event::Render` only when the last render was more than `FRAME_TIME_MS` ago.

### App State

`App` (in `src/tui/app.rs`) is the single mutable application state. It contains:

| Field | Type | Role |
|---|---|---|
| `item_pool` | `Arc<ItemPool>` | Shared with reader; accumulates raw items |
| `matcher` | `Matcher` | Engine factory + case + rank config |
| `matcher_control` | `MatcherControl` | Handle to stop/query current match pass |
| `item_list` | `ItemList` | Display list + selection state |
| `input` | `Input` | Query text + cursor |
| `preview` | `Preview` | Preview pane state + process handle |
| `header` | `Header` | Static + dynamic header lines |
| `thread_pool` | `Arc<ThreadPool>` | Worker threads for matching |
| `options` | `SkimOptions` | Full configuration snapshot |
| `layout_template` | `LayoutTemplate` | Pre-computed widget constraints |
| `layout` | `AppLayout` | Last-frame widget areas (updated in render) |
| `needs_render` | `Arc<AtomicBool>` | Signal from matcher → event loop |
| `yank_register` | `String` | Cut/yank buffer |
| `query_history` / `cmd_history` | `Vec<String>` | History for ↑/↓ navigation |

**`App::handle_event()`** dispatches on `Event`:

```
Event::Render     → tui.draw(|f| f.render_widget(&mut self, f.area()))
Event::Heartbeat  → update_spinner(); check pending_matcher_restart; throttled render
Event::RunPreview → run_preview(tui)
Event::Key(k)     → handle_key(k) → [Action…] → tui.event_tx.send(Event::Action)
Event::Action(a)  → handle_action(a) → [Event…] → tui.event_tx.send(…)
Event::Paste(t)   → input.insert_str(cleaned); on_query_changed()
Event::Resize(…)  → app.resize(); run_preview()
Event::Mouse(…)   → handle_mouse()
Event::PreviewReady → apply preview offset; needs_render()
Event::AppendItems  → item_pool.append(); restart_matcher(false)
Event::ClearItems   → item_pool.clear(); restart_matcher(true)
Event::Quit/Close   → tui.exit(); should_quit = true
Event::Reload(_)    → (handled by Skim::tick, not here)
```

**`App::restart_matcher(force)`:**
```
restart_matcher(force)
  ├─ if !force && matcher not stopped → skip (debounce)
  ├─ if pool has no un-taken items && !force → skip
  ├─ kill existing matcher_control
  ├─ determine MergeStrategy
  │    ├─ Replace  → if query changed / force
  │    └─ SortedMerge / Append otherwise
  └─ matcher.run(query, pool, thread_pool, processed_items, strategy, no_sort, needs_render)
       → returns new MatcherControl
```

A 200 ms debounce (`restart_matcher_debounced`) is applied to query-change events to avoid thrashing the matcher on rapid typing.

### Widget System

All TUI widgets implement `SkimWidget`:

```rust
pub trait SkimWidget: Sized {
    fn from_options(options: &SkimOptions, theme: Arc<ColorTheme>) -> Self;
    fn render(&mut self, area: Rect, buf: &mut Buffer) -> SkimRender;
}

pub struct SkimRender {
    pub items_updated: bool,
    pub run_preview: bool,   // signals that preview should be (re)spawned
}
```

`App` itself implements `ratatui::widgets::Widget` via `impl Widget for &mut App`, which calls each sub-widget's `render()` and ORs their `SkimRender` results. After render, `App` sets `cursor_pos` to the absolute screen coordinates of the input cursor.

### Layout Engine

`LayoutTemplate` pre-computes area splits from `SkimOptions` once and stores constraint trees. `apply(area: Rect) → AppLayout` is then a cheap, allocation-free split.

`AppLayout` has four optional areas:
```rust
pub struct AppLayout {
    pub list_area: Rect,
    pub input_area: Rect,
    pub header_area: Option<Rect>,
    pub preview_area: Option<Rect>,
}
```

Layout is rebuilt on `Event::Resize`, when the header height changes (multiline header items arriving), and on `TogglePreview`.

**Layout orientations** (`TuiLayout`):

| Mode | Description |
|---|---|
| `Default` | Input at bottom, list above, header above list (bottom-to-top reading) |
| `Reverse` | Input at top, list below (top-to-bottom reading) |
| `ReverseList` | List at top, input at bottom |

**Preview placement** is parsed from `--preview-window`:
- Direction: `left` / `right` / `up` / `down`
- Size: `50%` (default) or fixed cells
- Modifiers: `hidden`, `wrap`, `pty`, `+offset`

---

## Individual Widgets

### Input Widget

`Input` (`src/tui/input.rs`) maintains:
- `value: String` — the query text (primary mode)
- `alternate_value: String` — the command text (interactive mode)
- `cursor_pos: usize` — character-level cursor position

Text operations (used by `handle_action`):
- `insert(char)` / `insert_str(&str)` — insert at cursor
- `delete(n)` — delete n characters forward
- `delete_backward_word()` / `delete_to_beginning()` / `delete_forward_word()`
- `move_cursor(delta)` / `move_cursor_to(pos)` / `move_to_end()`
- `move_cursor_forward_word()` / `move_cursor_backward_word()`
- `switch_mode()` — swaps primary and alternate buffers (interactive mode toggle)

The `StatusInfo` struct rendered inside the input line shows:
`{spinner} {matched}/{total} ({processed}) [{matcher_mode}] [{multi_count}]`

### ItemList Widget

`ItemList` (`src/tui/item_list.rs`) maintains:
- `items: Vec<MatchedItem>` — the currently displayed matched items
- `processed_items: Arc<SpinLock<Option<ProcessedItems>>>` — shared with matcher
- `selection: Vec<usize>` — indices of multi-selected items
- `current: usize` — focused item index (0 = bottom in default layout)
- `offset: usize` — scroll offset (number of items scrolled)
- `manual_hscroll: i16` — user-driven horizontal scroll

On each render, `ItemList::render()` checks `processed_items` and swaps them in atomically via the `SpinLock`. Depending on `MergeStrategy`:
- `Replace`: replaces `items` entirely.
- `SortedMerge`: performs an O(n+m) merge preserving order.
- `Append`: extends `items`.

**Selection state management:**
- `toggle_at(idx)` / `toggle()` / `toggle_all()` / `select_all()` / `clear_selection()`
- `scroll_by_rows(n)` — scroll by terminal rows (accounting for multiline items)
- `scroll_by(n)` — scroll by item count
- `jump_to_first()` / `jump_to_last()`

Pre-selection is applied when items first appear: `DefaultSkimSelector::should_select(index, item)` is checked for each item and matching items are added to `selection`.

### ItemRenderer

`ItemRenderer` (`src/tui/item_renderer.rs`) is an ephemeral struct created per render frame. It handles all per-item display concerns:

1. **Selector icon rendering** — `>` (single-select cursor) or configurable icon.
2. **Multi-select icon** — space / `>` or configurable icon per selection state.
3. **Match highlight** — calls `item.display(DisplayContext)` which converts `MatchRange` into styled `Line<'_>` spans.
4. **Horizontal scroll** — `calc_hscroll()` finds the first matched character and auto-scrolls to show it; `apply_hscroll()` clips spans accordingly.
5. **Tab expansion** — `expand_tabs()` replaces `\t` with spaces at configurable width.
6. **Ellipsis truncation** — replaces overflowing content with `…` (or custom `--ellipsis`).
7. **Multiline items** — when `--multiline <sep>` is set, splits item text on the separator and renders sub-lines.
8. **Score / index display** — when feature flags `ShowScore` / `ShowIndex` are set.

### Preview Widget

`Preview` (`src/tui/preview.rs`) renders a side/top/bottom pane showing expanded information about the focused item. It supports two modes:

**Plain text mode** (no `pty`): spawns `sh -c <cmd>` as a child process, captures stdout (capped at `PREVIEW_MAX_BYTES`), parses it with `ansi_to_tui::IntoText`, stores as `PreviewContent::Text`. The child writes to `content: Arc<RwLock<PreviewContent>>` and sends `Event::PreviewReady`.

**PTY mode** (`--preview-window pty`): creates a real pseudo-terminal pair via `portable_pty`. The child process sees a properly sized terminal (via `ROWS`/`COLUMNS` env and PTY dimensions). Output is parsed by a `vt100::Parser` with a scrollback buffer, stored as `PreviewContent::Terminal(Arc<RwLock<vt100::Parser>>)`. This enables interactive preview programs (e.g. `bat`, `delta`).

`Preview::spawn()`:
```
kill() ← kill any running preview
reset scroll_y / scroll_x

if pty mode:
  init_pty()  ← create PtyPair
  resize PTY to current (rows, cols)
  spawn sh -c <cmd> in slave
  thread: read master → filter_and_respond_to_queries → vt100::Parser::process
          → Event::PreviewReady when EOF

else:
  sh -c <cmd>
  thread: wait for output → content.write() = PreviewContent::Text(…)
          → Event::PreviewReady
```

Scroll state: `scroll_y`, `scroll_x` (in lines/columns). `page_up/down`, `scroll_up/down/left/right` modify these. When `PreviewReady` fires, an optional offset expression (from `--preview-window +expr`) is evaluated to auto-scroll to the matched line.

### Header Widget

`Header` (`src/tui/header.rs`) renders two kinds of content:
- **Static** (`--header <text>`): shown at the top or bottom depending on layout; expanded for tab characters once at init.
- **Dynamic** (`--header-lines N`): first N items from `ItemPool::reserved()` are treated as header lines instead of selectable items.

When `--multiline <sep>` is active, each header-line item may span multiple terminal rows. The `height()` method returns the total rows needed; if this changes between frames (multiline items arrive), `App::render()` detects it and rebuilds `LayoutTemplate`.

### StatusLine / Info

`StatusInfo` is computed inside `Input::render()` from the current `App` state:

```
Left side:  "> " + spinner + " " + matched + "/" + total [+ "(N%)"]
Inline sep: " < " (when inline_info)
Right side: multi-select count when multi mode
```

`InfoDisplay` has three modes:
- `Default` — separate line above the prompt
- `Inline` — inside the prompt line (after the query text)
- `Hidden` — not shown

---

## Key Bindings & Action System

### Key Parsing

`parse_key(key_str)` (in `src/binds.rs`) converts strings like `"ctrl-a"`, `"alt-shift-f"`, `"f10"`, `"enter"` into `crossterm::event::KeyEvent { code, modifiers }`.

`parse_action(raw)` (in `src/tui/event.rs`) converts strings like `"down:2"`, `"execute(ls {})"`, `"if-query-empty:reload+up"` into `Action` variants.

`parse_action_chain(chain)` splits on `+` (respecting `if-*{…+…}` syntax) into `Vec<Action>`.

`parse_keymap("key:action+action")` → `(&str, Vec<Action>)`.

### Default Key Map (`get_default_key_map`)

Notable defaults:

| Key | Action |
|---|---|
| `Enter` | `Accept(None)` |
| `Esc` | `Abort` |
| `Ctrl-C` / `Ctrl-D` / `Ctrl-G` | `Abort` |
| `↑` / `Ctrl-K` / `Ctrl-P` | `Up(1)` |
| `↓` / `Ctrl-J` / `Ctrl-N` | `Down(1)` |
| `Tab` | `Toggle` + `Down(1)` |
| `Shift-Tab` / `BackTab` | `Toggle` + `Up(1)` |
| `Ctrl-A` | `BeginningOfLine` |
| `Ctrl-E` | `EndOfLine` |
| `Ctrl-U` | `UnixLineDiscard` |
| `Ctrl-W` | `UnixWordRubout` |
| `Ctrl-Y` | `Yank` |
| `Ctrl-Q` | `ToggleInteractive` |
| `Ctrl-R` | `RotateMode` |
| `Shift-↑` / `Shift-↓` | `PreviewUp(1)` / `PreviewDown(1)` |
| `Alt-H` / `Alt-L` | `ScrollLeft(1)` / `ScrollRight(1)` |

User bindings from `--bind key:action[+action]` are parsed at startup and merged via `KeyMap::add_keymaps()`.

### Action Dispatch

```
Event::Key(k) → handle_key(k)
  ├─ lookup k in options.keymap → [Action…]
  ├─ fallback: Ctrl-C → Quit
  ├─ fallback: printable char → AddChar(c) or AddChar(uppercase)
  └─ emit Vec<Event::Action(a)> to event queue

Event::Action(a) → handle_action(a) → Vec<Event>
```

`handle_action` is a large match statement covering all ~70+ `Action` variants. Key action categories:

| Category | Actions |
|---|---|
| Navigation | `Up/Down(n)`, `HalfPageUp/Down`, `PageUp/Down`, `First/Last/Top` |
| Text editing | `AddChar`, `BackwardChar/DeleteChar/Word`, `ForwardChar/Word`, `KillLine`, `Yank`, `UnixLineDiscard/WordRubout` |
| Selection | `Toggle`, `ToggleAll`, `ToggleIn/Out`, `Select`, `SelectAll`, `DeselectAll`, `AppendAndSelect` |
| Query | `SetQuery`, `NextHistory`, `PreviousHistory` |
| Preview | `TogglePreview`, `PreviewUp/Down/Left/Right`, `PreviewPageUp/Down`, `RefreshPreview`, `SetPreviewCmd` |
| Command | `Execute(cmd)`, `ExecuteSilent(cmd)`, `Reload(cmd?)`, `RefreshCmd` |
| Mode | `ToggleInteractive`, `ToggleSort`, `RotateMode` |
| Conditional | `IfQueryEmpty(then, else?)`, `IfQueryNotEmpty(then, else?)`, `IfNonMatched(then, else?)` |
| Lifecycle | `Accept(key?)`, `Abort`, `Cancel` |
| UI | `ClearScreen`, `Redraw`, `SetHeader(text?)`, `SelectRow(n)` |
| Custom | `Custom(ActionCallback)` — async or sync closure receiving `&mut App` |

`Action::Custom(ActionCallback)` is the library extension point: callers can inject arbitrary async logic into the action pipeline without forking skim.

---

## Preview System

The preview command string supports placeholder substitution via `App::expand_cmd()`:

| Placeholder | Expands to |
|---|---|
| `{}` | text of the focused item |
| `{q}` | current query string |
| `{n}` | index of the focused item |
| `{+}` | space-separated texts of all selected items |
| `{+n}` | space-separated indices of selected items |

Preview execution is debounced (`DEBOUNCE_MS` in `run_preview`). A change in the focused item or query triggers `pending_preview_run = true`; the next `Heartbeat` after the debounce window calls `Preview::spawn()`.

The `ItemPreview` enum (returned by `SkimItem::preview()`) gives library users full control:

```rust
pub enum ItemPreview {
    Command(String),          // run command, capture stdout
    Text(String),             // display plain text
    AnsiText(String),         // display ANSI-colored text
    CommandWithPos(String, PreviewPosition),
    TextWithPos(String, PreviewPosition),
    AnsiWithPos(String, PreviewPosition),
    Global,                   // fall back to --preview option
}
```

---

## Output & Result Collection

`Skim::output()` is called after the event loop exits:

```
Skim::output()
  ├─ reader_control.kill()        ← stop reader threads
  ├─ is_abort = !matches!(final_event, Action::Accept)
  ├─ selected_items = app.results()
  │     └─ item_list.items[selection indices] or [current] if no multi
  ├─ query = app.input.to_string()
  ├─ cmd = (interactive? input : options.cmd_query? : initial_cmd)
  ├─ current = item_list.selected()   ← focused item
  └─ header = app.header.header
```

`SkimOutput` fields returned to caller:
```rust
pub struct SkimOutput {
    pub final_event: Event,         // Action::Accept or Action::Abort
    pub is_abort: bool,
    pub final_key: crossterm::event::KeyEvent,
    pub query: String,
    pub cmd: String,
    pub selected_items: Vec<MatchedItem>,
    pub current: Option<MatchedItem>,
    pub header: String,
}
```

In the CLI binary, the output phase (`sk_main` after `Skim::run_with`):
1. Prints `query` if `--print-query`
2. Prints `cmd` if `--print-cmd`
3. Prints `header` if `--print-header`
4. Prints current item text if `--print-current`
5. Prints `accept_key` if `--expect` matched
6. For each selected item: strips ANSI if `--ansi && !--no-strip-ansi`, prints text + score if `--print-score`
7. If `--output-format <template>`: uses `printf()` to expand a format string with placeholders

Exit codes: `0` = items selected, `1` = no items selected, `130` = abort, `135` = tmux launch failed.

---

## IPC / Listen Socket

When `--listen <socket_name>` is set, `Skim::init_listener()` creates an `interprocess` local socket. The main event loop's `select!` accepts connections and spawns Tokio tasks to read RON-encoded `Action` values line by line:

```
listener.accept() → stream
  tokio::spawn:
    BufReader(stream).lines()
    for each line:
      ron::from_str::<Action>(&line)
      → tui.event_tx.send(Event::Action(act))
```

The remote client mode (`--remote <socket_name>`) reads action strings from stdin and sends them to an existing skim instance:

```
// src/bin/main.rs main()
if let Some(remote) = opts.remote {
    stream = LocalSocket::connect(socket_name)
    loop: read_line → parse_action_chain → ron::to_string → stream.write_all
}
```

This enables scripted control of a running skim session.

---

## Theming

`ColorTheme` (`src/theme.rs`) holds 12 named `ratatui::style::Style` values:

| Field | Covers |
|---|---|
| `normal` | Default item text |
| `matched` | Highlighted match characters |
| `current` | Focused item background |
| `current_match` | Match highlights on focused item |
| `query` | Query text in input box |
| `spinner` | Spinner animation character |
| `info` | Status info line |
| `prompt` | Prompt character |
| `cursor` | Cursor indicator |
| `selected` | Multi-selected item marker |
| `header` | Header text |
| `border` | Border lines |

Built-in palettes: `none`, `bw`, `default16`, `dark256`, `molokai256`, `light256`, `catppuccin_mocha`, `catppuccin_macchiato`, `catppuccin_latte`, `catppuccin_frappe`.

Selected via `--color base_theme[,component:color[:modifier]]`. Individual component overrides use CSS-style RGB hex (`#RRGGBB`), ANSI 256-color indices, or named modifiers (`bold`, `italic`, `underline`, `dim`, `reverse`).

---

## History

Query and command histories are managed in `SkimOptions`:
- Loaded at startup via `SkimOptions::init_histories()` from files specified by `--history-file` / `--cmd-history-file`.
- Stored in `App::query_history` / `App::cmd_history`.
- Navigation with `Action::NextHistory` / `Action::PreviousHistory` uses `history_index: Option<usize>` and `saved_input: String` to restore the original input when returning to the live query.
- Written back to file at exit in `sk_main` via `write_history_to_file()`, which deduplicates the last entry and enforces `--history-size`.

---

## Pre-Selection

`DefaultSkimSelector` (`src/helper/selector.rs`) implements the `Selector` trait:

```rust
pub trait Selector {
    fn should_select(&self, index: usize, item: &dyn SkimItem) -> bool;
}
```

Three modes (combinable):
- `first_n(N)` — selects the first N items by index
- `preset(iter)` — selects items whose `text()` is in a `HashSet`
- `regex(pattern)` — selects items matching a regex

Configured via `--pre-select-n`, `--pre-select-items`, `--pre-select-pat`, `--pre-select-file`.

Applied during `ItemList::from_options()` and re-applied when items are appended.

---

## Threading Model

Skim uses a mix of OS threads (for blocking I/O) and Tokio async tasks:

```
Main thread (Tokio runtime)
  ├─ Skim::run() async loop → tick() → select!
  │     ├─ process events synchronously (App::handle_event is sync)
  │     └─ block_in_place for ActionCallback::call (async closures)
  │
  └─ Tokio task: Tui event pump (crossterm EventStream + tick timer)

ThreadPool (N = num_cpus OS threads, persistent)
  ├─ Matcher coordinator (1 slot per match run)
  └─ Worker threads (N-1 slots per match run)

Reader threads (OS threads, per-invocation):
  ├─ collect_items thread: polls SkimItemReceiver, calls ItemPool::append
  ├─ I/O reader thread (parallel path only): reads large byte chunks
  ├─ Worker threads (parallel path only): parse lines, create DefaultSkimItem
  └─ Reorder thread (parallel path only): sequence-ordered output

Preview thread (OS thread, per preview spawn):
  └─ reads PTY/child stdout → vt100::Parser or content Arc<RwLock>
      → sends Event::PreviewReady

IPC handler task (Tokio, per connection):
  └─ reads RON actions → sends Event::Action to TUI channel

Popup stdin relay thread (OS thread, only in --popup/--tmux mode):
  └─ copies stdin → FIFO for child sk process
```

**Synchronization primitives used:**
- `Arc<SpinLock<Option<ProcessedItems>>>` — matcher-to-ItemList result handoff
- `Arc<AtomicBool>` — `needs_render` (matcher → event loop), `stopped` / `interrupt` (MatcherControl)
- `Arc<AtomicUsize>` — `processed` / `matched` counters, reader `components_to_stop`
- `Arc<tokio::sync::Notify>` — `items_available` (ItemPool → Skim::tick wakeup)
- `Arc<std::sync::RwLock<PreviewContent>>` — preview thread → Preview widget
- `kanal::Sender/Receiver<Vec<Arc<dyn SkimItem>>>` — item batches through pipeline

The global allocator is `mimalloc` (v3), chosen for its low-latency multi-threaded allocation characteristics critical to the concurrent item-creation pipeline.

---

## Important Call Sites (Cross-Reference)

| Call site | File | What it does |
|---|---|---|
| `Skim::run_with` | `src/skim.rs:56` | Top-level library entry point |
| `Skim::run_items` | `src/skim.rs:98` | Convenience wrapper for iterator inputs |
| `Skim::init` | `src/skim.rs:137` | Constructs all subsystems from options |
| `Skim::start` | `src/skim.rs:178` | Starts reader + initial matcher pass |
| `Skim::should_enter` | `src/skim.rs:346` | Filter/select-1/exit-0/sync gate |
| `Skim::tick` | `src/skim.rs:505` | Single async event loop iteration |
| `Skim::handle_reload` | `src/skim.rs:188` | Kills reader, clears pool, restarts |
| `Skim::output` | `src/skim.rs:440` | Collect & return SkimOutput |
| `App::from_options` | `src/tui/app.rs:250` | Build all widgets from options |
| `App::handle_event` | `src/tui/app.rs:511` | Dispatch all Event variants |
| `App::handle_action` | `src/tui/app.rs:658` | Dispatch all Action variants |
| `App::restart_matcher` | `src/tui/app.rs:1154` | Kill old match pass, start new one |
| `App::run_preview` | `src/tui/app.rs:394` | Expand cmd, debounce, call Preview::spawn |
| `App::expand_cmd` | `src/tui/app.rs:1227` | Substitute `{}`, `{q}`, `{n}` etc. |
| `Widget::render (App)` | `src/tui/app.rs:128` | Root render; calls all sub-widgets |
| `Matcher::run` | `src/matcher.rs:~200` | Parallel match dispatch |
| `merge_worker_results` | `src/matcher.rs:28` | Merge k sorted runs → ProcessedItems |
| `ItemPool::append` | `src/item.rs:467` | Add items, notify matcher |
| `ItemPool::take` | `src/item.rs:512` | Take un-matched items for matcher |
| `DefaultSkimItem::new` | `src/helper/item.rs:55` | ANSI strip, field transform, ranges |
| `SkimItemReader::raw_bufread` | `src/helper/item_reader.rs:300` | 3-stage parallel reader (simple path) |
| `SkimItemReader::read_lines_into_items` | `src/helper/item_reader.rs:217` | Single-threaded reader (complex path) |
| `Preview::spawn` | `src/tui/preview.rs:272` | Start PTY or plain child process |
| `Tui::new_with_height_and_backend` | `src/tui/backend.rs:68` | Terminal init + viewport sizing |
| `Tui::enter` | `src/tui/backend.rs:130` | Enable raw mode + start event pump |
| `Tui::start` | `src/tui/backend.rs:163` | Spawn crossterm EventStream task |
| `popup::run_with` | `src/popup/mod.rs:91` | Delegate to multiplexer popup + parse output |
| `popup::check_env` | `src/popup/mod.rs:78` | Guard: multiplexer present and not already in popup |
| `check_and_run_popup` | `src/bin/main.rs:130` | Check popup conditions, dispatch to popup::run_with |
| `sk_main` | `src/bin/main.rs:142` | CLI orchestration + output printing |
| `parse_key` | `src/binds.rs:130` | `"ctrl-a"` → `KeyEvent` |
| `parse_action_chain` | `src/binds.rs:214` | `"down+select"` → `Vec<Action>` |
| `Matcher::create_engine_factory_with_builder` | `src/matcher.rs:~140` | Build engine factory chain from options |
| `ExactOrFuzzyEngineFactory::create_engine_with_case` | `src/engine/factory.rs:93` | Parse query prefixes, build engine |
| `AndOrEngineFactory::parse_andor` | `src/engine/factory.rs:166` | Split query into AND/OR tree |
| `FuzzyEngine::match_item` | `src/engine/fuzzy.rs:~180` | Fuzzy match a single item |
| `LayoutTemplate::from_options` | `src/tui/layout.rs:76` | Compute widget constraint tree |
| `LayoutTemplate::apply` | `src/tui/layout.rs:164` | Split Rect into AppLayout |
| `ItemRenderer::render_item` | `src/tui/item_renderer.rs:69` | Full per-item render pipeline |
| `ColorTheme::init_from_options` | `src/theme.rs:56` | Parse `--color` spec |

---

## Public Library API

The minimum surface to embed skim in a Rust application:

```rust
use skim::prelude::*;

// 1. Build options
let options = SkimOptionsBuilder::default()
    .height("40%")
    .multi(true)
    .preview(Some("cat {}"))
    .build()
    .unwrap();

// 2a. Run with a static item slice
let output = Skim::run_items(options, ["foo", "bar", "baz"]).unwrap();

// 2b. Or stream items through a channel
let (tx, rx) = unbounded::<Vec<Arc<dyn SkimItem>>>();
// … send batches to tx from another thread …
let output = Skim::run_with(options, Some(rx)).unwrap();

// 3. Inspect results
if !output.is_abort {
    for item in &output.selected_items {
        println!("{}", item.output());
    }
}
```

**Implementing `SkimItem`** for custom types:

```rust
struct MyItem { id: u32, label: String }

impl SkimItem for MyItem {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.label)   // used for matching
    }
    fn output(&self) -> Cow<str> {
        Cow::Owned(self.id.to_string())   // printed on accept
    }
    fn preview(&self, _ctx: PreviewContext) -> ItemPreview {
        ItemPreview::Text(format!("ID: {}\nLabel: {}", self.id, self.label))
    }
}
```

**Custom `ActionCallback`** for inline async logic:

```rust
let cb = ActionCallback::new(|app| async move {
    // mutate app state, return follow-up events
    Ok(vec![Event::Action(Action::Accept(None))])
});
// bind it: options.keymap.insert(key, vec![Action::Custom(cb)]);
```

**`CommandCollector` trait** for custom item sources (e.g. async databases):

```rust
impl CommandCollector for MySource {
    fn invoke(&mut self, cmd: &str, components_to_stop: Arc<AtomicUsize>)
        -> (SkimItemReceiver, Sender<i32>)
    {
        // spawn a thread, send batches of Arc<dyn SkimItem>, return rx + kill-tx
    }
}
```

Set `options.cmd_collector = Rc::new(RefCell::new(my_source))` before calling `Skim::run_with`.