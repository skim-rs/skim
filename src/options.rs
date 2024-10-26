use clap::Parser;
use derive_builder::Builder;

use crate::item::RankCriteria;
use crate::{CaseMatching, FuzzyAlgorithm};

#[derive(Builder)]
#[builder(build_fn(name = "final_build"))]
#[builder(default)]
#[derive(Parser)]
#[command(name = "sk", about = "A rust fuzzy finder")]
pub struct SkimOptions {
    //  --- Search ---
    /// Show results in reverse order
    #[arg(long = "tac", help_heading = "Search")]
    pub tac: bool,
    /// Do not sort the results
    #[arg(long = "no-sort", help_heading = "Search")]
    pub nosort: bool,
    /// Comma separated tie breakder criteria
    #[arg(short, long, default_value = "score,begin,end", value_enum, value_delimiter = ',')]
    pub tiebreak: Vec<RankCriteria>,
    /// Fields to be matched
    ///
    /// Example: 1,3..5
    #[arg(short, long, default_value = "")]
    pub nth: String,
    /// Fields to be transformed
    ///
    /// Example: 1,3..5
    #[arg(long = "with-nth", default_value = "")]
    pub with_nth: String,
    /// Delimiter between fields
    ///
    /// In regex format
    #[arg(short, long, default_value = "\\t")]
    pub delimiter: String,
    /// Run in exact mode
    #[arg(short, long)]
    pub exact: bool,
    /// Start in regex mode
    #[arg(long)]
    pub regex: bool,
    /// Fuzzy matching algorithm
    #[arg(long = "algo", default_value = "skim_v2", value_enum)]
    pub algorithm: FuzzyAlgorithm,
    /// Case sensitivity
    #[arg(long, default_value = "smart", value_enum)]
    pub case: CaseMatching,
    //  --- Interface ---
    /// Comma separated list of bindings
    ///
    /// Example : ctrl-j:accept,ctrl-k:kill-line
    #[arg(short, long)]
    pub bind: Vec<String>,
    /// Enable multiple selection
    #[arg(short, long, overrides_with = "no_multi")]
    pub multi: bool,
    /// Disable multiple selection
    #[arg(long)]
    pub no_multi: bool,
    /// Disable mouse
    #[arg(long)]
    pub no_mouse: bool,
    /// Command to invoke dynamically
    ///
    /// Will be invoked using `sh -c`
    #[arg(short, long)]
    pub cmd: Option<String>,
    /// Run in interactive mode
    #[arg(short, long)]
    pub interactive: bool,
    /// Set color theme
    ///
    /// Format: [BASE][,COLOR:ANSI]
    #[arg(long)]
    pub color: Option<String>,
    /// Disable horizontal scroll
    #[arg(long = "no-hscroll")]
    pub no_hscroll: bool,
    /// Keep the right end of the line visible on overflow
    #[arg(long = "keep-right")]
    pub keep_right: bool,
    /// Show the matched pattern at the line start
    #[arg(long = "skip-to-pattern")]
    pub skip_to_pattern: Option<String>,
    /// Do not clear previous line if the command returns an empty result
    #[arg(long = "no-clear-if-empty")]
    pub no_clear_if_empty: bool,
    /// Do not clear items on start
    #[arg(long = "no-clear-start")]
    pub no_clear_start: bool,
    /// Do not clear screen on exit
    #[arg(long = "no-clear")]
    pub no_clear: bool,
    /// Show error message if command fails
    #[arg(long = "show-cmd-error")]
    pub show_cmd_error: bool,
    //  --- Layout ---
    /// Set layout
    #[arg(
        long,
        default_value = "default",
        value_parser = clap::builder::PossibleValuesParser::new(
            ["default", "reverse", "reverse-list"]
        )
    )]
    pub layout: String,
    /// Shorthand for reverse layout
    #[arg(long)]
    pub reverse: bool,
    /// Height of skim's window
    ///
    /// Can either be a row count or a percentage
    #[arg(long, default_value = "40%")]
    pub height: String,
    /// Disable height feature
    #[arg(long = "no-height")]
    pub no_height: bool,
    /// Minimum height of skim's window
    ///
    /// Useful when the height is set as a percentage
    #[arg(long, default_value = "10")]
    pub min_height: String,
    /// Screen margin
    ///
    /// For each side, can be either a row count or a percentage
    /// Format can be one of:
    ///     - TRBL
    ///     - TB,RL
    ///     - T,RL,B
    ///     - T,R,B,L
    /// Example: 1,10%
    #[arg(long, default_value = "0")]
    pub margin: String,
    /// Set prompt
    #[arg(long, short, default_value = "> ")]
    pub prompt: String,
    /// Set prompt in command mode
    #[arg(long = "cmd-prompt", default_value = "> ")]
    pub cmd_prompt: String,
    //  --- Display ---
    /// Parse ANSI color codes in input strings
    #[arg(long)]
    pub ansi: bool,
    /// Number of spaces that make up a tab
    #[arg(long, default_value = "8")]
    pub tabstop: usize,
    /// Display info next to the query
    #[arg(long = "inline-info")]
    pub inline_info: bool,
    /// Set header, displayed next to the info
    #[arg(long)]
    pub header: Option<String>,
    /// Number of lines of the input treated as header
    #[arg(long = "header-lines", default_value = "0")]
    pub header_lines: usize,
    //  --- History ---
    /// History file
    #[arg(long)]
    pub history: Option<String>,
    /// Maximum number of query history entries to keep
    #[arg(long = "history-size", default_value = "1000")]
    pub history_size: usize,
    /// Command history file
    #[arg(long = "cmd-history")]
    pub cmd_history: Option<String>,
    /// Maximum number of query history entries to keep
    #[arg(long = "cmd-history-size", default_value = "1000")]
    pub cmd_history_size: usize,
    //  --- Preview ---
    /// Preview command
    ///
    /// Will be run against the selected entry
    /// Fields can be specified using curly braces
    /// Example: `less {1}`, `grep {2..} {1}`
    #[arg(long)]
    pub preview: Option<String>,
    /// Preview window layout
    ///
    /// Format: [up|down|left|right][:SIZE[%]][:hidden][:+SCROLL[-OFFSET]]
    #[arg(long = "preview-window", default_value = "right:50%")]
    pub preview_window: String,
    //  --- Scripting ---
    /// Initial query
    #[arg(long, short)]
    pub query: Option<String>,
    /// Initial query in interactive mode
    #[arg(long = "cmd-query")]
    pub cmd_query: Option<String>,
    /// Comma separated list of keys used to complete skim
    #[arg(long)]
    pub expect: Vec<String>,
    /// Read input delimited by ASCII NUL(\\0) characters
    #[arg(long)]
    pub read0: bool,
    /// Print output delimited by ASCII NUL(\\0) characters
    #[arg(long)]
    pub print0: bool,
    /// Print the query as the first line
    #[arg(long = "print-query")]
    pub print_query: bool,
    /// Print the command as the first line (after print-query)
    #[arg(long = "print-cmd")]
    pub print_cmd: bool,
    /// Print the command as the first line (after print-cmd)
    #[arg(long = "print-score")]
    pub print_score: bool,
    /// Automatically select the match if there is only one
    #[arg(long = "select-1", short = '1')]
    pub select1: bool,
    /// Automatically exit when no match is left
    #[arg(long = "exit-0", short = '0')]
    pub exit0: bool,
    /// Synchronous search for multi-staged filtering
    #[arg(long)]
    pub sync: bool,
    /// Pre-select the first n items in multi-selection mode
    #[arg(long = "pre-select-n", default_value = "0")]
    pub pre_select_n: usize,
    /// Pre-select the matched items in multi-selection mode
    ///
    /// Format: regex
    #[arg(long = "pre-select-pat", default_value = "")]
    pub pre_select_pat: String,
    /// Pre-select the items separated by newline character
    ///
    /// Exemple: 'item1\nitem2'
    #[arg(long = "pre-select-items", default_value = "")]
    pub pre_select_items: String,
    /// Pre-select the items read from this file
    #[arg(long = "pre-select-file")]
    pub pre_select_file: Option<String>,

    // pub engine_factory: Option<Rc<dyn MatchEngineFactory>>,
    // pub query_history: &'a [String],
    // pub cmd_collector: Rc<RefCell<dyn CommandCollector>>,
    // pub selector: Option<Arc<dyn Selector + Send + Sync>>,

    pub filter: Option<String>,

    /// Reserved for later use
    #[arg(short = 'x', long, hide = true)]
    pub extended: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub literal: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub cycle: bool,
    /// Reserved for later use
    #[arg(long = "hscroll-off", hide = true, default_value = "10")]
    pub hscroll_off: usize,
    /// Reserved for later use
    #[arg(long = "filepath-word", hide = true)]
    pub filepath_word: bool,
    /// Reserved for later use
    #[arg(long = "jump-labels", hide = true, default_value = "abcdefghijklmnopqrstuvwxyz")]
    pub jump_labels: String,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub border: bool,
    /// Reserved for later use
    #[arg(long = "no-bold", hide = true)]
    pub no_bold: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub info: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub pointer: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub marker: bool,
    /// Reserved for later use
    #[arg(long, hide = true)]
    pub phony: bool,
}

impl Default for SkimOptions {
    fn default() -> Self {
        Self::parse_from::<_, &str>([])
    }
}

impl SkimOptionsBuilder {
    pub fn build(&mut self) -> Result<SkimOptions, SkimOptionsBuilderError> {
        if let Some(true) = self.no_height {
            self.height = Some("100%".to_string());
        }

        if let Some(true) = self.reverse {
            self.layout = Some("reverse".to_string());
        }

        self.final_build()
    }
}
