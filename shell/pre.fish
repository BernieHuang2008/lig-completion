# Wrap this interactive fish exactly once per terminal.
if status is-interactive; and test -z "$LIG_DISABLE"; and isatty stdin; and isatty stdout; and test "$TERM" != dumb; and test -z "$INSIDE_EMACS"; and test -x @LIG_BIN@; and test "$LIG_TTY" != (tty)
    if status is-login
        exec @LIG_BIN@ shell --shell (status fish-path) --login
    else
        exec @LIG_BIN@ shell --shell (status fish-path)
    end
end
