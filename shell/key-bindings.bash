#     ____      ____
#    / __/___  / __/
#   / /_/_  / / /_
#  / __/ / /_/ __/
# /_/   /___/_/ key-bindings.bash
#
# - $SKIM_TMUX_OPTS
# - $SKIM_CTRL_T_COMMAND
# - $SKIM_CTRL_T_OPTS
# - $SKIM_CTRL_R_OPTS
# - $SKIM_ALT_C_COMMAND
# - $SKIM_ALT_C_OPTS
# - $SKIM_COMPLETION_TRIGGER (default: '**')
# - $SKIM_COMPLETION_OPTS    (default: empty)

# Key bindings
# ------------
# copied and modified from https://github.com/junegunn/fzf/blob/master/shell/key-bindings.bash
__skim_select__() {
  local cmd="${SKIM_CTRL_T_COMMAND:-"command find -L . -mindepth 1 \\( -path '*/\\.*' -o -fstype 'sysfs' -o -fstype 'devfs' -o -fstype 'devtmpfs' -o -fstype 'proc' \\) -prune \
    -o -type f -print \
    -o -type d -print \
    -o -type l -print 2> /dev/null | cut -b3-"}"
  eval "$cmd" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_CTRL_T_OPTS" $(__skimcmd) -m "$@" | while read -r item; do
    printf '%q ' "$item"
  done
  echo
}

if [[ $- =~ i ]]; then


__skimcmd() {
  [ -n "$TMUX_PANE" ] && { [ "${SKIM_TMUX:-0}" != 0 ] || [ -n "$SKIM_TMUX_OPTS" ]; } &&
    echo "sk --tmux=${SKIM_TMUX_OPTS:-center,${SKIM_TMUX_HEIGHT:-40%}} -- " || echo "sk"
}

skim-file-widget() {
  local selected="$(__skim_select__)"
  READLINE_LINE="${READLINE_LINE:0:$READLINE_POINT}$selected${READLINE_LINE:$READLINE_POINT}"
  READLINE_POINT=$(( READLINE_POINT + ${#selected} ))
}

__skim_cd__() {
  local cmd dir
  cmd="${SKIM_ALT_C_COMMAND:-"command find -L . -mindepth 1 \\( -path '*/\\.*' -o -fstype 'sysfs' -o -fstype 'devfs' -o -fstype 'devtmpfs' -o -fstype 'proc' \\) -prune \
    -o -type d -print 2> /dev/null | cut -b3-"}"
  dir=$(eval "$cmd" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_ALT_C_OPTS" $(__skimcmd) --no-multi) && printf 'cd %q' "$dir"
}

__skim_history__() {
  local output
  output=$(
    builtin fc -lnr -2147483648 |
      last_hist=$(HISTTIMEFORMAT='' builtin history 1) perl -n -l0 -e 'BEGIN { getc; $/ = "\n\t"; $HISTCMD = $ENV{last_hist} + 1 } s/^[ *]//; print $HISTCMD - $. . "\t$_" if !$seen{$_}++' |
      SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} $SKIM_DEFAULT_OPTIONS -n2..,.. --bind=ctrl-r:toggle-sort $SKIM_CTRL_R_OPTS --no-multi --read0" $(__skimcmd) --query "$READLINE_LINE"
  ) || return
  READLINE_LINE=${output#*$'\t'}
  if [ -z "$READLINE_POINT" ]; then
    echo "$READLINE_LINE"
  else
    READLINE_POINT=0x7fffffff
  fi
}

# Required to refresh the prompt after skim
bind -m emacs-standard '"\er": redraw-current-line'

bind -m vi-command '"\C-z": emacs-editing-mode'
bind -m vi-insert '"\C-z": emacs-editing-mode'
bind -m emacs-standard '"\C-z": vi-editing-mode'

if [ "${BASH_VERSINFO[0]}" -lt 4 ]; then
  # CTRL-T - Paste the selected file path into the command line
  bind -m emacs-standard '"\C-t": " \C-b\C-k \C-u`__skim_select__`\e\C-e\er\C-a\C-y\C-h\C-e\e \C-y\ey\C-x\C-x\C-f"'
  bind -m vi-command '"\C-t": "\C-z\C-t\C-z"'
  bind -m vi-insert '"\C-t": "\C-z\C-t\C-z"'

  # CTRL-R - Paste the selected command from history into the command line
  bind -m emacs-standard '"\C-r": "\C-e \C-u\C-y\ey\C-u"$(__skim_history__)"\e\C-e\er"'
  bind -m vi-command '"\C-r": "\C-z\C-r\C-z"'
  bind -m vi-insert '"\C-r": "\C-z\C-r\C-z"'
else
  # CTRL-T - Paste the selected file path into the command line
  bind -m emacs-standard -x '"\C-t": skim-file-widget'
  bind -m vi-command -x '"\C-t": skim-file-widget'
  bind -m vi-insert -x '"\C-t": skim-file-widget'

  # CTRL-R - Paste the selected command from history into the command line
  bind -m emacs-standard -x '"\C-r": __skim_history__'
  bind -m vi-command -x '"\C-r": __skim_history__'
  bind -m vi-insert -x '"\C-r": __skim_history__'
fi

# ALT-C - cd into the selected directory
bind -m emacs-standard '"\ec": " \C-b\C-k \C-u`__skim_cd__`\e\C-e\er\C-m\C-y\C-h\e \C-y\ey\C-x\C-x\C-d"'
bind -m vi-command '"\ec": "\C-z\ec\C-z"'
bind -m vi-insert '"\ec": "\C-z\ec\C-z"'

# Completion

if ! declare -f _skim_compgen_path > /dev/null; then
  _skim_compgen_path() {
    echo "$1"
    command find -L "$1" \
      -name .git -prune -o -name .hg -prune -o -name .svn -prune -o \( -type d -o -type f -o -type l \) \
      -a -not -path "$1" -print 2> /dev/null | sed 's@^\./@@'
  }
fi

if ! declare -f _skim_compgen_dir > /dev/null; then
  _skim_compgen_dir() {
    command find -L "$1" \
      -name .git -prune -o -name .hg -prune -o -name .svn -prune -o -type d \
      -a -not -path "$1" -print 2> /dev/null | sed 's@^\./@@'
  }
fi

###########################################################

# To redraw line after skim closes (printf '\e[5n')
bind '"\e[0n": redraw-current-line'

__skim_comprun() {
  if [ "$(type -t _skim_comprun 2>&1)" = function ]; then
    _skim_comprun "$@"
  elif [ -n "$TMUX_PANE" ] && { [ "${SKIM_TMUX:-0}" != 0 ] || [ -n "$SKIM_TMUX_OPTS" ]; }; then
    shift
    sk --tmux=${SKIM_TMUX_OPTS:-center,${SKIM_TMUX_HEIGHT:-40%}} -- "$@"
  else
    shift
    sk "$@"
  fi
}

__skim_orig_completion_filter() {
  sed 's/^\(.*-F\) *\([^ ]*\).* \([^ ]*\)$/export _skim_orig_completion_\3="\1 %s \3 #\2"; [[ "\1" = *" -o nospace "* ]] \&\& [[ ! "$__skim_nospace_commands" = *" \3 "* ]] \&\& __skim_nospace_commands="$__skim_nospace_commands \3 ";/' |
  awk -F= '{OFS = FS} {gsub(/[^A-Za-z0-9_= ;]/, "_", $1);}1'
}

_skim_handle_dynamic_completion() {
  local cmd orig_var orig ret orig_cmd orig_complete
  cmd="$1"
  shift
  orig_cmd="$1"
  orig_var="_skim_orig_completion_$cmd"
  orig="${!orig_var##*#}"
  if [ -n "$orig" ] && type "$orig" > /dev/null 2>&1; then
    $orig "$@"
  elif [ -n "$_skim_completion_loader" ]; then
    orig_complete=$(complete -p "$orig_cmd" 2> /dev/null)
    _completion_loader "$@"
    ret=$?
    # _completion_loader may not have updated completion for the command
    if [ "$(complete -p "$orig_cmd" 2> /dev/null)" != "$orig_complete" ]; then
      eval "$(complete | command grep " -F.* $orig_cmd$" | __skim_orig_completion_filter)"
      if [[ "$__skim_nospace_commands" = *" $orig_cmd "* ]]; then
        eval "${orig_complete/ -F / -o nospace -F }"
      else
        eval "$orig_complete"
      fi
    fi
    return $ret
  fi
}

__skim_generic_path_completion() {
  local cur base dir leftover matches trigger cmd
  cmd="${COMP_WORDS[0]//[^A-Za-z0-9_=]/_}"
  COMPREPLY=()
  trigger=${SKIM_COMPLETION_TRIGGER-'**'}
  cur="${COMP_WORDS[COMP_CWORD]}"
  if [[ "$cur" == *"$trigger" ]]; then
    base=${cur:0:${#cur}-${#trigger}}
    eval "base=$base"

    [[ $base = *"/"* ]] && dir="$base"
    while true; do
      if [ -z "$dir" ] || [ -d "$dir" ]; then
        leftover=${base/#"$dir"}
        leftover=${leftover/#\/}
        [ -z "$dir" ] && dir='.'
        [ "$dir" != "/" ] && dir="${dir/%\//}"
        matches=$(eval "$1 $(printf %q "$dir")" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_COMPLETION_OPTS $2" __skim_comprun "$4" -q "$leftover" | while read -r item; do
          printf "%q$3 " "$item"
        done)
        matches=${matches% }
        [[ -z "$3" ]] && [[ "$__skim_nospace_commands" = *" ${COMP_WORDS[0]} "* ]] && matches="$matches "
        if [ -n "$matches" ]; then
          COMPREPLY=( "$matches" )
        else
          COMPREPLY=( "$cur" )
        fi
        printf '\e[5n'
        return 0
      fi
      dir=$(dirname "$dir")
      [[ "$dir" =~ /$ ]] || dir="$dir"/
    done
  else
    shift
    shift
    shift
    _skim_handle_dynamic_completion "$cmd" "$@"
  fi
}

_skim_complete() {
  # Split arguments around --
  local args rest str_arg i sep
  args=("$@")
  sep=
  for i in "${!args[@]}"; do
    if [[ "${args[$i]}" = -- ]]; then
      sep=$i
      break
    fi
  done
  if [[ -n "$sep" ]]; then
    str_arg=
    rest=("${args[@]:$((sep + 1)):${#args[@]}}")
    args=("${args[@]:0:$sep}")
  else
    str_arg=$1
    args=()
    shift
    rest=("$@")
  fi

  local cur selected trigger cmd post
  post="$(caller 0 | awk '{print $2}')_post"
  type -t "$post" > /dev/null 2>&1 || post=cat

  cmd="${COMP_WORDS[0]//[^A-Za-z0-9_=]/_}"
  trigger=${SKIM_COMPLETION_TRIGGER-'**'}
  cur="${COMP_WORDS[COMP_CWORD]}"
  if [[ "$cur" == *"$trigger" ]]; then
    cur=${cur:0:${#cur}-${#trigger}}

    selected=$(SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_COMPLETION_OPTS $str_arg" __skim_comprun "${rest[0]}" "${args[@]}" -q "$cur" | $post | tr '\n' ' ')
    selected=${selected% } # Strip trailing space not to repeat "-o nospace"
    if [ -n "$selected" ]; then
      COMPREPLY=("$selected")
    else
      COMPREPLY=("$cur")
    fi
    printf '\e[5n'
    return 0
  else
    _skim_handle_dynamic_completion "$cmd" "${rest[@]}"
  fi
}

_skim_path_completion() {
  __skim_generic_path_completion _skim_compgen_path "-m" "" "$@"
}

# Deprecated. No file only completion.
_skim_file_completion() {
  _skim_path_completion "$@"
}

_skim_dir_completion() {
  __skim_generic_path_completion _skim_compgen_dir "" "/" "$@"
}

_skim_complete_kill() {
  local trigger=${SKIM_COMPLETION_TRIGGER-'**'}
  local cur="${COMP_WORDS[COMP_CWORD]}"
  if [[ -z "$cur" ]]; then
    COMP_WORDS[$COMP_CWORD]=$trigger
  elif [[ "$cur" != *"$trigger" ]]; then
    return 1
  fi

  _skim_proc_completion "$@"
}

_skim_proc_completion() {
  _skim_complete -m --preview 'echo {}' --preview-window down:3:wrap --min-height 15 -- "$@" < <(
    command ps -ef | sed 1d
  )
}

_skim_proc_completion_post() {
  awk '{print $2}'
}

_skim_host_completion() {
  _skim_complete --no-multi -- "$@" < <(
    command cat <(command tail -n +1 ~/.ssh/config ~/.ssh/config.d/* /etc/ssh/ssh_config 2> /dev/null | command grep -i '^\s*host\(name\)\? ' | awk '{for (i = 2; i <= NF; i++) print $1 " " $i}' | command grep -v '[*?]') \
        <(command grep -oE '^[[a-z0-9.,:-]+' ~/.ssh/known_hosts | tr ',' '\n' | tr -d '[' | awk '{ print $1 " " $1 }') \
        <(command grep -v '^\s*\(#\|$\)' /etc/hosts | command grep -Fv '0.0.0.0') |
        awk '{if (length($2) > 0) {print $2}}' | sort -u
  )
}

_skim_var_completion() {
  _skim_complete -m -- "$@" < <(
    declare -xp | sed 's/=.*//' | sed 's/.* //'
  )
}

_skim_alias_completion() {
  _skim_complete -m -- "$@" < <(
    alias | sed 's/=.*//' | sed 's/.* //'
  )
}

d_cmds="${SKIM_COMPLETION_DIR_COMMANDS:-cd pushd rmdir}"
a_cmds="
  awk cat diff diff3
  emacs emacsclient ex file ftp g++ gcc gvim head hg java
  javac ld less more mvim nvim patch perl python ruby
  sed sftp sort source tail tee uniq vi view vim wc xdg-open
  basename bunzip2 bzip2 chmod chown curl cp dirname du
  find git grep gunzip gzip hg jar
  ln ls mv open rm rsync scp
  svn tar unzip zip"

# Preserve existing completion
eval "$(complete |
  sed -E '/-F/!d; / _skim/d; '"/ ($(echo $d_cmds $a_cmds | sed 's/ /|/g; s/+/\\+/g'))$/"'!d' |
  __skim_orig_completion_filter)"

if type _completion_loader > /dev/null 2>&1; then
  _skim_completion_loader=1
fi

__skim_defc() {
  local cmd func opts orig_var orig def
  cmd="$1"
  func="$2"
  opts="$3"
  orig_var="_skim_orig_completion_${cmd//[^A-Za-z0-9_]/_}"
  orig="${!orig_var}"
  if [ -n "$orig" ]; then
    printf -v def "$orig" "$func"
    eval "$def"
  else
    complete -F "$func" $opts "$cmd"
  fi
}

# Anything
for cmd in $a_cmds; do
  __skim_defc "$cmd" _skim_path_completion "-o default -o bashdefault"
done

# Directory
for cmd in $d_cmds; do
  __skim_defc "$cmd" _skim_dir_completion "-o nospace -o dirnames"
done

# Kill completion (supports empty completion trigger)
complete -F _skim_complete_kill -o default -o bashdefault kill

unset cmd d_cmds a_cmds

_skim_setup_completion() {
  local kind fn cmd
  kind=$1
  fn=_skim_${1}_completion
  if [[ $# -lt 2 ]] || ! type -t "$fn" > /dev/null; then
    echo "usage: ${FUNCNAME[0]} path|dir|var|alias|host|proc COMMANDS..."
    return 1
  fi
  shift
  eval "$(complete -p "$@" 2> /dev/null | grep -v "$fn" | __skim_orig_completion_filter)"
  for cmd in "$@"; do
    case "$kind" in
      dir)   __skim_defc "$cmd" "$fn" "-o nospace -o dirnames" ;;
      var)   __skim_defc "$cmd" "$fn" "-o default -o nospace -v" ;;
      alias) __skim_defc "$cmd" "$fn" "-a" ;;
      *)     __skim_defc "$cmd" "$fn" "-o default -o bashdefault" ;;
    esac
  done
}

# Environment variables / Aliases / Hosts
_skim_setup_completion 'var'   export unset
_skim_setup_completion 'alias' unalias
_skim_setup_completion 'host'  ssh telnet

fi
