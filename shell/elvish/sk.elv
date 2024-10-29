
use builtin;
use str;

set edit:completion:arg-completer[sk] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'sk'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'sk'= {
            cand -t 'Comma-separated list of sort criteria to apply when the scores are tied.'
            cand --tiebreak 'Comma-separated list of sort criteria to apply when the scores are tied.'
            cand -n 'Fields to be matched'
            cand --nth 'Fields to be matched'
            cand --with-nth 'Fields to be transformed'
            cand -d 'Delimiter between fields'
            cand --delimiter 'Delimiter between fields'
            cand --algo 'Fuzzy matching algorithm'
            cand --case 'Case sensitivity'
            cand -b 'Comma separated list of bindings'
            cand --bind 'Comma separated list of bindings'
            cand -c 'Command to invoke dynamically in interactive mode'
            cand --cmd 'Command to invoke dynamically in interactive mode'
            cand --color 'Set color theme'
            cand --skip-to-pattern 'Show the matched pattern at the line start'
            cand --layout 'Set layout'
            cand --height 'Height of skim''s window'
            cand --min-height 'Minimum height of skim''s window'
            cand --margin 'Screen margin'
            cand -p 'Set prompt'
            cand --prompt 'Set prompt'
            cand --cmd-prompt 'Set prompt in command mode'
            cand --tabstop 'Number of spaces that make up a tab'
            cand --header 'Set header, displayed next to the info'
            cand --header-lines 'Number of lines of the input treated as header'
            cand --history 'History file'
            cand --history-size 'Maximum number of query history entries to keep'
            cand --cmd-history 'Command history file'
            cand --cmd-history-size 'Maximum number of query history entries to keep'
            cand --preview 'Preview command'
            cand --preview-window 'Preview window layout'
            cand -q 'Initial query'
            cand --query 'Initial query'
            cand --cmd-query 'Initial query in interactive mode'
            cand --expect 'Comma separated list of keys used to complete skim'
            cand --pre-select-n 'Pre-select the first n items in multi-selection mode'
            cand --pre-select-pat 'Pre-select the matched items in multi-selection mode'
            cand --pre-select-items 'Pre-select the items separated by newline character'
            cand --pre-select-file 'Pre-select the items read from this file'
            cand -f 'Query for filter mode'
            cand --filter 'Query for filter mode'
            cand --hscroll-off 'Reserved for later use'
            cand --jump-labels 'Reserved for later use'
            cand --tac 'Show results in reverse order'
            cand --no-sort 'Do not sort the results'
            cand -e 'Run in exact mode'
            cand --exact 'Run in exact mode'
            cand --regex 'Start in regex mode instead of fuzzy-match'
            cand -m 'Enable multiple selection'
            cand --multi 'Enable multiple selection'
            cand --no-multi 'Disable multiple selection'
            cand --no-mouse 'Disable mouse'
            cand -i 'Run in interactive mode'
            cand --interactive 'Run in interactive mode'
            cand --no-hscroll 'Disable horizontal scroll'
            cand --keep-right 'Keep the right end of the line visible on overflow'
            cand --no-clear-if-empty 'Do not clear previous line if the command returns an empty result'
            cand --no-clear-start 'Do not clear items on start'
            cand --no-clear 'Do not clear screen on exit'
            cand --show-cmd-error 'Show error message if command fails'
            cand --reverse 'Shorthand for reverse layout'
            cand --no-height 'Disable height feature'
            cand --ansi 'Parse ANSI color codes in input strings'
            cand --inline-info 'Display info next to the query'
            cand --read0 'Read input delimited by ASCII NUL(\\0) characters'
            cand --print0 'Print output delimited by ASCII NUL(\\0) characters'
            cand --print-query 'Print the query as the first line'
            cand --print-cmd 'Print the command as the first line (after print-query)'
            cand --print-score 'Print the command as the first line (after print-cmd)'
            cand -1 'Automatically select the match if there is only one'
            cand --select-1 'Automatically select the match if there is only one'
            cand -0 'Automatically exit when no match is left'
            cand --exit-0 'Automatically exit when no match is left'
            cand --sync 'Synchronous search for multi-staged filtering'
            cand -x 'Reserved for later use'
            cand --extended 'Reserved for later use'
            cand --literal 'Reserved for later use'
            cand --cycle 'Reserved for later use'
            cand --filepath-word 'Reserved for later use'
            cand --border 'Reserved for later use'
            cand --no-bold 'Reserved for later use'
            cand --info 'Reserved for later use'
            cand --pointer 'Reserved for later use'
            cand --marker 'Reserved for later use'
            cand --phony 'Reserved for later use'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
    ]
    $completions[$command]
}
