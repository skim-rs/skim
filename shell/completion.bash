_sk() {
    local i cur prev opts cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=""
    opts=""

    for i in ${COMP_WORDS[@]}
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
            opts="-t -n -d -e -b -m -c -i -p -q -1 -0 -f -x -h --tac --no-sort --tiebreak --nth --with-nth --delimiter --exact --regex --algo --case --bind --multi --no-multi --no-mouse --cmd --interactive --color --no-hscroll --keep-right --skip-to-pattern --no-clear-if-empty --no-clear-start --no-clear --show-cmd-error --layout --reverse --height --no-height --min-height --margin --prompt --cmd-prompt --ansi --tabstop --inline-info --header --header-lines --tmux --history --history-size --cmd-history --cmd-history-size --preview --preview-window --query --cmd-query --expect --read0 --print0 --print-query --print-cmd --print-score --select-1 --exit-0 --sync --pre-select-n --pre-select-pat --pre-select-items --pre-select-file --filter --extended --literal --cycle --hscroll-off --filepath-word --jump-labels --border --no-bold --info --pointer --marker --phony --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --tiebreak)
                    COMPREPLY=($(compgen -W "score -score begin -begin end -end length -length" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "score -score begin -begin end -end length -length" -- "${cur}"))
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
                --tabstop)
                    COMPREPLY=($(compgen -f "${cur}"))
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
                --tmux)
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
                --expect)
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
                --hscroll-off)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --jump-labels)
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
