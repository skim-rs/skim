module completions {

  def "nu-complete sk tiebreak" [] {
    [ "score" "-score" "begin" "-begin" "end" "-end" "length" "-length" "index" "-index" ]
  }

  def "nu-complete sk algorithm" [] {
    [ "skim_v1" "skim_v2" "clangd" ]
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

  def "nu-complete sk shell" [] {
    [ "bash" "elvish" "fish" "nushell" "power-shell" "zsh" ]
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
    --bind(-b): string        # Comma separated list of bindings
    --multi(-m)               # Enable multiple selection
    --no-multi                # Disable multiple selection
    --no-mouse                # Disable mouse
    --cmd(-c): string         # Command to invoke dynamically in interactive mode
    --interactive(-i)         # Run in interactive mode
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
    --info: string@"nu-complete sk info" # Set matching result count display position
    --no-info                 # Alias for --info=hidden
    --inline-info             # Alias for --info=inline
    --header: string          # Set header, displayed next to the info
    --header-lines: string    # Number of lines of the input treated as header
    --border                  # Draw borders around the UI components
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
    --print-score             # Print the command as the first line (after print-cmd)
    --select-1(-1)            # Automatically select the match if there is only one
    --exit-0(-0)              # Automatically exit when no match is left
    --sync                    # Synchronous search for multi-staged filtering
    --pre-select-n: string    # Pre-select the first n items in multi-selection mode
    --pre-select-pat: string  # Pre-select the matched items in multi-selection mode
    --pre-select-items: string # Pre-select the items separated by newline character
    --pre-select-file: string # Pre-select the items read from this file
    --filter(-f): string      # Query for filter mode
    --shell: string@"nu-complete sk shell" # Generate shell completion script
    --shell-bindings          # Generate shell key bindings - only for bash, zsh and fish
    --man                     # Generate man page and output it to stdout
    --tmux: string            # Run in a tmux popup
    --log-file: string        # Pipe log output to a file
    --extended(-x)            # Reserved for later use
    --literal                 # Reserved for later use
    --hscroll-off: string     # Reserved for later use
    --filepath-word           # Reserved for later use
    --jump-labels: string     # Reserved for later use
    --no-bold                 # Reserved for later use
    --pointer                 # Reserved for later use
    --marker                  # Reserved for later use
    --phony                   # Reserved for later use
    --expect: string          # Deprecated, kept for compatibility purposes. See accept() bind instead
    --help(-h)                # Print help (see more with '--help')
    --version(-V)             # Print version
  ]

}

export use completions *
