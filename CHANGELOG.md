# Changelog

## [Unreleased]
## [0.19.0](https://github.com/skim-rs/skim/compare/v0.18.0...v0.19.0)

### ⛰️ Features


- Add min query length option ([#806](https://github.com/skim-rs/skim/pull/806)) - ([71b82d0](https://github.com/skim-rs/skim/commit/71b82d0f58f96788e509b5af5b92e292dadf4dd3)) (by @LoricAndre)

### ⚙️ Miscellaneous Tasks


- Update Cargo.toml dependencies - ([0000000](https://github.com/skim-rs/skim/commit/0000000))

### Contributors

* @LoricAndre
## [0.18.0](https://github.com/skim-rs/skim/compare/v0.17.3...v0.18.0)

### 🚀 Features

- *(shell)* Improve shell completion with dynamic generation (#790)

### 🐛 Bug Fixes

- *(ci)* Remove version from pr name

### 📚 Documentation

- *(contributing)* Refine guidelines for GPT-assisted development
- Improve theming documentation (#788)
- Improve wording in README and options.rs (#789)

## [0.17.3] - 2025-05-20

### 🐛 Bug Fixes

- *(shell)* Fix zsh tmux args in key bindings (#777)
- *(shell)* Remove duplocate tmux height arg fixes #776 (#778)

### 💼 Other

- Set keybinding right before printing special character (#774)

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

## [0.15.7] - 2024-12-27

### 🐛 Bug Fixes

- Remove atty (#671)

### ⚙️ Miscellaneous Tasks

- Release master (#670)

## [0.15.6] - 2024-12-26

### 🐛 Bug Fixes

- Fix non-functional vim plugin (#659)
- Update rank to follow the readded index tiebreak (#669)

### ⚙️ Miscellaneous Tasks

- Release master (#656)

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

## [0.15.1] - 2024-12-01

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
- Fix ci
- Fix urls in cargo.toml

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

## [0.14.0] - 2024-11-28

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

## [0.10.2] - 2022-11-08

### 🐛 Bug Fixes

- Print version from Cargo.toml with latest clap

## [0.10.0] - 2022-10-28

### ⚙️ Miscellaneous Tasks

- Update deps and fix lots of clippy lints

## [0.9.4] - 2021-02-15

### 💼 Other

- Update

### ⚙️ Miscellaneous Tasks

- *(cargo)* Fix documentation link

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

## [0.8.2] - 2020-06-26

### 🐛 Bug Fixes

- Preview's fields should based on orig text

### 💼 Other

- Move filter function to binary
- Exit gracefully on SIGPIPE error(see PR#279)
- Handle print0 parameters correctly in filter mode

### 🚜 Refactor

- DefaultSkimItem now accept string

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

## [0.6.7] - 2019-05-31

### 💼 Other

- Use as a library: remove extraneous line in example code.
- Remove extraneous line.
- Remove extraneous line.
- Add crates.io svg.

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

## [0.3.0] - 2017-09-21

### 🐛 Bug Fixes

- Main window did not earse correctly
- Some lines now shown if too long
- Skim cannot show empty lines
- Alternate screen is not switched off on exit
- Ansi color not shown correctly in main area
- Toggle will panic if there is no item matched

## [0.2.1-beta.2] - 2017-01-19

### 🚜 Refactor

- Use filter_map instead of map then filter

## [0.2.0] - 2017-01-03

### 🐛 Bug Fixes

- Model will not redraw from the 1 line
- Reader: reader and sender will lock each other.

## [0.1.1-rc2] - 2016-07-19

### 🐛 Bug Fixes

- #4 exit with non-zero status on cancel.
- Fields result in incorrect output with ANSI enabled.

### 💼 Other

- Remove debug code

<!-- generated by git-cliff -->
