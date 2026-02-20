complete -c sk -l min-query-length -d 'Minimum query length to start showing results' -r
complete -c sk -s t -l tiebreak -d 'Comma-separated list of sort criteria to apply when the scores are tied.' -r -f -a "score\t''
-score\t''
begin\t''
-begin\t''
end\t''
-end\t''
length\t''
-length\t''
index\t''
-index\t''"
complete -c sk -s n -l nth -d 'Fields to be matched' -r
complete -c sk -l with-nth -d 'Fields to be transformed' -r
complete -c sk -s d -l delimiter -d 'Delimiter between fields' -r
complete -c sk -l algo -d 'Fuzzy matching algorithm' -r -f -a "skim_v1\t'Original skim fuzzy matching algorithm (v1)'
skim_v2\t'Improved skim fuzzy matching algorithm (v2, default)'
clangd\t'Clangd fuzzy matching algorithm'
fzy\t'Fzy matching algorithm (https://github.com/jhawthorn/fzy)'
frizbee\t'Frizbee matching algorithm, typo resistant Will fallback to SkimV2 if the feature is not enabled'"
complete -c sk -l case -d 'Case sensitivity' -r -f -a "respect\t'Case-sensitive matching'
ignore\t'Case-insensitive matching'
smart\t'Smart case: case-insensitive unless query contains uppercase'"
complete -c sk -l typos -d 'Enable typo-tolerant matching' -r
complete -c sk -l split-match -d 'Enable split matching and set delimiter' -r
complete -c sk -s b -l bind -d 'Comma separated list of bindings' -r
complete -c sk -s c -l cmd -d 'Command to invoke dynamically in interactive mode' -r
complete -c sk -s I -d 'Replace replstr with the selected item in commands' -r
complete -c sk -l color -d 'Set color theme' -r
complete -c sk -l skip-to-pattern -d 'Show the matched pattern at the line start' -r
complete -c sk -l layout -d 'Set layout' -r -f -a "default\t'Display from the bottom of the screen'
reverse\t'Display from the top of the screen'
reverse-list\t'Display from the top of the screen, prompt at the bottom'"
complete -c sk -l height -d 'Height of skim\'s window' -r
complete -c sk -l min-height -d 'Minimum height of skim\'s window' -r
complete -c sk -l margin -d 'Screen margin' -r
complete -c sk -s p -l prompt -d 'Set prompt' -r
complete -c sk -l cmd-prompt -d 'Set prompt in command mode' -r
complete -c sk -l selector -d 'Set selected item icon' -r
complete -c sk -l multi-selector -d 'Set selected item icon' -r
complete -c sk -l tabstop -d 'Number of spaces that make up a tab' -r
complete -c sk -l ellipsis -d 'The characters used to display truncated lines' -r
complete -c sk -l info -d 'Set matching result count display position' -r -f -a "default\t''
inline\t''
hidden\t''"
complete -c sk -l header -d 'Set header, displayed next to the info' -r
complete -c sk -l header-lines -d 'Number of lines of the input treated as header' -r
complete -c sk -l border -d 'Draw borders around the UI components' -r -f -a "plain\t''
rounded\t''
double\t''
thick\t''
light-double-dashed\t''
heavy-double-dashed\t''
light-triple-dashed\t''
heavy-triple-dashed\t''
light-quadruple-dashed\t''
heavy-quadruple-dashed\t''
quadrant-inside\t''
quadrant-outside\t''"
complete -c sk -l history -d 'History file' -r
complete -c sk -l history-size -d 'Maximum number of query history entries to keep' -r
complete -c sk -l cmd-history -d 'Command history file' -r
complete -c sk -l cmd-history-size -d 'Maximum number of query history entries to keep' -r
complete -c sk -l preview -d 'Preview command' -r
complete -c sk -l preview-window -d 'Preview window layout' -r
complete -c sk -s q -l query -d 'Initial query' -r
complete -c sk -l cmd-query -d 'Initial query in interactive mode' -r
complete -c sk -l output-format -d 'Set the output format If set, overrides all print_ options Will be expanded the same way as preview or commands' -r
complete -c sk -l pre-select-n -d 'Pre-select the first n items in multi-selection mode' -r
complete -c sk -l pre-select-pat -d 'Pre-select the matched items in multi-selection mode' -r
complete -c sk -l pre-select-items -d 'Pre-select the items separated by newline character' -r
complete -c sk -l pre-select-file -d 'Pre-select the items read from this file' -r
complete -c sk -s f -l filter -d 'Query for filter mode' -r
complete -c sk -l shell -d 'Generate shell completion script' -r -f -a "bash\t'Bourne Again SHell'
elvish\t'Elvish shell'
fish\t'Friendly Interactive SHell'
nushell\t'Nushell (nu)'
power-shell\t'PowerShell'
zsh\t'Zsh'"
complete -c sk -l listen -d 'Run an IPC socket with optional name (defaults to sk)' -r
complete -c sk -l remote -d 'Send commands to an IPC socket with optional name (defaults to sk)' -r
complete -c sk -l tmux -d 'Run in a tmux popup' -r
complete -c sk -l log-file -d 'Pipe log output to a file' -r
complete -c sk -l flags -d 'Feature flags' -r -f -a "no-preview-pty\t'Disable preview PTY on linux'"
complete -c sk -l hscroll-off -r
complete -c sk -l jump-labels -r
complete -c sk -l scheme -r
complete -c sk -l tail -r
complete -c sk -l style -r
complete -c sk -l padding -r
complete -c sk -l border-label -r
complete -c sk -l border-label-pos -r
complete -c sk -l wrap-sign -r
complete -c sk -l gap -r
complete -c sk -l gap-line -r
complete -c sk -l freeze-left -r
complete -c sk -l freeze-right -r
complete -c sk -l scroll-off -r
complete -c sk -l gutter -r
complete -c sk -l gutter-raw -r
complete -c sk -l marker-multi-line -r
complete -c sk -l scrollbar -r
complete -c sk -l list-border -r
complete -c sk -l list-label -r
complete -c sk -l list-label-pos -r
complete -c sk -l info-command -r
complete -c sk -l separator -r
complete -c sk -l ghost -r
complete -c sk -l input-border -r
complete -c sk -l input-label -r
complete -c sk -l input-label-pos -r
complete -c sk -l preview-label -r
complete -c sk -l preview-label-pos -r
complete -c sk -l header-border -r
complete -c sk -l header-lines-border -r
complete -c sk -l footer -r
complete -c sk -l footer-border -r
complete -c sk -l footer-label -r
complete -c sk -l footer-label-pos -r
complete -c sk -l with-shell -r
complete -c sk -l expect -d 'Deprecated, kept for compatibility purposes. See accept() bind instead' -r
complete -c sk -l tac -d 'Show results in reverse order'
complete -c sk -l no-sort -d 'Do not sort the results'
complete -c sk -s e -l exact -d 'Run in exact mode'
complete -c sk -l regex -d 'Start in regex mode instead of fuzzy-match'
complete -c sk -l no-typos -d 'Disable typo-resistant matching'
complete -c sk -l normalize -d 'Normalize unicode characters'
complete -c sk -s m -l multi -d 'Enable multiple selection'
complete -c sk -l no-multi -d 'Disable multiple selection'
complete -c sk -l no-mouse -d 'Disable mouse'
complete -c sk -s i -l interactive -d 'Start skim in interactive mode'
complete -c sk -l no-hscroll -d 'Disable horizontal scroll'
complete -c sk -l keep-right -d 'Keep the right end of the line visible on overflow'
complete -c sk -l no-clear-if-empty -d 'Do not clear previous line if the command returns an empty result'
complete -c sk -l no-clear-start -d 'Do not clear items on start'
complete -c sk -l no-clear -d 'Do not clear screen on exit'
complete -c sk -l show-cmd-error -d 'Show error message if command fails'
complete -c sk -l cycle -d 'Cycle the results by wrapping around when scrolling'
complete -c sk -l disabled -d 'Disable matching entirely'
complete -c sk -l reverse -d 'Shorthand for reverse layout'
complete -c sk -l no-height -d 'Disable height (force full screen)'
complete -c sk -l ansi -d 'Parse ANSI color codes in input strings'
complete -c sk -l no-info -d 'Alias for --info=hidden'
complete -c sk -l inline-info -d 'Alias for --info=inline'
complete -c sk -l wrap -d 'Wrap items in the item list'
complete -c sk -l read0 -d 'Read input delimited by ASCII NUL(\\0) characters'
complete -c sk -l print0 -d 'Print output delimited by ASCII NUL(\\0) characters'
complete -c sk -l print-query -d 'Print the query as the first line'
complete -c sk -l print-cmd -d 'Print the command as the first line (after print-query)'
complete -c sk -l print-score -d 'Print the score after each item'
complete -c sk -l print-header -d 'Print the header as the first line (after print-score)'
complete -c sk -l print-current -d 'Print the current (highlighted) item as the first line (after print-header)'
complete -c sk -l no-strip-ansi -d 'Print the ANSI codes, making the output exactly match the input even when --ansi is on'
complete -c sk -s 1 -l select-1 -d 'Do not enter the TUI if the query passed in -q matches only one item and return it'
complete -c sk -s 0 -l exit-0 -d 'Do not enter the TUI if the query passed in -q does not match any item'
complete -c sk -l sync -d 'Synchronous search for multi-staged filtering'
complete -c sk -l shell-bindings -d 'Generate shell key bindings - only for bash, zsh and fish'
complete -c sk -l man -d 'Generate man page and output it to stdout'
complete -c sk -s x -l extended
complete -c sk -l literal
complete -c sk -l filepath-word
complete -c sk -l no-bold
complete -c sk -l phony
complete -c sk -l no-color
complete -c sk -l highlight-line
complete -c sk -l no-multi-line
complete -c sk -l raw
complete -c sk -l track
complete -c sk -l no-scrollbar
complete -c sk -l no-input
complete -c sk -l no-separator
complete -c sk -l header-first
complete -c sk -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sk -s V -l version -d 'Print version'
