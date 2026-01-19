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
            opts="-t -n -d -e -b -m -c -i -I -p -q -1 -0 -f -x -h -V --tac --min-query-length --no-sort --tiebreak --nth --with-nth --delimiter --exact --regex --algo --case --bind --multi --no-multi --no-mouse --cmd --interactive --color --no-hscroll --keep-right --skip-to-pattern --no-clear-if-empty --no-clear-start --no-clear --show-cmd-error --cycle --disabled --layout --reverse --height --no-height --min-height --margin --prompt --cmd-prompt --selector --multi-selector --ansi --tabstop --info --no-info --inline-info --header --header-lines --border --wrap --history --history-size --cmd-history --cmd-history-size --preview --preview-window --query --cmd-query --read0 --print0 --print-query --print-cmd --print-score --print-header --select-1 --exit-0 --sync --pre-select-n --pre-select-pat --pre-select-items --pre-select-file --filter --shell --shell-bindings --man --listen --tmux --log-file --extended --literal --hscroll-off --filepath-word --jump-labels --no-bold --phony --expect --help --version"
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
                    COMPREPLY=($(compgen -W "skim_v1 skim_v2 clangd" -- "${cur}"))
                    return 0
                    ;;
                --case)
                    COMPREPLY=($(compgen -W "respect ignore smart" -- "${cur}"))
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
                --tmux)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --log-file)
                    COMPREPLY=($(compgen -f "${cur}"))
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
