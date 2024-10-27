use clap::Parser;
use derive_builder::Builder;

use crate::item::RankCriteria;
use crate::{CaseMatching, FuzzyAlgorithm};

#[derive(Builder)]
#[builder(build_fn(name = "final_build"))]
#[builder(default)]
#[derive(Parser)]
#[command(name = "sk", about = "A rust fuzzy finder", args_override_self = true)]
pub struct SkimOptions {
    //  --- Search ---
    /// Show results in reverse order
    #[arg(long, help_heading = "Search")]
    pub tac: bool,
    /// Do not sort the results
    #[arg(long, help_heading = "Search")]
    pub no_sort: bool,
    /// Comma separated tie breakder criteria
    #[arg(
        short,
        long,
        default_value = "score,begin,end",
        value_enum,
        value_delimiter = ',',
        help_heading = "Search"
    )]
    pub tiebreak: Vec<RankCriteria>,
    /// Fields to be matched
    ///
    /// Example: 1,3..5
    #[arg(short, long, default_value = "", help_heading = "Search")]
    pub nth: Vec<String>,
    /// Fields to be transformed
    ///
    /// Example: 1,3..5
    #[arg(long, default_value = "", help_heading = "Search")]
    pub with_nth: Vec<String>,
    /// Delimiter between fields
    ///
    /// In regex format
    #[arg(short, long, default_value = r"[\t\n ]+", help_heading = "Search")]
    pub delimiter: String,
    /// Run in exact mode
    #[arg(short, long, help_heading = "Search")]
    pub exact: bool,
    /// Start in regex mode
    #[arg(long, help_heading = "Search")]
    pub regex: bool,
    /// Fuzzy matching algorithm
    #[arg(long = "algo", default_value = "skim_v2", value_enum, help_heading = "Search")]
    pub algorithm: FuzzyAlgorithm,
    /// Case sensitivity
    #[arg(long, default_value = "smart", value_enum, help_heading = "Search")]
    pub case: CaseMatching,
    //  --- Interface ---
    /// Comma separated list of bindings
    ///
    /// Example : ctrl-j:accept,ctrl-k:kill-line
    #[arg(short, long, help_heading = "Interface")]
    pub bind: Vec<String>,
    /// Enable multiple selection
    #[arg(short, long, overrides_with = "no_multi", help_heading = "Interface")]
    pub multi: bool,
    /// Disable multiple selection
    #[arg(long, conflicts_with = "multi" , help_heading = "Interface")]
    pub no_multi: bool,
    /// Disable mouse
    #[arg(long, help_heading = "Interface")]
    pub no_mouse: bool,
    /// Command to invoke dynamically
    ///
    /// Will be invoked using `sh -c`
    #[arg(short, long, help_heading = "Interface")]
    pub cmd: Option<String>,
    /// Run in interactive mode
    #[arg(short, long, help_heading = "Interface")]
    pub interactive: bool,
    /// Set color theme
    ///
    /// Format: [BASE][,COLOR:ANSI]
    #[arg(long, help_heading = "Interface")]
    pub color: Option<String>,
    /// Disable horizontal scroll
    #[arg(long, help_heading = "Interface")]
    pub no_hscroll: bool,
    /// Keep the right end of the line visible on overflow
    #[arg(long, help_heading = "Interface")]
    pub keep_right: bool,
    /// Show the matched pattern at the line start
    #[arg(long, help_heading = "Interface")]
    pub skip_to_pattern: Option<String>,
    /// Do not clear previous line if the command returns an empty result
    #[arg(long, help_heading = "Interface")]
    pub no_clear_if_empty: bool,
    /// Do not clear items on start
    #[arg(long, help_heading = "Interface")]
    pub no_clear_start: bool,
    /// Do not clear screen on exit
    #[arg(long, help_heading = "Interface")]
    pub no_clear: bool,
    /// Show error message if command fails
    #[arg(long, help_heading = "Interface")]
    pub show_cmd_error: bool,
    //  --- Layout ---
    /// Set layout
    #[arg(
        long,
        default_value = "default",
        value_parser = clap::builder::PossibleValuesParser::new(
            ["default", "reverse", "reverse-list"]
        ),
        help_heading = "Layout"
    )]
    pub layout: String,
    /// Shorthand for reverse layout
    #[arg(long, help_heading = "Layout")]
    pub reverse: bool,
    /// Height of skim's window
    ///
    /// Can either be a row count or a percentage
    #[arg(long, default_value = "40%", help_heading = "Layout")]
    pub height: String,
    /// Disable height feature
    #[arg(long, help_heading = "Layout")]
    pub no_height: bool,
    /// Minimum height of skim's window
    ///
    /// Useful when the height is set as a percentage
    #[arg(long, default_value = "10", help_heading = "Layout")]
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
    #[arg(long, default_value = "0", help_heading = "Layout")]
    pub margin: String,
    /// Set prompt
    #[arg(long, short, default_value = "> ", help_heading = "Layout")]
    pub prompt: String,
    /// Set prompt in command mode
    #[arg(long, default_value = "> ", help_heading = "Layout")]
    pub cmd_prompt: String,
    //  --- Display ---
    /// Parse ANSI color codes in input strings
    #[arg(long, help_heading = "Display")]
    pub ansi: bool,
    /// Number of spaces that make up a tab
    #[arg(long, default_value = "8", help_heading = "Display")]
    pub tabstop: usize,
    /// Display info next to the query
    #[arg(long, help_heading = "Display")]
    pub inline_info: bool,
    /// Set header, displayed next to the info
    #[arg(long, help_heading = "Display")]
    pub header: Option<String>,
    /// Number of lines of the input treated as header
    #[arg(long, default_value = "0", help_heading = "Display")]
    pub header_lines: usize,
    //  --- History ---
    /// History file
    #[arg(long, help_heading = "History")]
    pub history: Option<String>,
    /// Maximum number of query history entries to keep
    #[arg(long, default_value = "1000", help_heading = "History")]
    pub history_size: usize,
    /// Command history file
    #[arg(long, help_heading = "History")]
    pub cmd_history: Option<String>,
    /// Maximum number of query history entries to keep
    #[arg(long, default_value = "1000", help_heading = "History")]
    pub cmd_history_size: usize,
    //  --- Preview ---
    /// Preview command
    ///
    /// Will be run against the selected entry
    /// Fields can be specified using curly braces
    /// Example: `less {1}`, `grep {2..} {1}`
    #[arg(long, help_heading = "Preview")]
    pub preview: Option<String>,
    /// Preview window layout
    ///
    /// Format: [up|down|left|right][:SIZE[%]][:hidden][:+SCROLL[-OFFSET]]
    #[arg(long, default_value = "right:50%", help_heading = "Preview")]
    pub preview_window: String,
    //  --- Scripting ---
    /// Initial query
    #[arg(long, short, help_heading = "Scripting")]
    pub query: Option<String>,
    /// Initial query in interactive mode
    #[arg(long, help_heading = "Scripting")]
    pub cmd_query: Option<String>,
    /// Comma separated list of keys used to complete skim
    #[arg(long, help_heading = "Scripting")]
    pub expect: Vec<String>,
    /// Read input delimited by ASCII NUL(\\0) characters
    #[arg(long, help_heading = "Scripting")]
    pub read0: bool,
    /// Print output delimited by ASCII NUL(\\0) characters
    #[arg(long, help_heading = "Scripting")]
    pub print0: bool,
    /// Print the query as the first line
    #[arg(long, help_heading = "Scripting")]
    pub print_query: bool,
    /// Print the command as the first line (after print-query)
    #[arg(long, help_heading = "Scripting")]
    pub print_cmd: bool,
    /// Print the command as the first line (after print-cmd)
    #[arg(long, help_heading = "Scripting")]
    pub print_score: bool,
    /// Automatically select the match if there is only one
    #[arg(long, short = '1', help_heading = "Scripting")]
    pub select_1: bool,
    /// Automatically exit when no match is left
    #[arg(long, short = '0', help_heading = "Scripting")]
    pub exit_0: bool,
    /// Synchronous search for multi-staged filtering
    #[arg(long, help_heading = "Scripting")]
    pub sync: bool,
    /// Pre-select the first n items in multi-selection mode
    #[arg(long, default_value = "0", help_heading = "Scripting")]
    pub pre_select_n: usize,
    /// Pre-select the matched items in multi-selection mode
    ///
    /// Format: regex
    #[arg(long, default_value = "", help_heading = "Scripting")]
    pub pre_select_pat: String,
    /// Pre-select the items separated by newline character
    ///
    /// Exemple: 'item1\nitem2'
    #[arg(long, default_value = "", help_heading = "Scripting")]
    pub pre_select_items: String,
    /// Pre-select the items read from this file
    #[arg(long, help_heading = "Scripting")]
    pub pre_select_file: Option<String>,

    /// Query for filter mode
    #[arg(long, help_heading = "Scripting")]
    pub filter: Option<String>,

    /// Reserved for later use
    #[arg(short = 'x', long, hide = true, help_heading = "Reserved for later use")]
    pub extended: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub literal: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub cycle: bool,
    /// Reserved for later use
    #[arg(
        long,
        hide = true,
        default_value = "10",
        help_heading = "Reserved for later use"
    )]
    pub hscroll_off: usize,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub filepath_word: bool,
    /// Reserved for later use
    #[arg(
        long,
        hide = true,
        default_value = "abcdefghijklmnopqrstuvwxyz",
        help_heading = "Reserved for later use"
    )]
    pub jump_labels: String,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub border: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub no_bold: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub info: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub pointer: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
    pub marker: bool,
    /// Reserved for later use
    #[arg(long, hide = true, help_heading = "Reserved for later use")]
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

impl SkimOptions {
    pub fn build(mut self) -> Self {
        if self.no_height {
            self.height = String::from("100%");
        }

        if self.reverse {
            self.layout = String::from("reverse");
        }
        let history_binds = String::from("ctrl-p:previous-history,ctrl-n:next-history");
        if self.history.is_some() && self.cmd_history.is_some() {
            self.bind.insert(0, history_binds.clone());
        }

        self
    }
}
