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
'--min-query-length=[Minimum query length to start showing results]:MIN_QUERY_LENGTH:_default' \
'*-t+[Comma-separated list of sort criteria to apply when the scores are tied.]:TIEBREAK:(score -score begin -begin end -end length -length index -index)' \
'*--tiebreak=[Comma-separated list of sort criteria to apply when the scores are tied.]:TIEBREAK:(score -score begin -begin end -end length -length index -index)' \
'*-n+[Fields to be matched]:NTH:_default' \
'*--nth=[Fields to be matched]:NTH:_default' \
'*--with-nth=[Fields to be transformed]:WITH_NTH:_default' \
'-d+[Delimiter between fields]:DELIMITER:_default' \
'--delimiter=[Delimiter between fields]:DELIMITER:_default' \
'--algo=[Fuzzy matching algorithm]:ALGORITHM:((skim_v1\:"Original skim fuzzy matching algorithm (v1)"
skim_v2\:"Improved skim fuzzy matching algorithm (v2, default)"
clangd\:"Clangd fuzzy matching algorithm"
fzy\:"Fzy matching algorithm (https\://github.com/jhawthorn/fzy)"
frizbee\:"Frizbee matching algorithm, typo resistant Will fallback to SkimV2 if the feature is not enabled"))' \
'--case=[Case sensitivity]:CASE:((respect\:"Case-sensitive matching"
ignore\:"Case-insensitive matching"
smart\:"Smart case\: case-insensitive unless query contains uppercase"))' \
'--typos=[Enable typo-tolerant matching]::TYPOS:_default' \
'--split-match=[Enable split matching and set delimiter]::SPLIT_MATCH:_default' \
'*-b+[Comma separated list of bindings]::BIND:_default' \
'*--bind=[Comma separated list of bindings]::BIND:_default' \
'-c+[Command to invoke dynamically in interactive mode]:CMD:_default' \
'--cmd=[Command to invoke dynamically in interactive mode]:CMD:_default' \
'-I+[Replace replstr with the selected item in commands]:REPLSTR:_default' \
'--color=[Set color theme]:COLOR:_default' \
'--skip-to-pattern=[Show the matched pattern at the line start]:SKIP_TO_PATTERN:_default' \
'--layout=[Set layout]:LAYOUT:((default\:"Display from the bottom of the screen"
reverse\:"Display from the top of the screen"
reverse-list\:"Display from the top of the screen, prompt at the bottom"))' \
'--height=[Height of skim'\''s window]:HEIGHT:_default' \
'--min-height=[Minimum height of skim'\''s window]:MIN_HEIGHT:_default' \
'--margin=[Screen margin]:MARGIN:_default' \
'-p+[Set prompt]:PROMPT:_default' \
'--prompt=[Set prompt]:PROMPT:_default' \
'--cmd-prompt=[Set prompt in command mode]:CMD_PROMPT:_default' \
'--selector=[Set selected item icon]:SELECTOR_ICON:_default' \
'--multi-selector=[Set selected item icon]:MULTI_SELECT_ICON:_default' \
'--tabstop=[Number of spaces that make up a tab]:TABSTOP:_default' \
'--ellipsis=[The characters used to display truncated lines]:ELLIPSIS:_default' \
'--info=[Set matching result count display position]:INFO:(default inline hidden)' \
'--header=[Set header, displayed next to the info]:HEADER:_default' \
'--header-lines=[Number of lines of the input treated as header]:HEADER_LINES:_default' \
'--border=[Draw borders around the UI components]::BORDER:(plain rounded double thick light-double-dashed heavy-double-dashed light-triple-dashed heavy-triple-dashed light-quadruple-dashed heavy-quadruple-dashed quadrant-inside quadrant-outside)' \
'--history=[History file]:HISTORY_FILE:_default' \
'--history-size=[Maximum number of query history entries to keep]:HISTORY_SIZE:_default' \
'--cmd-history=[Command history file]:CMD_HISTORY_FILE:_default' \
'--cmd-history-size=[Maximum number of query history entries to keep]:CMD_HISTORY_SIZE:_default' \
'--preview=[Preview command]:PREVIEW:_default' \
'--preview-window=[Preview window layout]:PREVIEW_WINDOW:_default' \
'-q+[Initial query]:QUERY:_default' \
'--query=[Initial query]:QUERY:_default' \
'--cmd-query=[Initial query in interactive mode]:CMD_QUERY:_default' \
'--output-format=[Set the output format If set, overrides all print_ options Will be expanded the same way as preview or commands]:OUTPUT_FORMAT:_default' \
'--pre-select-n=[Pre-select the first n items in multi-selection mode]:PRE_SELECT_N:_default' \
'--pre-select-pat=[Pre-select the matched items in multi-selection mode]:PRE_SELECT_PAT:_default' \
'--pre-select-items=[Pre-select the items separated by newline character]:PRE_SELECT_ITEMS:_default' \
'--pre-select-file=[Pre-select the items read from this file]:PRE_SELECT_FILE:_default' \
'-f+[Query for filter mode]:FILTER:_default' \
'--filter=[Query for filter mode]:FILTER:_default' \
'--shell=[Generate shell completion script]:SHELL:((bash\:"Bourne Again SHell"
elvish\:"Elvish shell"
fish\:"Friendly Interactive SHell"
nushell\:"Nushell (nu)"
power-shell\:"PowerShell"
zsh\:"Zsh"))' \
'--listen=[Run an IPC socket with optional name (defaults to sk)]::LISTEN:_default' \
'--remote=[Send commands to an IPC socket with optional name (defaults to sk)]::REMOTE:_default' \
'--tmux=[Run in a tmux popup]::TMUX:_default' \
'--log-file=[Pipe log output to a file]:LOG_FILE:_default' \
'*--flags=[Feature flags]:FLAGS:((no-preview-pty\:"Disable preview PTY on linux"))' \
'--hscroll-off=[]:HSCROLL_OFF:_default' \
'--jump-labels=[]:JUMP_LABELS:_default' \
'--scheme=[]:SCHEME:_default' \
'--tail=[]:TAIL:_default' \
'--style=[]:STYLE:_default' \
'--padding=[]:PADDING:_default' \
'--border-label=[]:BORDER_LABEL:_default' \
'--border-label-pos=[]:BORDER_LABEL_POS:_default' \
'--wrap-sign=[]:WRAP_SIGN:_default' \
'--gap=[]:GAP:_default' \
'--gap-line=[]:GAP_LINE:_default' \
'--freeze-left=[]:FREEZE_LEFT:_default' \
'--freeze-right=[]:FREEZE_RIGHT:_default' \
'--scroll-off=[]:SCROLL_OFF:_default' \
'--gutter=[]:GUTTER:_default' \
'--gutter-raw=[]:GUTTER_RAW:_default' \
'--marker-multi-line=[]:MARKER_MULTI_LINE:_default' \
'--scrollbar=[]:SCROLLBAR:_default' \
'--list-border=[]:LIST_BORDER:_default' \
'--list-label=[]:LIST_LABEL:_default' \
'--list-label-pos=[]:LIST_LABEL_POS:_default' \
'--info-command=[]:INFO_COMMAND:_default' \
'--separator=[]:SEPARATOR:_default' \
'--ghost=[]:GHOST:_default' \
'--input-border=[]:INPUT_BORDER:_default' \
'--input-label=[]:INPUT_LABEL:_default' \
'--input-label-pos=[]:INPUT_LABEL_POS:_default' \
'--preview-label=[]:PREVIEW_LABEL:_default' \
'--preview-label-pos=[]:PREVIEW_LABEL_POS:_default' \
'--header-border=[]:HEADER_BORDER:_default' \
'--header-lines-border=[]:HEADER_LINES_BORDER:_default' \
'--footer=[]:FOOTER:_default' \
'--footer-border=[]:FOOTER_BORDER:_default' \
'--footer-label=[]:FOOTER_LABEL:_default' \
'--footer-label-pos=[]:FOOTER_LABEL_POS:_default' \
'--with-shell=[]:WITH_SHELL:_default' \
'--expect=[Deprecated, kept for compatibility purposes. See accept() bind instead]:EXPECT:_default' \
'--tac[Show results in reverse order]' \
'--no-sort[Do not sort the results]' \
'-e[Run in exact mode]' \
'--exact[Run in exact mode]' \
'--regex[Start in regex mode instead of fuzzy-match]' \
'--normalize[Normalize unicode characters]' \
'-m[Enable multiple selection]' \
'--multi[Enable multiple selection]' \
'(-m --multi)--no-multi[Disable multiple selection]' \
'--no-mouse[Disable mouse]' \
'-i[Start skim in interactive mode]' \
'--interactive[Start skim in interactive mode]' \
'--no-hscroll[Disable horizontal scroll]' \
'--keep-right[Keep the right end of the line visible on overflow]' \
'--no-clear-if-empty[Do not clear previous line if the command returns an empty result]' \
'--no-clear-start[Do not clear items on start]' \
'--no-clear[Do not clear screen on exit]' \
'--show-cmd-error[Show error message if command fails]' \
'--cycle[Cycle the results by wrapping around when scrolling]' \
'--disabled[Disable matching entirely]' \
'--reverse[Shorthand for reverse layout]' \
'--no-height[Disable height (force full screen)]' \
'--ansi[Parse ANSI color codes in input strings]' \
'--no-info[Alias for --info=hidden]' \
'--inline-info[Alias for --info=inline]' \
'--wrap[Wrap items in the item list]' \
'--read0[Read input delimited by ASCII NUL(\\0) characters]' \
'--print0[Print output delimited by ASCII NUL(\\0) characters]' \
'--print-query[Print the query as the first line]' \
'--print-cmd[Print the command as the first line (after print-query)]' \
'--print-score[Print the score after each item]' \
'--print-header[Print the header as the first line (after print-score)]' \
'--print-current[Print the current (highlighted) item as the first line (after print-header)]' \
'--no-strip-ansi[Print the ANSI codes, making the output exactly match the input even when --ansi is on]' \
'-1[Do not enter the TUI if the query passed in -q matches only one item and return it]' \
'--select-1[Do not enter the TUI if the query passed in -q matches only one item and return it]' \
'-0[Do not enter the TUI if the query passed in -q does not match any item]' \
'--exit-0[Do not enter the TUI if the query passed in -q does not match any item]' \
'--sync[Synchronous search for multi-staged filtering]' \
'--shell-bindings[Generate shell key bindings - only for bash, zsh and fish]' \
'--man[Generate man page and output it to stdout]' \
'-x[]' \
'--extended[]' \
'--literal[]' \
'--filepath-word[]' \
'--no-bold[]' \
'--phony[]' \
'--no-color[]' \
'--highlight-line[]' \
'--no-multi-line[]' \
'--raw[]' \
'--track[]' \
'--no-scrollbar[]' \
'--no-input[]' \
'--no-separator[]' \
'--header-first[]' \
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
