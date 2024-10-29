const completion: Fig.Spec = {
  name: "sk",
  description: "sk - fuzzy finder in Rust",
  options: [
    {
      name: ["-t", "--tiebreak"],
      description: "Comma-separated list of sort criteria to apply when the scores are tied.",
      isRepeatable: true,
      args: {
        name: "tiebreak",
        isOptional: true,
        suggestions: [
          "score",
          "begin",
          "end",
          "-score",
          "-begin",
          "-end",
          "length",
          "-length",
        ],
      },
    },
    {
      name: ["-n", "--nth"],
      description: "Fields to be matched",
      isRepeatable: true,
      args: {
        name: "nth",
        isOptional: true,
      },
    },
    {
      name: "--with-nth",
      description: "Fields to be transformed",
      isRepeatable: true,
      args: {
        name: "with_nth",
        isOptional: true,
      },
    },
    {
      name: ["-d", "--delimiter"],
      description: "Delimiter between fields",
      isRepeatable: true,
      args: {
        name: "delimiter",
        isOptional: true,
      },
    },
    {
      name: "--algo",
      description: "Fuzzy matching algorithm",
      isRepeatable: true,
      args: {
        name: "algorithm",
        isOptional: true,
        suggestions: [
          "skim_v1",
          "skim_v2",
          "clangd",
        ],
      },
    },
    {
      name: "--case",
      description: "Case sensitivity",
      isRepeatable: true,
      args: {
        name: "case",
        isOptional: true,
        suggestions: [
          "respect",
          "ignore",
          "smart",
        ],
      },
    },
    {
      name: ["-b", "--bind"],
      description: "Comma separated list of bindings",
      isRepeatable: true,
      args: {
        name: "bind",
        isOptional: true,
      },
    },
    {
      name: ["-c", "--cmd"],
      description: "Command to invoke dynamically in interactive mode",
      isRepeatable: true,
      args: {
        name: "cmd",
        isOptional: true,
      },
    },
    {
      name: "--color",
      description: "Set color theme",
      isRepeatable: true,
      args: {
        name: "color",
        isOptional: true,
      },
    },
    {
      name: "--skip-to-pattern",
      description: "Show the matched pattern at the line start",
      isRepeatable: true,
      args: {
        name: "skip_to_pattern",
        isOptional: true,
      },
    },
    {
      name: "--layout",
      description: "Set layout",
      isRepeatable: true,
      args: {
        name: "layout",
        isOptional: true,
        suggestions: [
          "default",
          "reverse",
          "reverse-list",
        ],
      },
    },
    {
      name: "--height",
      description: "Height of skim's window",
      isRepeatable: true,
      args: {
        name: "height",
        isOptional: true,
      },
    },
    {
      name: "--min-height",
      description: "Minimum height of skim's window",
      isRepeatable: true,
      args: {
        name: "min_height",
        isOptional: true,
      },
    },
    {
      name: "--margin",
      description: "Screen margin",
      isRepeatable: true,
      args: {
        name: "margin",
        isOptional: true,
      },
    },
    {
      name: ["-p", "--prompt"],
      description: "Set prompt",
      isRepeatable: true,
      args: {
        name: "prompt",
        isOptional: true,
      },
    },
    {
      name: "--cmd-prompt",
      description: "Set prompt in command mode",
      isRepeatable: true,
      args: {
        name: "cmd_prompt",
        isOptional: true,
      },
    },
    {
      name: "--tabstop",
      description: "Number of spaces that make up a tab",
      isRepeatable: true,
      args: {
        name: "tabstop",
        isOptional: true,
      },
    },
    {
      name: "--header",
      description: "Set header, displayed next to the info",
      isRepeatable: true,
      args: {
        name: "header",
        isOptional: true,
      },
    },
    {
      name: "--header-lines",
      description: "Number of lines of the input treated as header",
      isRepeatable: true,
      args: {
        name: "header_lines",
        isOptional: true,
      },
    },
    {
      name: "--history",
      description: "History file",
      isRepeatable: true,
      args: {
        name: "history",
        isOptional: true,
      },
    },
    {
      name: "--history-size",
      description: "Maximum number of query history entries to keep",
      isRepeatable: true,
      args: {
        name: "history_size",
        isOptional: true,
      },
    },
    {
      name: "--cmd-history",
      description: "Command history file",
      isRepeatable: true,
      args: {
        name: "cmd_history",
        isOptional: true,
      },
    },
    {
      name: "--cmd-history-size",
      description: "Maximum number of query history entries to keep",
      isRepeatable: true,
      args: {
        name: "cmd_history_size",
        isOptional: true,
      },
    },
    {
      name: "--preview",
      description: "Preview command",
      isRepeatable: true,
      args: {
        name: "preview",
        isOptional: true,
      },
    },
    {
      name: "--preview-window",
      description: "Preview window layout",
      isRepeatable: true,
      args: {
        name: "preview_window",
        isOptional: true,
      },
    },
    {
      name: ["-q", "--query"],
      description: "Initial query",
      isRepeatable: true,
      args: {
        name: "query",
        isOptional: true,
      },
    },
    {
      name: "--cmd-query",
      description: "Initial query in interactive mode",
      isRepeatable: true,
      args: {
        name: "cmd_query",
        isOptional: true,
      },
    },
    {
      name: "--expect",
      description: "Comma separated list of keys used to complete skim",
      isRepeatable: true,
      args: {
        name: "expect",
        isOptional: true,
      },
    },
    {
      name: "--pre-select-n",
      description: "Pre-select the first n items in multi-selection mode",
      isRepeatable: true,
      args: {
        name: "pre_select_n",
        isOptional: true,
      },
    },
    {
      name: "--pre-select-pat",
      description: "Pre-select the matched items in multi-selection mode",
      isRepeatable: true,
      args: {
        name: "pre_select_pat",
        isOptional: true,
      },
    },
    {
      name: "--pre-select-items",
      description: "Pre-select the items separated by newline character",
      isRepeatable: true,
      args: {
        name: "pre_select_items",
        isOptional: true,
      },
    },
    {
      name: "--pre-select-file",
      description: "Pre-select the items read from this file",
      isRepeatable: true,
      args: {
        name: "pre_select_file",
        isOptional: true,
      },
    },
    {
      name: ["-f", "--filter"],
      description: "Query for filter mode",
      isRepeatable: true,
      args: {
        name: "filter",
        isOptional: true,
      },
    },
    {
      name: "--hscroll-off",
      description: "Reserved for later use",
      hidden: true,
      isRepeatable: true,
      args: {
        name: "hscroll_off",
        isOptional: true,
      },
    },
    {
      name: "--jump-labels",
      description: "Reserved for later use",
      hidden: true,
      isRepeatable: true,
      args: {
        name: "jump_labels",
        isOptional: true,
      },
    },
    {
      name: "--tac",
      description: "Show results in reverse order",
    },
    {
      name: "--no-sort",
      description: "Do not sort the results",
    },
    {
      name: ["-e", "--exact"],
      description: "Run in exact mode",
    },
    {
      name: "--regex",
      description: "Start in regex mode instead of fuzzy-match",
    },
    {
      name: ["-m", "--multi"],
      description: "Enable multiple selection",
    },
    {
      name: "--no-multi",
      description: "Disable multiple selection",
      exclusiveOn: [
        "-m",
        "--multi",
      ],
    },
    {
      name: "--no-mouse",
      description: "Disable mouse",
    },
    {
      name: ["-i", "--interactive"],
      description: "Run in interactive mode",
    },
    {
      name: "--no-hscroll",
      description: "Disable horizontal scroll",
    },
    {
      name: "--keep-right",
      description: "Keep the right end of the line visible on overflow",
    },
    {
      name: "--no-clear-if-empty",
      description: "Do not clear previous line if the command returns an empty result",
    },
    {
      name: "--no-clear-start",
      description: "Do not clear items on start",
    },
    {
      name: "--no-clear",
      description: "Do not clear screen on exit",
    },
    {
      name: "--show-cmd-error",
      description: "Show error message if command fails",
    },
    {
      name: "--reverse",
      description: "Shorthand for reverse layout",
    },
    {
      name: "--no-height",
      description: "Disable height feature",
    },
    {
      name: "--ansi",
      description: "Parse ANSI color codes in input strings",
    },
    {
      name: "--inline-info",
      description: "Display info next to the query",
    },
    {
      name: "--read0",
      description: "Read input delimited by ASCII NUL(\\\\0) characters",
    },
    {
      name: "--print0",
      description: "Print output delimited by ASCII NUL(\\\\0) characters",
    },
    {
      name: "--print-query",
      description: "Print the query as the first line",
    },
    {
      name: "--print-cmd",
      description: "Print the command as the first line (after print-query)",
    },
    {
      name: "--print-score",
      description: "Print the command as the first line (after print-cmd)",
    },
    {
      name: ["-1", "--select-1"],
      description: "Automatically select the match if there is only one",
    },
    {
      name: ["-0", "--exit-0"],
      description: "Automatically exit when no match is left",
    },
    {
      name: "--sync",
      description: "Synchronous search for multi-staged filtering",
    },
    {
      name: ["-x", "--extended"],
      description: "Reserved for later use",
    },
    {
      name: "--literal",
      description: "Reserved for later use",
    },
    {
      name: "--cycle",
      description: "Reserved for later use",
    },
    {
      name: "--filepath-word",
      description: "Reserved for later use",
    },
    {
      name: "--border",
      description: "Reserved for later use",
    },
    {
      name: "--no-bold",
      description: "Reserved for later use",
    },
    {
      name: "--info",
      description: "Reserved for later use",
    },
    {
      name: "--pointer",
      description: "Reserved for later use",
    },
    {
      name: "--marker",
      description: "Reserved for later use",
    },
    {
      name: "--phony",
      description: "Reserved for later use",
    },
    {
      name: ["-h", "--help"],
      description: "Print help (see more with '--help')",
    },
  ],
};

export default completion;
