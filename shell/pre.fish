# Wrap this interactive fish in lig, unless lig already runs it: lig started
# this process (a fish tab, or `exec fish` in one). A fish started from another
# shell inside lig (`fish` typed in a zsh tab) gets its own lig, like a tmux pane.
if status is-interactive; and test -z "$LIG_DISABLE"; and isatty stdin; and isatty stdout; and test "$TERM" != dumb; and test -z "$INSIDE_EMACS"; and test -x @LIG_BIN@
    # The parent's pid: the 2nd field after the command name in /proc/<pid>/stat.
    set -l __lig_parent (string split ' ' -- (string replace -r '^.*\) ' '' < /proc/$fish_pid/stat))[2]
    if test "$LIG_TTY" != (tty); or test "$__lig_parent" != "$LIG_PID"
        if status is-login
            exec @LIG_BIN@ shell --shell (status fish-path) --login
        else
            exec @LIG_BIN@ shell --shell (status fish-path)
        end
    end
end
