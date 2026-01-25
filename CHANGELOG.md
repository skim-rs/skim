# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.7.2] - 2026-01-25

### 🐛 Bug Fixes

- Correct cursor position when using reverse and border (closes #928)

## [1.7.1] - 2026-01-25

### 🐛 Bug Fixes

- Y cursor pos in reverse mode (closes #931)

## [1.7.0] - 2026-01-25

### 🚀 Features

- Add borders to all widgets (#930)

### 🐛 Bug Fixes

- Correctly merge base styles
- Correctly display all header lines
- Correctly toggle prompt on ToggleInteractive (closes #925)
- Fix printf sometimes replacing recursively
- Interrupt the reader thread when stopping
- Replace {n} with an empty string when no item is selected
- Revert case-insensitive action_chain
- Re-enable query/cmd-query distinction and switching
- Correctly compute character width for cursor display (closes #929)

### ⚙️ Miscellaneous Tasks

- Cleanup changelog [skip ci]

## [1.6.0] - 2026-01-23

### 🚀 Features

- Add `--remote` flag to call remote (`--listen`) instances (#915)

### 🐛 Bug Fixes

- Make no-sort work again

### 🧪 Testing

- Remove insta_ prefixes after finalizing tests migration

## [1.5.4] - 2026-01-23

### 🐛 Bug Fixes

- Do not override {} with {q} in interactive mode
- Remove unnecessary clone in printf
- Correctly merge styles & do not reset them by default (#918)
- Translate frizbee's byte indices into char indices

### 📚 Documentation

- Customize man page

## [1.5.3] - 2026-01-22

### 🐛 Bug Fixes

- Quote expanded items independently (#910)
- Escape last `;` in env var value before passing to tmux (#912)

### New Contributors
* @mathieu-lemay made their first contribution in [#912](https://github.com/skim-rs/skim/pull/912)

## [1.5.2] - 2026-01-22

### 🐛 Bug Fixes

- Ignore `{+}` expressions when splitting action chains (closes #910)
- Strip ansi from expanded items (#910)

## [1.5.1] - 2026-01-22

### 🐛 Bug Fixes

- Correctly expand `{+}` to current when no items are selected (cl… (#913)

## [1.5.0] - 2026-01-22

### 🚀 Features

- Add `set-query` action to update the input (closes #657) (#907)

### 🐛 Bug Fixes

- Make case option work with non-ascii input (closes #454)

### ⚙️ Miscellaneous Tasks

- Fix tests link in PR template [skip ci]

## [1.4.0] - 2026-01-21

### 🚀 Features

- Split-match (#906)

### 📚 Documentation

- Reflect need for nightly rust in install section [skip ci]

## [1.3.2] - 2026-01-21

### 🐛 Bug Fixes

- Better spinner debounce behavior to avoid flickering (closes #904)

### 📚 Documentation

- Update README install section
- Add details to interactive mode in manpage (closes #805) (#816)

### 🧪 Testing

- Use insta for applicable integration tests, making them cross-p… (#903)

## [1.3.1] - 2026-01-21

### 🐛 Bug Fixes

- Allow layout to override reverse (closes #901)

### 🧪 Testing

- Allow multiple bench runs for better consistency
- More reproducible and more precise bench [skip ci]

### ⚙️ Miscellaneous Tasks

- Optimized release builds

## [1.3.0] - 2026-01-20

### 🚀 Features

- Typo resistant matcher using frizbee from blink.cmp (#891)

## [1.2.0] - 2026-01-20

### 🚀 Features

- Add no-strip-ansi flag (#898)

### 🐛 Bug Fixes

- Run preview in a PTY (closes #894)

## [1.1.2] - 2026-01-20

### 🐛 Bug Fixes

- Half page down scrolls down
- Use ansi-stripped raw item in preview expansion

## [1.1.1] - 2026-01-19

### 🐛 Bug Fixes

- Use item text in printf
- Parse ansi codes in header
- Use item output for fields

### 🧪 Testing

- Fix preview_nul

### ⚙️ Miscellaneous Tasks

- Update crossterm version requirement to pass crates.io publish checks

## [1.1.0] - 2026-01-19

### 🚀 Features

- Wrap items

### 🐛 Bug Fixes

- Delete outside char boundaries
- Preview on large binaries does not hang or mangle the tui

### 🧪 Testing

- Fix wrap test (#896)

## [1.0.1] - 2026-01-19

### 🐛 Bug Fixes

- Disable compact_matcher feature

## [1.0.0-pre11] - 2026-01-17

### 🐛 Bug Fixes

- Always use cursor/selector colors (#892)

### 🧪 Testing

- Fix flaky tests

### ⚙️ Miscellaneous Tasks

- *(changelog)* Ignore release commits

## [1.0.0-pre10] - 2026-01-17

### 🐛 Bug Fixes

- Only expand selection in {+} for commands

### ⚙️ Miscellaneous Tasks

- Add pointer/marker as aliases for selector/multi-selector

## [1.0.0-pre9] - 2026-01-16

### 🐛 Bug Fixes

- Matcher race condition at startup

## [1.0.0-pre8] - 2026-01-16

### 🚀 Features

- Add print-header flag (and readd print-score) (closes #470)

### 🐛 Bug Fixes

- *(ui)* Use current highlight for the current item (closes #889) (#890)

### 🧪 Testing

- Remove useless listen tests

## [1.0.0-pre7] - 2026-01-16

### 🚀 Features

- Add `listen` flag (closes #719)

### 🐛 Bug Fixes

- Fix listen flag on macos (#888)
- Correctly parse wrap arg in preview options

### 🧪 Testing

- Add tests for listen flag

## [1.0.0-pre6] - 2026-01-15

### 🚀 Features

- Add cycle flag (closes #553)
- Add disabled flag (closes #500)
- Add nushell completion support (closes #459)
- Add --shell-bindings flag to get bindings at runtime

### 🐛 Bug Fixes

- Disable completions without cli feature
- Fix build without default features

### ⚙️ Miscellaneous Tasks

- Add exhaustive_match macro for enum building from str

## [1.0.0-pre5] - 2026-01-15

### 🚀 Features

- *(ui)* Add selector and multi-selector options to set the itemlist icons
- *(ui)* Allow setting modifiers (closes #871)

## [1.0.0-pre4] - 2026-01-14

### 🚀 Features

- 120 FPS

### 🐛 Bug Fixes

- *(cmd)* [**breaking**] Always use `sh` for all command executions

### ⚙️ Miscellaneous Tasks

- Regenerate CHANGELOG.md

## [1.0.0-pre3] - 2026-01-14

### 🐛 Bug Fixes

- Fix terminal height management

### ⚙️ Miscellaneous Tasks

- Release v1.0.0-pre3

## [1.0.0-pre2] - 2026-01-14

### 🚀 Features

- *(ci)* Add crates.io publish to release CI

### 🐛 Bug Fixes

- Manually acquire cursor pos (closes #885) (#886)

### ⚙️ Miscellaneous Tasks

- Remove unneeded deps (#884)
- Release

## [1.0.0-pre1] - 2026-01-13

### 🚀 Features

- *(ui)* [**breaking**] Ratatui migration (#864)

### ⚙️ Miscellaneous Tasks

- Remove workspace (#883)

### New Contributors
* @rusty-snake made their first contribution in [#872](https://github.com/skim-rs/skim/pull/872)
* @peccu made their first contribution in [#845](https://github.com/skim-rs/skim/pull/845)
* @azarmadr made their first contribution in [#841](https://github.com/skim-rs/skim/pull/841)

## [0.20.5] - 2025-08-09

### 🐛 Bug Fixes

- Compile without the cli feature (#834)

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#835)

## [0.20.4] - 2025-08-02

### 🚀 Features

- *(e2e)* Add Dockerfile to run E2E

### 🐛 Bug Fixes

- *(options)* Allow border to be used without args
- *(ci)* Fetch whole history to avoid PR recreation

### ⚙️ Miscellaneous Tasks

- *(ci)* Revert to a more vanilla release-plz config
- Remove unreleased section from changelog
- *(release)* Release (#831)

## [0.20.3] - 2025-07-27

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#826)

## [0.20.2] - 2025-06-29

### 📚 Documentation

- *(e2e)* Add contributing section (#817)

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#818)

### New Contributors
* @azat made their first contribution in [#783](https://github.com/skim-rs/skim/pull/783)

## [0.20.1] - 2025-06-21

### 🐛 Bug Fixes

- Min-query-length in interactive mode (#814)

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#815)

## [0.20.0] - 2025-06-21

### 🚀 Features

- *(ui)* Respect NO_COLOR environment variable (#804)

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#813)

### New Contributors
* @saidelmark made their first contribution in [#804](https://github.com/skim-rs/skim/pull/804)

## [0.19.0] - 2025-06-21

### 🚀 Features

- Add min query length option (#806)

### ⚙️ Miscellaneous Tasks

- *(release)* Release (#811)

## [0.18.0] - 2025-05-30

### 🚀 Features

- *(shell)* Improve shell completion with dynamic generation (#790)

### 🐛 Bug Fixes

- *(ci)* Remove version from pr name

### 📚 Documentation

- *(contributing)* Refine guidelines for GPT-assisted development
- Improve theming documentation (#788)
- Improve wording in README and options.rs (#789)

### ⚙️ Miscellaneous Tasks

- Generate changelog
- *(release)* Release (#792)

## [0.17.3] - 2025-05-20

### 🐛 Bug Fixes

- *(shell)* Fix zsh tmux args in key bindings (#777)
- *(shell)* Remove duplocate tmux height arg fixes #776 (#778)

### 💼 Other

- Set keybinding right before printing special character (#774)

### ⚙️ Miscellaneous Tasks

- Generate changelog using git cliff
- *(release)* Release v0.17.3 (#782)

### New Contributors
* @ajeetdsouza made their first contribution in [#774](https://github.com/skim-rs/skim/pull/774)

## [0.17.2] - 2025-05-04

### 🐛 Bug Fixes

- *(tmux)* Force sh as shell for tmux mode (#765)
- *(ci)* Remove release commits filter

### ⚙️ Miscellaneous Tasks

- *(ci)* Remove temp workflow
- *(release)* Release v0.17.2 (#766)

## [0.17.1] - 2025-05-04

### 🚀 Features

- *(ci)* Manually update versions

### 🐛 Bug Fixes

- *(cargo)* Fix tuikit re-export
- *(ci)* More generic pr name
- *(ci)* Split release pr and gh release
- *(cargo)* Fix tuikit readme path
- *(ci)* Fix broken ci after migration

### 🧪 Testing

- *(ci)* Show context
- *(ci)* Test trigger (#761)

### ⚙️ Miscellaneous Tasks

- *(ci)* Only release after merge
- Release (#760)
- *(cargo)* Update to 2024 edition (#764)
- *(ci)* Update dependencies

## [0.17.0] - 2025-05-04

### 🐛 Bug Fixes

- Fix local dependencies

## [common-v0.1.0] - 2025-05-04

### 🚀 Features

- *(tui)* Add tuikit as workspace member and update (#741)
- *(shell)* Readd completions (#726) (#739)

### 🐛 Bug Fixes

- *(cargo)* Fix workspace packages
- *(ci)* Remove leftover package
- *(ci)* Add metadata to common package

### ⚙️ Miscellaneous Tasks

- *(tuikit)* Bring skim-rs/tuikit#43 (#743)
- *(ci)* Back to manifest release
- *(ci)* Readd manifest manually
- *(ci)* Revert action
- *(ci)* Use linked changelog
- *(ci)* Disable skim prefix in tag
- *(ci)* Test without extra packages
- *(ci)* Readd all components
- *(ci)* Release every package at the same version
- *(ci)* Release whole workspace at once
- *(ci)* Update manifest
- *(ci)* Readd all packages as well as root
- *(ci)* Better handling of packages in release
- *(ci)* Unlink versions
- *(ci)* Set package names
- *(ci)* Explicitely set root component
- *(ci)* Explicitely set last release sha
- *(ci)* Use previous versions for packages
- *(ci)* Migrate to release-plz
- *(ci)* Update release-plz changelog format
- *(ci)* Update release-plz changelog format
- *(ci)* Split release actions
- Release (#756)
- *(ci)* Do not publish extra packages
- *(ci)* Release on all commits
- *(ci)* Make local packages publishable

## [0.16.2] - 2025-04-26

### 🚀 Features

- *(zsh)* [**breaking**] Sort history items by timestamp

### 🐛 Bug Fixes

- *(tmux)* Check if TMUX is set (closes #734) (#736)
- *(filter)* Fix broken pipe while writing results to locked stdout (closes #733) (#737)

### 📚 Documentation

- *(tmux)* Add note about env var (#732)
- *(tmux)* Fix docs formatting

### 🧪 Testing

- *(ci)* Try a simpler release-please config

### ⚙️ Miscellaneous Tasks

- Move changelog to subdir (#740)
- *(master)* Release 0.16.2 (#738)

## [0.16.1] - 2025-03-06

### 🐛 Bug Fixes

- Hasten deprecation of expect after #703

### ⚙️ Miscellaneous Tasks

- Manually update release-please manifest after release
- *(master)* Release 0.16.1 (#712)

## [0.16.0] - 2025-01-23

### 🚀 Features

- Add preview callback (#407)

### 🐛 Bug Fixes

- *(docs)* Fix README lib example
- *(term)* Clamp height option (#690)

### 📚 Documentation

- *(readme)* Correct fzf library statement in README (#679)

### 🧪 Testing

- *(ci)* Test previous fixes
- *(ci)* Test previous fixes
- *(ci)* Try removing the packages altogether

### ⚙️ Miscellaneous Tasks

- Remove lazy_static (#687)
- Fix clippy warning in rust 1.84 (#688)
- *(ci)* Try to fix release-please on extra packages
- *(ci)* Do not search commits on e2e & xtask
- *(ci)* Try releasing as 0.1.0
- Release master (#672)
- Release master (#691)

### New Contributors
* @alexxbb made their first contribution in [#407](https://github.com/skim-rs/skim/pull/407)
* @alexandregv made their first contribution in [#679](https://github.com/skim-rs/skim/pull/679)

## [0.15.7] - 2024-12-27

### 🐛 Bug Fixes

- Remove atty (#671)

### ⚙️ Miscellaneous Tasks

- Release master (#670)

### New Contributors
* @gallois made their first contribution in [#671](https://github.com/skim-rs/skim/pull/671)

## [0.15.6] - 2024-12-26

### 🐛 Bug Fixes

- Fix non-functional vim plugin (#659)
- Update rank to follow the readded index tiebreak (#669)

### ⚙️ Miscellaneous Tasks

- Release master (#656)

### New Contributors
* @egrieco made their first contribution
* @dotdash made their first contribution in [#659](https://github.com/skim-rs/skim/pull/659)

## [0.15.5] - 2024-12-04

### 🐛 Bug Fixes

- Revert README overwrite
- Fix --tmux quoting (#643)

### 📚 Documentation

- Missing backtick in install commands (#646)
- Add note about fuzziness of interactive examples (fixes #543)

### ⚙️ Miscellaneous Tasks

- Release master (#647)
- Fix release-please config
- Fix release config
- Release master (#655)

### New Contributors
* @genskyff made their first contribution in [#646](https://github.com/skim-rs/skim/pull/646)

## [0.15.4] - 2024-12-01

### 🐛 Bug Fixes

- Fix token permissions for release file
- Clippy pedantic on lib.rs

### ⚙️ Miscellaneous Tasks

- Cargo fmt
- Release master (#642)

## [0.15.3] - 2024-12-01

### 🐛 Bug Fixes

- Fix missing var in CI
- Clippy pedantic on main.rs

### ⚙️ Miscellaneous Tasks

- Remove cli feature from skim
- Cargo fmt
- Release master (#641)

## [0.15.2] - 2024-12-01

### 🐛 Bug Fixes

- Do not run tests in release workflow
- Make item module public (closes #568)

### ⚙️ Miscellaneous Tasks

- Release master (#640)

### New Contributors
* @skim-rs-bot[bot] made their first contribution in [#640](https://github.com/skim-rs/skim/pull/640)

## [0.15.1] - 2024-12-01

### 🐛 Bug Fixes

- Fix ci
- Fix urls in cargo.toml

### ⚙️ Miscellaneous Tasks

- Generate files in PR (#638)
- Fix push
- Test push with explicit ref
- Use cache for xtask
- Simplify release ci
- Use PAT for release-please to trigger downstream ci
- Use gh app for token
- Use gh app for push
- Manually use gh app for push
- Skip ci on modified files
- Use token in checkout
- Exit success when nothing to commit
- Avoid duplicate test runs
- Cleanup
- Release master (#639)

## [0.15.0] - 2024-12-01

### 🚀 Features

- *(tui)* Add info hidden (#630)

### 🐛 Bug Fixes

- *(ci)* Fix clippy os
- *(ci)* Set release-please path
- Undo sk-tmux deprecation
- *(ci)* Release-please permissions on job level
- *(ci)* Use subpath for release-please outputs
- *(ci)* Remove needs in release-please condition
- *(ci)* Use different syntax for conditions
- *(ci)* Add intermediary step for release
- *(ci)* Use release-please in workspace root
- *(ci)* Test with different release-please config
- *(ci)* Set skim version
- *(ci)* Set skim changelog path
- *(ci)* Use absolute path for changelog
- *(ci)* Do not bump major
- *(ci)* Bump minor for feat
- *(ci)* Use correct tag
- *(ci)* Remove string from cond
- *(ci)* Fix templating
- *(ci)* Fix extra dot
- *(ci)* Use stable toolchain
- *(ci)* Remove extra modules
- *(ci)* Skip extra packages
- *(ci)* Replace underscore with dashes
- Set toolchain

### 🧪 Testing

- Migrate e2e to rust (#629)
- *(ci)* Try downgrading cargo.toml
- *(ci)* Test with crate root
- *(ci)* Test with subpath
- *(ci)* Add debug
- *(ci)* Fix dash in test
- *(ci)* Check for string

### ⚙️ Miscellaneous Tasks

- Readd crate to release-please
- Fix release-please target branch
- Fix condition
- Release master (#632)
- Release master (#633)
- Cleanup failed releases
- Release master (#634)
- Release master (#635)
- Release master (#636)
- Release master (#637)

### New Contributors
* @github-actions[bot] made their first contribution in [#637](https://github.com/skim-rs/skim/pull/637)

## [0.14.3] - 2024-11-28

### 🚀 Features

- Readd index tiebreak (#609)
- [**breaking**] Do not check for expect before printing the argument of accept… (#625)
- Add `--tmux` flag (deprecates sk-tmux, fixes #596) (#603)

### 🐛 Bug Fixes

- Allow combined multiple args (fixes #622) (#623)

### 📚 Documentation

- Update changelog from github releases (#620)
- Link all PRs, issues, commits and authors in CHANGELOG (#621)
- Add fzf-lua and nu_plugin_skim to the README (#626)

### ⚙️ Miscellaneous Tasks

- Bump unicode-width from 0.1.14 to 0.2.0 (#616)
- Bump nix from 0.25.1 to 0.29.0 (#614)
- Bump env_logger from 0.9.3 to 0.11.5 (#615)
- Improve PR ci (#617)
- Remove ci dir (#627)

### New Contributors
* @khafatech made their first contribution in [#605](https://github.com/skim-rs/skim/pull/605)
* @praveenperera made their first contribution in [#621](https://github.com/skim-rs/skim/pull/621)

## [0.13.0] - 2024-11-25

### 🚀 Features

- Allow more flexibility for use as a library (#613)

### ⚙️ Miscellaneous Tasks

- Add pull request template (#608)

## [0.12.0] - 2024-11-24

### 🚀 Features

- Add reload action (#604)

## [0.11.12] - 2024-11-24

### 🐛 Bug Fixes

- Remove index tiebreak from shell bindings (#611)

### ⚙️ Miscellaneous Tasks

- Remove some platform-specific quirkinesses from e2e (#602)

### New Contributors
* @crodjer made their first contribution in [#413](https://github.com/skim-rs/skim/pull/413)

## [0.11.11] - 2024-11-22

### 💼 Other

- Readd version arg (#606)

## [0.11.1] - 2024-11-21

### 🐛 Bug Fixes

- Fix github publish action

## [0.11.0] - 2024-11-20

### 🚀 Features

- Use clap & derive for options, manpage & completions (#586)

### 💼 Other

- "Package Managers": add Portage
- Remove unuseful entries (#382)

### 📚 Documentation

- *(discord)* Discord invitation link

### ⚙️ Miscellaneous Tasks

- Fix clippy
- Remove atty (#587)
- Remove bitflags (#579)

### New Contributors
* @LoricAndre made their first contribution in [#586](https://github.com/skim-rs/skim/pull/586)
* @otto-dev made their first contribution in [#468](https://github.com/skim-rs/skim/pull/468)
* @jgarte made their first contribution in [#487](https://github.com/skim-rs/skim/pull/487)
* @iamb4uc made their first contribution in [#560](https://github.com/skim-rs/skim/pull/560)
* @hellux made their first contribution in [#563](https://github.com/skim-rs/skim/pull/563)
* @reneegyllensvaan made their first contribution in [#461](https://github.com/skim-rs/skim/pull/461)
* @jirutka made their first contribution in [#449](https://github.com/skim-rs/skim/pull/449)
* @rspencer01 made their first contribution in [#433](https://github.com/skim-rs/skim/pull/433)
* @marcoieni made their first contribution in [#382](https://github.com/skim-rs/skim/pull/382)
* @ymnejmi made their first contribution in [#551](https://github.com/skim-rs/skim/pull/551)
* @sisrfeng made their first contribution
* @vitaly-zdanevich made their first contribution

## [0.10.2] - 2022-11-08

### 🐛 Bug Fixes

- Print version from Cargo.toml with latest clap

### New Contributors
* @anthraxx made their first contribution

## [0.10.0] - 2022-10-28

### ⚙️ Miscellaneous Tasks

- Update deps and fix lots of clippy lints

### New Contributors
* @yazgoo made their first contribution in [#472](https://github.com/skim-rs/skim/pull/472)
* @EdenEast made their first contribution
* @grant0417 made their first contribution
* @mgttlinger made their first contribution
* @TD-Sky made their first contribution
* @dependabot[bot] made their first contribution
* @io12 made their first contribution
* @terror made their first contribution
* @PCouaillier made their first contribution
* @sweenu made their first contribution

## [0.9.4] - 2021-02-15

### 💼 Other

- Update

### ⚙️ Miscellaneous Tasks

- *(cargo)* Fix documentation link

### New Contributors
* @x4121 made their first contribution
* @Mephistophiles made their first contribution
* @n8henrie made their first contribution
* @marcusbuffett made their first contribution
* @mb720 made their first contribution
* @pickfire made their first contribution
* @sirwindfield made their first contribution

## [0.9.3] - 2020-11-02

### 🐛 Bug Fixes

- Ansi parse error for multi-bytes string

## [0.9.1] - 2020-10-20

### 🚀 Features

- Support initial scroll for preview window

### 🐛 Bug Fixes

- Ansi merge fragments (typo)
- Tiebreak should contains score by default
- Reduce flickering of preview window
- Multiple preview options won't merge
- Clippy
- Pre-select-items select '' by default
- Preview's scroll could be 0

## [0.9.0] - 2020-10-18

### 🚀 Features

- Unicode spinner
- Implement `--keep-right`
- Support skip-to-pattern

### 🐛 Bug Fixes

- Orderedvec won't preserve insertion order
- Upgrade fuzzy-matcher to fix wrong matching indices
- Ensure the matching range is within bound
- Some options are broken (introduced by 08bc067)
- Do no auto scroll for customized items
- Multiple selection (regression in 1d72fca)

### 💼 Other

- Ansi color were not shown for DefaultSkimItem

### 🚜 Refactor

- Demangle lib and bin implementations
- Separate MatchResult from MatchedItem

### New Contributors
* @pkubik made their first contribution
* @wucke13 made their first contribution

## [0.8.2] - 2020-06-26

### 🐛 Bug Fixes

- Preview's fields should based on orig text

### 💼 Other

- Move filter function to binary
- Exit gracefully on SIGPIPE error(see PR#279)
- Handle print0 parameters correctly in filter mode

### 🚜 Refactor

- DefaultSkimItem now accept string

### New Contributors
* @marsam made their first contribution
* @caixiangyue made their first contribution
* @emmanueltouzery made their first contribution
* @BlindingDark made their first contribution
* @aldhsu made their first contribution

## [0.8.0] - 2020-02-23

### 🚀 Features

- Support left click event on selection list

### 🐛 Bug Fixes

- Ensure screen is rendered with item

### 💼 Other

- "enter" key not printed with expect keys
- Support case insensitive in exact mode
- Case insensitive + refactor engine

## [0.7.0] - 2020-01-15

### 💼 Other

- *(src/ansi.rs)* Use pattern match to destruct Option wrapper.

### 📚 Documentation

- Add installation instructions for arch linux

### ⚙️ Miscellaneous Tasks

- Update derive_builder to 0.9

### New Contributors
* @ammgws made their first contribution
* @alexreg made their first contribution
* @cireu made their first contribution

## [0.6.7] - 2019-05-31

### 💼 Other

- Use as a library: remove extraneous line in example code.
- Remove extraneous line.
- Remove extraneous line.
- Add crates.io svg.

### New Contributors
* @chmp made their first contribution
* @ngirard made their first contribution

## [0.6.5] - 2019-04-01

### 🐛 Bug Fixes

- Wrong matches on empty lines

## [0.6.3] - 2019-03-25

### 🐛 Bug Fixes

- Number of matched items not show correctly
- Matcher is slow to kill

## [0.6.2] - 2019-03-19

### 🚀 Features

- Header-lines

### 🐛 Bug Fixes

- Compilation error of examples

## [0.6.0] - 2019-03-17

### 💼 Other

- Rotate mode

## [0.5.3] - 2019-02-20

### 💼 Other

- Create new variable for lines used by skim
- Update usage string.
- Return slice instead of new vector
- Draw status after query
- Return early if possible

### New Contributors
* @dfreese made their first contribution
* @lilydjwg made their first contribution
* @RemiliaForever made their first contribution
* @bennyyip made their first contribution
* @Konfekt made their first contribution
* @Lompik made their first contribution
* @light4 made their first contribution

## [0.3.0] - 2017-09-21

### 🐛 Bug Fixes

- Main window did not earse correctly
- Some lines now shown if too long
- Skim cannot show empty lines
- Alternate screen is not switched off on exit
- Ansi color not shown correctly in main area
- Toggle will panic if there is no item matched

### New Contributors
* @tiziano88 made their first contribution
* @supermarin made their first contribution

## [0.2.1-beta.2] - 2017-01-19

### 🚜 Refactor

- Use filter_map instead of map then filter

### New Contributors
* @anchepiece made their first contribution
* @brookst made their first contribution
* @SirVer made their first contribution
* @akiradeveloper made their first contribution

## [0.2.0] - 2017-01-03

### 🐛 Bug Fixes

- Model will not redraw from the 1 line
- Reader: reader and sender will lock each other.

### New Contributors
* @leoyvens made their first contribution
* @mohamedhayibor made their first contribution

## [0.1.1-rc2] - 2016-07-19

### 🐛 Bug Fixes

- #4 exit with non-zero status on cancel.
- Fields result in incorrect output with ANSI enabled.

### 💼 Other

- Remove debug code

## [0.1-alpha] - 2016-07-01

### New Contributors
* @lotabout made their first contribution
* @ made their first contribution

<!-- generated by git-cliff -->
