#     ____      ____
#    / __/___  / __/
#   / /_/_  / / /_
#  / __/ / /_/ __/
# /_/   /___/_/ key-bindings.zsh
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

# The code at the top and the bottom of this file is the same as in completion.zsh.
# Refer to that file for explanation.
if 'zmodload' 'zsh/parameter' 2>'/dev/null' && (( ${+options} )); then
  __skim_key_bindings_options="options=(${(j: :)${(kv)options[@]}})"
  __skim_completion_options="options=(${(j: :)${(kv)options[@]}})"
else
  () {
    __skim_key_bindings_options="setopt"
    'local' '__skim_opt'
    for __skim_opt in "${(@)${(@f)$(set -o)}%% *}"; do
      if [[ -o "$__skim_opt" ]]; then
        __skim_key_bindings_options+=" -o $__skim_opt"
      else
        __skim_key_bindings_options+=" +o $__skim_opt"
      fi
    done
  }
  () {
    'local' '__skim_opt'
    __skim_completion_options="setopt"
    for __skim_opt in "${(@)${(@f)$(set -o)}%% *}"; do
      if [[ -o "$__skim_opt" ]]; then
        __skim_completion_options+=" -o $__skim_opt"
      else
        __skim_completion_options+=" +o $__skim_opt"
      fi
    done
  }
fi

'emulate' 'zsh' '-o' 'no_aliases'

{

[[ -o interactive ]] || return 0

# CTRL-T - Paste the selected file path(s) into the command line
__fsel() {
  local cmd="${SKIM_CTRL_T_COMMAND:-"command find -L . -mindepth 1 \\( -path '*/\\.*' -o -fstype 'sysfs' -o -fstype 'devfs' -o -fstype 'devtmpfs' -o -fstype 'proc' \\) -prune \
    -o -type f -print \
    -o -type d -print \
    -o -type l -print 2> /dev/null | cut -b3-"}"
  setopt localoptions pipefail no_aliases 2> /dev/null
  REPORTTIME_=$REPORTTIME
  unset REPORTTIME
  eval "$cmd" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_CTRL_T_OPTS" $(__skimcmd) -m "$@" | while read item; do
    echo -n "${(q)item} "
  done
  local ret=$?
  echo
  if ! [ -z $REPORTTIME_ ]; then
      REPORTTIME=$REPORTTIME_
  fi
  unset REPORTTIME_
  return $ret
}

__skimcmd() {
  [ -n "$TMUX_PANE" ] && { [ "${SKIM_TMUX:-0}" != 0 ] || [ -n "$SKIM_TMUX_OPTS" ]; } &&
    echo "sk --tmux=${SKIM_TMUX_OPTS:-center,${SKIM_TMUX_HEIGHT:-40%}} -- " || echo "sk"
}

skim-file-widget() {
  LBUFFER="${LBUFFER}$(__fsel)"
  local ret=$?
  zle reset-prompt
  return $ret
}
zle     -N   skim-file-widget
bindkey '^T' skim-file-widget

# Ensure precmds are run after cd
skim-redraw-prompt() {
  local precmd
  for precmd in $precmd_functions; do
    $precmd
  done
  zle reset-prompt
}
zle -N skim-redraw-prompt

# ALT-C - cd into the selected directory
skim-cd-widget() {
  local cmd="${SKIM_ALT_C_COMMAND:-"command find -L . -mindepth 1 \\( -path '*/\\.*' -o -fstype 'sysfs' -o -fstype 'devfs' -o -fstype 'devtmpfs' -o -fstype 'proc' \\) -prune \
    -o -type d -print 2> /dev/null | cut -b3-"}"
  setopt localoptions pipefail no_aliases 2> /dev/null
  REPORTTIME_=$REPORTTIME
  unset REPORTTIME
  local dir="$(eval "$cmd" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_ALT_C_OPTS" $(__skimcmd) --no-multi)"
  if ! [ -z $REPORTTIME_ ]; then
      REPORTTIME=$REPORTTIME_
  fi
  unset REPORTTIME_
  if [[ -z "$dir" ]]; then
    zle redisplay
    return 0
  fi
  if [ -z "$BUFFER" ]; then
    BUFFER="cd ${(q)dir}"
    zle accept-line
  else
    print -sr "cd ${(q)dir}"
    cd "$dir"
  fi
  local ret=$?
  unset dir # ensure this doesn't end up appearing in prompt expansion
  zle skim-redraw-prompt
  tput cnorm
  return $ret
}
zle     -N    skim-cd-widget
bindkey '\ec' skim-cd-widget

# CTRL-R - Paste the selected command from history into the command line
skim-history-widget() {
  local selected num
  setopt localoptions noglobsubst noposixbuiltins pipefail no_aliases 2> /dev/null
  local awk_filter='{ cmd=$0; sub(/^\s*[0-9]+\**\s+/, "", cmd); if (!seen[cmd]++) print $0 }'  # filter out duplicates
  local n=2 fc_opts=''
  if [[ -o extended_history ]]; then
    local today=$(date +%Y-%m-%d)
    # For today's commands, replace date ($2) with "today", otherwise remove time ($3).
    # And filter out duplicates.
    awk_filter='{
      if ($2 == "'$today'") sub($2 " ", "today'\''")
      else sub($3, "")
      line=$0; $1=""; $2=""; $3=""
      if (!seen[$0]++) print line
    }'
    fc_opts='-i'
    n=3
  fi
  selected=( $(fc -rl $fc_opts 1 | awk "$awk_filter" |
    SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} $SKIM_DEFAULT_OPTIONS -n$n..,.. --bind=ctrl-r:toggle-sort $SKIM_CTRL_R_OPTS --query=${(qqq)LBUFFER} --no-multi" $(__skimcmd)) )
  local ret=$?
  if [ -n "$selected" ]; then
    num=$selected[1]
    if [ -n "$num" ]; then
      zle vi-fetch-history -n $num
    fi
  fi
  zle reset-prompt
  tput cnorm
  return $ret
}
zle     -N   skim-history-widget
bindkey '^R' skim-history-widget

# Completion
# To use custom commands instead of find, override _skim_compgen_{path,dir}
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

__skim_comprun() {
  if [[ "$(type _skim_comprun 2>&1)" =~ function ]]; then
    _skim_comprun "$@"
  elif [ -n "$TMUX_PANE" ] && { [ "${SKIM_TMUX:-0}" != 0 ] || [ -n "$SKIM_TMUX_OPTS" ]; }; then
    shift
    if [ -n "$SKIM_TMUX_OPTS" ]; then
      sk --tmux=${(Q)${(Z+n+)SKIM_TMUX_OPTS}} -- "$@"
    else
      sk --tmux=${SKIM_TMUX_OPTS:-center,${SKIM_TMUX_HEIGHT:-40%}} -- "$@"
    fi
  else
    shift
    sk "$@"
  fi
}

# Extract the name of the command. e.g. foo=1 bar baz**<tab>
__skim_extract_command() {
  local token tokens
  tokens=(${(z)1})
  for token in $tokens; do
    token=${(Q)token}
    if [[ "$token" =~ [[:alnum:]] && ! "$token" =~ "=" ]]; then
      echo "$token"
      return
    fi
  done
  echo "${tokens[1]}"
}

__skim_generic_path_completion() {
  local base lbuf cmd compgen skim_opts suffix tail dir leftover matches
  base=$1
  lbuf=$2
  cmd=$(__skim_extract_command "$lbuf")
  compgen=$3
  skim_opts=$4
  suffix=$5
  tail=$6

  setopt localoptions nonomatch
  eval "base=$base"
  [[ $base = *"/"* ]] && dir="$base"
  while [ 1 ]; do
    if [[ -z "$dir" || -d ${dir} ]]; then
      leftover=${base/#"$dir"}
      leftover=${leftover/#\/}
      [ -z "$dir" ] && dir='.'
      [ "$dir" != "/" ] && dir="${dir/%\//}"
      matches=$(eval "$compgen $(printf %q "$dir")" | SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_COMPLETION_OPTS" __skim_comprun "$cmd" ${(Q)${(Z+n+)skim_opts}} -q "$leftover" | while read item; do
        echo -n "${(q)item}$suffix "
      done)
      matches=${matches% }
      if [ -n "$matches" ]; then
        LBUFFER="$lbuf$matches$tail"
      fi
      zle reset-prompt
      break
    fi
    dir=$(dirname "$dir")
    dir=${dir%/}/
  done
}

_skim_path_completion() {
  __skim_generic_path_completion "$1" "$2" _skim_compgen_path \
    "-m" "" " "
}

_skim_dir_completion() {
  __skim_generic_path_completion "$1" "$2" _skim_compgen_dir \
    "" "/" ""
}

_skim_feed_fifo() (
  command rm -f "$1"
  mkfifo "$1"
  cat <&0 > "$1" &
)

_skim_complete() {
  setopt localoptions ksh_arrays
  # Split arguments around --
  local args rest str_arg i sep
  args=("$@")
  sep=
  for i in {0..${#args[@]}}; do
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

  local fifo lbuf cmd matches post
  fifo="${TMPDIR:-/tmp}/skim-complete-fifo-$$"
  lbuf=${rest[0]}
  cmd=$(__skim_extract_command "$lbuf")
  post="${funcstack[1]}_post"
  type $post > /dev/null 2>&1 || post=cat

  _skim_feed_fifo "$fifo"
  matches=$(SKIM_DEFAULT_OPTIONS="--height ${SKIM_TMUX_HEIGHT:-40%} --reverse $SKIM_DEFAULT_OPTIONS $SKIM_COMPLETION_OPTS $str_arg" __skim_comprun "$cmd" "${args[@]}" -q "${(Q)prefix}" < "$fifo" | $post | tr '\n' ' ')
  if [ -n "$matches" ]; then
    LBUFFER="$lbuf$matches"
  fi
  zle reset-prompt
  command rm -f "$fifo"
}

_skim_complete_telnet() {
  _skim_complete --no-multi -- "$@" < <(
    command grep -v '^\s*\(#\|$\)' /etc/hosts | command grep -Fv '0.0.0.0' |
        awk '{if (length($2) > 0) {print $2}}' | sort -u
  )
}

_skim_complete_ssh() {
  _skim_complete --no-multi -- "$@" < <(
    setopt localoptions nonomatch
    command cat <(command tail -n +1 ~/.ssh/config ~/.ssh/config.d/* /etc/ssh/ssh_config 2> /dev/null | command grep -i '^\s*host\(name\)\? ' | awk '{for (i = 2; i <= NF; i++) print $1 " " $i}' | command grep -v '[*?]') \
        <(command grep -oE '^[[a-z0-9.,:-]+' ~/.ssh/known_hosts | tr ',' '\n' | tr -d '[' | awk '{ print $1 " " $1 }') \
        <(command grep -v '^\s*\(#\|$\)' /etc/hosts | command grep -Fv '0.0.0.0') |
        awk '{if (length($2) > 0) {print $2}}' | sort -u
  )
}

_skim_complete_export() {
  _skim_complete -m -- "$@" < <(
    declare -xp | sed 's/=.*//' | sed 's/.* //'
  )
}

_skim_complete_unset() {
  _skim_complete -m -- "$@" < <(
    declare -xp | sed 's/=.*//' | sed 's/.* //'
  )
}

_skim_complete_unalias() {
  _skim_complete --no-multi -- "$@" < <(
    alias | sed 's/=.*//'
  )
}

_skim_complete_kill() {
  _skim_complete -m --preview 'echo {}' --preview-window down:3:wrap --min-height 15 -- "$@" < <(
    command ps -ef | sed 1d
  )
}

_skim_complete_kill_post() {
  awk '{print $2}'
}

skim-completion() {
  local tokens cmd prefix trigger tail matches lbuf d_cmds
  setopt localoptions noshwordsplit noksh_arrays noposixbuiltins

  # http://zsh.sourceforge.net/FAQ/zshfaq03.html
  # http://zsh.sourceforge.net/Doc/Release/Expansion.html#Parameter-Expansion-Flags
  tokens=(${(z)LBUFFER})
  if [ ${#tokens} -lt 1 ]; then
    zle ${skim_default_completion:-expand-or-complete}
    return
  fi

  cmd=$(__skim_extract_command "$LBUFFER")

  # Explicitly allow for empty trigger.
  trigger=${SKIM_COMPLETION_TRIGGER-'**'}
  [ -z "$trigger" -a ${LBUFFER[-1]} = ' ' ] && tokens+=("")

  # When the trigger starts with ';', it becomes a separate token
  if [[ ${LBUFFER} = *"${tokens[-2]}${tokens[-1]}" ]]; then
    tokens[-2]="${tokens[-2]}${tokens[-1]}"
    tokens=(${tokens[0,-2]})
  fi

  lbuf=$LBUFFER
  tail=${LBUFFER:$(( ${#LBUFFER} - ${#trigger} ))}
  # Kill completion (do not require trigger sequence)
  if [ "$cmd" = kill -a ${LBUFFER[-1]} = ' ' ]; then
    tail=$trigger
    tokens+=$trigger
    lbuf="$lbuf$trigger"
  fi

  # Trigger sequence given
  if [ ${#tokens} -gt 1 -a "$tail" = "$trigger" ]; then
    d_cmds=(${=SKIM_COMPLETION_DIR_COMMANDS:-cd pushd rmdir})

    [ -z "$trigger"      ] && prefix=${tokens[-1]} || prefix=${tokens[-1]:0:-${#trigger}}
    [ -n "${tokens[-1]}" ] && lbuf=${lbuf:0:-${#tokens[-1]}}

    if eval "type _skim_complete_${cmd} > /dev/null"; then
      prefix="$prefix" eval _skim_complete_${cmd} ${(q)lbuf}
    elif [ ${d_cmds[(i)$cmd]} -le ${#d_cmds} ]; then
      _skim_dir_completion "$prefix" "$lbuf"
    else
      _skim_path_completion "$prefix" "$lbuf"
    fi
    tput cnorm
  # Fall back to default completion
  else
    zle ${skim_default_completion:-expand-or-complete}
  fi
}

[ -z "$skim_default_completion" ] && {
  binding=$(bindkey '^I')
  [[ $binding =~ 'undefined-key' ]] || skim_default_completion=$binding[(s: :w)2]
  unset binding
}

zle     -N   skim-completion
bindkey '^I' skim-completion

} always {
  eval $__skim_key_bindings_options
  'unset' '__skim_key_bindings_options'
  eval $__skim_completion_options
  'unset' '__skim_completion_options'
}
