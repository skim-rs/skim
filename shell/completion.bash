_sk() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="sk"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        sk)
            opts="-t -n -d -e -b -m -c -i -I -p -q -1 -0 -f -x -h -V --tac --min-query-length --no-sort --tiebreak --nth --with-nth --delimiter --exact --regex --algo --case --normalize --split-match --bind --multi --no-multi --no-mouse --cmd --interactive --color --no-hscroll --keep-right --skip-to-pattern --no-clear-if-empty --no-clear-start --no-clear --show-cmd-error --cycle --disabled --layout --reverse --height --no-height --min-height --margin --prompt --cmd-prompt --selector --multi-selector --ansi --tabstop --ellipsis --info --no-info --inline-info --header --header-lines --border --wrap --history --history-size --cmd-history --cmd-history-size --preview --preview-window --query --cmd-query --read0 --print0 --print-query --print-cmd --print-score --print-header --print-current --output-format --no-strip-ansi --select-1 --exit-0 --sync --pre-select-n --pre-select-pat --pre-select-items --pre-select-file --filter --shell --shell-bindings --man --listen --remote --tmux --log-file --flags --extended --literal --hscroll-off --filepath-word --jump-labels --no-bold --phony --scheme --tail --style --no-color --padding --border-label --border-label-pos --highlight-line --wrap-sign --no-multi-line --raw --track --gap --gap-line --freeze-left --freeze-right --scroll-off --gutter --gutter-raw --marker-multi-line --scrollbar --no-scrollbar --list-border --list-label --list-label-pos --no-input --info-command --separator --no-separator --ghost --input-border --input-label --input-label-pos --preview-label --preview-label-pos --header-first --header-border --header-lines-border --footer --footer-border --footer-label --footer-label-pos --with-shell --expect --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --min-query-length)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tiebreak)
                    COMPREPLY=($(compgen -W "score -score begin -begin end -end length -length index -index" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "score -score begin -begin end -end length -length index -index" -- "${cur}"))
                    return 0
                    ;;
                --nth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --with-nth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --delimiter)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -d)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --algo)
                    COMPREPLY=($(compgen -W "skim_v1 skim_v2 clangd frizbee" -- "${cur}"))
                    return 0
                    ;;
                --case)
                    COMPREPLY=($(compgen -W "respect ignore smart" -- "${cur}"))
                    return 0
                    ;;
                --split-match)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --bind)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -b)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cmd)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -I)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --skip-to-pattern)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --layout)
                    COMPREPLY=($(compgen -W "default reverse reverse-list" -- "${cur}"))
                    return 0
                    ;;
                --height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --min-height)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --margin)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --prompt)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cmd-prompt)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --multi-selector)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tabstop)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ellipsis)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --info)
                    COMPREPLY=($(compgen -W "default inline hidden" -- "${cur}"))
                    return 0
                    ;;
                --header)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --header-lines)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --border)
                    COMPREPLY=($(compgen -W "plain rounded double thick light-double-dashed heavy-double-dashed light-triple-dashed heavy-triple-dashed light-quadruple-dashed heavy-quadruple-dashed quadrant-inside quadrant-outside" -- "${cur}"))
                    return 0
                    ;;
                --history)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --history-size)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cmd-history)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cmd-history-size)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --preview)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --preview-window)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -q)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cmd-query)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output-format)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pre-select-n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pre-select-pat)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pre-select-items)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pre-select-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --filter)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --shell)
                    COMPREPLY=($(compgen -W "bash elvish fish nushell power-shell zsh" -- "${cur}"))
                    return 0
                    ;;
                --listen)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --remote)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tmux)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --flags)
                    COMPREPLY=($(compgen -W "no-preview-pty" -- "${cur}"))
                    return 0
                    ;;
                --hscroll-off)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --jump-labels)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --scheme)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tail)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --style)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --padding)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --border-label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --border-label-pos)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --wrap-sign)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gap)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gap-line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --freeze-left)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --freeze-right)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --scroll-off)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gutter)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gutter-raw)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --marker-multi-line)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --scrollbar)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --list-border)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --list-label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --list-label-pos)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --info-command)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --separator)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ghost)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input-border)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input-label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --input-label-pos)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --preview-label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --preview-label-pos)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --header-border)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --header-lines-border)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --footer)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --footer-border)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --footer-label)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --footer-label-pos)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --with-shell)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --expect)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _sk -o nosort -o bashdefault -o default sk
else
    complete -F _sk -o bashdefault -o default sk
fi
