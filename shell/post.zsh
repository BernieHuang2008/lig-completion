# lig: report the ZLE edit buffer to the lig PTY and accept insertions from it.
if [[ -o interactive && -z "$LIG_DISABLE" && -n "$LIG_TTY" && "$LIG_TTY" == "$TTY" ]] && (( ! ${+_lig_loaded} )); then
  typeset -g _lig_loaded=1 _lig_last=

  # Ghostty only injects its integration into the first shell; the shell lig
  # spawns needs it sourced by hand.
  if [[ -n "$GHOSTTY_RESOURCES_DIR" && -r "$GHOSTTY_RESOURCES_DIR/shell-integration/zsh/ghostty-integration" ]] \
      && (( ! ${+_ghostty_state} )); then
    builtin source "$GHOSTTY_RESOURCES_DIR/shell-integration/zsh/ghostty-integration"
  fi

  _lig_report() {
    builtin emulate -L zsh
    local msg
    if (( ${ISEARCHMATCH_ACTIVE:-0} )) || [[ $KEYMAP == (vicmd|menuselect) ]] || [[ -n $PREBUFFER ]]; then
      msg=$'\e]6973;H\a'
    else
      # Control characters are masked so they cannot terminate the OSC early;
      # replacing keeps the character offsets (and so $CURSOR) valid.
      msg=$'\e]6973;B;'"${CURSOR};${BUFFER//[$'\a\e']/ }"$'\a'
    fi
    [[ $msg == $_lig_last ]] && return
    _lig_last=$msg
    builtin print -rn -- "$msg" >$TTY
  }
  _lig_line_init() { _lig_last=; _lig_report }
  _lig_line_finish() {
    _lig_last=
    builtin print -rn -- $'\e]6973;F\a' >$TTY
  }

  # lig sends: ESC [9147~ <delete>;<back>;<text> BEL
  _lig_insert() {
    builtin emulate -L zsh
    local c payload=
    while builtin read -k 1 c; do
      [[ $c == $'\a' ]] && break
      payload+=$c
    done
    local del=${payload%%;*}
    payload=${payload#*;}
    local back=${payload%%;*}
    local text=${payload#*;}
    (( del > 0 )) && LBUFFER=${LBUFFER[1,-del-1]}
    LBUFFER+=$text
    (( back > 0 )) && (( CURSOR -= back ))
    _lig_last=
  }

  # Generators run in the shell's directory with the shell's PATH.
  _lig_precmd() {
    builtin print -rn -- $'\e]6973;C;'"${PWD//[$'\a\e']/}"$'\a'$'\e]6973;P;'"${PATH//[$'\a\e']/}"$'\a' >$TTY
  }
  builtin autoload -Uz add-zsh-hook
  add-zsh-hook precmd _lig_precmd

  builtin autoload -Uz add-zle-hook-widget
  add-zle-hook-widget line-init _lig_line_init
  add-zle-hook-widget line-pre-redraw _lig_report
  add-zle-hook-widget line-finish _lig_line_finish
  builtin zle -N _lig_insert
  builtin bindkey -M emacs '\e[9147~' _lig_insert
  builtin bindkey -M viins '\e[9147~' _lig_insert
  builtin bindkey -M vicmd '\e[9147~' _lig_insert
fi
