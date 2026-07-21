//! Provides what's needed to generate skim's man page
use std::io::Write;

use clap::CommandFactory;
use clap_mangen::Man;
use eyre::Result;
use roff::{Inline, Roff};

use crate::SkimOptions;

const THEME_SECTION: &str = "
Available themes:
    * none: base color scheme
    * molokai: molokai 256color
    * light: light 256color
    * 16: dark base16 theme
    * bw: black & white theme
    * dark | default: dark 256color, default value
    * all 4 catppuccin variants:
        * catppuccin-latte
        * catppuccin-macchiato
        * catppuccin-frappe
        * catppuccin-mocha

Available color names:
    * normal (or empty string): normal text
    * matched (or hl): matched text
    * current (or fg+): current line foreground
    * bg+: current line background (special case, always sets background)
    * current_match (or hl+): matched text in current line
    * query: query text
    * spinner: spinner character
    * info: info text (match count)
    * prompt: prompt text
    * cursor (or pointer): cursor/pointer
    * selected (or marker): selected item marker
    * header: header text
    * border: border lines
    * scrollbar: item list scrollbar thumb

Adding `-fg`, `_fg`, `-bg`, `_bg`, `-underline`, `_underline` sets the corresponding part of
the color. For instance, `normal-fg` (or simply `fg`) will set the foreground normal color.

Color formats:
    * 0-255: ANSI terminal color
    * #rrggbb: 24-bit color

Available attrs:
    * x | regular: resets the modifiers, use it before the others
    * b | bold
    * u | underline
    * c | crossed-out
    * d | dim
    * i | italic
    * r | reverse

Example: `--color '16,normal-fg:0+bold,matched-fg:#ffffff+u,cursor-bg:#deadbe'` will start with the
 base 16 theme and override it with a bold ANSI color 0 foreground (black), a hex ffffff (full
 white) underlined foreground for matched parts and a #deadbe (pale rose, apparently) cursor background.
";

const EXIT_CODES_SECTION: &str = "
* 0: success
* 1: no match
* 130: interrupt (ctrl-c or esc)
* others: error
";

const NORMAL_MODE_SS: &str = "
In normal mode, sk reads the input from stdin and displays the results interactively,
and the query is then used to fuzzily filter among the input lines.
";

const INTERACTIVE_MODE_SS: &str = "
Interactive mode is a special mode that allows you to run a command interactively and display
the results. It is enabled by the `--interactive` (or `-i`) option or by binding the
`toggle-interactive` action (default: <ctrl-q>).
The command is specified with the `--cmd` option.

Example: `sk --cmd 'rg {} --color=always' --interactive` will use `rg` to search for the query
in the current directory and display the results interactively.
";

const KEYS_SS: &str = "
* ctrl-[a-z]
* ctrl-space
* ctrl-alt-[a-z]
* alt-[a-zA-Z]
* alt-[0-9]
* f[1-12]
* enter
* space
* bspace      (bs)
* alt-up
* alt-down
* alt-left
* alt-right
* alt-enter
* alt-space
* alt-bspace  (alt-bs)
* alt-/
* tab
* btab        (shift-tab)
* esc
* del
* up
* down
* left
* right
* home
* end
* pgup
* pgdn
* shift-up
* shift-down
* shift-left
* shift-right
* alt-shift-up
* alt-shift-down
* alt-shift-left
* alt-shift-right
* any single character
";
const BINDABLE_EVENTS_SS: &str = concat!(
    "\n",
    "* change: the query changes\n",
    "* start: skim enters its event loop; fired once\n",
    "* load: the reader and matcher finish consuming the current input; ",
    "fired once per read, including reloads\n",
    "* result: filtering for the current query completes\n",
    "* focus: the focused item changes because of cursor movement or a result update\n",
    "* zero: the input stream is complete and the final search has no matches\n",
    "* one: the input stream is complete and the final search has exactly one match\n",
);

const ACTION_BINDINGS_SS: &str = concat!(
    "\n",
    "Actions can also be used as binding triggers. A follow-up chain bound to an action name runs immediately ",
    "after that action. Use the `act-` prefix for action triggers; it is recommended to avoid ambiguity and ",
    "required when the action name is also a key, for example `act-up:last`.\n\n",
    "Follow-up chains use non-recursive (`noremap`) semantics: their actions do not trigger further action ",
    "bindings. Add `suppress` to skip the triggering action's default behavior, for example ",
    "`act-up:suppress+down`.\n",
);

const ACTIONS_SS: &str = concat!(
    "\n* abort: ctrl-c  ctrl-q  esc
* accept(...): enter *the argument will be printed when the binding is triggered*
* append-and-select
* backward-char: ctrl-b  left
* backward-delete-char: ctrl-h  bspace
* backward-delete-char/eof
* backward-kill-word: alt-bs
* backward-word: alt-b   shift-left
* beginning-of-line: ctrl-a  home
* bind(...): *arg is a comma-separated list of `trigger:action[+action]` bindings to add (same syntax as --bind, including action triggers such as `act-up:last`)
* clear-screen: ctrl-l
* delete-char: del
* delete-char/eof: ctrl-d
* deselect-all
* down: ctrl-j  ctrl-n  down
* end-of-line: ctrl-e  end
* execute(...): *arg will be a command, see COMMAND EXPANSION for details
* execute-silent(...): *arg will be a command, see COMMAND EXPANSION for details
* forward-char: ctrl-f  right
* forward-word: alt-f   shift-right
* if-non-matched
* if-query-empty
* if-query-not-empty
* ignore
* kill-line
* kill-word: alt-d
* next-history: ctrl-n with `--history` or `--cmd-history`
* page-down: pgdn
* page-up: pgup
* half-page-down
* half-page-up
* preview-up: shift-up
* preview-down: shift-down
* preview-left
* preview-right
* preview-page-down
* preview-page-up
* previous-history: ctrl-p with `--history` or `--cmd-history`
* redraw
* refresh-cmd
* refresh-preview
* reload(...)
* select-all
* select-row
* set-preview-cmd(...): *arg will be a expanded expression, see COMMAND EXPANSION for details
* set-query(...): *arg will be a expanded expression, see COMMAND EXPANSION for details
",
    "* suppress: *if bound to an action (e.g. `act-up:suppress`), suppresses that action's default behavior ",
    "so the rest of the non-recursive chain runs once in its place; if bound to a key, equivalent to `ignore`\n",
    "* toggle
* toggle-all
* toggle+down: ctrl-i  tab
* toggle-in: (--layout=reverse ? toggle+up:  toggle+down)
* toggle-interactive
* toggle-out: (--layout=reverse ? toggle+down:  toggle+up)
* toggle-preview
* toggle-preview-wrap
* toggle-sort
* toggle+up: btab    shift-tab
* top
* unbind(...): *arg is a comma-separated list of keys or action triggers (e.g. `act-up`) to unbind
* unix-line-discard: ctrl-u
* unix-word-rubout: ctrl-w
* up: ctrl-k  ctrl-p  up
* yank: ctrl-y
",
);

#[cfg(feature = "listen")]
const REMOTE_SECTION: &str = "
skim can be controlled from other processes, using the `--listen` (and optionally `--remote`) flags.

To achieve this, run the server instance using `sk --listen optional_address` (the address defaults to `sk`).
It will then start listening on a named socket for instructions.

To send instructions, you can use `sk --remote optional_address` or any other tool that allows us to interact with such sockets,
such as `socat` on linux: `echo 'ToggleIn' | socat -u STDIN ABSTRACT-CONNECT:optional_address`. Instructions correspond to skim's Actions and need to be sent in Ron format.
When using `sk --remote`, pipe in action chains (see the KEYBINDS section), for instance `echo 'up+select-row' | sk --remote optional_address`
";

fn parse_str(src: &str) -> Vec<Inline> {
    let mut res = Vec::new();
    for line in src.lines() {
        res.push(Inline::Roman(line.to_string()));
        res.push(Inline::LineBreak);
    }
    res
}

fn section(c: &mut Roff, name: &str, content: &str) {
    c.control("SH", [name]);
    c.text(parse_str(content));
}

fn subsection(c: &mut Roff, name: &str, content: &str) {
    c.control("SS", [name]);
    c.text(parse_str(content));
}

/// Generate skim's manpage and write it to the writer
///
/// # Errors
///
/// Returns an error if writing to the output fails.
// The manpage generator writes many sections sequentially; splitting it would add
// indirection without meaningful simplification.
#[allow(clippy::too_many_lines)]
pub fn generate<W>(w: &mut W) -> Result<()>
where
    W: Write,
{
    let base = Man::new(SkimOptions::command());
    let mut custom = Roff::default();

    // Render normal sections, as would mangen do
    base.render_title(w)?;
    base.render_name_section(w)?;
    base.render_synopsis_section(w)?;

    // Render options
    base.render_options_section(w)?;

    // Add custom sections
    section(&mut custom, "MODES", "");
    subsection(&mut custom, "Normal mode", NORMAL_MODE_SS);
    subsection(&mut custom, "Interactive mode", INTERACTIVE_MODE_SS);
    section(
        &mut custom,
        "SEARCH",
        "
By default, skim will start in `extended search`, giving some characters will have meaning.
Example: `^test rs$ | sh$` will match items starting with test and ending with rs or sh.
",
    );
    subsection(
        &mut custom,
        "AND (space)",
        "A space between terms will act as an 'and' operator and will filter for items matching all terms.",
    );
    subsection(
        &mut custom,
        "OR (|)",
        "A vertical bar between terms will act as an 'or' operator and will filter for items matching one of the terms.",
    );
    subsection(
        &mut custom,
        "Exact match (')",
        "
If a term is prefixed by a `'`, sk will search for exact occurrences of that term.
Exact search can be enabled by default by the `--exact` command-line flag. In exact mode, `'` will disable exact matching for that term.
",
    );
    subsection(
        &mut custom,
        "Anchored match (^|$)",
        "If a term is prefixed by a `^` (resp. suffixed by a `$`), sk will search for matches starting (resp. ending) with that exact term.",
    );
    subsection(
        &mut custom,
        "Negation (!)",
        "If a term is prefixed by `!`, sk will exclude the items that match this term.",
    );

    section(
        &mut custom,
        "KEYBINDS",
        concat!(
            "\nBindings can be set by the `--bind` option, which takes a comma-separated list of ",
            "`<trigger>:<action>[+action2]` expressions. A trigger can be a key, a finder event, or an action ",
            "name.\n",
            "Actions can take arguments, specified either between parentheses `reload(ls)` or after a colon ",
            "`reload:ls`.\n",
        ),
    );
    subsection(&mut custom, "Available keys (aliases in parentheses)", KEYS_SS);
    subsection(&mut custom, "Bindable finder events", BINDABLE_EVENTS_SS);
    subsection(&mut custom, "Actions as binding triggers", ACTION_BINDINGS_SS);
    subsection(&mut custom, "Actions[:default keys][*notes]", ACTIONS_SS);

    section(
        &mut custom,
        "COMMAND EXPANSION",
        "
In the `preview` flag, `execute`, `reload`, `set-query`... binds, sk will expand placeholders:
* {} (or --replstr if used) will be expanded to the current item.
* {q} (or {cq} for legacy reasons) will be expanded to the current query input.
* {+} will be expanded to either the currently selected items in multi-select mode, or the current
 item in single-select.
* {n} will be expanded to the index of the current item.
* {+n} will be expanded to the index(es) of the corresponding {+} item(s).
* {FIELD_INDEX_EXPRESSION} will be expanded to the field index expression run against the current
 item.
* {+FIELD_INDEX_EXPRESSION} will be expanded to the field index expression run against the {+}
 item(s).
",
    );
    subsection(
        &mut custom,
        "Field index expression",
        "
skim will expand some expressions between {..}.
It will expand to the corresponding fields, separated by the `--delimiter|-d` option (see there for details).
* `{n}` will be the n-th field.
* `{n..m}` will be the fields from n to m, inclusive, separated by a space
* `{-n}` will be the n-th, starting from the end, -1 will be the last field etc.
",
    );

    section(&mut custom, "ENVIRONMENT VARIABLES", "");
    subsection(
        &mut custom,
        "SKIM_DEFAULT_COMMAND",
        "
If set, skim will collect items with this command if no input is piped in.
If not set, defaults to `find .` on unix-like systems and `dir /s /b /A:-D` on Windows.",
    );
    subsection(
        &mut custom,
        "SKIM_DEFAULT_OPTIONS",
        "Will be parsed and used as default options. Example: `--reverse --multi`",
    );
    subsection(
        &mut custom,
        "SKIM_OPTIONS_FILE",
        "
If the variable is set to the path of an existing file, the contents of this file will be parsed and used as default options.
It supports `#` as a comment start, which can be escaped using `##`.
Example:
```
# Preview
--preview 'echo {}'
--preview-window 'left:30%' # Preview window
--reverse
--prompt '## '
```
",
    );

    subsection(
        &mut custom,
        "NO_COLOR",
        "If set to a non-empty value, will disable coloring",
    );

    section(&mut custom, "THEME", THEME_SECTION);

    #[cfg(feature = "listen")]
    section(&mut custom, "LISTEN/REMOTE", REMOTE_SECTION);

    section(&mut custom, "EXIT CODES", EXIT_CODES_SECTION);

    custom.to_writer(w)?;

    // Finish with mangen version section
    base.render_version_section(w)?;
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    fn manpage_str() -> String {
        let mut buf = Vec::new();
        generate(&mut buf).expect("manpage generation should not fail");
        String::from_utf8(buf).expect("manpage output is valid UTF-8")
    }

    #[test]
    fn manpage_contains_version() {
        let out = manpage_str();
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            out.contains(version),
            "manpage should contain package version {version}"
        );
    }

    #[test]
    fn manpage_contains_key_options() {
        let out = manpage_str();
        for flag in ["query", "multi", "preview", "bind", "color"] {
            assert!(out.contains(flag), "manpage should mention '--{flag}'");
        }
    }

    #[test]
    fn manpage_contains_sections() {
        let out = manpage_str();
        for section in ["MODES", "SEARCH", "KEYBINDS", "EXIT CODES"] {
            assert!(out.contains(section), "manpage should contain section '{section}'");
        }
    }

    #[test]
    fn manpage_documents_bindable_events_and_actions() {
        let out = manpage_str();
        for event in ["change", "start", "load", "result", "focus", "zero", "one"] {
            assert!(out.contains(event), "manpage should document the '{event}' event");
        }
        assert!(out.contains("Actions as binding triggers"));
        assert!(out.contains("act\\-"));
        assert!(out.contains("noremap"));
        assert!(out.contains("suppress"));
    }
}
