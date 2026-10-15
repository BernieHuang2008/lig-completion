# lig: wrap this interactive bash in the lig PTY (once per terminal).
if [[ $- == *i* && -z "$LIG_DISABLE" && -t 0 && -t 1 && "$TERM" != dumb && -z "$INSIDE_EMACS" \
      && -x "@LIG_BIN@" && "$LIG_TTY" != "$(tty)" ]]; then
  if shopt -q login_shell; then
    exec "@LIG_BIN@" shell --shell "$BASH" --login
  else
    exec "@LIG_BIN@" shell --shell "$BASH"
  fi
fi
