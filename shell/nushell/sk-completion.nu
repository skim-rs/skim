module completions {

  def "nu-complete sk tiebreak" [] {
    [ "score" "begin" "end" "-score" "-begin" "-end" "length" "-length" ]
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

  # sk - fuzzy finder in Rust
  export extern sk [
    --tac                     # Show results in reverse order
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
    --color: string           # Set color theme
    --no-hscroll              # Disable horizontal scroll
    --keep-right              # Keep the right end of the line visible on overflow
    --skip-to-pattern: string # Show the matched pattern at the line start
    --no-clear-if-empty       # Do not clear previous line if the command returns an empty result
    --no-clear-start          # Do not clear items on start
    --no-clear                # Do not clear screen on exit
    --show-cmd-error          # Show error message if command fails
    --layout: string@"nu-complete sk layout" # Set layout
    --reverse                 # Shorthand for reverse layout
    --height: string          # Height of skim's window
    --no-height               # Disable height feature
    --min-height: string      # Minimum height of skim's window
    --margin: string          # Screen margin
    --prompt(-p): string      # Set prompt
    --cmd-prompt: string      # Set prompt in command mode
    --ansi                    # Parse ANSI color codes in input strings
    --tabstop: string         # Number of spaces that make up a tab
    --inline-info             # Display info next to the query
    --header: string          # Set header, displayed next to the info
    --header-lines: string    # Number of lines of the input treated as header
    --history: string         # History file
    --history-size: string    # Maximum number of query history entries to keep
    --cmd-history: string     # Command history file
    --cmd-history-size: string # Maximum number of query history entries to keep
    --preview: string         # Preview command
    --preview-window: string  # Preview window layout
    --query(-q): string       # Initial query
    --cmd-query: string       # Initial query in interactive mode
    --expect: string          # Comma separated list of keys used to complete skim
    --read0                   # Read input delimited by ASCII NUL(\\0) characters
    --print0                  # Print output delimited by ASCII NUL(\\0) characters
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
    --extended(-x)            # Reserved for later use
    --literal                 # Reserved for later use
    --cycle                   # Reserved for later use
    --hscroll-off: string     # Reserved for later use
    --filepath-word           # Reserved for later use
    --jump-labels: string     # Reserved for later use
    --border                  # Reserved for later use
    --no-bold                 # Reserved for later use
    --info                    # Reserved for later use
    --pointer                 # Reserved for later use
    --marker                  # Reserved for later use
    --phony                   # Reserved for later use
    --help(-h)                # Print help (see more with '--help')
  ]

}

export use completions *
