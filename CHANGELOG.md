# Changelog

## [Unreleased]
## [0.17.2](https://github.com/skim-rs/skim/compare/v0.17.1...v0.17.2)

### 🐛 Bug Fixes


- **ci:** Remove release commits filter - ([e0d0169](https://github.com/skim-rs/skim/commit/e0d01690d3b1d819f09f2df71f7e621c171eb0dc))
## [0.17.0](https://github.com/skim-rs/skim/compare/v0.16.2...v0.17.0)

### ⛰️ Features


- **shell:** Readd completions ([#726](https://github.com/skim-rs/skim/pull/726)) ([#739](https://github.com/skim-rs/skim/pull/739)) - ([f87ff67](https://github.com/skim-rs/skim/commit/f87ff6740b20794eaf6288b901f85b7737a28bcf)) (by @LoricAndre)
- **tui:** Add tuikit as workspace member and update ([#741](https://github.com/skim-rs/skim/pull/741)) - ([73fd406](https://github.com/skim-rs/skim/commit/73fd4068274bda836b71bffaf5d41d4746b29a0d)) (by @LoricAndre)

### ⚙️ Miscellaneous Tasks


- **ci:** Migrate to release-plz - ([adc4298](https://github.com/skim-rs/skim/commit/adc4298f94daa283843aa435ed428704f85e8706))

### Contributors

* @LoricAndre

## [0.16.2](https://github.com/skim-rs/skim/compare/v0.16.1...v0.16.2) (2025-04-26)

### Bug Fixes

* **filter:** fix broken pipe while writing results to locked stdout (closes [#733](https://github.com/skim-rs/skim/issues/733)) ([#737](https://github.com/skim-rs/skim/issues/737)) ([ed1c48d](https://github.com/skim-rs/skim/commit/ed1c48d6d0e232ffe4e63e3294cd41664dfdf654))
* **tmux:** check if TMUX is set (closes [#734](https://github.com/skim-rs/skim/issues/734)) ([#736](https://github.com/skim-rs/skim/issues/736)) ([a9ba87b](https://github.com/skim-rs/skim/commit/a9ba87b892eccc847b39e7f228051f2789ebf813))

## [0.16.1](https://github.com/skim-rs/skim/compare/v0.16.0...v0.16.1) (2025-03-05)

### Bug Fixes

* hasten deprecation of expect after [#703](https://github.com/skim-rs/skim/issues/703) ([47c5e1e](https://github.com/skim-rs/skim/commit/47c5e1e54ec848c61d1cfafdaf6aecd3ff4aef86))

## [0.16.0](https://github.com/skim-rs/skim/compare/v0.15.7...v0.16.0) (2025-01-23)


### Features

* add preview callback ([#407](https://github.com/skim-rs/skim/issues/407)) ([226d995](https://github.com/skim-rs/skim/commit/226d9951fa585958e0497e038729c1b6312d36fd))


### Bug Fixes

* **term:** clamp height option ([#690](https://github.com/skim-rs/skim/issues/690)) ([5152be1](https://github.com/skim-rs/skim/commit/5152be1945a8704062c4788f994d385e904d931f))

## [0.15.7](https://github.com/skim-rs/skim/compare/v0.15.6...v0.15.7) (2024-12-27)


### Bug Fixes

* remove atty ([#671](https://github.com/skim-rs/skim/issues/671)) ([b265179](https://github.com/skim-rs/skim/commit/b265179f200fe4fff50c3d39b7a8b19d2bdeaf6f))

## [0.15.6](https://github.com/skim-rs/skim/compare/v0.15.5...v0.15.6) (2024-12-26)


### Bug Fixes

* update rank to follow the readded index tiebreak ([#669](https://github.com/skim-rs/skim/issues/669)) ([920e774](https://github.com/skim-rs/skim/commit/920e7743e992405f6a79a6bdba834c96e6e1cd5c))

## [0.15.5](https://github.com/skim-rs/skim/compare/v0.15.4...v0.15.5) (2024-12-04)


### Bug Fixes

* fix --tmux quoting ([#643](https://github.com/skim-rs/skim/issues/643)) ([1abf545](https://github.com/skim-rs/skim/commit/1abf545ed953dcb9b26a7926df2df105be662c6f))

## [0.15.4](https://github.com/skim-rs/skim/compare/v0.15.3...v0.15.4) (2024-12-01)


### Bug Fixes

* clippy pedantic on lib.rs ([e3637e0](https://github.com/skim-rs/skim/commit/e3637e068ebfeb389993697e36ae9631cb1a659d))

## [0.15.3](https://github.com/skim-rs/skim/compare/v0.15.2...v0.15.3) (2024-12-01)


### Bug Fixes

* clippy pedantic on main.rs ([04541d5](https://github.com/skim-rs/skim/commit/04541d515b3f46243a7f590a81022ee0a2d1a34e))

## [0.15.2](https://github.com/skim-rs/skim/compare/v0.15.1...v0.15.2) (2024-12-01)


### Bug Fixes

* make item module public (closes [#568](https://github.com/skim-rs/skim/issues/568)) ([0963b97](https://github.com/skim-rs/skim/commit/0963b974ffdead23fad3a0db19b92229bf2ab606))

## [0.15.1](https://github.com/skim-rs/skim/compare/v0.15.0...v0.15.1) (2024-12-01)


### Bug Fixes

* fix urls in cargo.toml ([36c4757](https://github.com/skim-rs/skim/commit/36c47578f2e1b7603db6d28f829920243eb8b51e))

## [0.15.0](https://github.com/skim-rs/skim/compare/v0.14.4...v0.15.0) (2024-12-01)


### ⚠ BREAKING CHANGES

* do not check for expect before printing the argument of accept… ([#625](https://github.com/skim-rs/skim/issues/625))

### Features

* add `--tmux` flag (deprecates sk-tmux, fixes [#596](https://github.com/skim-rs/skim/issues/596)) ([#603](https://github.com/skim-rs/skim/issues/603)) ([a2d8c3f](https://github.com/skim-rs/skim/commit/a2d8c3f6022197727b3907562068053a8326a2a2))
* add reload action ([#604](https://github.com/skim-rs/skim/issues/604)) ([4b47244](https://github.com/skim-rs/skim/commit/4b47244922c8910930d5c02016b1c5e99409754a))
* allow more flexibility for use as a library ([#613](https://github.com/skim-rs/skim/issues/613)) ([33ca402](https://github.com/skim-rs/skim/commit/33ca4023c16b20a4ba6f3e1889efddd78ead15d6))
* do not check for expect before printing the argument of accept… ([#625](https://github.com/skim-rs/skim/issues/625)) ([bcee1f4](https://github.com/skim-rs/skim/commit/bcee1f4c028012a24ef7ebbda1f80c0decb2375e))
* readd index tiebreak ([#609](https://github.com/skim-rs/skim/issues/609)) ([0befe8d](https://github.com/skim-rs/skim/commit/0befe8d20659ef90b564f59c07a908ab0953dc0a))
* **tui:** add info hidden ([#630](https://github.com/skim-rs/skim/issues/630)) ([a5b8181](https://github.com/skim-rs/skim/commit/a5b81818d6eb8bfe4c2ceeed3b4cc6e22cc95731))
* use clap & derive for options, manpage & completions ([#586](https://github.com/skim-rs/skim/issues/586)) ([7df8b77](https://github.com/skim-rs/skim/commit/7df8b77739ae5a05e8cd87bff905ee091e5afd7f))


### Bug Fixes

* allow combined multiple args (fixes [#622](https://github.com/skim-rs/skim/issues/622)) ([#623](https://github.com/skim-rs/skim/issues/623)) ([4144879](https://github.com/skim-rs/skim/commit/4144879f00f6a541637112bdb96e23101eb4acda))
* undo sk-tmux deprecation ([c9f9025](https://github.com/skim-rs/skim/commit/c9f9025da9cf0bae7802f725eebd28ebac324378))

## [0.14.4](https://github.com/skim-rs/skim/compare/v0.14.4...v0.14.4) (2024-12-01)


### ⚠ BREAKING CHANGES

* do not check for expect before printing the argument of accept… ([#625](https://github.com/skim-rs/skim/issues/625))

### Features

* add `--tmux` flag (deprecates sk-tmux, fixes [#596](https://github.com/skim-rs/skim/issues/596)) ([#603](https://github.com/skim-rs/skim/issues/603)) ([a2d8c3f](https://github.com/skim-rs/skim/commit/a2d8c3f6022197727b3907562068053a8326a2a2))
* add reload action ([#604](https://github.com/skim-rs/skim/issues/604)) ([4b47244](https://github.com/skim-rs/skim/commit/4b47244922c8910930d5c02016b1c5e99409754a))
* allow more flexibility for use as a library ([#613](https://github.com/skim-rs/skim/issues/613)) ([33ca402](https://github.com/skim-rs/skim/commit/33ca4023c16b20a4ba6f3e1889efddd78ead15d6))
* do not check for expect before printing the argument of accept… ([#625](https://github.com/skim-rs/skim/issues/625)) ([bcee1f4](https://github.com/skim-rs/skim/commit/bcee1f4c028012a24ef7ebbda1f80c0decb2375e))
* readd index tiebreak ([#609](https://github.com/skim-rs/skim/issues/609)) ([0befe8d](https://github.com/skim-rs/skim/commit/0befe8d20659ef90b564f59c07a908ab0953dc0a))
* **tui:** add info hidden ([#630](https://github.com/skim-rs/skim/issues/630)) ([b0868e8](https://github.com/skim-rs/skim/commit/b0868e849a64265618696c071b963b89577f46cd))
* use clap & derive for options, manpage & completions ([#586](https://github.com/skim-rs/skim/issues/586)) ([7df8b77](https://github.com/skim-rs/skim/commit/7df8b77739ae5a05e8cd87bff905ee091e5afd7f))

### Bug Fixes

* allow combined multiple args (fixes [#622](https://github.com/skim-rs/skim/issues/622)) ([#623](https://github.com/skim-rs/skim/issues/623)) ([4144879](https://github.com/skim-rs/skim/commit/4144879f00f6a541637112bdb96e23101eb4acda))
* undo sk-tmux deprecation ([c9f9025](https://github.com/skim-rs/skim/commit/c9f9025da9cf0bae7802f725eebd28ebac324378))


## 0.13.0: 2024-11-25

Features:

- [33ca402](https://github.com/skim-rs/skim/commit/33ca402) - allow more flexibility for use as a library (PR [#613](https://github.com/skim-rs/skim/pull/613) by [@LoricAndre](https://github.com/LoricAndre))
  - ↘️ addresses issue [#612](https://github.com/skim-rs/skim/issues/612) opened by [@idanarye](https://github.com/idanarye)

Chores:

- [53612a7](https://github.com/skim-rs/skim/commit/53612a7) - add pull request template (PR [#608](https://github.com/skim-rs/skim/pull/608) by [@LoricAndre](https://github.com/LoricAndre))

## 0.12.0: 2024-11-24

Features:

- [4b47244](https://github.com/skim-rs/skim/commit/4b47244) - add reload action (PR [#604](https://github.com/skim-rs/skim/pull/604) by [@LoricAndre](https://github.com/LoricAndre))

## 0.11.12: 2024-11-24

Fixes:

- [bd73f62](https://github.com/skim-rs/skim/commit/bd73f62) - remove index tiebreak from shell bindings (PR [#611](https://github.com/skim-rs/skim/pull/611) by [@LoricAndre](https://github.com/LoricAndre))

Chores:

- [63f4e33](https://github.com/skim-rs/skim/commit/63f4e33) - remove some platform-specific quirkinesses from e2e (PR [#602](https://github.com/skim-rs/skim/pull/602) by [@LoricAndre](https://github.com/LoricAndre))

## 0.11.10: 2024-11-21

Features:

- [7df8b77](https://github.com/skim-rs/skim/commit/7df8b77) - use clap & derive for options, manpage & completions (PR [#586](https://github.com/skim-rs/skim/pull/586) by [@LoricAndre](https://github.com/LoricAndre))

Fixes:

- [966d8f5](https://github.com/skim-rs/skim/commit/966d8f5) - 398 shift-up/down was bind to wrong action (PR [#399](https://github.com/skim-rs/skim/pull/399) by [@lotabout](https://github.com/lotabout))
- [aa03781](https://github.com/skim-rs/skim/commit/aa03781) - fix github publish action

Chores:

- [8a57983](https://github.com/skim-rs/skim/commit/8a57983) - fix clippy
- [838ba21](https://github.com/skim-rs/skim/commit/838ba21) - remove atty (PR [#587](https://github.com/skim-rs/skim/pull/587) by [@LoricAndre](https://github.com/LoricAndre))
- [c932a1f](https://github.com/skim-rs/skim/commit/c932a1f) - remove bitflags (PR [#579](https://github.com/skim-rs/skim/pull/579) by [@LoricAndre](https://github.com/LoricAndre))

## 0.10.4: 2023-03-02

- Fix release issue

## 0.10.3: 2023-02-23

- Update README.md

## 0.10.2: 2022-11-08

- Use crate version

## 0.10.1: 2022-12-28

Features:

- transparency on start
- add light colors parsing support
- Update --tiebreak options with length

Fixes:

- fix ci.yml
- update deps and fix lots of clippy lints

## 0.10.0: 2022-10-28

Features:

- transparency on start
- add light colors parsing support
- Update --tiebreak options with length

Fixes:

- fix ci.yml
- update deps and fix lots of clippy lints

## 0.9.4: 2021-02-15

Feature:

- Upgrade dependency versions
- use Github Actions for CI
- Support bracketed paste
- [#384](https://github.com/lotabout/skim/issues/384) support ctrl-left/right
  for cursor movement between words

Fix:

- [#386](https://github.com/lotabout/skim/issues/386) freeze on unknown
  keystrokes
- [#376](https://github.com/lotabout/skim/issues/376) noticeable delay in
  interactive mode

## 0.9.3: 2020-11-02

Fix:

- [#370](https://github.com/lotabout/skim/issues/370) Ansi parse error for
  multi-byte strings
- [#372](https://github.com/lotabout/skim/issues/372) Can't bind `Enter` key
  with `expect` specified
- [#369](https://github.com/lotabout/skim/issues/369) `--select-1` and
  `--exit-0` still take effect after all items are read and matched.

## 0.9.2: 2020-10-24

Feature:

- new action `refresh-cmd`: call the interactive command and refresh the
  items accordingly.
- new action `refresh-preview`: call the preview command and refresh the
  preview display. Will only refresh if the preview window is shown.

Fix:

- zsh corrupt `REPORTTIME` settings.
- [#359](https://github.com/lotabout/skim/issues/359) panic with multi-byte and regex
- [#361](https://github.com/lotabout/skim/issues/361) support literal space by `\ `
- [#365](https://github.com/lotabout/skim/issues/365) new option
  `--show-cmd-error` to retrieve error message of failed interactive command
  and display as items. Served as a debug helper.

## 0.9.1: 2020-10-20

Feature:

- Support preview scroll offset relative to window height
  ```sh
  git grep --line-number '' |
    sk --delimiter : \
        --preview 'bat --style=numbers --color=always --highlight-line {2} {1}' \
        --preview-window +{2}-/2
  ```

Fix:

- [#356](https://github.com/lotabout/skim/issues/356) panic on ANSI enabled.
- `tiebreak` would now include `score` in the front of criterion if not specified.
- Reduce preview window flicking when moving cursor fast.
- Multiple preview window options weren't merged.
- `pre-select-items` should not contain empty string by default.
- click/wheel events's row weren't correct if `--height` is specified.

## 0.9.0: 2020-10-18

Breaking Change to the Library:

- `SkimItem::display` now accepts a `DisplayContext` that provide more
  information such as container width, score, matches, etc.
- `SkimItem::preview` now accepts a `PreviewContext` that provide more
  information such as query, width, selections, etc.
- `Skim::run_as` now returns `Some` on both `Accept` and `Abort`, so that user
  could collect and react on abort events.
- `SkimOutput` now provides the final key received before return.

Features:

- Reduce memory usage
- Defer drops of items, to improve interaction speed
- support `--tac` and `--nosort`
- new action: `half-page-up` and `half-page-down`
- support tiebreak by `length`
- [#344](https://github.com/lotabout/skim/issues/344) expose preview context
  in `preview()` function
- [#341](https://github.com/lotabout/skim/issues/341) support multiline header
- use unicode spinner
- [#324](https://github.com/lotabout/skim/issues/324) support option
  `--no-clear` to keep the content drawn on screen
- [#300](https://github.com/lotabout/skim/issues/300) library: move reader
  options to default reader
- support new option `--keep-right` to show the right most text if it is too
  long.
- support negative horizontal scroll
- support `--skip-to-pattern` to start item display with the pattern matched
- support `--select-1` that automatically select the only match
- support `--exit-0` that exit automatically if no item matched
- support `--sync` that waits for all inputs to be ready and then starts the
  selection UI
- [#309](https://github.com/lotabout/skim/issues/309) support pre-selection
  - `pre-select-n`: select first `n` items
  - `pre-select-pat`: select items that matches regex
  - `pre-select-items`: select items from a preset
  - `pre-select-file`: select items from a preset that's loaded from file
- [#328](https://github.com/lotabout/skim/issues/328) support
  `--no-clear-if-empty` that preserve selection if the new command query
  returns nothing. Was designed to reduce flicking.

Fixes:

- [#326](https://github.com/lotabout/skim/issues/326) preview not updated anymore
- [#349](https://github.com/lotabout/skim/issues/349) kill-line and
  discard-line in interactive mode
- [#344](https://github.com/lotabout/skim/issues/344) implement `text()` and
  `display()` correctly
- [#312](https://github.com/lotabout/skim/issues/312) mouse click and page
  up/down out of bound
- Do not auto-scroll for customized items
- [#321](https://github.com/lotabout/skim/issues/321) fix annoyance through
  ZSH's REPORTTIME

## 0.8.2: 2020-06-26

Bug fixes:

- fix skim executable in bash completion
- fix [#291](https://github.com/lotabout/skim/issues/291) hide scroll in when
  content fit
- fix [#308](https://github.com/lotabout/skim/issues/308) hangs on
  initialization

## 0.8.1: 2020-02-23

Feature:

- [#63](https://github.com/lotabout/skim/issues/63) could save to and read
  from history for query and command query via `--history` and `--cmd-history`
- [#273](https://github.com/lotabout/skim/issues/273) inline-info now has
  spinner
- [#276](https://github.com/lotabout/skim/issues/276) new action:
  `if-non-matched` will execute if non of the items matches
- reduce memory footprint
- [#248](https://github.com/lotabout/skim/issues/248) implement `{n}`
  placeholder, used to refer to current items's index(zero based).

Bug fixes:

- [PR #279](https://github.com/lotabout/skim/pull/279) exit gracefully on
  SIGPIPE error. (e.g. Ctrl-C on pipes)
- [#276](https://github.com/lotabout/skim/issues/276) `execute` panic on zero
  results
- [#278](https://github.com/lotabout/skim/issues/278) `NUL` character not
  working in preview command
- handle `print0` correctly in filter mode
- Preview's fields now based on original text, not transformed.
- [#295](https://github.com/lotabout/skim/issues/295) skim not exits
  sometimes (occasionally happens on Ubuntu)

## 0.8.0: 2020-02-23

**Breaking Changes in API**

- `Skim::run_with` now accept a stream of `SkimItem` instead of a `BufRead`.

Feature:

- [#233](https://github.com/lotabout/skim/issues/233) support mouse
  scroll/click event
- [#254](https://github.com/lotabout/skim/issues/254) support `{+}` in preview
  and execute command
- [#226](https://github.com/lotabout/skim/issues/226) support exact match
  combination(e.g. `^abc$`)
- [#216](https://github.com/lotabout/skim/issues/216) support item specific
  preview hook method
- [#219](https://github.com/lotabout/skim/issues/219) support case insensitive
  match

Bug fixes:

- [#252](https://github.com/lotabout/skim/issues/252) Deal with `\b` correctly
- [#210](https://github.com/lotabout/skim/issues/210) exclude current item in
  multi-selection
- [#225](https://github.com/lotabout/skim/issues/225) disable score in filter
  output

## 0.7.0: 2020-01-15

Feature:

- New fuzzy matching algorithm, should be more precise and faster.

Bug fixes:

- [PR #227](https://github.com/lotabout/skim/pull/227)
  Fix `isatty` check on more OS.
- Fix various cases where ANSI code not behave correctly.

## 0.6.9: 2019-09-22

Bug fixes:

- [PR #171](https://github.com/lotabout/skim/pull/171)
  search with more than one multi-byte condition would crash skim.
- [#194](https://github.com/lotabout/skim/issues/194)
  color not working with ag
- [#196](https://github.com/lotabout/skim/issues/196)
  `+` in execute expression was eaten by skim
- bind `Home` key to `begining-of-line` by default.
- [#192](https://github.com/lotabout/skim/issues/192)
  Prompt was eaten in shell completion
- [#205](https://github.com/lotabout/skim/issues/205)
  tabstop of selection was initialized to `0`, now to `8`.
- [#207](https://github.com/lotabout/skim/issues/207)
  color config not working for header

## 0.6.8: 2019-06-23

Feature:

- New action: `if-query-empty`, `if-query-not-empty`. Execute actions on
  certain query conditions.
- New action: `append-and-select` allows you to append current query to the
  item pool and select it. It would help to turn skim into a tag manager
  where new tags could be added to the candidate list.

Bug fixes:

- Fix [#188](https://github.com/skim-rs/skim/issues/188): crates.io breaks on 0.6.7
- Fix: `run_with` will break if called multiple times from the same process.
- Update nix to 0.14

## 0.6.7: 2019-05-31

Feature:

- Refer to query and command query with `{q}` and `{cq}` in preview command.
- Support fzf's theme strings, e.g. `bg+` for current line's background.
- Support customizing styles of query strings.

Bug fixes:

- skim would crash if multiple CJK items are matched in an `OR` query.
- SKIM_DEFAULT_COMMAND not correctly recognized in `sk-tmux`
- UI responses are slow on large input

## 0.6.6: 2019-04-03

fix [#158](https://github.com/skim-rs/skim/issues/158): preview window not udpate correctly.

## 0.6.5: 2019-04-01

Bug Fixes:

- [#155](https://github.com/skim-rs/skim/issues/155): screen is not fully cleared upon resize
- [#156](https://github.com/skim-rs/skim/issues/156): preview dies on large chunk of input
- [#157](https://github.com/skim-rs/skim/issues/157): cursor overflow on empty input
- [#154](https://github.com/skim-rs/skim/issues/154): reduce CPU usage on idle
- wrong matches on empty input lines

## 0.6.4: 2019-03-26

Fix: [#153](https://github.com/skim-rs/skim/issues/153) build fail with rust 2018 (1.31.0)

## 0.6.3: 2019-03-25

Feature:

- support action: `execute`
- support action chaining
- preview window actions: `toggle-preview-wrap`, `preview-[up|down|left|right]`, `preview-page-[up|down]`
- support `--filter` mode, it will print out the screen and matched item
- support more (alt) keys

Bug Fixes:

- wrong cursor position after item changed
- [#142](https://github.com/skim-rs/skim/issues/142): NULL character was dropped with `--ansi`
- regression: `--margin` not working
- [#148](https://github.com/skim-rs/skim/issues/148): screen won't clear in interactive mode
- number of matched item not showing correctly (during matching)
- lag in changing query on large collection of inputs

## 0.6.2: 2019-03-19

Feature:

- Support `--header-lines`
- Support `--layout`
- Update the latest fzf.vim

## 0.6.1: 2019-03-17

Fix:

- compile fail with rust 2018 (1.31.0)
- reduce the time on exit. It took time to free memories on large
  collections.

## 0.6.0: 2019-03-17

Performance improvement.

This is a large rewrite of skim, previously there are 4 major components of
skim:

- reader: for reading from command or piped input
- sender: will cache the lines from reader and re-send all lines to matcher on restart
- matcher: match against the lines and send the matched items to model
- model: handle the selection of items and draw on screen.

They are communicated using rust's `channel` which turned out to be too slow
in skim's use case. Now we use `SpinLock` for sharing data. The performance on
large collections are greatly improved.

Besides, use `tuikit` for buferred rendering.

## 0.5.5: 2019-02-23

Bug fixes:

- fix: regression on `--with-nth` feature
- fix: 100% CPU on not enough printing area

## 0.5.4: 2019-02-20

Emergency release that fix test failures which breaks
[APKBUILD](https://github.com/5paceToast/user-aports/blob/master/toast/skim/APKBUILD).
Check out [#128](https://github.com/lotabout/skim/issues/128).

## 0.5.3: 2019-02-20

Features:

- `--header` for adding header line
- `--inline-info` for displaying info besides query
- run preview commands asynchronizely
- implement action `delete-charEOF`
- support key: `ctrl+space`

More bug fixes, noticable ones are:

- Panic on reading non-utf8 characters
- 100% CPU when input is not ready

## 0.5.2: 2018-10-22

- fix: stop command immediately on accept or abort.
- minor optimization over ASCII inputs.
- [#90](https://github.com/skim-rs/skim/issues/90): escape quotes in specified preview command

## 0.5.1: 2018-06-24

Use [cross](https://github.com/japaric/cross) to build targets.

## 0.5.0: 2018-06-12

Change the field syntax to be fzf compatible.

- Previously it was git style
  - fields starts with `0`
  - `1..3` results in `2, 3` (which is `0, 1, 2, 3` minus `0, 1`)
- Now it is `cut` style
  - fields starts with `1`
  - `1..3` results in `1, 2, 3`

## 0.4.0: 2018-06-03

Refactor skim into a library. With minor bug fixes:

- support multiple arguments, to be a drop-in replacement of fzf.
- support negative range field. (e.g. `-1` to specify the last field)
- respond to terminal resize event on Mac.

## 0.3.2: 2018-01-18

Some minor enhancements that might comes handy.

- Reserve all fzf options, so that skim can be a drop-in replacement of fzf.
- Fix: the number of columns a unicode character occupies
- Accept multiple values for most options. So that you can safely put them
  in `$SKIM_DEFAULT_OPTIONS` and override it in command line.

Thanks to [@magnetophon](https://github.com/magnetophon) for the bug report and feature requests.

## 0.3.1: 2017-12-04

Support more options, and reserve several others. The purpose is to reuse
`fzf.vim` as much as possible.

- `--print0`: use NUL(\0) as field separator for output.
- `--read0`: read input delimited by NUL(\0) characters
- `--tabstop`: allow customizing tabstop (default to 8).
- `--no-hscroll`: disable hscroll on match.
- reserve several other options, skim will do nothing on them instead of throwing errors.

## 0.3.0: 2017-09-21

This release starts from adding `--height` featuren, ends up a big change in
the code base.

- feature: `--bind` accept character keys. Only Ctrl/Alt/F keys were accepted.
- feature: support multiple `--bind` options. (replace getopts with clap.rs)
- feature: `--tac` to reverse the order of input lines.
- feature: `--preview` to show preview of current selected line.
- feature: `--height` to use only part instead of full of the screen.
- test: use tmux for integration test
- replace [ncurses-rs](https://github.com/jeaye/ncurses-rs) with [termion](https://github.com/ticki/termion), now skim is fully rust, no C bindings.
