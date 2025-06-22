complete -c sk -l min-query-length -d 'Minimum query length to start showing results' -r
complete -c sk -s t -l tiebreak -d 'Comma-separated list of sort criteria to apply when the scores are tied' -r -f -a "score\t''
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
complete -c sk -l algo -d 'Fuzzy matching algorithm' -r -f -a "skim_v1\t''
skim_v2\t''
clangd\t''"
complete -c sk -l case -d 'Case sensitivity' -r -f -a "respect\t''
ignore\t''
smart\t''"
complete -c sk -s b -l bind -d 'Comma separated list of bindings' -r
complete -c sk -s c -l cmd -d 'Command to invoke dynamically in interactive mode' -r
complete -c sk -s I -d 'Replace replstr with the selected item in commands' -r
complete -c sk -l color -d 'Set color theme' -r
complete -c sk -l skip-to-pattern -d 'Show the matched pattern at the line start' -r
complete -c sk -l layout -d 'Set layout' -r -f -a "default\t''
reverse\t''
reverse-list\t''"
complete -c sk -l height -d 'Height of skim\'s window' -r
complete -c sk -l min-height -d 'Minimum height of skim\'s window' -r
complete -c sk -l margin -d 'Screen margin' -r
complete -c sk -s p -l prompt -d 'Set prompt' -r
complete -c sk -l cmd-prompt -d 'Set prompt in command mode' -r
complete -c sk -l tabstop -d 'Number of spaces that make up a tab' -r
complete -c sk -l info -d 'Set matching result count display position' -r -f -a "default\t''
inline\t''
hidden\t''"
complete -c sk -l header -d 'Set header, displayed next to the info' -r
complete -c sk -l header-lines -d 'Number of lines of the input treated as header' -r
complete -c sk -l history -d 'History file' -r
complete -c sk -l history-size -d 'Maximum number of query history entries to keep' -r
complete -c sk -l cmd-history -d 'Command history file' -r
complete -c sk -l cmd-history-size -d 'Maximum number of query history entries to keep' -r
complete -c sk -l preview -d 'Preview command' -r
complete -c sk -l preview-window -d 'Preview window layout' -r
complete -c sk -s q -l query -d 'Initial query' -r
complete -c sk -l cmd-query -d 'Initial query in interactive mode' -r
complete -c sk -l expect -d '[Deprecated: Use --bind=<key>:accept(<key>) instead] Comma separated list of keys used to complete skim' -r
complete -c sk -l pre-select-n -d 'Pre-select the first n items in multi-selection mode' -r
complete -c sk -l pre-select-pat -d 'Pre-select the matched items in multi-selection mode' -r
complete -c sk -l pre-select-items -d 'Pre-select the items separated by newline character' -r
complete -c sk -l pre-select-file -d 'Pre-select the items read from this file' -r
complete -c sk -s f -l filter -d 'Query for filter mode' -r
complete -c sk -l shell -d 'Generate shell completion script' -r -f -a "bash\t''
elvish\t''
fish\t''
powershell\t''
zsh\t''"
complete -c sk -l tmux -d 'Run in a tmux popup' -r
complete -c sk -l hscroll-off -d 'Reserved for later use' -r
complete -c sk -l jump-labels -d 'Reserved for later use' -r
complete -c sk -l tac -d 'Show results in reverse order'
complete -c sk -l no-sort -d 'Do not sort the results'
complete -c sk -s e -l exact -d 'Run in exact mode'
complete -c sk -l regex -d 'Start in regex mode instead of fuzzy-match'
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
complete -c sk -l reverse -d 'Shorthand for reverse layout'
complete -c sk -l no-height -d 'Disable height feature'
complete -c sk -l ansi -d 'Parse ANSI color codes in input strings'
complete -c sk -l no-info -d 'Alias for --info=hidden'
complete -c sk -l inline-info -d 'Alias for --info=inline'
complete -c sk -l read0 -d 'Read input delimited by ASCII NUL(\\0) characters'
complete -c sk -l print0 -d 'Print output delimited by ASCII NUL(\\0) characters'
complete -c sk -l print-query -d 'Print the query as the first line'
complete -c sk -l print-cmd -d 'Print the command as the first line (after print-query)'
complete -c sk -l print-score -d 'Print the command as the first line (after print-cmd)'
complete -c sk -s 1 -l select-1 -d 'Automatically select the match if there is only one'
complete -c sk -s 0 -l exit-0 -d 'Automatically exit when no match is left'
complete -c sk -l sync -d 'Synchronous search for multi-staged filtering'
complete -c sk -s x -l extended -d 'Reserved for later use'
complete -c sk -l literal -d 'Reserved for later use'
complete -c sk -l cycle -d 'Reserved for later use'
complete -c sk -l filepath-word -d 'Reserved for later use'
complete -c sk -l border -d 'Reserved for later use'
complete -c sk -l no-bold -d 'Reserved for later use'
complete -c sk -l pointer -d 'Reserved for later use'
complete -c sk -l marker -d 'Reserved for later use'
complete -c sk -l phony -d 'Reserved for later use'
complete -c sk -s h -l help -d 'Print help (see more with \'--help\')'
complete -c sk -s V -l version -d 'Print version'
