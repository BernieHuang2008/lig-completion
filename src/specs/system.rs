use super::gens::{self, SIGNALS};
use crate::spec::{Arg, Opt, Subcommand, arg, cmd, opt, sug};

fn units() -> Arg {
    arg("unit").variadic().generator(gens::systemd_units())
}

pub fn systemctl() -> Subcommand {
    let unit_cmd = |n: &str, d: &str| cmd(n, d).arg(units());
    cmd("systemctl", "Control the systemd system and service manager")
        .opts([
            opt("--user", "Talk to the user service manager").persistent(),
            opt("--system", "Talk to the system service manager").persistent(),
            opt("--now", "Also start/stop the unit when enabling/disabling").persistent(),
            opt("-a, --all", "Show all units").persistent(),
            opt("-t, --type", "Unit type").persistent().eq().arg(arg("type").suggest(&["service", "socket", "timer", "mount", "target", "path", "slice", "scope", "device"])),
            opt("--state", "Unit state").persistent().eq().arg(arg("state").suggest(&["running", "failed", "active", "inactive", "enabled", "disabled", "exited"])),
            opt("-l, --full", "Don't ellipsize").persistent(),
            opt("--no-pager", "Don't pipe into a pager").persistent(),
            opt("-q, --quiet", "Suppress output").persistent(),
            opt("-f, --force", "Force the operation").persistent(),
            opt("-H, --host", "Operate on a remote host").persistent().arg(arg("host").generator(gens::ssh_hosts())),
        ])
        .subs([
            unit_cmd("status", "Show runtime status of units"),
            unit_cmd("start", "Start units"),
            unit_cmd("stop", "Stop units"),
            unit_cmd("restart", "Restart units"),
            unit_cmd("reload", "Reload units"),
            unit_cmd("reload-or-restart", "Reload units if supported, otherwise restart"),
            unit_cmd("try-restart", "Restart units if running"),
            unit_cmd("enable", "Enable units"),
            unit_cmd("disable", "Disable units"),
            unit_cmd("reenable", "Reenable units"),
            unit_cmd("mask", "Mask units"),
            unit_cmd("unmask", "Unmask units"),
            unit_cmd("is-active", "Check whether units are active"),
            unit_cmd("is-enabled", "Check whether units are enabled"),
            unit_cmd("is-failed", "Check whether units are failed"),
            unit_cmd("cat", "Show unit files"),
            unit_cmd("show", "Show properties of units"),
            unit_cmd("edit", "Edit units"),
            unit_cmd("kill", "Send a signal to the processes of units"),
            unit_cmd("reset-failed", "Reset the failed state of units"),
            unit_cmd("list-dependencies", "Show unit dependency tree"),
            cmd("list-units", "List loaded units").arg(arg("pattern").optional()),
            cmd("list-unit-files", "List installed unit files").arg(arg("pattern").optional()),
            cmd("list-timers", "List timer units"),
            cmd("list-sockets", "List socket units"),
            cmd("daemon-reload", "Reload the systemd manager configuration"),
            cmd("daemon-reexec", "Re-execute the systemd manager"),
            cmd("poweroff", "Shut down and power off"),
            cmd("reboot", "Shut down and reboot"),
            cmd("suspend", "Suspend the system"),
            cmd("hibernate", "Hibernate the system"),
            cmd("get-default", "Show the default target"),
            cmd("set-default", "Set the default target").arg(arg("target").suggest(&["graphical.target", "multi-user.target"])),
            cmd("isolate", "Start one unit and stop all others").arg(arg("unit").generator(gens::systemd_units())),
        ])
}

pub fn journalctl() -> Subcommand {
    cmd("journalctl", "Query the systemd journal").opts([
        opt("-u, --unit", "Show messages for a unit").repeatable().arg(arg("unit").generator(gens::systemd_units())),
        opt("--user-unit", "Show messages for a user unit").arg(arg("unit")),
        opt("--user", "Show the user journal"),
        opt("-f, --follow", "Follow the journal"),
        opt("-e, --pager-end", "Jump to the end"),
        opt("-r, --reverse", "Newest entries first"),
        opt("-n, --lines", "Number of entries to show").arg(arg("n").suggest(&["50", "100", "500", "all"])),
        opt("-b, --boot", "Show messages from a boot").arg(arg("id").optional().suggest(&["0", "-1", "-2"])),
        opt("-k, --dmesg", "Show kernel messages"),
        opt("-x, --catalog", "Add explanatory help texts"),
        opt("-p, --priority", "Filter by priority").arg(arg("level").suggest(&["emerg", "alert", "crit", "err", "warning", "notice", "info", "debug"])),
        opt("-g, --grep", "Filter by pattern").arg(arg("pattern")),
        opt("-S, --since", "Entries since a date").arg(arg("date").suggest(&["today", "yesterday", "-1h", "-10m"])),
        opt("-U, --until", "Entries until a date").arg(arg("date").suggest(&["now", "today"])),
        opt("-o, --output", "Output format").arg(arg("mode").suggest(&["short", "short-iso", "short-precise", "verbose", "json", "json-pretty", "cat"])),
        opt("--no-pager", "Don't pipe into a pager"),
        opt("--list-boots", "Show boots"),
        opt("--disk-usage", "Show disk usage"),
        opt("--vacuum-size", "Reduce the journal to a size").eq().arg(arg("size")),
        opt("--vacuum-time", "Remove entries older than").eq().arg(arg("time")),
    ])
}

pub fn apt(name: &str) -> Subcommand {
    let installed = || arg("package").variadic().generator(gens::apt_installed());
    let available = || arg("package").variadic().files().generator(gens::apt_available());
    let common: Vec<Opt> = vec![
        opt("-y, --yes, --assume-yes", "Assume yes to all prompts").persistent(),
        opt("-q, --quiet", "Less output").persistent(),
        opt("-s, --simulate, --dry-run", "Simulate").persistent(),
    ];
    let mut c = cmd(name, "Package manager").opts(common).subs([
        cmd("install", "Install packages")
            .opts([
                opt("--no-install-recommends", "Don't install recommended packages"),
                opt("--reinstall", "Reinstall packages"),
                opt("-f, --fix-broken", "Fix broken dependencies"),
                opt("--only-upgrade", "Only upgrade installed packages"),
                opt("-t, --target-release", "Target release").arg(arg("release")),
            ])
            .arg(available()),
        cmd("remove", "Remove packages").opt(opt("--purge", "Also remove configuration")).arg(installed()),
        cmd("purge", "Remove packages and their configuration").arg(installed()),
        cmd("autoremove", "Remove automatically installed unused packages").opt(opt("--purge", "Also remove configuration")),
        cmd("update", "Update the package index"),
        cmd("upgrade", "Upgrade installed packages").arg(installed().optional()),
        cmd("full-upgrade, dist-upgrade", "Upgrade, removing packages if needed"),
        cmd("reinstall", "Reinstall packages").arg(installed()),
        cmd("show", "Show package details").arg(available()),
        cmd("source", "Download source packages").arg(available()),
        cmd("download", "Download the .deb file").arg(available()),
        cmd("clean", "Clear the package cache"),
        cmd("autoclean", "Clear obsolete packages from the cache"),
        cmd("policy", "Show policy settings").arg(available().optional()),
        cmd("depends", "Show package dependencies").arg(available()),
        cmd("rdepends", "Show reverse dependencies").arg(available()),
    ]);
    if name == "apt" {
        c = c.subs([
            cmd("search", "Search package descriptions").opt(opt("--names-only", "Search names only")).arg(arg("regex")),
            cmd("list", "List packages").opts([opt("--installed", "Installed packages"), opt("--upgradable", "Upgradable packages"), opt("--manual-installed", "Manually installed packages"), opt("-a, --all-versions", "All versions")]).arg(arg("pattern").optional()),
            cmd("edit-sources", "Edit the sources list"),
            cmd("satisfy", "Satisfy dependency strings").arg(arg("deps")),
        ]);
    }
    c
}

fn signal_opts() -> Vec<Opt> {
    let mut v = vec![];
    for (n, d) in SIGNALS {
        v.push(opt(&format!("-{n}, -SIG{n}"), d).priority(40.0));
    }
    for (n, d) in [("9", "KILL"), ("15", "TERM"), ("1", "HUP"), ("2", "INT"), ("3", "QUIT"), ("19", "STOP"), ("18", "CONT")] {
        v.push(opt(&format!("-{n}"), d).priority(45.0));
    }
    v
}

fn signal_arg() -> Arg {
    arg("signal").suggest_desc(SIGNALS)
}

pub fn kill() -> Subcommand {
    cmd("kill", "Send a signal to a process")
        .posix_noncompliant()
        .opts([opt("-s", "Signal to send").arg(signal_arg()), opt("-l, -L", "List signal names")])
        .opts(signal_opts())
        .arg(arg("pid").variadic().generator(gens::processes()))
}

pub fn killall(name: &str) -> Subcommand {
    let mut c = cmd(name, "Signal processes by name")
        .opts([
            opt("-s, --signal", "Signal to send").arg(signal_arg()),
            opt("-u, --user", "Only processes of this user").arg(arg("user")),
            opt("-i, --interactive", "Ask before killing"),
            opt("-e, --exact", "Require an exact match"),
            opt("-I, --ignore-case", "Case insensitive match"),
            opt("-w, --wait", "Wait for processes to die"),
        ])
        .arg(arg("name").variadic().generator(gens::process_names()));
    if name == "pkill" || name == "pgrep" {
        c = c.opts([opt("-f, --full", "Match the full command line"), opt("-x, --exact", "Match exactly"), opt("-n, --newest", "Newest matching process"), opt("-o, --oldest", "Oldest matching process"), opt("-l, --list-name", "List names too")]);
    }
    c
}

pub fn ssh() -> Subcommand {
    cmd("ssh", "OpenSSH remote login client")
        .opts([
            opt("-p", "Port").arg(arg("port")),
            opt("-i", "Identity file").arg(arg("file").files()),
            opt("-l", "Login name").arg(arg("user")),
            opt("-L", "Local port forwarding").arg(arg("[bind:]port:host:hostport")),
            opt("-R", "Remote port forwarding").arg(arg("[bind:]port:host:hostport")),
            opt("-D", "Dynamic SOCKS forwarding").arg(arg("port")),
            opt("-J", "Jump host").arg(arg("destination").generator(gens::ssh_hosts())),
            opt("-o", "Configuration option").arg(arg("option").suggest(&["StrictHostKeyChecking=no", "UserKnownHostsFile=/dev/null", "ServerAliveInterval=60", "ForwardAgent=yes"])),
            opt("-F", "Config file").arg(arg("file").files()),
            opt("-A", "Forward the authentication agent"),
            opt("-X", "Enable X11 forwarding"),
            opt("-N", "Don't execute a remote command"),
            opt("-T", "Disable pseudo-terminal allocation"),
            opt("-t", "Force pseudo-terminal allocation"),
            opt("-C", "Compress data"),
            opt("-v", "Verbose").repeatable(),
            opt("-q", "Quiet mode"),
            opt("-4", "IPv4 only"),
            opt("-6", "IPv6 only"),
        ])
        .arg(arg("destination").generator(gens::ssh_hosts()))
        .arg(arg("command").optional().variadic())
}

pub fn scp() -> Subcommand {
    let hosts = crate::spec::gen_fn("ssh:hosts-colon", |_| {
        gens::ssh_host_list()
            .into_iter()
            .map(|s| {
                let n = format!("{}:", s.meta.names[0]);
                sug(n.clone()).desc("remote host").insert(n)
            })
            .collect()
    })
    .ttl(30_000);
    cmd("scp", "OpenSSH secure file copy")
        .opts([
            opt("-r", "Copy directories recursively"),
            opt("-P", "Port").arg(arg("port")),
            opt("-i", "Identity file").arg(arg("file").files()),
            opt("-p", "Preserve times and modes"),
            opt("-C", "Compress"),
            opt("-q", "Quiet"),
            opt("-J", "Jump host").arg(arg("destination").generator(gens::ssh_hosts())),
            opt("-o", "ssh option").arg(arg("option")),
        ])
        .arg(arg("source").variadic().files().generator(hosts))
}

pub fn make() -> Subcommand {
    cmd("make", "Maintain program dependencies")
        .opts([
            opt("-C, --directory", "Change to directory").arg(arg("dir").folders()),
            opt("-f, --file, --makefile", "Read this makefile").arg(arg("file").files()),
            opt("-j, --jobs", "Parallel jobs").arg(arg("n").optional()),
            opt("-k, --keep-going", "Keep going when some targets fail"),
            opt("-n, --dry-run, --just-print", "Print commands without running them"),
            opt("-B, --always-make", "Unconditionally make all targets"),
            opt("-s, --silent, --quiet", "Don't echo recipes"),
            opt("-e, --environment-overrides", "Environment overrides makefile variables"),
            opt("-i, --ignore-errors", "Ignore errors from recipes"),
            opt("-q, --question", "Exit status says whether up to date"),
            opt("-p, --print-data-base", "Print make's internal database"),
        ])
        .arg(arg("target").optional().variadic().generator(gens::make_targets()))
}

pub fn tmux() -> Subcommand {
    let target = || opt("-t", "Target session").arg(arg("session").generator(gens::tmux_sessions()));
    cmd("tmux", "Terminal multiplexer")
        .opts([opt("-L", "Socket name").arg(arg("name")), opt("-S", "Socket path").arg(arg("path").files()), opt("-f", "Config file").arg(arg("file").files()), opt("-V", "Print version")])
        .subs([
            cmd("new-session, new", "Create a new session")
                .opts([opt("-s", "Session name").arg(arg("name")), opt("-d", "Detached"), opt("-n", "Window name").arg(arg("name")), opt("-c", "Start directory").arg(arg("dir").folders()), opt("-A", "Attach if the session exists")]),
            cmd("attach-session, attach, a", "Attach to a session").opts([target(), opt("-d", "Detach other clients")]),
            cmd("list-sessions, ls", "List sessions"),
            cmd("kill-session", "Kill a session").opts([target(), opt("-a", "Kill all other sessions")]),
            cmd("kill-server", "Kill the server and all sessions"),
            cmd("switch-client, switch", "Switch the client to a session").opt(target()),
            cmd("rename-session, rename", "Rename a session").opt(target()).arg(arg("new-name")),
            cmd("detach-client, detach", "Detach the current client"),
            cmd("new-window, neww", "Create a new window").opts([opt("-n", "Window name").arg(arg("name")), opt("-c", "Start directory").arg(arg("dir").folders())]),
            cmd("split-window, splitw", "Split a pane").opts([opt("-h", "Horizontal split"), opt("-v", "Vertical split"), opt("-c", "Start directory").arg(arg("dir").folders())]),
            cmd("source-file, source", "Execute commands from a file").arg(arg("file").files()),
            cmd("list-keys, lsk", "List key bindings"),
            cmd("show-options, show", "Show options").opts([opt("-g", "Global options"), opt("-w", "Window options")]),
            cmd("set-option, set", "Set an option").opts([opt("-g", "Global"), opt("-w", "Window option")]).arg(arg("option")).arg(arg("value").optional()),
        ])
}

pub fn kubectl() -> Subcommand {
    let resource = || arg("type").suggest(gens::KUBE_RESOURCES);
    let name = || arg("name").optional().variadic().generator(gens::kube_names(None));
    let pod = || arg("pod").generator(gens::kube_names(Some("pods")));
    let output = || opt("-o, --output", "Output format").arg(arg("format").suggest(&["json", "yaml", "wide", "name", "jsonpath=", "custom-columns="]));
    cmd("kubectl", "Kubernetes command-line tool")
        .opts([
            opt("-n, --namespace", "Namespace").persistent().arg(arg("namespace").generator(gens::kube_namespaces())),
            opt("-A, --all-namespaces", "All namespaces").persistent(),
            opt("--context", "Kubeconfig context").persistent().arg(arg("context").generator(gens::kube_contexts())),
            opt("--kubeconfig", "Kubeconfig file").persistent().arg(arg("file").files()),
            opt("-l, --selector", "Label selector").persistent().arg(arg("selector")),
        ])
        .subs([
            cmd("get", "Display resources").opts([output(), opt("-w, --watch", "Watch for changes"), opt("--show-labels", "Show labels")]).arg(resource()).arg(name()),
            cmd("describe", "Show details of resources").arg(resource()).arg(name()),
            cmd("delete", "Delete resources").opts([opt("-f, --filename", "File").arg(arg("file").files()), opt("--force", "Force"), opt("--grace-period", "Seconds").arg(arg("n"))]).arg(resource().optional()).arg(name()),
            cmd("edit", "Edit a resource").arg(resource()).arg(name()),
            cmd("apply", "Apply a configuration").opts([opt("-f, --filename", "File or directory").repeatable().arg(arg("file").files()), opt("-k, --kustomize", "Kustomization directory").arg(arg("dir").folders()), opt("--dry-run", "Dry run").eq().arg(arg("mode").suggest(&["client", "server", "none"])), opt("--server-side", "Server-side apply")]),
            cmd("create", "Create a resource").opt(opt("-f, --filename", "File").arg(arg("file").files())).arg(arg("type").optional().suggest(&["deployment", "namespace", "secret", "configmap", "service", "job", "cronjob"])),
            cmd("logs", "Print the logs of a container")
                .opts([opt("-f, --follow", "Stream logs"), opt("-c, --container", "Container").arg(arg("name")), opt("--tail", "Lines").arg(arg("n")), opt("-p, --previous", "Previous container instance"), opt("--since", "Duration").arg(arg("duration")), opt("--all-containers", "All containers")])
                .arg(pod()),
            cmd("exec", "Execute a command in a container")
                .opts([opt("-i, --stdin", "Pass stdin"), opt("-t, --tty", "Allocate a TTY"), opt("-it", "Interactive TTY"), opt("-c, --container", "Container").arg(arg("name"))])
                .arg(pod())
                .arg(arg("command").optional().variadic()),
            cmd("port-forward", "Forward local ports to a pod").arg(pod()).arg(arg("ports").variadic()),
            cmd("rollout", "Manage rollouts").subs([
                cmd("status", "Show rollout status").arg(arg("resource").generator(gens::kube_names(Some("deployments")))),
                cmd("restart", "Restart a resource").arg(arg("resource").generator(gens::kube_names(Some("deployments")))),
                cmd("history", "View rollout history").arg(arg("resource").generator(gens::kube_names(Some("deployments")))),
                cmd("undo", "Undo a rollout").arg(arg("resource").generator(gens::kube_names(Some("deployments")))),
            ]),
            cmd("scale", "Set a new size").opt(opt("--replicas", "Replicas").eq().arg(arg("n"))).arg(resource()).arg(name()),
            cmd("top", "Display resource usage").subs([cmd("pod", "Pods").arg(pod().optional()), cmd("node", "Nodes")]),
            cmd("config", "Modify kubeconfig files").subs([
                cmd("use-context", "Set the current context").arg(arg("context").generator(gens::kube_contexts())),
                cmd("get-contexts", "List contexts"),
                cmd("current-context", "Display the current context"),
                cmd("set-context", "Set a context entry").opt(opt("--namespace", "Namespace").eq().arg(arg("ns").generator(gens::kube_namespaces()))).arg(arg("context").generator(gens::kube_contexts())),
                cmd("view", "Display merged kubeconfig settings"),
            ]),
            cmd("cp", "Copy files to and from containers").arg(arg("src").files()).arg(arg("dest").files()),
            cmd("label", "Update labels").arg(resource()).arg(name()),
            cmd("api-resources", "Print supported API resources"),
            cmd("cluster-info", "Display cluster information"),
            cmd("version", "Print version information"),
            cmd("explain", "Documentation of resources").arg(resource()),
        ])
}

/// Commands whose argument is another command line.
pub fn wrapper(name: &str, desc: &str, opts: Vec<Opt>) -> Subcommand {
    cmd(name, desc).opts(opts).arg(arg("command").command())
}

pub fn sudo(name: &str) -> Subcommand {
    wrapper(
        name,
        "Execute a command as another user",
        vec![
            opt("-u, --user", "Run as this user").arg(arg("user").generator(crate::spec::gen_fn("sys:users", |_| {
                std::fs::read_to_string("/etc/passwd")
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|l| l.split(':').next())
                    .map(sug)
                    .collect()
            }))),
            opt("-i, --login", "Run a login shell"),
            opt("-s, --shell", "Run a shell"),
            opt("-E, --preserve-env", "Preserve the environment"),
            opt("-k, --reset-timestamp", "Invalidate cached credentials"),
            opt("-v, --validate", "Refresh cached credentials"),
            opt("-l, --list", "List allowed commands"),
            opt("-b, --background", "Run in the background"),
        ],
    )
}

pub fn cd(name: &str) -> Subcommand {
    cmd(name, "Change the working directory").arg(arg("dir").optional().folders().suggest_desc(&[("-", "Previous directory")]))
}

pub fn snap() -> Subcommand {
    let installed = || arg("snap").variadic().generator(gens::snaps_installed());
    cmd("snap", "Manage snap packages").subs([
        cmd("install", "Install snaps")
            .opts([
                opt("--classic", "Put the snap in classic mode and disable security confinement"),
                opt("--channel", "Use this channel").eq().arg(arg("channel").suggest(&["stable", "candidate", "beta", "edge"])),
                opt("--edge", "Install from the edge channel"),
                opt("--beta", "Install from the beta channel"),
                opt("--candidate", "Install from the candidate channel"),
                opt("--devmode", "Put the snap in development mode"),
                opt("--dangerous", "Install the given snap file even if there are no pre-acknowledged signatures"),
            ])
            .arg(arg("snap").variadic().files().generator(gens::snap_store())),
        cmd("remove", "Remove snaps").opt(opt("--purge", "Don't save a snapshot")).arg(installed()),
        cmd("refresh", "Refresh snaps").opts([opt("--list", "Show available refreshes"), opt("--channel", "Channel").eq().arg(arg("channel"))]).arg(installed().optional()),
        cmd("revert", "Revert a snap to its previous revision").arg(installed()),
        cmd("list", "List installed snaps").opt(opt("--all", "Show all revisions")),
        cmd("find, search", "Find packages to install").arg(arg("query").optional().generator(gens::snap_store())),
        cmd("info", "Show detailed information about snaps").arg(arg("snap").variadic().generator(gens::snaps_installed()).generator(gens::snap_store())),
        cmd("run", "Run the given snap command").arg(arg("app").generator(gens::snaps_installed())).arg(arg("args").optional().variadic()),
        cmd("enable", "Enable a snap").arg(installed()),
        cmd("disable", "Disable a snap").arg(installed()),
        cmd("connections", "List interface connections").arg(installed().optional()),
        cmd("services", "Query the status of services").arg(installed().optional()),
        cmd("start", "Start services").arg(installed()),
        cmd("stop", "Stop services").arg(installed()),
        cmd("restart", "Restart services").arg(installed()),
        cmd("logs", "Retrieve logs for services").opts([opt("-f", "Follow"), opt("-n", "Lines").arg(arg("n"))]).arg(installed()),
        cmd("changes", "List system changes"),
        cmd("version", "Show version details"),
    ])
}

/// Debian's external repository manager.
pub fn extrepo() -> Subcommand {
    cmd("extrepo", "Manage external Debian repositories")
        .opts([
            opt("--url", "URL to fetch the repository metadata from").persistent().arg(arg("url").suggest(&[
                "https://extrepo-team.pages.debian.net/extrepo-data",
                "file:///usr/share/extrepo/offline-data",
            ])),
            opt("--offlinedata", "Use the data from the extrepo-offline-data package").persistent(),
            opt("--mirror", "Use this mirror of the repository (enable, update)").persistent().arg(arg("url")),
            opt("--tor", "Tor mode for the repository").persistent().arg(arg("mode").suggest_desc(&[
                ("off", "No tor configuration (default)"),
                ("auto", "The .onion URL if there is one, else tunnel through tor"),
                ("onion", "Use the .onion URL; fail if there is none"),
                ("tunnel", "Tunnel the regular URL through tor"),
                ("if-onion", "The .onion URL if there is one, else no tor"),
            ])),
        ])
        .subs([
            cmd("search", "Search repositories by name, description or URL (a regex)")
                .arg(arg("key").optional().generator(gens::extrepo_available())),
            cmd("enable", "Enable a repository (re-enables a disabled one without updating it)")
                .arg(arg("repository").generator(gens::extrepo_available())),
            cmd("disable", "Disable a repository (adds Enabled: no to its apt sources)")
                .arg(arg("repository").generator(gens::extrepo_installed(Some(true)))),
            cmd("update", "Rewrite a repository's apt sources and keyring from the current metadata (all, without a name)")
                .arg(arg("repository").optional().generator(gens::extrepo_installed(None))),
        ])
}

/// zoxide's `z` / `zi` (shell functions), also rupa/z.
pub fn z(name: &str) -> Subcommand {
    let desc = if name == "zi" { "Pick a frecent directory interactively" } else { "Jump to a frecent directory" };
    cmd(name, desc).arg(arg("keywords").optional().variadic().folders().generator(gens::zoxide_dirs()))
}

/// `which`, `man`, `type`, …: arguments are command names.
pub fn command_lookup(name: &str, desc: &str) -> Subcommand {
    cmd(name, desc).arg(arg("command").variadic().command_names())
}

pub fn wrappers() -> Vec<Subcommand> {
    vec![
        wrapper("time", "Time a command", vec![]),
        wrapper("nohup", "Run a command immune to hangups", vec![]),
        wrapper("exec", "Replace the shell with a command", vec![]),
        wrapper("command", "Run a command bypassing functions", vec![opt("-v", "Print how the command would be resolved")]),
        wrapper("builtin", "Run a shell builtin", vec![]),
        wrapper("nice", "Run with a modified scheduling priority", vec![opt("-n, --adjustment", "Niceness").arg(arg("n"))]),
        wrapper("ionice", "Set IO scheduling class", vec![opt("-c, --class", "Class").arg(arg("class").suggest(&["1", "2", "3"])), opt("-n, --classdata", "Priority").arg(arg("n"))]),
        cmd("timeout", "Run a command with a time limit")
            .opts([opt("-s, --signal", "Signal").arg(signal_arg()), opt("-k, --kill-after", "Kill after").arg(arg("duration"))])
            .arg(arg("duration").suggest(&["10s", "1m", "5m"]))
            .arg(arg("command").command()),
        wrapper("watch", "Execute a program periodically", vec![opt("-n, --interval", "Seconds between updates").arg(arg("seconds")), opt("-d, --differences", "Highlight changes"), opt("-t, --no-title", "Turn off the header")]),
        wrapper("xargs", "Build command lines from standard input", vec![opt("-0, --null", "Items are null terminated"), opt("-n, --max-args", "Max args per command").arg(arg("n")), opt("-P, --max-procs", "Parallel processes").arg(arg("n")), opt("-I", "Replace string").arg(arg("replstr")), opt("-r, --no-run-if-empty", "Don't run if input is empty")]),
        wrapper("env", "Run a program in a modified environment", vec![opt("-i, --ignore-environment", "Start with an empty environment"), opt("-u, --unset", "Remove a variable").arg(arg("name"))]),
        wrapper("strace", "Trace system calls", vec![opt("-f", "Follow forks"), opt("-e", "Filter expression").arg(arg("expr")), opt("-o", "Output file").arg(arg("file").files()), opt("-p", "Attach to PID").arg(arg("pid").generator(gens::processes()))]),
        wrapper("flatpak-spawn", "Run a command on the host", vec![opt("--host", "Run on the host")]),
        cmd("source", "Read and run commands from a file").arg(arg("file").files()).arg(arg("args").optional().variadic()),
        cmd(".", "Read and run commands from a file").arg(arg("file").files()).arg(arg("args").optional().variadic()),
        command_lookup("which", "Locate a command"),
        command_lookup("whereis", "Locate the binary, source, and manual page files"),
        command_lookup("type", "Describe how a name would be interpreted"),
        command_lookup("man", "Display manual pages"),
        command_lookup("tldr", "Simplified man pages"),
    ]
}
