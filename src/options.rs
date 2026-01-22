//! Configuration options for skim.
//!
//! This module provides the `SkimOptions` struct and builder for configuring
//! all aspects of skim's behavior, including search, display, layout, and interaction settings.

use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use derive_builder::Builder;
use regex::Regex;

use crate::binds::KeyMap;
use crate::item::RankCriteria;
use crate::prelude::SkimItemReader;
use crate::reader::CommandCollector;
use crate::tui::PreviewCallback;
use crate::tui::event::Action;
use crate::tui::options::{PreviewLayout, TuiLayout};
use crate::tui::statusline::InfoDisplay;
use crate::util::read_file_lines;
use crate::{CaseMatching, FuzzyAlgorithm, Selector};

#[cfg(feature = "cli")]
/// Custom value parser for delimiter that handles escape sequences
fn parse_delimiter_value(s: &str) -> Result<Regex, String> {
    let unescaped = crate::util::unescape_delimiter(s);
    Regex::new(&unescaped).map_err(|e| format!("Invalid regex delimiter: {}", e))
}

/// sk - fuzzy finder in Rust
///
/// sk is a general purpose command-line fuzzy finder.
#[derive(Builder)]
#[builder(build_fn(name = "final_build"))]
#[builder(default)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
#[cfg_attr(
    feature = "cli",
    command(name = "sk", args_override_self = true, verbatim_doc_comment, version, about)
)]
pub struct SkimOptions {
    //  --- Search ---
    /// Show results in reverse order
    ///
    /// Often used in combination with --no-sort
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Search"))]
    pub tac: bool,

    /// Minimum query length to start showing results
    ///
    /// Only show results when the query is at least this many characters long
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Search"))]
    pub min_query_length: Option<usize>,

    /// Do not sort the results
    ///
    /// Often used in combination with --tac
    /// Example: `history | sk --tac --no-sort`
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Search"))]
    pub no_sort: bool,

    /// Comma-separated list of sort criteria to apply when the scores are tied.
    ///
    /// * **score**: Score of the fuzzy match algorithm
    ///
    ///     - Each criterion could be negated, e.g. (-index)
    ///     - Each criterion should appear only once in the list
    #[cfg_attr(
        feature = "cli",
        arg(
            short,
            long,
            default_value = "score,begin,end",
            value_enum,
            value_delimiter = ',',
            help_heading = "Search",
            allow_hyphen_values = true,
            verbatim_doc_comment
        )
    )]
    pub tiebreak: Vec<RankCriteria>,

    /// Fields to be matched
    ///
    /// A field index expression can be a non-zero integer or a range expression (`[BEGIN]..[END]`).
    /// `--nth` and `--with-nth` take a comma-separated list of field index expressions.
    ///
    /// **Examples:**
    ///     1      The 1st field
    ///     2      The 2nd field
    ///     -1     The last field
    ///     -2     The 2nd to last field
    ///     3..5   From the 3rd field to the 5th field
    ///     2..    From the 2nd field to the last field
    ///     ..-3   From the 1st field to the 3rd to the last field
    ///     ..     All the fields
    #[cfg_attr(
        feature = "cli",
        arg(
            short,
            long,
            default_value = "",
            help_heading = "Search",
            verbatim_doc_comment,
            value_delimiter = ',',
            allow_hyphen_values = true,
        )
    )]
    pub nth: Vec<String>,

    /// Fields to be transformed
    ///
    /// See **nth** for the details
    #[cfg_attr(
        feature = "cli",
        arg(long, default_value = "", help_heading = "Search", value_delimiter = ',')
    )]
    pub with_nth: Vec<String>,

    /// Delimiter between fields
    ///
    /// In regex format, default to AWK-style. Escape sequences like \x00, \t, \n are supported.
    #[cfg_attr(
        feature = "cli",
        arg(short, long, default_value = r"[\t\n ]+", value_parser = parse_delimiter_value, help_heading = "Search")
    )]
    pub delimiter: Regex,

    /// Run in exact mode
    #[cfg_attr(feature = "cli", arg(short, long, help_heading = "Search"))]
    pub exact: bool,

    /// Start in regex mode instead of fuzzy-match
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Search"))]
    pub regex: bool,

    /// Fuzzy matching algorithm
    ///
    /// skim_v2 Latest skim algorithm, should be better in almost any case
    /// skim_v1 Legacy skim algorithm
    /// clangd Used in clangd for keyword completion
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "algo",
            default_value = "skim_v2",
            value_enum,
            help_heading = "Search",
            verbatim_doc_comment
        )
    )]
    pub algorithm: FuzzyAlgorithm,

    /// Case sensitivity
    ///
    /// Determines whether or not to ignore case while matching
    /// Note: this is not used for the Frizbee matcher, it uses a penalty system to favor
    /// case-sensitivity without enforcing it
    #[cfg_attr(
        feature = "cli",
        arg(long, default_value = "smart", value_enum, help_heading = "Search")
    )]
    pub case: CaseMatching,

    /// Enable split matching and set delimiter
    ///
    /// Split matching runs the matcher in splits: `foo:bar` will match all items matching `foo`, then
    /// `:`, then `bar` if the delimiter is present, or match normally if not.
    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            default_missing_value = ":",
            help_heading = "Search",
            num_args=0..
        )
    )]
    pub split_match: Option<char>,

    //  --- Interface ---
    /// Comma separated list of bindings
    ///
    /// You can customize key bindings of sk with `--bind` option which takes a  comma-separated  list  of
    /// key binding expressions. Each key binding expression follows the following format: `<key>:<action>`
    /// See the [KEYBINDS] section for details
    ///
    /// **Example**: `sk --bind=ctrl-j:accept,ctrl-k:kill-line`
    ///
    /// ## Multiple actions can be chained using + separator.
    ///
    /// **Example**: `sk --bind 'ctrl-a:select-all+accept'`
    ///
    /// # Special behaviors
    ///
    /// With `execute(...)` and `reload(...)` action, you can execute arbitrary commands without leaving sk.
    /// For example, you can turn sk into a simple file browser by binding enter key to less command like follows:
    ///
    /// ```bash
    /// sk --bind "enter:execute(less {})"
    /// ```
    ///
    /// Note: if no argument is supplied to reload, the default command is run.
    ///
    /// You can use the same placeholder expressions as in --preview.
    ///
    /// sk  switches  to  the  alternate screen when executing a command. However, if the command is ex‐
    /// pected to complete quickly, and you are not interested in its output, you might want to use exe‐
    /// cute-silent instead, which silently executes the command without the  switching.  Note  that  sk
    /// will  not  be  responsive  until the command is complete. For asynchronous execution, start your
    /// command as a background process (i.e. appending &).
    ///
    /// With if-query-empty and if-query-not-empty action, you could specify the action to  execute  de‐
    /// pends on the query condition. For example:
    ///
    /// `sk --bind 'ctrl-d:if-query-empty(abort)+delete-char'`
    ///
    /// If  the query is empty, skim will execute abort action, otherwise execute delete-char action. It
    /// is equal to ‘delete-char/eof‘.
    #[cfg_attr(
        feature = "cli",
        arg(short, long, help_heading = "Interface", verbatim_doc_comment, default_value = "", num_args=0..)
    )]
    pub bind: Vec<String>,

    /// Enable multiple selection
    ///
    /// Uses Tab and S-Tab by default for selection
    #[cfg_attr(
        feature = "cli",
        arg(short, long, overrides_with = "no_multi", help_heading = "Interface")
    )]
    pub multi: bool,

    /// Disable multiple selection
    #[cfg_attr(feature = "cli", arg(long, conflicts_with = "multi", help_heading = "Interface"))]
    pub no_multi: bool,

    /// Disable mouse
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub no_mouse: bool,

    /// Command to invoke dynamically in interactive mode
    ///
    /// Will be invoked using `sh -c`
    #[cfg_attr(feature = "cli", arg(short, long, help_heading = "Interface"))]
    pub cmd: Option<String>,

    /// Start skim in interactive mode
    ///
    /// In interactive mode, sk will run the command specified by `--cmd` option and display the
    /// results.
    #[cfg_attr(feature = "cli", arg(short, long, help_heading = "Interface"))]
    pub interactive: bool,

    /// Replace replstr with the selected item in commands
    #[cfg_attr(feature = "cli", arg(short = 'I', default_value = "{}", help_heading = "Interface"))]
    pub replstr: String,

    /// Set color theme
    ///
    /// Format: [BASE][,COLOR:ANSI[:ATTR1:ATTR2:..]]
    /// See [THEME] section for details
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface", verbatim_doc_comment))]
    pub color: Option<String>,

    /// Disable horizontal scroll
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub no_hscroll: bool,

    /// Keep the right end of the line visible on overflow
    ///
    /// Effective only when the query string is empty
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub keep_right: bool,

    /// Show the matched pattern at the line start
    ///
    /// Line  will  start  with  the  start of the matched pattern. Effective only when the query
    /// string is empty. Was designed to skip showing starts of paths of rg/grep results.
    ///
    /// e.g. sk -i -c "rg {} --color=always" --skip-to-pattern '[^/]*:' --ansi
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface", verbatim_doc_comment))]
    pub skip_to_pattern: Option<String>,

    /// Do not clear previous line if the command returns an empty result
    ///
    /// Do not clear previous items if new command returns empty result. This might be useful  to
    /// reduce flickering when typing new commands and the half-complete commands are not valid.
    ///
    /// This is not the default behavior because similar use cases for grep and rg have already been op‐
    /// timized where empty query results actually mean "empty" and previous results should be
    /// cleared.
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface", verbatim_doc_comment))]
    pub no_clear_if_empty: bool,

    /// Do not clear items on start
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub no_clear_start: bool,

    /// Do not clear screen on exit
    ///
    /// Do not clear finder interface on exit. If skim was started in full screen mode, it will not switch back to the
    /// original  screen, so you'll have to manually run tput rmcup to return. This option can be used to avoid
    /// flickering of the screen when your application needs to start skim multiple times in order.
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub no_clear: bool,

    /// Show error message if command fails
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub show_cmd_error: bool,

    /// Cycle the results by wrapping around when scrolling
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub cycle: bool,

    /// Disable matching entirely
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
    pub disabled: bool,

    //  --- Layout ---
    /// Set layout
    ///
    #[cfg_attr(
        feature = "cli",
        arg(long, help_heading = "Layout", verbatim_doc_comment, default_value = "default")
    )]
    pub layout: TuiLayout,

    /// Shorthand for reverse layout
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Layout", overrides_with = "layout"))]
    pub reverse: bool,

    /// Height of skim's window
    ///
    /// Can either be a row count or a percentage
    #[cfg_attr(feature = "cli", arg(long, default_value = "100%", help_heading = "Layout"))]
    pub height: String,

    /// Disable height (force full screen)
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Layout"))]
    pub no_height: bool,

    /// Minimum height of skim's window
    ///
    /// Useful when the height is set as a percentage
    /// Ignored when --height is not specified
    #[cfg_attr(
        feature = "cli",
        arg(long, default_value = "10", help_heading = "Layout", verbatim_doc_comment)
    )]
    pub min_height: String,

    /// Screen margin
    ///
    /// For each side, can be either a row count or a percentage of the terminal size
    ///
    /// Format can be one of:
    ///     - TRBL
    ///     - TB,RL
    ///     - T,RL,B
    ///     - T,R,B,L
    /// Example: 1,10%
    #[cfg_attr(
        feature = "cli",
        arg(long, default_value = "0", help_heading = "Layout", verbatim_doc_comment)
    )]
    pub margin: String,

    /// Set prompt
    #[cfg_attr(feature = "cli", arg(long, short, default_value = "> ", help_heading = "Layout"))]
    pub prompt: String,

    /// Set prompt in command mode
    #[cfg_attr(feature = "cli", arg(long, default_value = "c> ", help_heading = "Layout"))]
    pub cmd_prompt: String,

    /// Set selected item icon
    #[cfg_attr(
        feature = "cli",
        arg(long = "selector", alias = "pointer", default_value = ">", help_heading = "Layout")
    )]
    pub selector_icon: String,

    /// Set selected item icon
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "multi-selector",
            alias = "marker",
            default_value = ">",
            help_heading = "Layout"
        )
    )]
    pub multi_select_icon: String,

    //  --- Display ---
    /// Parse ANSI color codes in input strings
    ///
    /// When using skim as a library, this has no effect and ansi parsing should
    /// be enabled by manually injecting a cmd_collector like so:
    /// ```rust
    /// use skim::prelude::*;
    ///
    /// let _options = SkimOptionsBuilder::default()
    ///   .cmd(ls --color)
    ///   .cmd_collector(Rc::new(RefCell::new(SkimItemReader::new(
    ///     SkimItemReaderOption::default().ansi(true),
    ///     ))) as Rc<RefCell<dyn CommandCollector>>)
    ///   .build()
    ///   .unwrap()
    /// ```
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display"))]
    pub ansi: bool,

    /// Number of spaces that make up a tab
    #[cfg_attr(feature = "cli", arg(long, default_value = "8", help_heading = "Display"))]
    pub tabstop: usize,

    /// Set matching result count display position
    ///
    ///     hidden: do not display info
    ///     inline: display info in the same row as the input
    ///     default: display info in a dedicated row above the input
    #[cfg_attr(
        feature = "cli",
        arg(long, help_heading = "Display", value_enum, default_value = "default")
    )]
    pub info: InfoDisplay,

    /// Alias for --info=hidden
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display"))]
    pub no_info: bool,

    /// Alias for --info=inline
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display"))]
    pub inline_info: bool,

    /// Set header, displayed next to the info
    ///
    /// The  given  string  will  be printed as the sticky header. The lines are displayed in the
    /// given order from top to bottom regardless of --layout option, and  are  not  affected  by
    /// --with-nth. ANSI color codes are processed even when --ansi is not set.
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display"))]
    pub header: Option<String>,

    /// Number of lines of the input treated as header
    ///
    /// The  first N lines of the input are treated as the sticky header. When `--with-nth` is set,
    /// the lines are transformed just like the other lines that follow.
    #[cfg_attr(feature = "cli", arg(long, default_value = "0", help_heading = "Display"))]
    pub header_lines: usize,

    /// Draw borders around the UI components
    ///
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display"))]
    pub border: bool,

    /// Wrap items in the item list
    #[cfg_attr(feature = "cli", arg(long = "wrap", help_heading = "Display"))]
    pub wrap_items: bool,

    //  --- History ---
    /// History file
    ///
    /// Load search history from the specified file and update the file on completion.
    ///
    /// When enabled, CTRL-N and CTRL-P are automatically remapped
    /// to next-history and previous-history.
    #[cfg_attr(feature = "cli", arg(long = "history", help_heading = "History"))]
    pub history_file: Option<String>,

    /// Maximum number of query history entries to keep
    #[cfg_attr(feature = "cli", arg(long, default_value = "1000", help_heading = "History"))]
    pub history_size: usize,

    /// Command history file
    ///
    /// Load command query history from the specified file and update the file on completion.
    ///
    /// When enabled, CTRL-N and CTRL-P are automatically remapped
    /// to next-history and previous-history.
    #[cfg_attr(feature = "cli", arg(long = "cmd-history", help_heading = "History"))]
    pub cmd_history_file: Option<String>,

    /// Maximum number of query history entries to keep
    #[cfg_attr(feature = "cli", arg(long, default_value = "1000", help_heading = "History"))]
    pub cmd_history_size: usize,

    //  --- Preview ---
    /// Preview command
    ///
    /// Execute the given command for the current line and display the result on the preview window. {} in the command
    /// is the placeholder that is replaced to the single-quoted string of the current line. To transform the replace‐
    /// ment string, specify field index expressions between the braces (See FIELD INDEX EXPRESSION for the details).
    ///
    /// **Examples**:
    ///
    /// ```bash
    /// sk --preview='head -$LINES {}'
    /// ls -l | sk --preview="echo user={3} when={-4..-2}; cat {-1}" --header-lines=1
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Preview", verbatim_doc_comment))]
    pub preview: Option<String>,

    /// Preview window layout
    ///
    /// Format: [up|down|left|right][:SIZE[%]][:hidden][:+SCROLL[-OFFSET]]
    ///
    /// Determine  the  layout of the preview window. If the argument ends with: hidden, the preview window will be hidden by
    /// default until toggle-preview action is triggered. Long lines are truncated by default.  Line wrap can be enabled with
    ///: wrap flag.
    ///
    /// If size is given as 0, preview window will not be visible, but sk will still execute the command in the background.
    ///
    /// +SCROLL[-OFFSET] determines the initial scroll offset of the preview window. SCROLL can be either a  numeric  integer
    /// or  a  single-field index expression that refers to a numeric integer. The optional -OFFSET part is for adjusting the
    /// base offset so that you can see the text above it. It should be given as a numeric integer (-INTEGER), or as a denom‐
    /// inator form (-/INTEGER) for specifying a fraction of the preview window height.
    ///
    /// **Examples**:
    /// ```bash
    /// # Non-default scroll window positions and sizes
    /// sk --preview="head {}" --preview-window=up:30%
    /// sk --preview="file {}" --preview-window=down:2
    ///
    /// # Initial scroll offset is set to the line number of each line of
    /// # git grep output *minus* 5 lines (-5)
    /// git grep --line-number '' |
    ///   sk --delimiter:  --preview 'nl {1}' --preview-window +{2}-5
    ///
    ///             # Preview with bat, matching line in the middle of the window (-/2)
    ///             git grep --line-number '' |
    ///               sk --delimiter : \
    ///                   --preview 'bat --style=numbers --color=always --highlight-line {2} {1}' \
    ///                   --preview-window +{2}-/2
    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            default_value = "right:50%",
            help_heading = "Preview",
            allow_hyphen_values = true
        )
    )]
    pub preview_window: PreviewLayout,

    //  --- Scripting ---
    /// Initial query
    #[cfg_attr(feature = "cli", arg(long, short, help_heading = "Scripting"))]
    pub query: Option<String>,

    /// Initial query in interactive mode
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub cmd_query: Option<String>,

    /// Read input delimited by ASCII NUL(\\0) characters
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub read0: bool,

    /// Print output delimited by ASCII NUL(\\0) characters
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print0: bool,

    /// Print the query as the first line
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print_query: bool,

    /// Print the command as the first line (after print-query)
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print_cmd: bool,

    /// Print the score after each item
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print_score: bool,

    /// Print the header as the first line (after print-score)
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print_header: bool,

    /// Print the ANSI codes, making the output exactly match the input even when `--ansi` is on
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting", requires = "ansi"))]
    pub no_strip_ansi: bool,

    /// Automatically select the match if there is only one
    #[cfg_attr(feature = "cli", arg(long, short = '1', help_heading = "Scripting"))]
    pub select_1: bool,

    /// Automatically exit when no match is left
    #[cfg_attr(feature = "cli", arg(long, short = '0', help_heading = "Scripting"))]
    pub exit_0: bool,

    /// Synchronous search for multi-staged filtering
    ///
    /// Synchronous search for multi-staged filtering. If specified,
    /// skim will launch ncurses finder only after the input stream is complete.
    ///
    ///     e.g. sk --multi | sk --sync
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub sync: bool,

    /// Pre-select the first n items in multi-selection mode
    #[cfg_attr(feature = "cli", arg(long, default_value = "0", help_heading = "Scripting"))]
    pub pre_select_n: usize,

    /// Pre-select the matched items in multi-selection mode
    ///
    /// Check the doc for the detailed syntax:
    /// https://docs.rs/regex/1.4.1/regex/
    #[cfg_attr(feature = "cli", arg(long, default_value = "", help_heading = "Scripting"))]
    pub pre_select_pat: String,

    /// Pre-select the items separated by newline character
    ///
    /// Example: 'item1\nitem2'
    #[cfg_attr(feature = "cli", arg(long, default_value = "", help_heading = "Scripting"))]
    pub pre_select_items: String,

    /// Pre-select the items read from this file
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub pre_select_file: Option<String>,

    /// Query for filter mode
    #[cfg_attr(feature = "cli", arg(long, short, help_heading = "Scripting"))]
    pub filter: Option<String>,

    /// Generate shell completion script
    ///
    /// Generate completion script for the specified shell: bash, zsh, fish, etc.
    /// The output can be directly sourced or saved to a file for automatic loading.
    /// Examples: `source <(sk --shell bash)` (immediate use)
    ///          `sk --shell bash >> ~/.bash_completion` (persistent use)
    ///
    /// Supported shells: bash, zsh, fish, powershell, elvish
    ///
    /// Note: While PowerShell completions are supported, Windows is not supported for now.
    #[cfg(feature = "cli")]
    #[cfg_attr(
        feature = "cli",
        arg(long, value_name = "SHELL", help_heading = "Scripting", value_enum)
    )]
    pub shell: Option<crate::completions::Shell>,

    /// Generate shell key bindings - only for bash, zsh and fish
    ///
    /// Generate key bindings script after the shell completions
    /// See the `shell` option for more details
    #[cfg(feature = "cli")]
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting", requires = "shell"))]
    pub shell_bindings: bool,

    /// Generate man page and output it to stdout
    #[cfg(feature = "cli")]
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub man: bool,

    /// Run an IPC socket with optional name (defaults to `sk`)
    ///
    /// The socket expects Actions in Ron format (similar to Rust code), see `./src/tui/event.rs` for all possible Actions
    /// To write to it, you can use socat, for example with `--listen sk`:
    /// `echo 'ToggleIn' | socat -u STDIN ABSTRACT-CONNECT:sk`
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting", default_missing_value = "sk", num_args=0..))]
    pub listen: Option<String>,

    /// Send commands to an IPC socket with optional name (defaults to `sk`)
    ///
    /// The commands are read from stdin, one per line, in the same format as the actions in the
    /// bind flag. They can also be chained using `+` as a separator.
    /// All other arguments will be ignored
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting", default_missing_value = "sk", num_args=0..))]
    pub remote: Option<String>,

    /// Run in a tmux popup
    ///
    /// Format: `sk --tmux <center|top|bottom|left|right>[,SIZE[%]][,SIZE[%]]`
    ///
    /// Depending on the direction, the order and behavior of the sizes varies:
    ///
    /// Default: center,50%
    #[cfg_attr(feature = "cli", arg(long, verbatim_doc_comment, help_heading = "Display", default_missing_value = "center,50%", num_args=0..))]
    pub tmux: Option<String>,

    /// Pipe log output to a file
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub log_file: Option<String>,

    /// Reserved for later use
    #[cfg_attr(
        feature = "cli",
        arg(short = 'x', long, hide = true, help_heading = "Reserved for later use")
    )]
    pub extended: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub literal: bool,

    /// Reserved for later use
    #[cfg_attr(
        feature = "cli",
        arg(long, hide = true, default_value = "10", help_heading = "Reserved for later use")
    )]
    pub hscroll_off: usize,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub filepath_word: bool,

    /// Reserved for later use
    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            hide = true,
            default_value = "abcdefghijklmnopqrstuvwxyz",
            help_heading = "Reserved for later use"
        )
    )]
    pub jump_labels: String,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub no_bold: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub phony: bool,

    /// Deprecated, kept for compatibility purposes. See accept() bind instead.
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Deprecated", default_value = ""))]
    pub expect: String,

    /// Command collector for reading items from commands
    #[cfg_attr(feature = "cli", clap(skip = Rc::new(RefCell::new(SkimItemReader::default())) as Rc<RefCell<dyn CommandCollector>>))]
    pub cmd_collector: Rc<RefCell<dyn CommandCollector>>,
    /// Query history entries loaded from history file
    #[cfg_attr(feature = "cli", clap(skip))]
    pub query_history: Vec<String>,
    /// Command history entries loaded from cmd history file
    #[cfg_attr(feature = "cli", clap(skip))]
    pub cmd_history: Vec<String>,
    /// Selector for pre-selecting items
    #[cfg_attr(feature = "cli", clap(skip))]
    pub selector: Option<Rc<dyn Selector>>,
    /// Preview Callback
    ///
    /// Used to define a function or closure for the preview window, instead of a shell command.
    ///
    /// The function will take a `Vec<Arc<dyn SkimItem>>>` containing the currently selected items
    /// and return a Vec<String> with the lines to display in UTF-8
    #[cfg_attr(feature = "cli", clap(skip))]
    pub preview_fn: Option<PreviewCallback>,

    /// The internal (parsed) keymap
    #[cfg_attr(feature = "cli", clap(skip))]
    pub keymap: KeyMap,
}

impl Default for SkimOptions {
    fn default() -> Self {
        Self {
            split_match: None,
            no_strip_ansi: false,
            wrap_items: false,
            listen: None,
            remote: None,
            print_header: false,
            disabled: false,
            tac: Default::default(),
            min_query_length: Default::default(),
            no_sort: Default::default(),
            tiebreak: vec![RankCriteria::Score, RankCriteria::Begin, RankCriteria::End],
            nth: Default::default(),
            with_nth: Default::default(),
            delimiter: Regex::new(r"[\t\n ]+").unwrap(),
            exact: Default::default(),
            regex: Default::default(),
            algorithm: Default::default(),
            case: Default::default(),
            bind: Default::default(),
            multi: Default::default(),
            no_multi: Default::default(),
            no_mouse: Default::default(),
            cmd: Default::default(),
            interactive: Default::default(),
            replstr: String::from("{}"),
            color: Default::default(),
            no_hscroll: Default::default(),
            keep_right: Default::default(),
            skip_to_pattern: Default::default(),
            no_clear_if_empty: Default::default(),
            no_clear_start: Default::default(),
            no_clear: Default::default(),
            show_cmd_error: Default::default(),
            layout: TuiLayout::default(),
            reverse: Default::default(),
            height: String::from("100%"),
            no_height: Default::default(),
            min_height: String::from("10"),
            margin: Default::default(),
            prompt: String::from("> "),
            cmd_prompt: String::from("c> "),
            selector_icon: String::from(">"),
            multi_select_icon: String::from(">"),
            ansi: Default::default(),
            tabstop: 8,
            info: Default::default(),
            no_info: Default::default(),
            inline_info: Default::default(),
            header: Default::default(),
            header_lines: Default::default(),
            history_file: Default::default(),
            history_size: 1000,
            cmd_history_file: Default::default(),
            cmd_history_size: 1000,
            preview: Default::default(),
            preview_window: PreviewLayout::default(),
            query: Default::default(),
            cmd_query: Default::default(),
            read0: Default::default(),
            print0: Default::default(),
            print_query: Default::default(),
            print_cmd: Default::default(),
            print_score: Default::default(),
            select_1: Default::default(),
            exit_0: Default::default(),
            sync: Default::default(),
            pre_select_n: Default::default(),
            pre_select_pat: Default::default(),
            pre_select_items: Default::default(),
            pre_select_file: Default::default(),
            filter: Default::default(),
            tmux: Default::default(),
            log_file: Default::default(),
            extended: Default::default(),
            literal: Default::default(),
            cycle: Default::default(),
            hscroll_off: 10,
            filepath_word: Default::default(),
            jump_labels: String::from("abcdefghijklmnopqrstuvwxyz"),
            border: Default::default(),
            no_bold: Default::default(),
            phony: Default::default(),
            expect: Default::default(),
            cmd_collector: Rc::new(RefCell::new(SkimItemReader::default())) as Rc<RefCell<dyn CommandCollector>>,
            query_history: Default::default(),
            cmd_history: Default::default(),
            selector: Default::default(),
            preview_fn: Default::default(),
            keymap: Default::default(),
            #[cfg(feature = "cli")]
            shell: Default::default(),
            #[cfg(feature = "cli")]
            man: false,
            #[cfg(feature = "cli")]
            shell_bindings: false,
        }
    }
}

impl SkimOptionsBuilder {
    /// Builds the SkimOptions from the builder
    pub fn build(&mut self) -> Result<SkimOptions, SkimOptionsBuilderError> {
        self.final_build().map(|opts| opts.build())
    }
}

impl SkimOptions {
    /// Finalizes the options by applying defaults and initializing components
    pub fn build(mut self) -> Self {
        if self.no_height {
            self.height = String::from("100%");
        }

        self.keymap = self.bind.iter().fold(KeyMap::default(), |mut res, part| {
            res.add_keymaps(part.split(','));
            res
        });

        if self.reverse {
            self.layout = TuiLayout::Reverse
        }
        if self.history_file.is_some() || self.cmd_history_file.is_some() {
            self.init_histories();
            self.keymap.insert(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
                vec![Action::PreviousHistory],
            );
            self.keymap.insert(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
                vec![Action::NextHistory],
            );
        }
        if self.inline_info {
            self.info = InfoDisplay::Inline;
        }
        if self.no_info {
            self.info = InfoDisplay::Hidden;
        }

        self
    }
    /// Initializes history from configured history files
    pub fn init_histories(&mut self) {
        if let Some(histfile) = &self.history_file {
            self.query_history.extend(read_file_lines(histfile).unwrap_or_default());
        }

        if let Some(cmd_histfile) = &self.cmd_history_file {
            self.cmd_history
                .extend(read_file_lines(cmd_histfile).unwrap_or_default());
        }
    }
}
