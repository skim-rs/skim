#compdef sk

autoload -U is-at-least

_sk() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'*-t+[Comma-separated list of sort criteria to apply when the scores are tied.]:TIEBREAK:(score -score begin -begin end -end length -length index -index)' \
'*--tiebreak=[Comma-separated list of sort criteria to apply when the scores are tied.]:TIEBREAK:(score -score begin -begin end -end length -length index -index)' \
'*-n+[Fields to be matched]:NTH:_default' \
'*--nth=[Fields to be matched]:NTH:_default' \
'*--with-nth=[Fields to be transformed]:WITH_NTH:_default' \
'-d+[Delimiter between fields]:DELIMITER:_default' \
'--delimiter=[Delimiter between fields]:DELIMITER:_default' \
'--algo=[Fuzzy matching algorithm]:ALGORITHM:(skim_v1 skim_v2 clangd)' \
'--case=[Case sensitivity]:CASE:(respect ignore smart)' \
'*-b+[Comma separated list of bindings]:BIND:_default' \
'*--bind=[Comma separated list of bindings]:BIND:_default' \
'-c+[Command to invoke dynamically in interactive mode]:CMD:_default' \
'--cmd=[Command to invoke dynamically in interactive mode]:CMD:_default' \
'-I+[Replace replstr with the selected item in commands]:REPLSTR:_default' \
'--color=[Set color theme]:COLOR:_default' \
'--skip-to-pattern=[Show the matched pattern at the line start]:SKIP_TO_PATTERN:_default' \
'--layout=[Set layout]:LAYOUT:(default reverse reverse-list)' \
'--height=[Height of skim'\''s window]:HEIGHT:_default' \
'--min-height=[Minimum height of skim'\''s window]:MIN_HEIGHT:_default' \
'--margin=[Screen margin]:MARGIN:_default' \
'-p+[Set prompt]:PROMPT:_default' \
'--prompt=[Set prompt]:PROMPT:_default' \
'--cmd-prompt=[Set prompt in command mode]:CMD_PROMPT:_default' \
'--tabstop=[Number of spaces that make up a tab]:TABSTOP:_default' \
'--info=[Set matching result count display position]:INFO:(default inline hidden)' \
'--header=[Set header, displayed next to the info]:HEADER:_default' \
'--header-lines=[Number of lines of the input treated as header]:HEADER_LINES:_default' \
'--history=[History file]:HISTORY_FILE:_default' \
'--history-size=[Maximum number of query history entries to keep]:HISTORY_SIZE:_default' \
'--cmd-history=[Command history file]:CMD_HISTORY_FILE:_default' \
'--cmd-history-size=[Maximum number of query history entries to keep]:CMD_HISTORY_SIZE:_default' \
'--preview=[Preview command]:PREVIEW:_default' \
'--preview-window=[Preview window layout]:PREVIEW_WINDOW:_default' \
'-q+[Initial query]:QUERY:_default' \
'--query=[Initial query]:QUERY:_default' \
'--cmd-query=[Initial query in interactive mode]:CMD_QUERY:_default' \
'*--expect=[Comma separated list of keys used to complete skim]:EXPECT:_default' \
'--pre-select-n=[Pre-select the first n items in multi-selection mode]:PRE_SELECT_N:_default' \
'--pre-select-pat=[Pre-select the matched items in multi-selection mode]:PRE_SELECT_PAT:_default' \
'--pre-select-items=[Pre-select the items separated by newline character]:PRE_SELECT_ITEMS:_default' \
'--pre-select-file=[Pre-select the items read from this file]:PRE_SELECT_FILE:_default' \
'-f+[Query for filter mode]:FILTER:_default' \
'--filter=[Query for filter mode]:FILTER:_default' \
'--tmux=[Run in a tmux popup]' \
'--hscroll-off=[Reserved for later use]:HSCROLL_OFF:_default' \
'--jump-labels=[Reserved for later use]:JUMP_LABELS:_default' \
'--tac[Show results in reverse order]' \
'--no-sort[Do not sort the results]' \
'-e[Run in exact mode]' \
'--exact[Run in exact mode]' \
'--regex[Start in regex mode instead of fuzzy-match]' \
'-m[Enable multiple selection]' \
'--multi[Enable multiple selection]' \
'(-m --multi)--no-multi[Disable multiple selection]' \
'--no-mouse[Disable mouse]' \
'-i[Run in interactive mode]' \
'--interactive[Run in interactive mode]' \
'--no-hscroll[Disable horizontal scroll]' \
'--keep-right[Keep the right end of the line visible on overflow]' \
'--no-clear-if-empty[Do not clear previous line if the command returns an empty result]' \
'--no-clear-start[Do not clear items on start]' \
'--no-clear[Do not clear screen on exit]' \
'--show-cmd-error[Show error message if command fails]' \
'--reverse[Shorthand for reverse layout]' \
'--no-height[Disable height feature]' \
'--ansi[Parse ANSI color codes in input strings]' \
'--no-info[Alias for --info=hidden]' \
'--inline-info[Alias for --info=inline]' \
'--read0[Read input delimited by ASCII NUL(\\\\0) characters]' \
'--print0[Print output delimited by ASCII NUL(\\\\0) characters]' \
'--print-query[Print the query as the first line]' \
'--print-cmd[Print the command as the first line (after print-query)]' \
'--print-score[Print the command as the first line (after print-cmd)]' \
'-1[Automatically select the match if there is only one]' \
'--select-1[Automatically select the match if there is only one]' \
'-0[Automatically exit when no match is left]' \
'--exit-0[Automatically exit when no match is left]' \
'--sync[Synchronous search for multi-staged filtering]' \
'-x[Reserved for later use]' \
'--extended[Reserved for later use]' \
'--literal[Reserved for later use]' \
'--cycle[Reserved for later use]' \
'--filepath-word[Reserved for later use]' \
'--border[Reserved for later use]' \
'--no-bold[Reserved for later use]' \
'--pointer[Reserved for later use]' \
'--marker[Reserved for later use]' \
'--phony[Reserved for later use]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
&& ret=0
}

(( $+functions[_sk_commands] )) ||
_sk_commands() {
    local commands; commands=()
    _describe -t commands 'sk commands' commands "$@"
}

if [ "$funcstack[1]" = "_sk" ]; then
    _sk "$@"
else
    compdef _sk sk
fi
