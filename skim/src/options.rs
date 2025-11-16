use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "cli")]
use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use derive_builder::Builder;
use regex::Regex;

use crate::binds::KeyMap;
use crate::item::RankCriteria;
use crate::tui::statusline::InfoDisplay;
use crate::prelude::SkimItemReader;
use crate::tui::PreviewCallback;
use crate::reader::CommandCollector;
use crate::tui::event::Action;
use crate::tui::options::{PreviewLayout, TuiLayout};
use crate::util::read_file_lines;
use crate::{CaseMatching, FuzzyAlgorithm, Selector};

/// sk - fuzzy finder in Rust
///
/// sk is a general purpose command-line fuzzy finder.
///
///
/// # ENVIRONMENT VARIABLES
///
///      NO_COLOR
///
///          If set and not empty, sk will not use any colors in the output.
///
///      SKIM_DEFAULT_COMMAND
///
/// If set and not empty, sk will not use any colors in the output.
///
/// ## SKIM_DEFAULT_COMMAND
///
/// Default command to use when input is tty. On *nix systems, sk runs the command with sh -c, so make  sure  that
/// it's POSIX-compliant.
///
/// ## SKIM_DEFAULT_OPTIONS
///
/// Default options. e.g. `export SKIM_DEFAULT_OPTIONS="--multi"`
///
/// # EXTENDED SEARCH MODE
///
/// Unless specified otherwise, sk will start in "extended-search mode". In this mode, you can specify multiple  patterns
/// delimited by spaces, such as: 'wild ^music .mp3$ sbtrkt !rmx
///
/// You can prepend a backslash to a space (\ ) to match a literal space character.
///
/// ## Exact-match (quoted)
///
/// A term that is prefixed by a single-quote character (') is interpreted as an "exact-match" (or "non-fuzzy") term. sk
/// will search for the exact occurrences of the string.
///
/// ## Anchored-match
///
/// A term can be prefixed by ^, or suffixed by $ to become an anchored-match term. Then sk will  search  for  the  lines
/// that start with or end with the given string. An anchored-match term is also an exact-match term.
///
/// ## Negation
///
/// If  a  term  is prefixed by !, sk will exclude the lines that satisfy the term from the result. In this case, `sk` per‐
/// forms exact match by default.
///
/// ## Exact-match by default
///
/// If you don't prefer fuzzy matching and do not wish to "quote" (prefixing with ') every word,
/// start `sk` with `-e` or
/// `--exact` option. Note that when `--exact` is set, '-prefix "unquotes" the term.
///
/// ## OR operator
///
///  A  single bar character term acts as an OR operator. For example, the following query matches entries that start with
/// core and end with either go, rb, or py.
///
///
/// **Example**: `^core go$ | rb$ | py$`
///
///
/// # EXIT STATUS
///
/// * 0:      Normal exit
///
/// * 1:      No match
///
/// * 2:      Error
///
/// * 130:    Interrupted with CTRL-C or ESC

#[derive(Builder)]
#[builder(build_fn(name = "final_build"))]
#[builder(default)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
#[cfg_attr(feature = "cli", command(name = "sk", args_override_self = true, verbatim_doc_comment, version, about))]
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
    #[cfg_attr(feature = "cli", arg(
        short,
        long,
        default_value = "score,begin,end",
        value_enum,
        value_delimiter = ',',
        help_heading = "Search",
        verbatim_doc_comment
    ))]
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
    #[cfg_attr(feature = "cli", arg(
        short,
        long,
        default_value = "",
        help_heading = "Search",
        verbatim_doc_comment,
        value_delimiter = ','
    ))]
    pub nth: Vec<String>,

    /// Fields to be transformed
    ///
    /// See **nth** for the details
    #[cfg_attr(feature = "cli", arg(long, default_value = "", help_heading = "Search", value_delimiter = ','))]
    pub with_nth: Vec<String>,

    /// Delimiter between fields
    ///
    /// In regex format, default to AWK-style
    #[cfg_attr(feature = "cli", arg(short, long, default_value = r"[\t\n ]+", help_heading = "Search"))]
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
    #[cfg_attr(feature = "cli", arg(
        long = "algo",
        default_value = "skim_v2",
        value_enum,
        help_heading = "Search",
        verbatim_doc_comment
    ))]
    pub algorithm: FuzzyAlgorithm,

    /// Case sensitivity
    ///
    /// Determines whether or not to ignore case while matching
    #[cfg_attr(feature = "cli", arg(long, default_value = "smart", value_enum, help_heading = "Search"))]
    pub case: CaseMatching,

    //  --- Interface ---
    /// Comma separated list of bindings
    ///
    /// You can customize key bindings of sk with `--bind` option which takes a  comma-separated  list  of
    /// key binding expressions. Each key binding expression follows the following format: `<key>:<action>`
    ///
    /// **Example**: `sk --bind=ctrl-j:accept,ctrl-k:kill-line`
    ///
    /// ## AVAILABLE KEYS: (SYNONYMS)
    ///
    /// * ctrl-[a-z]
    ///
    /// * ctrl-space
    ///
    /// * ctrl-alt-[a-z]
    ///
    /// * alt-[a-zA-Z]
    ///
    /// * alt-[0-9]
    ///
    /// * f[1-12]
    ///
    /// * enter       (ctrl-m)
    ///
    /// * space
    ///
    /// * bspace      (bs)
    ///
    /// * alt-up
    ///
    /// * alt-down
    ///
    /// * alt-left
    ///
    /// * alt-right
    ///
    /// * alt-enter   (alt-ctrl-m)
    ///
    /// * alt-space
    ///
    /// * alt-bspace  (alt-bs)
    ///
    /// * alt-/
    ///
    /// * tab
    ///
    /// * btab        (shift-tab)
    ///
    /// * esc
    ///
    /// * del
    ///
    /// * up
    ///
    /// * down
    ///
    /// * left
    ///
    /// * right
    ///
    /// * home
    ///
    /// * end
    ///
    /// * pgup        (page-up)
    ///
    /// * pgdn        (page-down)
    ///
    /// * shift-up
    ///
    /// * shift-down
    ///
    /// * shift-left
    ///
    /// * shift-right
    ///
    /// * alt-shift-up
    ///
    /// * alt-shift-down
    ///
    /// * alt-shift-left
    ///
    /// * alt-shift-right
    ///
    /// * any single character
    ///
    /// ## ACTION: DEFAULT BINDINGS [NOTES]
    ///
    /// * abort: ctrl-c  ctrl-q  esc
    ///
    /// * accept(...): enter *the argument will be printed when the binding is triggered*
    ///
    /// * append-and-select:
    ///
    /// * backward-char: ctrl-b  left
    ///
    /// * backward-delete-char: ctrl-h  bspace
    ///
    /// * backward-kill-word: alt-bs
    ///
    /// * backward-word: alt-b   shift-left
    ///
    /// * beginning-of-line: ctrl-a  home
    ///
    /// * clear-screen: ctrl-l
    ///
    /// * delete-char: del
    ///
    /// * delete-charEOF: ctrl-d
    ///
    /// * deselect-all:
    ///
    /// * down: ctrl-j  ctrl-n  down
    ///
    /// * end-of-line: ctrl-e  end
    ///
    /// * execute(...): *see below for the details*
    ///
    /// * execute-silent(...): *see below for the details*
    ///
    /// * forward-char: ctrl-f  right
    ///
    /// * forward-word: alt-f   shift-right
    ///
    /// * if-non-matched:
    ///
    /// * if-query-empty:
    ///
    /// * if-query-not-empty:
    ///
    /// * ignore:
    ///
    /// * kill-line:
    ///
    /// * kill-word: alt-d
    ///
    /// * next-history: ctrl-n with `--history` or `--cmd-history`
    ///
    /// * page-down: pgdn
    ///
    /// * page-up: pgup
    ///
    /// * half-page-down:
    ///
    /// * half-page-up:
    ///
    /// * preview-up: shift-up
    ///
    /// * preview-down: shift-down
    ///
    /// * preview-left:
    ///
    /// * preview-right:
    ///
    /// * preview-page-down:
    ///
    /// * preview-page-up:
    ///
    /// * previous-history: ctrl-p with `--history` or `--cmd-history`
    ///
    /// * reload(...):
    ///
    /// * select-all:
    ///
    /// * toggle:
    ///
    /// * toggle-all:
    ///
    /// * toggle+down: ctrl-i  tab
    ///
    /// * toggle-in: (--layout=reverse ? toggle+up:  toggle+down)
    ///
    /// * toggle-out: (--layout=reverse ? toggle+down:  toggle+up)
    ///
    /// * toggle-preview:
    ///
    /// * toggle-preview-wrap:
    ///
    /// * toggle-sort:
    ///
    /// * toggle+up: btab    shift-tab
    ///
    /// * unix-line-discard: ctrl-u
    ///
    /// * unix-word-rubout: ctrl-w
    ///
    /// * up: ctrl-k  ctrl-p  up
    ///
    /// * yank: ctrl-y
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
    /// If the command contains parentheses, sk may fail to parse the expression. In that case, you  can
    /// use any of the following alternative notations to avoid parse errors.
    ///
    /// * `execute[...]`
    ///
    /// * `execute'...'`
    ///
    /// * `execute"..."`
    ///
    /// * `execute:...`
    ///
    /// This is the special form that frees you from parse errors as it does not expect the clos‐
    /// ing character. The catch is that it should be the last one in the comma-separated list of
    /// key-action pairs.
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
    #[cfg_attr(feature = "cli", arg(short, long, help_heading = "Interface", verbatim_doc_comment, default_value = ""))]
    pub bind: KeyMap,

    /// Enable multiple selection
    ///
    /// Uses Tab and S-Tab by default for selection
    #[cfg_attr(feature = "cli", arg(short, long, overrides_with = "no_multi", help_heading = "Interface"))]
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

    /// Run in interactive mode
    #[cfg_attr(feature = "cli", arg(short, long, help_heading = "Interface"))]
    pub interactive: bool,

    /// Replace replstr with the selected item in commands
    #[cfg_attr(feature = "cli", arg(short = 'I', default_value = "{}", help_heading = "Interface"))]
    pub replstr: String,

    /// Set color theme
    ///
    /// Format: [BASE][,COLOR:ANSI]
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Interface"))]
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

    //  --- Layout ---
    /// Set layout
    ///
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Layout", verbatim_doc_comment, default_value = "default"))]
    pub layout: TuiLayout,

    /// Shorthand for reverse layout
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Layout"))]
    pub reverse: bool,

    /// Height of skim's window
    ///
    /// Can either be a row count or a percentage
    #[cfg_attr(feature = "cli", arg(long, default_value = "100%", help_heading = "Layout"))]
    pub height: String,

    /// Disable height feature
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Layout"))]
    pub no_height: bool,

    /// Minimum height of skim's window
    ///
    /// Useful when the height is set as a percentage
    /// Ignored when --height is not specified
    #[cfg_attr(feature = "cli", arg(long, default_value = "10", help_heading = "Layout", verbatim_doc_comment))]
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
    #[cfg_attr(feature = "cli", arg(long, default_value = "0", help_heading = "Layout", verbatim_doc_comment))]
    pub margin: String,

    /// Set prompt
    #[cfg_attr(feature = "cli", arg(long, short, default_value = "> ", help_heading = "Layout"))]
    pub prompt: String,

    /// Set prompt in command mode
    #[cfg_attr(feature = "cli", arg(long, default_value = "c> ", help_heading = "Layout"))]
    pub cmd_prompt: String,

    //  --- Display ---
    /// Parse ANSI color codes in input strings
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
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Display", value_enum, default_value = "default"))]
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
    /// ```
    ///
    /// sk overrides $LINES and $COLUMNS so that they represent the exact size of the preview window.
    ///
    /// A placeholder expression starting with + flag will be replaced to the space-separated  list  of  the  selected
    /// lines (or the current line if no selection was made) individually quoted.
    ///
    /// **Examples**:
    /// ```bash
    /// sk --multi --preview='head -10 {+}'
    /// git log --oneline | sk --multi --preview 'git show {+1}'
    /// ```
    ///
    /// Note that you can escape a placeholder pattern by prepending a backslash.
    ///
    /// Also, `{q}`  is replaced to the current query string. `{cq}` is replaced to the current command query string.
    /// `{n}` is replaced to zero-based ordinal index of the line. Use `{+n}` if you want all index numbers when multiple
    /// lines are selected
    ///
    /// Preview window will be updated even when there is no match for the current query if any of the placeholder ex‐
    /// pressions evaluates to a non-empty string.
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
    #[cfg_attr(feature = "cli", arg(long, default_value = "right:50%", help_heading = "Preview"))]
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

    /// Print the command as the first line (after print-cmd)
    #[cfg_attr(feature = "cli", arg(long, help_heading = "Scripting"))]
    pub print_score: bool,

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
    /// Exemple: 'item1\nitem2'
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
    pub shell: Option<clap_complete::Shell>,

    /// Run in a tmux popup
    ///
    /// Format: `sk --tmux <center|top|bottom|left|right>[,SIZE[%]][,SIZE[%]]`
    ///
    /// Depending on the direction, the order and behavior of the sizes varies:
    ///
    /// Default: center,50%
    #[cfg_attr(feature = "cli", arg(long, verbatim_doc_comment, help_heading = "Display", default_missing_value = "center,50%", num_args=0..))]
    pub tmux: Option<String>,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(short = 'x', long, hide = true, help_heading = "Reserved for later use"))]
    pub extended: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub literal: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub cycle: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, default_value = "10", help_heading = "Reserved for later use"))]
    pub hscroll_off: usize,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub filepath_word: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(
        long,
        hide = true,
        default_value = "abcdefghijklmnopqrstuvwxyz",
        help_heading = "Reserved for later use"
    ))]
    pub jump_labels: String,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub no_bold: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub pointer: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub marker: bool,

    /// Reserved for later use
    #[cfg_attr(feature = "cli", arg(long, hide = true, help_heading = "Reserved for later use"))]
    pub phony: bool,

    #[cfg_attr(feature = "cli", clap(skip = Rc::new(RefCell::new(SkimItemReader::default())) as Rc<RefCell<dyn CommandCollector>>))]
    pub cmd_collector: Rc<RefCell<dyn CommandCollector>>,
    #[cfg_attr(feature = "cli", clap(skip))]
    pub query_history: Vec<String>,
    #[cfg_attr(feature = "cli", clap(skip))]
    pub cmd_history: Vec<String>,
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
}

#[cfg(feature = "cli")]
impl Default for SkimOptions {
    fn default() -> Self {
        Self {
            tac: Default::default(),
            min_query_length: Default::default(),
            no_sort: Default::default(),
            tiebreak: vec![RankCriteria::Score, RankCriteria::Begin, RankCriteria::End],
            nth: Default::default(),
            with_nth: Default::default(),
            delimiter: regex::Regex::new(r"[\t\n ]+").unwrap(),
            exact: Default::default(),
            regex: Default::default(),
            algorithm: Default::default(),
            case: Default::default(),
            bind: KeyMap::default(),
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
            #[cfg(feature = "cli")]
            shell: Default::default(),
            tmux: Default::default(),
            extended: Default::default(),
            literal: Default::default(),
            cycle: Default::default(),
            hscroll_off: 10,
            filepath_word: Default::default(),
            jump_labels: String::from("abcdefghijklmnopqrstuvwxyz"),
            border: Default::default(),
            no_bold: Default::default(),
            pointer: Default::default(),
            marker: Default::default(),
            phony: Default::default(),
            cmd_collector: Rc::new(RefCell::new(SkimItemReader::default())) as Rc<RefCell<dyn CommandCollector>>,
            query_history: Default::default(),
            cmd_history: Default::default(),
            selector: Default::default(),
            preview_fn: Default::default(),
        }
    }
}

#[cfg(not(feature = "cli"))]
impl Default for SkimOptions {
    fn default() -> Self {
        Self {
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
            bind: crate::binds::get_default_key_map(),
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
            layout: TuiLayout::Default,
            reverse: Default::default(),
            height: String::from("100%"),
            no_height: Default::default(),
            min_height: String::from("10"),
            margin: Default::default(),
            prompt: String::from("> "),
            cmd_prompt: String::from("c> "),
            ansi: Default::default(),
            tabstop: 8,
            info: Default::default(),
            no_info: Default::default(),
            inline_info: Default::default(),
            header: Default::default(),
            header_lines: Default::default(),
            border: Default::default(),
            history_file: Default::default(),
            history_size: 1000,
            cmd_history_file: Default::default(),
            cmd_history_size: 1000,
            preview: Default::default(),
            preview_window: Default::default(),
            read0: Default::default(),
            print0: Default::default(),
            query: Default::default(),
            cmd_query: Default::default(),
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
            extended: Default::default(),
            literal: Default::default(),
            cycle: Default::default(),
            hscroll_off: 10,
            filepath_word: Default::default(),
            jump_labels: String::from("abcdefghijklmnopqrstuvwxyz"),
            no_bold: Default::default(),
            pointer: Default::default(),
            marker: Default::default(),
            phony: Default::default(),
            cmd_collector: Rc::new(RefCell::new(SkimItemReader::default())) as Rc<RefCell<dyn CommandCollector>>,
            query_history: Default::default(),
            cmd_history: Default::default(),
            selector: Default::default(),
            preview_fn: Default::default(),
        }
    }
}

impl SkimOptionsBuilder {
    pub fn build(&mut self) -> Result<SkimOptions, SkimOptionsBuilderError> {
        self.final_build().map(|opts| opts.build())
    }
}

impl SkimOptions {
    pub fn build(mut self) -> Self {
        if self.no_height {
            self.height = String::from("100%");
        }

        if self.reverse {
            self.layout = TuiLayout::Reverse
        }
        if self.history_file.is_some() || self.cmd_history_file.is_some() {
            self.init_histories();
            self.bind.insert(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
                vec![Action::PreviousHistory],
            );
            self.bind.insert(
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
