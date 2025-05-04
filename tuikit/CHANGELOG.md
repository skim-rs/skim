# Change Logs

## [0.7.0](https://github.com/skim-rs/skim/compare/v0.6.0...tuikit@v0.7.0) (2025-05-04)


### Features

* **tui:** add tuikit as workspace member and update ([#741](https://github.com/skim-rs/skim/issues/741)) ([73fd406](https://github.com/skim-rs/skim/commit/73fd4068274bda836b71bffaf5d41d4746b29a0d))

## v0.4.6: 2022-05-04

Feature:
- parse `alt-space` to `Alt(' ')`
- implement binding of usercase chars(e.g. `shift-x`)

Fix:
- update `term` to `0.7`
- update `nix` to `0.24.1`
- layout example on README won't compile

## v0.4.5: 2021-02-15

Feature:
- Travis CI -> Github Actions

Fix:
- parse missing keynames(ctrl-up/down/left/right)

## v0.4.4: 2021-02-14

Feature:
- tuikit now returns concrete errors

Fix:
- restore the `clear_on_exit` behavior
- key listener no longer quit(hang) on unknown sequence

## v0.4.3: 2021-01-03

Feature:
- support bracketed paste mode

## v0.4.2: 2020-10-20

Fix:
- click/wheel events' row were not adjusted in non-fullscreen mode

## v0.4.1: 2020-10-18

Fix:
- `Term` not paused on drop.

## v0.4.0: 2020-10-15

Feature:
- support `hold` option that don't start term on creation.
- support user defined event.
- unify result types

## v0.3.4: 2020-10-06

Feature:
- widget `win` support header and right prompt
- new widget: `stack` for stacking widget bottom up
- keyboard now parses double click events
    - in this mode, `MousePress` event would no longer be generated
- keyboard now merges consecutive wheel events

Fix:
- show cursor when quiting alternate screen

## v0.3.3: 2020-06-26

- fix [skim#308](https://github.com/lotabout/skim/issues/308): skim hang on
    initialization

## v0.3.2: 2020-04-01

- fix skim#259 release lock correctly on pause
- fix skim#277: x10 mouse event was capped

## v0.3.1: 2020-02-05

- fix skim#232: use `cfmakeraw` to enable raw mode
- fix build with rust 1.32.0

## v0.3.0: 2020-01-30

Feature:
- Feature: option to clear screen or not after exit.
- Feature: new trait `Widget`

Bug fixes:
- fix skim#255: parse `space` as key ` `
- reset mouse status before exit.
- fix: adjust mouse position(row)'s origin

Examples:
- 256color_on_screen: reset attributes before flush
- fix #10: output help in split example
- get_keys: disable mouse before existing
- all: make examples quit on Ctrl-C

Depedency Update:
- `term` to `0.6`

## v0.2.9: 2019-07-28

Fix: [skim#192](https://github.com/lotabout/skim/issues/192): Start drawing in
a clean line.

## v0.2.8: 2019-06-05

Update dependenncy `nix` to `0.14`.

## v0.2.7: 2019-06-04

Features:
- Implement `From` trait for variaous struct
    * `From<Color> for Attr`
    * `From<Effect> for Attr`
    * `From<char> for Cell`
- `win/split` now accept `Into<...>` struct. Previously when initializing
    splits, you need to write `split.basis(10.into())`,
    now it's just `split.basis(10)`.
- Implement builder pattern for `Attr`. We could now do
    `Attr::default().fg(...).bg(...)`.
- Add two user defined event(`User1` and `User2`). Use it for your own need.

Bug fixes:
- fix compilation error on FreeBSD.

## v0.2.6: 2019-03-28

Reduce CPU usage on idle.

## v0.2.5: 2019-03-28

Clear screen on resize

## v0.2.4: 2019-03-22

Fix: ESC key not working

## v0.2.3: 2019-03-23

- Support more alt keys
- impl `Draw` for `Box<T: Draw>`

## v0.2.2: 2019-03-19

API change: `Draw::content_size` -> `Draw::size_hint` and returns
`Option<usize>`. So that `None` could indicates "I don't know".

## v0.2.1: 2019-03-17

- fix: build failed with rust 2018 (1.31.0)

## v0.2.0: 2019-03-17

Feature:
- Support layout(e.g. `HSplit`, `VSplit`)
- `term.send_event` to inject event to `Term`'s event loop
- `use tuikit::prelude::*` to simplify import

## v0.1.5: 2019-03-02

Fix: Synchronize the pause and restart event.

## v0.1.4: 2019-02-25

Fix: output will replace raw ESC(`\x1b`) with `?` so that terminal won't mess up.

## v0.1.3: 2019-02-24

Fix: report cursor position (0, 0) on terminals that doesn't support CPR.

## v0.1.2: 2019-02-24

Features:
- support specifying `min-height` and `max-height`
- screen: able to iterate over all cells
- attr: add `extend` method for composing `Attr`.

Bug Fixes:
- #1 Increase timeout(to 300ms) on initialize to support slow terminals
- #3 erase contents on exit
- screen: fix panic on height/width of `0`
- fix some key parsing error
