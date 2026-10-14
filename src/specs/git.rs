use super::gens::{self, Changes};
use crate::spec::{Arg, Opt, Subcommand, arg, cmd, opt};

fn branch() -> Arg {
    arg("branch").generator(gens::git_branches())
}

fn commit() -> Arg {
    arg("commit").generator(gens::git_branches()).generator(gens::git_tags()).generator(gens::git_commits())
}

fn pathspec() -> Arg {
    arg("pathspec").optional().variadic().files()
}

fn remote() -> Arg {
    arg("remote").generator(gens::git_remotes())
}

fn quiet() -> Opt {
    opt("-q, --quiet", "Be quiet")
}

fn verbose() -> Opt {
    opt("-v, --verbose", "Be verbose")
}

pub fn spec() -> Subcommand {
    cmd("git", "The stupid content tracker")
        .opts([
            opt("-C", "Run as if git was started in <path>").arg(arg("path").folders()),
            opt("-c", "Set a configuration variable").arg(arg("name=value")),
            opt("--no-pager", "Do not pipe output into a pager"),
            opt("-p, --paginate", "Pipe output into a pager"),
            opt("--git-dir", "Path to the repository").eq().arg(arg("path").folders()),
            opt("--work-tree", "Path to the working tree").eq().arg(arg("path").folders()),
            opt("--bare", "Treat the repository as a bare repository"),
            opt("--version", "Print the git version"),
            opt("-h, --help", "Show help"),
        ])
        .subs([
            cmd("add", "Add file contents to the index")
                .opts([
                    opt("-A, --all", "Add changes from all tracked and untracked files"),
                    opt("-u, --update", "Update tracked files"),
                    opt("-p, --patch", "Interactively choose hunks to add"),
                    opt("-i, --interactive", "Add modified contents interactively"),
                    opt("-n, --dry-run", "Don't actually add the files"),
                    opt("-f, --force", "Allow adding otherwise ignored files"),
                    opt("-N, --intent-to-add", "Record only the fact that the path will be added later"),
                    verbose(),
                ])
                .arg(pathspec().generator(gens::git_changed(Changes::Unstaged))),
            cmd("bisect", "Use binary search to find the commit that introduced a bug").subs([
                cmd("start", "Start bisecting").arg(commit().optional().variadic()),
                cmd("good", "Mark a commit as good").arg(commit().optional()),
                cmd("bad", "Mark a commit as bad").arg(commit().optional()),
                cmd("skip", "Skip a commit").arg(commit().optional()),
                cmd("reset", "Finish bisecting").arg(commit().optional()),
                cmd("log", "Show the bisect log"),
                cmd("run", "Run a script to decide good/bad").arg(arg("cmd").command()),
            ]),
            cmd("blame", "Show what revision and author last modified each line")
                .opts([opt("-L", "Annotate only the line range").arg(arg("start,end")), opt("-w", "Ignore whitespace"), opt("-C", "Detect moved or copied lines")])
                .arg(arg("file").files()),
            cmd("branch", "List, create, or delete branches")
                .opts([
                    opt("-a, --all", "List both remote-tracking and local branches"),
                    opt("-r, --remotes", "List the remote-tracking branches"),
                    opt("-d, --delete", "Delete a branch").arg(arg("branch").variadic().generator(gens::git_local_branches())),
                    opt("-D", "Force-delete a branch").arg(arg("branch").variadic().generator(gens::git_local_branches())),
                    opt("-m, --move", "Rename a branch").arg(arg("old").generator(gens::git_local_branches())).arg(arg("new").optional()),
                    opt("-M", "Force-rename a branch").arg(arg("old").generator(gens::git_local_branches())).arg(arg("new").optional()),
                    opt("-c, --copy", "Copy a branch").arg(arg("old").generator(gens::git_local_branches())),
                    opt("-u, --set-upstream-to", "Set the upstream").arg(branch()),
                    opt("--unset-upstream", "Remove the upstream information"),
                    opt("--show-current", "Print the name of the current branch"),
                    opt("--merged", "List branches merged into a commit").arg(commit().optional()),
                    opt("--no-merged", "List branches not merged into a commit").arg(commit().optional()),
                    opt("--sort", "Sort by key").eq().arg(arg("key").suggest(&["-committerdate", "refname", "-creatordate"])),
                    opt("-l, --list", "List branches matching a pattern"),
                    verbose(),
                ])
                .arg(arg("name").optional())
                .arg(commit().optional()),
            cmd("checkout", "Switch branches or restore working tree files")
                .opts([
                    opt("-b", "Create a new branch and switch to it").arg(arg("new-branch")).arg(commit().optional()),
                    opt("-B", "Create or reset a branch and switch to it").arg(arg("new-branch")).arg(commit().optional()),
                    opt("-t, --track", "Set upstream info for the new branch"),
                    opt("--detach", "Detach HEAD at the commit"),
                    opt("-f, --force", "Throw away local modifications"),
                    opt("-p, --patch", "Interactively select hunks"),
                    opt("--ours", "Check out our version for unmerged files"),
                    opt("--theirs", "Check out their version for unmerged files"),
                    quiet(),
                ])
                .arg(branch().optional().generator(gens::git_changed(Changes::Modified)).suggest(&["-"]))
                .arg(pathspec()),
            cmd("cherry-pick", "Apply the changes introduced by some existing commits")
                .opts([
                    opt("-e, --edit", "Edit the commit message"),
                    opt("-n, --no-commit", "Don't commit"),
                    opt("-x", "Append the original commit hash to the message"),
                    opt("--continue", "Continue the operation"),
                    opt("--abort", "Cancel the operation"),
                    opt("--skip", "Skip the current commit"),
                ])
                .arg(commit().variadic()),
            cmd("clean", "Remove untracked files from the working tree")
                .opts([
                    opt("-n, --dry-run", "Only show what would be done"),
                    opt("-f, --force", "Actually delete"),
                    opt("-d", "Also remove untracked directories"),
                    opt("-x", "Also remove ignored files"),
                    opt("-X", "Remove only ignored files"),
                    opt("-i, --interactive", "Clean interactively"),
                ])
                .arg(pathspec()),
            cmd("clone", "Clone a repository into a new directory")
                .opts([
                    opt("-b, --branch", "Check out this branch").arg(arg("branch")),
                    opt("--depth", "Create a shallow clone").arg(arg("depth")),
                    opt("--recurse-submodules", "Initialize submodules"),
                    opt("--bare", "Make a bare repository"),
                    opt("--single-branch", "Clone only one branch"),
                    opt("--filter", "Partial clone filter").eq().arg(arg("filter").suggest(&["blob:none", "tree:0"])),
                    opt("-o, --origin", "Name of the remote").arg(arg("name")),
                ])
                .arg(arg("repository"))
                .arg(arg("directory").optional().folders()),
            cmd("commit", "Record changes to the repository")
                .opts([
                    opt("-m, --message", "Use the given message").arg(arg("message")),
                    opt("-a, --all", "Stage all modified and deleted files"),
                    opt("-am", "Stage all and commit with a message").insert("-am '{cursor}'"),
                    opt("--amend", "Replace the tip of the current branch"),
                    opt("--no-edit", "Use the selected commit message without launching an editor"),
                    opt("-v, --verbose", "Show the diff in the message editor"),
                    opt("-p, --patch", "Interactively choose hunks to commit"),
                    opt("-s, --signoff", "Add a Signed-off-by trailer"),
                    opt("-S, --gpg-sign", "GPG-sign the commit"),
                    opt("--no-verify", "Bypass the pre-commit and commit-msg hooks"),
                    opt("--allow-empty", "Allow an empty commit"),
                    opt("--fixup", "Create a fixup commit").arg(commit()),
                    opt("--squash", "Create a squash commit").arg(commit()),
                    opt("-C, --reuse-message", "Reuse the message of a commit").arg(commit()),
                    opt("-F, --file", "Take the message from a file").arg(arg("file").files()),
                    opt("--author", "Override the author").eq().arg(arg("author")),
                    opt("--date", "Override the date").eq().arg(arg("date")),
                ])
                .arg(pathspec()),
            cmd("config", "Get and set repository or global options")
                .opts([
                    opt("--global", "Use the global config file"),
                    opt("--system", "Use the system config file"),
                    opt("--local", "Use the repository config file"),
                    opt("-l, --list", "List all variables"),
                    opt("--unset", "Remove a variable"),
                    opt("-e, --edit", "Open an editor"),
                    opt("--get", "Get a value"),
                ])
                .arg(arg("name").optional().suggest(&[
                    "user.name", "user.email", "core.editor", "init.defaultBranch", "pull.rebase", "push.autoSetupRemote",
                    "commit.gpgsign", "core.autocrlf", "merge.conflictStyle", "rerere.enabled", "fetch.prune",
                ]))
                .arg(arg("value").optional()),
            cmd("diff", "Show changes between commits, commit and working tree, etc")
                .opts([
                    opt("--staged, --cached", "Show staged changes"),
                    opt("--stat", "Show a diffstat"),
                    opt("--name-only", "Show only names of changed files"),
                    opt("--name-status", "Show names and status of changed files"),
                    opt("-w, --ignore-all-space", "Ignore whitespace"),
                    opt("--word-diff", "Show a word diff"),
                    opt("--color-words", "Show colored word diff"),
                    opt("--check", "Warn about whitespace errors"),
                    opt("-U, --unified", "Lines of context").eq().arg(arg("n")),
                ])
                .arg(commit().optional().generator(gens::git_changed(Changes::Modified)))
                .arg(pathspec()),
            cmd("fetch", "Download objects and refs from another repository")
                .opts([
                    opt("--all", "Fetch all remotes"),
                    opt("-p, --prune", "Remove deleted remote-tracking refs"),
                    opt("-t, --tags", "Fetch all tags"),
                    opt("--depth", "Limit fetching depth").arg(arg("depth")),
                    opt("--unshallow", "Convert a shallow repository to a complete one"),
                    verbose(),
                ])
                .arg(remote().optional())
                .arg(branch().optional().variadic()),
            cmd("grep", "Print lines matching a pattern")
                .opts([
                    opt("-i, --ignore-case", "Ignore case"),
                    opt("-n, --line-number", "Show line numbers"),
                    opt("-w, --word-regexp", "Match whole words"),
                    opt("-l, --files-with-matches", "Show only file names"),
                    opt("-E, --extended-regexp", "Use extended regexps"),
                ])
                .arg(arg("pattern"))
                .arg(pathspec()),
            cmd("init", "Create an empty Git repository")
                .opts([
                    opt("-b, --initial-branch", "Name of the initial branch").arg(arg("name").suggest(&["main", "master"])),
                    opt("--bare", "Create a bare repository"),
                    quiet(),
                ])
                .arg(arg("directory").optional().folders()),
            cmd("log", "Show commit logs")
                .opts([
                    opt("--oneline", "One commit per line"),
                    opt("--graph", "Draw the commit graph"),
                    opt("--all", "Show all refs"),
                    opt("--decorate", "Show ref names"),
                    opt("-p, --patch", "Show the diff of each commit"),
                    opt("--stat", "Show a diffstat for each commit"),
                    opt("-n, --max-count", "Limit the number of commits").arg(arg("n")),
                    opt("--author", "Filter by author").eq().arg(arg("pattern")),
                    opt("--grep", "Filter by message").eq().arg(arg("pattern")),
                    opt("--since, --after", "Commits newer than a date").eq().arg(arg("date")),
                    opt("--until, --before", "Commits older than a date").eq().arg(arg("date")),
                    opt("--follow", "Follow renames of a file"),
                    opt("--reverse", "Show commits in reverse order"),
                    opt("--first-parent", "Follow only the first parent"),
                    opt("--format, --pretty", "Pretty-print format")
                        .eq()
                        .arg(arg("format").suggest(&["oneline", "short", "medium", "full", "fuller", "raw"])),
                    opt("-S", "Find commits that add or remove a string").arg(arg("string")),
                ])
                .arg(commit().optional())
                .arg(pathspec()),
            cmd("merge", "Join two or more development histories together")
                .opts([
                    opt("--no-ff", "Always create a merge commit"),
                    opt("--ff-only", "Refuse to merge unless fast-forward"),
                    opt("--squash", "Squash the changes into the working tree"),
                    opt("--abort", "Abort the current merge"),
                    opt("--continue", "Continue the current merge"),
                    opt("-m", "Merge commit message").arg(arg("message")),
                    opt("--no-edit", "Accept the auto-generated message"),
                    opt("-s, --strategy", "Merge strategy").arg(arg("strategy").suggest(&["ort", "recursive", "resolve", "octopus", "ours", "subtree"])),
                    opt("-X, --strategy-option", "Strategy option").arg(arg("option").suggest(&["ours", "theirs", "patience"])),
                ])
                .arg(commit().variadic().optional()),
            cmd("mv", "Move or rename a file").opts([opt("-f, --force", "Overwrite"), opt("-n, --dry-run", "Dry run")]).arg(arg("source").variadic().files()),
            cmd("pull", "Fetch from and integrate with another repository or branch")
                .opts([
                    opt("-r, --rebase", "Rebase instead of merge"),
                    opt("--no-rebase", "Merge instead of rebase"),
                    opt("--ff-only", "Only fast-forward"),
                    opt("--autostash", "Stash local changes before the operation"),
                    opt("--all", "Fetch all remotes"),
                    opt("-p, --prune", "Prune remote-tracking branches"),
                    verbose(),
                    quiet(),
                ])
                .arg(remote().optional())
                .arg(branch().optional()),
            cmd("push", "Update remote refs along with associated objects")
                .opts([
                    opt("-u, --set-upstream", "Set the upstream for the branch"),
                    opt("-f, --force", "Force the update"),
                    opt("--force-with-lease", "Force, but only if the remote is as expected"),
                    opt("--force-if-includes", "Force only if the remote's tip is integrated locally"),
                    opt("--tags", "Push all tags"),
                    opt("--follow-tags", "Push annotated tags reachable from pushed refs"),
                    opt("-d, --delete", "Delete remote refs"),
                    opt("--all", "Push all branches"),
                    opt("-n, --dry-run", "Do everything except send the updates"),
                    opt("--no-verify", "Bypass the pre-push hook"),
                    opt("-o, --push-option", "Transmit an option to the server").arg(arg("option")),
                    verbose(),
                ])
                .arg(remote().optional())
                .arg(branch().optional().variadic()),
            cmd("rebase", "Reapply commits on top of another base tip")
                .opts([
                    opt("-i, --interactive", "Make a list of commits to edit"),
                    opt("--onto", "Starting point for the new commits").arg(commit()),
                    opt("--continue", "Continue the rebase"),
                    opt("--abort", "Abort the rebase"),
                    opt("--skip", "Skip the current patch"),
                    opt("--autosquash", "Move fixup/squash commits"),
                    opt("--autostash", "Stash local changes before the operation"),
                    opt("--root", "Rebase all commits reachable from the branch"),
                    opt("-x, --exec", "Run a command after each commit").arg(arg("cmd")),
                ])
                .arg(commit().optional())
                .arg(branch().optional()),
            cmd("reflog", "Manage reflog information").arg(branch().optional()),
            cmd("remote", "Manage set of tracked repositories")
                .opt(verbose())
                .subs([
                    cmd("add", "Add a remote").opt(opt("-f", "Fetch after adding")).arg(arg("name")).arg(arg("url")),
                    cmd("remove, rm", "Remove a remote").arg(remote()),
                    cmd("rename", "Rename a remote").arg(remote()).arg(arg("new")),
                    cmd("set-url", "Change the URL of a remote").opt(opt("--push", "Change the push URL")).arg(remote()).arg(arg("url")),
                    cmd("get-url", "Show the URL of a remote").arg(remote()),
                    cmd("show", "Show information about a remote").arg(remote()),
                    cmd("prune", "Delete stale remote-tracking branches").arg(remote()),
                    cmd("update", "Fetch updates for remotes").arg(remote().optional()),
                ]),
            cmd("reset", "Reset current HEAD to the specified state")
                .opts([
                    opt("--soft", "Keep index and working tree"),
                    opt("--mixed", "Reset the index, keep the working tree (default)"),
                    opt("--hard", "Discard all changes"),
                    opt("--merge", "Reset, keeping unmerged changes"),
                    opt("--keep", "Reset, keeping local changes"),
                    opt("-p, --patch", "Interactively select hunks"),
                    quiet(),
                ])
                .arg(commit().optional().generator(gens::git_changed(Changes::Staged)).suggest(&["HEAD", "HEAD~1", "HEAD^"]))
                .arg(pathspec()),
            cmd("restore", "Restore working tree files")
                .opts([
                    opt("-S, --staged", "Restore the index"),
                    opt("-W, --worktree", "Restore the working tree (default)"),
                    opt("-s, --source", "Restore from this tree").arg(commit()),
                    opt("-p, --patch", "Interactively select hunks"),
                    opt("--ours", "Use our version for unmerged files"),
                    opt("--theirs", "Use their version for unmerged files"),
                ])
                .arg(arg("pathspec").variadic().generator(gens::git_changed(Changes::Modified)).generator(gens::git_changed(Changes::Staged))),
            cmd("revert", "Revert some existing commits")
                .opts([
                    opt("-n, --no-commit", "Don't commit"),
                    opt("--no-edit", "Don't open the editor"),
                    opt("--continue", "Continue the revert"),
                    opt("--abort", "Abort the revert"),
                ])
                .arg(commit().variadic()),
            cmd("rm", "Remove files from the working tree and from the index")
                .opts([
                    opt("--cached", "Only remove from the index"),
                    opt("-f, --force", "Override the up-to-date check"),
                    opt("-r", "Allow recursive removal"),
                    opt("-n, --dry-run", "Dry run"),
                ])
                .arg(arg("pathspec").variadic().files()),
            cmd("show", "Show various types of objects")
                .opts([opt("--stat", "Show a diffstat"), opt("--name-only", "Show only names of changed files"), opt("--oneline", "Short format")])
                .arg(commit().optional().variadic()),
            cmd("stash", "Stash the changes in a dirty working directory away")
                .opts([opt("-u, --include-untracked", "Include untracked files"), opt("-m, --message", "Stash message").arg(arg("message"))])
                .subs([
                    cmd("push", "Save local modifications to a new stash")
                        .opts([
                            opt("-m, --message", "Stash message").arg(arg("message")),
                            opt("-u, --include-untracked", "Include untracked files"),
                            opt("-a, --all", "Include ignored files"),
                            opt("-k, --keep-index", "Keep staged changes in the index"),
                            opt("-S, --staged", "Stash only staged changes"),
                            opt("-p, --patch", "Interactively select hunks"),
                        ])
                        .arg(pathspec()),
                    cmd("pop", "Apply a stash and remove it").opt(opt("--index", "Restore the index too")).arg(arg("stash").optional().generator(gens::git_stashes())),
                    cmd("apply", "Apply a stash").opt(opt("--index", "Restore the index too")).arg(arg("stash").optional().generator(gens::git_stashes())),
                    cmd("drop", "Remove a stash").arg(arg("stash").optional().generator(gens::git_stashes())),
                    cmd("show", "Show the changes in a stash").opt(opt("-p, --patch", "Show the diff")).arg(arg("stash").optional().generator(gens::git_stashes())),
                    cmd("list", "List stashes"),
                    cmd("clear", "Remove all stashes"),
                    cmd("branch", "Create a branch from a stash").arg(arg("branch")).arg(arg("stash").optional().generator(gens::git_stashes())),
                ]),
            cmd("status", "Show the working tree status")
                .opts([
                    opt("-s, --short", "Short format"),
                    opt("-b, --branch", "Show branch info in short format"),
                    opt("--porcelain", "Machine-readable output"),
                    opt("-u, --untracked-files", "Show untracked files").eq().arg(arg("mode").optional().suggest(&["no", "normal", "all"])),
                    opt("--ignored", "Show ignored files"),
                ])
                .arg(pathspec()),
            cmd("switch", "Switch branches")
                .opts([
                    opt("-c, --create", "Create a new branch").arg(arg("new-branch")).arg(commit().optional()),
                    opt("-C, --force-create", "Create or reset a branch").arg(arg("new-branch")).arg(commit().optional()),
                    opt("-d, --detach", "Detach HEAD at the commit"),
                    opt("--orphan", "Create a new orphan branch").arg(arg("new-branch")),
                    opt("-t, --track", "Set upstream info"),
                    opt("-f, --force, --discard-changes", "Throw away local modifications"),
                ])
                .arg(branch().suggest(&["-"])),
            cmd("tag", "Create, list, delete or verify a tag object")
                .opts([
                    opt("-a, --annotate", "Make an annotated tag"),
                    opt("-m, --message", "Tag message").arg(arg("message")),
                    opt("-d, --delete", "Delete tags").arg(arg("tag").variadic().generator(gens::git_tags())),
                    opt("-l, --list", "List tags"),
                    opt("-s, --sign", "Make a GPG-signed tag"),
                    opt("-f, --force", "Replace an existing tag"),
                ])
                .arg(arg("tagname").optional())
                .arg(commit().optional()),
            cmd("worktree", "Manage multiple working trees").subs([
                cmd("add", "Create a new working tree")
                    .opts([opt("-b", "Create a new branch").arg(arg("new-branch")), opt("--detach", "Detach HEAD"), opt("-f, --force", "Force")])
                    .arg(arg("path").folders())
                    .arg(branch().optional()),
                cmd("list", "List working trees"),
                cmd("remove", "Remove a working tree").opt(opt("-f, --force", "Force")).arg(arg("worktree").folders()),
                cmd("prune", "Prune working tree information"),
                cmd("move", "Move a working tree").arg(arg("worktree").folders()).arg(arg("new-path").folders()),
            ]),
            cmd("submodule", "Initialize, update or inspect submodules").subs([
                cmd("add", "Add a submodule").arg(arg("repository")).arg(arg("path").optional().folders()),
                cmd("update", "Update submodules")
                    .opts([opt("--init", "Initialize uninitialized submodules"), opt("--recursive", "Recurse into nested submodules"), opt("--remote", "Use the remote-tracking branch")])
                    .arg(pathspec()),
                cmd("init", "Initialize submodules").arg(pathspec()),
                cmd("status", "Show submodule status").arg(pathspec()),
                cmd("sync", "Sync submodule URLs").arg(pathspec()),
                cmd("foreach", "Run a command in each submodule").arg(arg("command").command()),
            ]),
            cmd("describe", "Give an object a human readable name").opts([opt("--tags", "Use any tag"), opt("--always", "Show a hash as fallback")]).arg(commit().optional()),
            cmd("shortlog", "Summarize git log output").opts([opt("-s, --summary", "Only show counts"), opt("-n, --numbered", "Sort by number of commits"), opt("-e, --email", "Show emails")]),
            cmd("archive", "Create an archive of files from a named tree")
                .opts([opt("--format", "Archive format").eq().arg(arg("fmt").suggest(&["tar", "zip", "tar.gz"])), opt("-o, --output", "Output file").arg(arg("file").files())])
                .arg(commit()),
            cmd("gc", "Cleanup unnecessary files and optimize the local repository").opts([opt("--aggressive", "Optimize more aggressively"), opt("--prune", "Prune loose objects").eq().arg(arg("date").optional())]),
            cmd("help", "Display help information about Git").arg(arg("command").optional()),
        ])
}
