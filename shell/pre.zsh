# lig: wrap this interactive shell in the lig PTY (once per terminal).
if [[ -o interactive && -z "$LIG_DISABLE" && "$LIG_TTY" != "$TTY" && -t 0 && -t 1 \
      && "$TERM" != dumb && -z "$INSIDE_EMACS" && -x "@LIG_BIN@" ]]; then
  if [[ -o login ]]; then
    builtin exec "@LIG_BIN@" shell --shell "${${:-/proc/$$/exe}:A}" --login
  else
    builtin exec "@LIG_BIN@" shell --shell "${${:-/proc/$$/exe}:A}"
  fi
fi
