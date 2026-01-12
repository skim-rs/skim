<p align="center">
  <a href="https://crates.io/crates/skim">
    <img src="https://img.shields.io/crates/v/skim.svg" alt="Crates.io" />
  </a>
  <a href="https://github.com/skim-rs/skim/actions?query=workflow%3A%22Build+%26+Test%22+event%3Apush">
    <img src="https://github.com/skim-rs/skim/actions/workflows/test.yml/badge.svg?event=push" alt="Build & Test" />
  </a>
  <a href="https://repology.org/project/skim-fuzzy-finder/versions">
    <img src="https://repology.org/badge/tiny-repos/skim-fuzzy-finder.svg" alt="Packaging status" />
  </a>
  <a href="https://discord.gg/23PuxttufP">
    <img alt="Skim Discord" src="https://img.shields.io/discord/1031830957432504361?label=&color=7389d8&labelColor=6a7ec2&logoColor=ffffff&logo=discord" />
  </a>
  <a href="https://ratatui.rs">
    <img alt="Built with Ratatui" src="https://ratatui.rs/built-with-ratatui/badge.svg" />
  </a>
</p>

> Life is short, skim!

We spend so much of our time navigating through files, lines, and commands. That's where Skim comes in!
It's a powerful fuzzy finder designed to make your workflow faster and more efficient.

[![skim demo](https://asciinema.org/a/pIfwazaM0mTHA8F7qRbjrqOnm.svg)](https://asciinema.org/a/pIfwazaM0mTHA8F7qRbjrqOnm)

Skim provides a single executable called `sk`. Think of it as a smarter alternative to tools like
`grep` - once you try it, you'll wonder how you ever lived without it!

# Table of contents

- [Installation](#installation)
   * [Package Managers](#package-managers)
   * [Manually](#manually)
- [Usage](#usage)
   * [As Vim plugin](#as-vim-plugin)
   * [As filter](#as-filter)
   * [As Interactive Interface](#as-interactive-interface)
   * [Shell Bindings](#shell-bindings)
   * [Key Bindings](#key-bindings)
   * [Search Syntax](#search-syntax)
   * [exit code](#exit-code)
- [Tools compatible with `skim`](#tools-compatible-with-skim)
   * [fzf-lua neovim plugin](#fzf-lua-neovim-plugin)
   * [nu_plugin_skim](#nu_plugin_skim)
- [Customization](#customization)
   * [Keymap](#keymap)
   * [Sort Criteria](#sort-criteria)
   * [Color Scheme](#color-scheme)
   * [Misc](#misc)
- [Advanced Topics](#advanced-topics)
   * [Interactive mode](#interactive-mode)
      + [How does it work?](#how-does-it-work)
   * [Executing external programs](#executing-external-programs)
   * [Preview Window](#preview-window)
      + [How does it work?](#how-does-it-work-1)
   * [Fields support](#fields-support)
   * [Use as a library](#use-as-a-library)
- [FAQ](#faq)
   * [How to ignore files?](#how-to-ignore-files)
   * [Some files are not shown in Vim plugin](#some-files-are-not-shown-in-vim-plugin)
- [Differences from fzf](#differences-from-fzf)
- [How to contribute](#how-to-contribute)
- [Troubleshooting](#troubleshooting)
   * [No line feed issues with nix, FreeBSD, termux](#no-line-feed-issues-with-nix-freebsd-termux)

# Installation

The skim project contains several components:

1. `sk` executable - the core program
2. Vim/Nvim plugin - to call `sk` inside Vim/Nvim. Check [skim.vim](https://github.com/skim-rs/skim/blob/master/plugin/skim.vim) for Vim support.

## Package Managers

| OS             | Package Manager   | Command                      |
| -------------- | ----------------- | ---------------------------- |
| macOS          | Homebrew          | `brew install sk`            |
| macOS          | MacPorts          | `sudo port install skim`     |
| Alpine         | apk               | `apk add skim`               |
| Arch           | pacman            | `pacman -S skim`             |
| Gentoo         | Portage           | `emerge --ask app-misc/skim` |
| Guix           | guix              | `guix install skim`          |
| Void           | XBPS              | `xbps-install -S skim`       |

<a href="https://repology.org/project/skim-fuzzy-finder/versions">
    <img src="https://repology.org/badge/vertical-allrepos/skim-fuzzy-finder.svg?columns=4" alt="Packaging status">
</a>

## Manually

Any of the following applies:

- Using Git
    ```sh
    $ git clone --depth 1 git@github.com:skim-rs/skim.git ~/.skim
    $ ~/.skim/install
    ```
- Using Binary: Simply [download the sk executable](https://github.com/skim-rs/skim/releases) directly.
- Install from [crates.io](https://crates.io/): Run `cargo install skim`
- Build Manually:
    ```sh
    $ git clone --depth 1 git@github.com:skim-rs/skim.git ~/.skim
    $ cd ~/.skim
    $ cargo install
    $ cargo build --release
    $ # Add the resulting `target/release/sk` executable to your PATH
    ```

# Usage

Skim can be used either as a general filter (similar to `grep`) or as an interactive
interface for running commands.

## As Vim plugin

Via vim-plug (recommended):

Install skim, then :

```vim
Plug 'skim-rs/skim'
```


## As filter

Here are some examples to get you started:

```bash
# directly invoke skim
sk

# Or pipe some input to it (press TAB key to select multiple items when -m is enabled)
vim $(find . -name "*.rs" | sk -m)
```
This last command lets you select files with the ".rs" extension and opens
your selections in Vim - a great time-saver for developers!

## As Interactive Interface

`skim` can invoke other commands dynamically. Normally you would want to
integrate it with [grep](https://www.gnu.org/software/grep/),
[ack](https://github.com/petdance/ack2),
[ag](https://github.com/ggreer/the_silver_searcher), or
[rg](https://github.com/BurntSushi/ripgrep) for searching contents in a
project directory:

```sh
# works with grep
sk --ansi -i -c 'grep -rI --color=always --line-number "{}" .'
# works with ack
sk --ansi -i -c 'ack --color "{}"'
# works with ag
sk --ansi -i -c 'ag --color "{}"'
# works with rg
sk --ansi -i -c 'rg --color=always --line-number "{}"'
```

> **Note**: In these examples, `{}` will be literally expanded to the current input query.
> This means these examples will search for the exact query string, not fuzzily.
> For fuzzy searching, pipe the command output into `sk` without using interactive mode.

![interactive mode demo](https://cloud.githubusercontent.com/assets/1527040/21603930/655d859a-d1db-11e6-9fec-c25099d30a12.gif)

## Shell Bindings

Bindings for Fish, Bash and Zsh are available in the `shell` directory:
- `completion.{shell}` contains the completion scripts for `sk` cli usage
- `key-bindings.{shell}` contains key-binds and shell integrations:
    - `ctrl-t` to select a file through `sk`
    - `ctrl-r` to select an history entry through `sk`
    - `alt-c`  to `cd` into a directory selected through `sk`
    - (not available in `fish`) `**` to complete file paths, for example `ls **<tab>` will show a `sk` widget to select a folder

To enable these features, source the `key-bindings.{shell}` file and set up completions according to your shell's documentation or see below.

### Shell Completions

You can generate shell completions for your preferred shell using the `--shell` flag with one of the supported shells: `bash`, `zsh`, `fish`, `powershell`, or `elvish`:

> **Note:** While PowerShell completions are supported, Windows is not supported for now.

#### Option 1: Source directly in your current shell session

```sh
# For bash
source <(sk --shell bash)

# For zsh
source <(sk --shell zsh)

# For fish
sk --shell fish | source
```

#### Option 2: Save to a file to be loaded automatically on shell startup

```sh
# For bash, add to ~/.bashrc
echo 'source <(sk --shell bash)' >> ~/.bashrc  # Or save to ~/.bash_completion

# For zsh, add to ~/.zshrc
sk --shell zsh > ~/.zfunc/_sk  # Create ~/.zfunc directory and add to fpath in ~/.zshrc

# For fish, add to ~/.config/fish/completions/
sk --shell fish > ~/.config/fish/completions/sk.fish
```

## Key Bindings

Some commonly used key bindings:

| Key               | Action                                     |
|------------------:|--------------------------------------------|
| Enter             | Accept (select current one and quit)       |
| ESC/Ctrl-G        | Abort                                      |
| Ctrl-P/Up         | Move cursor up                             |
| Ctrl-N/Down       | Move cursor Down                           |
| TAB               | Toggle selection and move down (with `-m`) |
| Shift-TAB         | Toggle selection and move up (with `-m`)   |

For a complete list of key bindings, refer to the [man
page](https://github.com/skim-rs/skim/blob/master/man/man1/sk.1) (`man sk`).

## Search Syntax

`skim` borrows `fzf`'s syntax for matching items:

| Token    | Match type                 | Description                       |
|----------|----------------------------|-----------------------------------|
| `text`   | fuzzy-match                | items that match `text`           |
| `^music` | prefix-exact-match         | items that start with `music`     |
| `.mp3$`  | suffix-exact-match         | items that end with `.mp3`        |
| `'wild`  | exact-match (quoted)       | items that include `wild`         |
| `!fire`  | inverse-exact-match        | items that do not include `fire`  |
| `!.mp3$` | inverse-suffix-exact-match | items that do not end with `.mp3` |

`skim` also supports the combination of tokens.

- Whitespace has the meaning of `AND`. With the term `src main`, `skim` will search
    for items that match **both** `src` and `main`.
- ` | ` means `OR` (note the spaces around `|`). With the term `.md$ |
    .markdown$`, `skim` will search for items ends with either `.md` or
    `.markdown`.
- `OR` has higher precedence. For example, `readme .md$ | .markdown$` is interpreted as
    `readme AND (.md$ OR .markdown$)`.

If you prefer using regular expressions, `skim` offers a `regex` mode:

```sh
sk --regex
```

You can switch to `regex` mode dynamically by pressing `Ctrl-R` (Rotate Mode).

## exit code

| Exit Code | Meaning                             |
|-----------|-------------------------------------|
| 0         | Exited normally                     |
| 1         | No Match found                      |
| 130       | Aborted by Ctrl-C/Ctrl-G/ESC/etc... |

# Tools compatible with `skim`

These tools are or aim to be compatible with `skim`:

## [fzf-lua neovim plugin](https://github.com/ibhagwan/fzf-lua)

A [neovim](https://neovim.io) plugin allowing fzf and skim to be used in a to navigate your code.

Install it with your package manager, following the README. For instance, with `lazy.nvim`:

```lua
{
  "ibhagwan/fzf-lua",
  -- enable `sk` support instead of the default `fzf`
  opts = {'skim'}
}
```

## [nu_plugin_skim](https://github.com/idanarye/nu_plugin_skim)

A [nushell](https://www.nushell.sh/) plugin to allow for better interaction between skim and nushell.

Following the instruction in the plugin's README, you can install it with cargo:
```nu
cargo install nu_plugin_skim
plugin add ~/.cargo/bin/nu_plugin_skim
```

# Customization

The doc here is only a preview, please check the man page (`man sk`) for a full
list of options.

## Keymap

Specify the bindings with comma separated pairs (no space allowed). For example:

```sh
sk --bind 'alt-a:select-all,alt-d:deselect-all'
```

Additionally, use `+` to concatenate actions, such as `execute-silent(echo {} | pbcopy)+abort`.

See the _KEY BINDINGS_ section of the man page for details.

## Sort Criteria

There are five sort keys for results: `score, index, begin, end, length`. You can
specify how the records are sorted by `sk --tiebreak score,index,-begin` or any
other order you want.

## Color Scheme

You probably have your own aesthetic preferences! Fortunately, you aren't
limited to the default appearance - Skim supports comprehensive customization of its color scheme.

```sh
--color=[BASE_SCHEME][,COLOR:ANSI]
```

Skim also respects the `NO_COLOR` environment variable. Set it to anything and `sk` (and many other terminal apps) will disable all colored output. See [no-color.org](https://no-color.org/) for more details.

### Available Base Color Schemes

Skim comes with several built-in color schemes that you can use as a starting point:

```sh
sk --color=dark      # Default dark theme (256 colors)
sk --color=light     # Light theme (256 colors)
sk --color=16        # Simple 16-color theme
sk --color=bw        # Minimal black & white theme (no colors, just styles)
sk --color=none      # Minimal black & white theme (no colors, no styles)
sk --color=molokai   # Molokai-inspired theme (256 colors)
```

### Customizing Colors

You can customize individual UI elements by specifying color values after the base scheme:

```sh
sk --color=light,fg:232,bg:255,current_bg:116,info:27
```

Colors can be specified in several ways:

- ANSI colors (0-255): `sk --color=fg:232,bg:255`
- RGB hex values: `sk --color=fg:#FF0000` (red text)

### Available Color Customization Options

The following UI elements can be customized:

| Element            | Description                                 | Example                  |
|--------------------|---------------------------------------------|-------------------------|
| `fg`               | Normal text foreground color                | `--color=fg:232`        |
| `bg`               | Normal text background color                | `--color=bg:255`        |
| `matched`          | Matched text in search results              | `--color=matched:108`   |
| `matched_bg`       | Background of matched text                  | `--color=matched_bg:0`  |
| `current`          | Current line foreground color               | `--color=current:254`   |
| `current_bg`       | Current line background color               | `--color=current_bg:236`|
| `current_match`    | Matched text in current line                | `--color=current_match:151` |
| `current_match_bg` | Background of matched text in current line  | `--color=current_match_bg:236` |
| `spinner`          | Progress indicator color                     | `--color=spinner:148`   |
| `info`             | Information line color                      | `--color=info:144`      |
| `prompt`           | Prompt color                                | `--color=prompt:110`    |
| `cursor`           | Cursor color                                | `--color=cursor:161`    |
| `selected`         | Selected item marker color                  | `--color=selected:168`  |
| `header`           | Header text color                           | `--color=header:109`    |
| `border`           | Border color for preview/layout             | `--color=border:59`     |

### Examples

```sh
# Use light theme but change the current line background
sk --color=light,current_bg:24

# Custom theme with multiple colors
sk --color=dark,matched:#00FF00,current:#FFFFFF,current_bg:#000080

# High contrast theme
sk --color=fg:232,bg:255,matched:160,current:255,current_bg:20
```

For more details, check the man page (`man sk`).

## Misc

- `--ansi`: to parse ANSI color codes (e.g., `\e[32mABC`) of the data source
- `--regex`: use the query as regular expression to match the data source

# Advanced Topics

## Interactive mode

In **interactive mode**, you can invoke a command dynamically. Try it out:

```sh
sk --ansi -i -c 'rg --color=always --line-number "{}"'
```

### How does it work?

![How Skim's interactive mode works](https://user-images.githubusercontent.com/1527040/53381293-461ce380-39ab-11e9-8e86-7c3bbfd557bc.png)

- Skim  accepts two kinds of sources: Command output or piped input
- Skim has two kinds of prompts: A query prompt to specify the query pattern and a
    command prompt to specify the "arguments" of the command
- `-c` is used to specify the command to execute and defaults to `SKIM_DEFAULT_COMMAND`
- `-i` tells skim to open command prompt on startup, which will show `c>` by default.

To further narrow down the results returned by the command, press
`Ctrl-Q` to toggle interactive mode.

## Executing external programs

You can configure key bindings to start external processes without leaving Skim (`execute`, `execute-silent`).

```sh
# Press F1 to open the file with less without leaving skim
# Press CTRL-Y to copy the line to clipboard and aborts skim (requires pbcopy)
sk --bind 'f1:execute(less -f {}),ctrl-y:execute-silent(echo {} | pbcopy)+abort'
```

## Preview Window

This is a great feature of fzf that skim borrows. For example, we use 'ag' to
find the matched lines, and once we narrow down to the target lines, we want to
finally decide which lines to pick by checking the context around the line.
`grep` and `ag` have the option `--context`, and skim can make use of `--context` for
a better preview window. For example:

```sh
sk --ansi -i -c 'ag --color "{}"' --preview "preview.sh {}"
```

(Note that [preview.sh](https://github.com/junegunn/fzf.vim/blob/master/bin/preview.sh) is a script to print the context given filename:lines:columns)

You get things like this:

![preview demo](https://user-images.githubusercontent.com/1527040/30677573-0cee622e-9ebf-11e7-8316-c741324ecb3a.png)

### How does it work?

If the preview command is given by the `--preview` option, skim will replace the
`{}` with the current highlighted line surrounded by single quotes, call the
command to get the output, and print the output on the preview window.

Sometimes you don't need the whole line for invoking the command. In this case
you can use `{}`, `{1..}`, `{..3}` or `{1..5}` to select the fields. The
syntax is explained in the section [Fields Support](#filds-support).

Lastly, you might want to configure the position of preview window with `--preview-window`:
- `--preview-window up:30%` to put the window in the up position with height
    30% of the total height of skim.
- `--preview-window left:10:wrap` to specify the `wrap` allows the preview
    window to wrap the output of the preview command.
- `--preview-window wrap:hidden` to hide the preview window at startup, later
    it can be shown by the action `toggle-preview`.

## Fields support

Normally only plugin users need to understand this.

For example, you have the data source with the format:

```sh
<filename>:<line number>:<column number>
```

However, you want to search `<filename>` only when typing in queries. That
means when you type `21`, you want to find a `<filename>` that contains `21`,
but not matching line number or column number.

You can use `sk --delimiter ':' --nth 1` to achieve this.

You can also use `--with-nth` to re-arrange the order of fields.

**Range Syntax**

- `<num>` -- to specify the `num`-th fields, starting with 1.
- `start..` -- starting from the `start`-th fields and the rest.
- `..end` -- starting from the `0`-th field, all the way to `end`-th field,
    including `end`.
- `start..end` -- starting from `start`-th field, all the way to `end`-th
    field, including `end`.

## Use as a library

Skim can be used as a library in your Rust crates.

First, add skim into your `Cargo.toml`:

```toml
[dependencies]
skim = "*"
```

Then try to run this simple example:

```rust
extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

pub fn main() {
    let options = SkimOptionsBuilder::default()
        .height(String::from("50%"))
        .multi(true)
        .build()
        .unwrap();

    let input = "aaaaa\nbbbb\nccc".to_string();

    // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
    // `SkimItem` was implemented for `AsRef<str>` by default
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    // `run_with` would read and show items from the stream
    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(|| Vec::new());

    for item in selected_items.iter() {
        println!("{}", item.output());
    }
}
```

Given an `Option<SkimItemReceiver>`, skim will read items accordingly, do its
job and bring us back the user selection including the selected items, the
query, etc. Note that:

- `SkimItemReceiver` is `crossbeam::channel::Receiver<Arc<dyn SkimItem>>`
- If it is none, it will invoke the given command and read items from command output
- Otherwise, it will read the items from the (crossbeam) channel.

Trait `SkimItem` is provided to customize how a line could be displayed,
compared and previewed. It is implemented by default for `AsRef<str>`

Plus, `SkimItemReader` is a helper to convert a `BufRead` into
`SkimItemReceiver` (we can easily turn a `File` or `String` into `BufRead`),
so that you could deal with strings or files easily.

Check out more examples under the [examples/](https://github.com/skim-rs/skim/tree/master/skim/examples) directory.

# FAQ

## How to ignore files?

Skim invokes `find .` to fetch a list of files for filtering. You can override
this by setting the environment variable `SKIM_DEFAULT_COMMAND`. For example:

```sh
$ SKIM_DEFAULT_COMMAND="fd --type f || git ls-tree -r --name-only HEAD || rg --files || find ."
$ sk
```

You could put it in your `.bashrc` or `.zshrc` if you like it to be default.

## Some files are not shown in Vim plugin

If you use the Vim plugin and execute the `:SK` command, you may find some
of your files not shown.

As described in [#3](https://github.com/skim-rs/skim/issues/3), in the Vim
plugin, `SKIM_DEFAULT_COMMAND` is set to the command by default:

```vim
let $SKIM_DEFAULT_COMMAND = "git ls-tree -r --name-only HEAD || rg --files || ag -l -g \"\" || find ."
```

This means files not recognized by git won't be shown. You can either override the
default with `let $SKIM_DEFAULT_COMMAND = ''` or locate the missing files by
yourself.

# Differences from fzf

[fzf](https://github.com/junegunn/fzf) is a command-line fuzzy finder written
in Go and [skim](https://github.com/skim-rs/skim) tries to implement a new one
in Rust!

This project is written from scratch. Some decisions of implementation are
different from fzf. For example:

1. `skim` has an interactive mode.
2. `skim` supports pre-selection.
3. The fuzzy search algorithm is different.

More generally, `skim`'s maintainers allow themselves some freedom of implementation.
The goal is to keep `skim` as feature-full as `fzf` is, but the command flags might differ.

# How to contribute

[Create new issues](https://github.com/skim-rs/skim/issues/new) if you encounter any bugs
or have any ideas. Pull requests are warmly welcomed.

# Troubleshooting

To troubleshoot what's happening, you can set the environment variable `RUST_LOG` to either `debug` or even `trace`, and set `--log-file` to a path. You can then read those logs during or after the execution to better understand what's happening. Don't hesitate to add those logs to an issue if you need help.

## No line feed issues with nix, FreeBSD, termux

If you encounter display issues like:

```bash
$ for n in {1..10}; do echo "$n"; done | sk
  0/10 0/0.> 10/10  10  9  8  7  6  5  4  3  2> 1
```

For example

- https://github.com/skim-rs/skim/issues/412
- https://github.com/skim-rs/skim/issues/455

You need to set TERMINFO or TERMINFO_DIRS to the path of a correct terminfo database path

For example, with termux, you can add this in your bashrc:

```
export TERMINFO=/data/data/com.termux/files/usr/share/terminfo
```
