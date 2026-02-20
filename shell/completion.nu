module completions {

  def "nu-complete sk tiebreak" [] {
    [ "score" "-score" "begin" "-begin" "end" "-end" "length" "-length" "index" "-index" ]
  }

  def "nu-complete sk algorithm" [] {
    [ "skim_v1" "skim_v2" "clangd" "fzy" "frizbee" ]
  }

  def "nu-complete sk case" [] {
    [ "respect" "ignore" "smart" ]
  }

  def "nu-complete sk layout" [] {
    [ "default" "reverse" "reverse-list" ]
  }

  def "nu-complete sk info" [] {
    [ "default" "inline" "hidden" ]
  }

  def "nu-complete sk border" [] {
    [ "plain" "rounded" "double" "thick" "light-double-dashed" "heavy-double-dashed" "light-triple-dashed" "heavy-triple-dashed" "light-quadruple-dashed" "heavy-quadruple-dashed" "quadrant-inside" "quadrant-outside" ]
  }

  def "nu-complete sk shell" [] {
    [ "bash" "elvish" "fish" "nushell" "power-shell" "zsh" ]
  }

  def "nu-complete sk flags" [] {
    [ "no-preview-pty" ]
  }

  # Fuzzy Finder in rust!
  export extern sk [
    --tac                     # Show results in reverse order
    --min-query-length: string # Minimum query length to start showing results
    --no-sort                 # Do not sort the results
    --tiebreak(-t): string@"nu-complete sk tiebreak" # Comma-separated list of sort criteria to apply when the scores are tied.
    --nth(-n): string         # Fields to be matched
    --with-nth: string        # Fields to be transformed
    --delimiter(-d): string   # Delimiter between fields
    --exact(-e)               # Run in exact mode
    --regex                   # Start in regex mode instead of fuzzy-match
    --algo: string@"nu-complete sk algorithm" # Fuzzy matching algorithm
    --case: string@"nu-complete sk case" # Case sensitivity
    --typos: string           # Enable typo-tolerant matching
    --normalize               # Normalize unicode characters
    --split-match: string     # Enable split matching and set delimiter
    --bind(-b): string        # Comma separated list of bindings
    --multi(-m)               # Enable multiple selection
    --no-multi                # Disable multiple selection
    --no-mouse                # Disable mouse
    --cmd(-c): string         # Command to invoke dynamically in interactive mode
    --interactive(-i)         # Start skim in interactive mode
    -I: string                # Replace replstr with the selected item in commands
    --color: string           # Set color theme
    --no-hscroll              # Disable horizontal scroll
    --keep-right              # Keep the right end of the line visible on overflow
    --skip-to-pattern: string # Show the matched pattern at the line start
    --no-clear-if-empty       # Do not clear previous line if the command returns an empty result
    --no-clear-start          # Do not clear items on start
    --no-clear                # Do not clear screen on exit
    --show-cmd-error          # Show error message if command fails
    --cycle                   # Cycle the results by wrapping around when scrolling
    --disabled                # Disable matching entirely
    --layout: string@"nu-complete sk layout" # Set layout
    --reverse                 # Shorthand for reverse layout
    --height: string          # Height of skim's window
    --no-height               # Disable height (force full screen)
    --min-height: string      # Minimum height of skim's window
    --margin: string          # Screen margin
    --prompt(-p): string      # Set prompt
    --cmd-prompt: string      # Set prompt in command mode
    --selector: string        # Set selected item icon
    --multi-selector: string  # Set selected item icon
    --ansi                    # Parse ANSI color codes in input strings
    --tabstop: string         # Number of spaces that make up a tab
    --ellipsis: string        # The characters used to display truncated lines
    --info: string@"nu-complete sk info" # Set matching result count display position
    --no-info                 # Alias for --info=hidden
    --inline-info             # Alias for --info=inline
    --header: string          # Set header, displayed next to the info
    --header-lines: string    # Number of lines of the input treated as header
    --border: string@"nu-complete sk border" # Draw borders around the UI components
    --wrap                    # Wrap items in the item list
    --history: string         # History file
    --history-size: string    # Maximum number of query history entries to keep
    --cmd-history: string     # Command history file
    --cmd-history-size: string # Maximum number of query history entries to keep
    --preview: string         # Preview command
    --preview-window: string  # Preview window layout
    --query(-q): string       # Initial query
    --cmd-query: string       # Initial query in interactive mode
    --read0                   # Read input delimited by ASCII NUL(\0) characters
    --print0                  # Print output delimited by ASCII NUL(\0) characters
    --print-query             # Print the query as the first line
    --print-cmd               # Print the command as the first line (after print-query)
    --print-score             # Print the score after each item
    --print-header            # Print the header as the first line (after print-score)
    --print-current           # Print the current (highlighted) item as the first line (after print-header)
    --output-format: string   # Set the output format If set, overrides all print_ options Will be expanded the same way as preview or commands
    --no-strip-ansi           # Print the ANSI codes, making the output exactly match the input even when --ansi is on
    --select-1(-1)            # Do not enter the TUI if the query passed in -q matches only one item and return it
    --exit-0(-0)              # Do not enter the TUI if the query passed in -q does not match any item
    --sync                    # Synchronous search for multi-staged filtering
    --pre-select-n: string    # Pre-select the first n items in multi-selection mode
    --pre-select-pat: string  # Pre-select the matched items in multi-selection mode
    --pre-select-items: string # Pre-select the items separated by newline character
    --pre-select-file: string # Pre-select the items read from this file
    --filter(-f): string      # Query for filter mode
    --shell: string@"nu-complete sk shell" # Generate shell completion script
    --shell-bindings          # Generate shell key bindings - only for bash, zsh and fish
    --man                     # Generate man page and output it to stdout
    --listen: string          # Run an IPC socket with optional name (defaults to sk)
    --remote: string          # Send commands to an IPC socket with optional name (defaults to sk)
    --tmux: string            # Run in a tmux popup
    --log-file: string        # Pipe log output to a file
    --flags: string@"nu-complete sk flags" # Feature flags
    --extended(-x)
    --literal
    --hscroll-off: string
    --filepath-word
    --jump-labels: string
    --no-bold
    --phony
    --scheme: string
    --tail: string
    --style: string
    --no-color
    --padding: string
    --border-label: string
    --border-label-pos: string
    --highlight-line
    --wrap-sign: string
    --no-multi-line
    --raw
    --track
    --gap: string
    --gap-line: string
    --freeze-left: string
    --freeze-right: string
    --scroll-off: string
    --gutter: string
    --gutter-raw: string
    --marker-multi-line: string
    --scrollbar: string
    --no-scrollbar
    --list-border: string
    --list-label: string
    --list-label-pos: string
    --no-input
    --info-command: string
    --separator: string
    --no-separator
    --ghost: string
    --input-border: string
    --input-label: string
    --input-label-pos: string
    --preview-label: string
    --preview-label-pos: string
    --header-first
    --header-border: string
    --header-lines-border: string
    --footer: string
    --footer-border: string
    --footer-label: string
    --footer-label-pos: string
    --with-shell: string
    --expect: string          # Deprecated, kept for compatibility purposes. See accept() bind instead
    --help(-h)                # Print help (see more with '--help')
    --version(-V)             # Print version
  ]

}

export use completions *
