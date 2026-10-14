//! Generators shared by the built-in specs.

use std::path::{Path, PathBuf};

use crate::spec::{GenCtx, Generator, Kind, SuggestionDef, Trigger, gen_fn, sug};

// ---------------------------------------------------------------- git

/// Local and remote branches, most recently committed first.
pub fn git_branches() -> Generator {
    gen_fn("git:branches", |cx| {
        let out = cx.lines(&[
            "git",
            "--no-optional-locks",
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname)%09%(subject)",
            "refs/heads",
            "refs/remotes",
        ]);
        out.iter()
            .filter_map(|l| {
                let (r, subject) = l.split_once('\t').unwrap_or((l, ""));
                if let Some(b) = r.strip_prefix("refs/heads/") {
                    Some(sug(b).desc(subject).priority(60.0))
                } else {
                    let b = r.strip_prefix("refs/remotes/")?;
                    (!b.ends_with("/HEAD")).then(|| sug(b).desc(subject).priority(40.0))
                }
            })
            .collect()
    })
}

pub fn git_local_branches() -> Generator {
    gen_fn("git:local-branches", |cx| {
        cx.lines(&["git", "--no-optional-locks", "for-each-ref", "--sort=-committerdate", "--format=%(refname:short)%09%(subject)", "refs/heads"])
            .iter()
            .map(|l| {
                let (b, s) = l.split_once('\t').unwrap_or((l, ""));
                sug(b).desc(s)
            })
            .collect()
    })
}

pub fn git_remotes() -> Generator {
    gen_fn("git:remotes", |cx| cx.lines(&["git", "remote", "-v"]).iter().filter_map(|l| {
        let mut p = l.split_whitespace();
        let (name, url) = (p.next()?, p.next().unwrap_or(""));
        l.ends_with("(fetch)").then(|| sug(name).desc(url))
    }).collect())
}

pub fn git_tags() -> Generator {
    gen_fn("git:tags", |cx| cx.lines(&["git", "tag", "--sort=-creatordate"]).into_iter().map(|t| sug(t).desc("tag")).collect())
}

pub fn git_commits() -> Generator {
    gen_fn("git:commits", |cx| {
        cx.lines(&["git", "--no-optional-locks", "log", "-n", "100", "--format=%h%x09%s%x09%cr"])
            .iter()
            .map(|l| {
                let mut p = l.splitn(3, '\t');
                let (h, s, when) = (p.next().unwrap_or(""), p.next().unwrap_or(""), p.next().unwrap_or(""));
                sug(h).desc(format!("{s} ({when})"))
            })
            .collect()
    })
}

pub fn git_stashes() -> Generator {
    gen_fn("git:stashes", |cx| {
        cx.lines(&["git", "stash", "list", "--format=%gd%x09%s"])
            .iter()
            .map(|l| {
                let (n, s) = l.split_once('\t').unwrap_or((l, ""));
                sug(n).desc(s)
            })
            .collect()
    })
}

/// Which changed files a git command is interested in.
#[derive(Clone, Copy)]
pub enum Changes {
    /// Modified in the worktree or untracked (`git add`).
    Unstaged,
    /// In the index (`git restore --staged`, `git reset`).
    Staged,
    /// Modified tracked files (`git restore`, `git diff`, `git checkout --`).
    Modified,
}

/// Files from `git status`, as paths relative to the shell's directory.
pub fn git_changed(which: Changes) -> Generator {
    let id = match which {
        Changes::Unstaged => "git:unstaged",
        Changes::Staged => "git:staged",
        Changes::Modified => "git:modified",
    };
    gen_fn(id, move |cx| {
        let prefix = cx.run(&["git", "rev-parse", "--show-prefix"]).unwrap_or_default();
        let prefix = prefix.trim();
        let Some(out) = cx.run(&["git", "--no-optional-locks", "-c", "core.quotePath=false", "status", "--porcelain", "-uall"]) else {
            return vec![];
        };
        let mut v = vec![];
        for l in out.lines() {
            if l.len() < 4 {
                continue;
            }
            let (x, y) = (l.as_bytes()[0], l.as_bytes()[1]);
            let keep = match which {
                Changes::Unstaged => y != b' ' || x == b'?',
                Changes::Staged => x != b' ' && x != b'?',
                Changes::Modified => y != b' ' && x != b'?',
            };
            if !keep {
                continue;
            }
            let mut path = &l[3..];
            if let Some((_, new)) = path.split_once(" -> ") {
                path = new;
            }
            let path = path.trim_matches('"');
            let state = match (x, y) {
                (b'?', _) => "untracked",
                (_, b'M') | (b'M', _) => "modified",
                (_, b'D') | (b'D', _) => "deleted",
                (b'A', _) => "added",
                (b'R', _) => "renamed",
                (b'U', _) | (_, b'U') => "conflict",
                _ => "changed",
            };
            v.push(sug(relative_to(path, prefix)).desc(state).kind(Kind::File).priority(55.0));
        }
        v
    })
}

/// `path` is relative to the repo root; `prefix` is the cwd relative to it.
fn relative_to(path: &str, prefix: &str) -> String {
    if let Some(p) = path.strip_prefix(prefix) {
        return p.to_string();
    }
    let up = prefix.trim_end_matches('/').split('/').filter(|s| !s.is_empty()).count();
    format!("{}{path}", "../".repeat(up))
}

// ---------------------------------------------------------------- docker

pub fn docker_containers(all: bool) -> Generator {
    let id = if all { "docker:containers" } else { "docker:running" };
    gen_fn(id, move |cx| {
        let mut argv = vec!["docker", "ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Status}}"];
        if all {
            argv.push("-a");
        }
        cx.lines(&argv)
            .iter()
            .map(|l| {
                let p: Vec<&str> = l.split('\t').collect();
                sug(p[0]).desc(format!("{} — {}", p.get(1).unwrap_or(&""), p.get(2).unwrap_or(&"")))
            })
            .collect()
    })
}

pub fn docker_images() -> Generator {
    gen_fn("docker:images", |cx| {
        cx.lines(&["docker", "images", "--format", "{{.Repository}}:{{.Tag}}\t{{.Size}}"])
            .iter()
            .filter(|l| !l.starts_with("<none>"))
            .map(|l| {
                let (n, s) = l.split_once('\t').unwrap_or((l, ""));
                sug(n).desc(s)
            })
            .collect()
    })
}

pub fn docker_list(id: &'static str, what: &'static str) -> Generator {
    gen_fn(id, move |cx| cx.lines(&["docker", what, "ls", "--format", "{{.Name}}"]).into_iter().map(sug).collect())
}

pub fn compose_services() -> Generator {
    gen_fn("docker:compose-services", |cx| {
        cx.lines(&["docker", "compose", "config", "--services"]).into_iter().map(|s| sug(s).desc("service")).collect()
    })
}

// ---------------------------------------------------------------- files in the project

/// Walk up from `cwd` to find `name`.
pub fn find_up(cwd: &Path, name: &str) -> Option<PathBuf> {
    cwd.ancestors().map(|d| d.join(name)).find(|p| p.is_file())
}

/// `package.json` scripts.
pub fn npm_scripts() -> Generator {
    gen_fn("npm:scripts", |cx| {
        let Some(pj) = find_up(cx.cwd, "package.json") else { return vec![] };
        let Ok(v) = std::fs::read_to_string(pj).map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap_or_default()) else {
            return vec![];
        };
        v["scripts"]
            .as_object()
            .map(|o| o.iter().map(|(k, c)| sug(k.clone()).desc(c.as_str().unwrap_or("")).priority(60.0)).collect())
            .unwrap_or_default()
    })
}

/// `package.json` dependencies.
pub fn npm_deps() -> Generator {
    gen_fn("npm:deps", |cx| {
        let Some(pj) = find_up(cx.cwd, "package.json") else { return vec![] };
        let v: serde_json::Value = std::fs::read_to_string(pj).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut out = vec![];
        for (section, label) in [("dependencies", "dependency"), ("devDependencies", "dev dependency"), ("peerDependencies", "peer dependency")] {
            if let Some(o) = v[section].as_object() {
                out.extend(o.iter().map(|(k, ver)| sug(k.clone()).desc(format!("{label} {}", ver.as_str().unwrap_or("")))));
            }
        }
        out
    })
}

/// Cargo targets of a kind (`bin`, `example`, `test`, `bench`) from the
/// conventional directories and `[[bin]]`-style tables.
pub fn cargo_targets(kind: &'static str) -> Generator {
    gen_fn(&format!("cargo:{kind}"), move |cx| {
        let Some(manifest) = find_up(cx.cwd, "Cargo.toml") else { return vec![] };
        let root = manifest.parent().unwrap_or(cx.cwd);
        let dir = match kind {
            "bin" => "src/bin",
            "example" => "examples",
            "test" => "tests",
            _ => "benches",
        };
        let mut names: Vec<String> = std::fs::read_dir(root.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "rs") {
                    p.file_stem().map(|s| s.to_string_lossy().into_owned())
                } else if p.join("main.rs").is_file() {
                    p.file_name().map(|s| s.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        if let Ok(v) = std::fs::read_to_string(&manifest).map(|t| t.parse::<toml::Table>().unwrap_or_default()) {
            if let Some(arr) = v.get(kind).and_then(|x| x.as_array()) {
                names.extend(arr.iter().filter_map(|t| t.get("name")?.as_str().map(str::to_string)));
            }
            if kind == "bin" && root.join("src/main.rs").is_file()
                && let Some(n) = v.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
                    names.push(n.to_string());
                }
        }
        names.sort();
        names.dedup();
        names.into_iter().map(|n| sug(n).desc(kind)).collect()
    })
}

pub fn cargo_features() -> Generator {
    gen_fn("cargo:features", |cx| {
        let Some(manifest) = find_up(cx.cwd, "Cargo.toml") else { return vec![] };
        let v = std::fs::read_to_string(manifest).ok().and_then(|t| t.parse::<toml::Table>().ok()).unwrap_or_default();
        v.get("features")
            .and_then(|f| f.as_table())
            .map(|t| t.keys().map(|k| sug(k.clone()).desc("feature")).collect())
            .unwrap_or_default()
    })
    .query_after(",")
}

pub fn cargo_packages() -> Generator {
    gen_fn("cargo:packages", |cx| {
        let Some(out) = cx.run(&["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"]) else { return vec![] };
        let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
        v["packages"]
            .as_array()
            .map(|a| a.iter().filter_map(|p| Some(sug(p["name"].as_str()?).desc(p["version"].as_str().unwrap_or("")))).collect())
            .unwrap_or_default()
    })
    .ttl(30_000)
}

pub fn cargo_installed() -> Generator {
    gen_fn("cargo:installed", |cx| {
        cx.lines(&["cargo", "install", "--list"])
            .iter()
            .filter(|l| !l.starts_with(' '))
            .filter_map(|l| {
                let (n, rest) = l.split_once(' ')?;
                Some(sug(n).desc(rest.trim_end_matches(':')))
            })
            .collect()
    })
}

pub fn make_targets() -> Generator {
    gen_fn("make:targets", |cx| {
        let dir = cx.flag_value(&["-C", "--directory"]).map_or_else(|| cx.cwd.to_path_buf(), |d| cx.cwd.join(d));
        let file = cx.flag_value(&["-f", "--file", "--makefile"]).map(|f| dir.join(f)).or_else(|| {
            ["GNUmakefile", "makefile", "Makefile"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
        });
        let Some(text) = file.and_then(|f| std::fs::read_to_string(f).ok()) else { return vec![] };
        let mut out: Vec<SuggestionDef> = vec![];
        let mut last_comment = String::new();
        for line in text.lines() {
            if let Some(c) = line.strip_prefix("##").or_else(|| line.strip_prefix('#')) {
                last_comment = c.trim().to_string();
                continue;
            }
            if line.starts_with(['\t', ' ', '.']) || line.contains(":=") || line.contains("?=") {
                last_comment.clear();
                continue;
            }
            if let Some((targets, rest)) = line.split_once(':') {
                if rest.starts_with('=') || targets.contains(['$', '%', '=']) {
                    continue;
                }
                // `target: deps ## description`
                let desc = rest.split_once("##").map(|(_, d)| d.trim().to_string()).unwrap_or_else(|| last_comment.clone());
                for t in targets.split_whitespace() {
                    if !out.iter().any(|s| s.meta.names[0] == t) {
                        out.push(sug(t).desc(desc.clone()));
                    }
                }
            }
            last_comment.clear();
        }
        out
    })
}

// ---------------------------------------------------------------- system

pub fn processes() -> Generator {
    gen_fn("sys:pids", |cx| {
        cx.lines(&["ps", "-eo", "pid=,comm="])
            .iter()
            .filter_map(|l| {
                let (pid, name) = l.trim().split_once(' ')?;
                Some(sug(pid).desc(name.trim()))
            })
            .rev()
            .collect()
    })
    .ttl(2000)
}

pub fn process_names() -> Generator {
    gen_fn("sys:process-names", |cx| {
        let mut names = cx.lines(&["ps", "-eo", "comm="]);
        names.sort();
        names.dedup();
        names.into_iter().map(|n| sug(n).desc("process")).collect()
    })
    .ttl(2000)
}

pub const SIGNALS: &[(&str, &str)] = &[
    ("HUP", "Hangup / reload"),
    ("INT", "Interrupt (Ctrl-C)"),
    ("QUIT", "Quit and dump core"),
    ("KILL", "Kill, cannot be caught"),
    ("TERM", "Terminate gracefully (default)"),
    ("STOP", "Stop, cannot be caught"),
    ("CONT", "Continue if stopped"),
    ("TSTP", "Stop from terminal (Ctrl-Z)"),
    ("USR1", "User-defined signal 1"),
    ("USR2", "User-defined signal 2"),
    ("ALRM", "Alarm clock"),
    ("CHLD", "Child status changed"),
    ("WINCH", "Window size changed"),
    ("PIPE", "Broken pipe"),
];

/// `systemctl` units; `--user` in the line lists user units.
pub fn systemd_units() -> Generator {
    gen_fn("systemd:units", |cx| {
        let user = cx.has_flag(&["--user"]);
        let mut base = vec!["systemctl", "--no-legend", "--plain", "--no-pager"];
        if user {
            base.push("--user");
        }
        let mut out: Vec<SuggestionDef> = vec![];
        let mut argv = base.clone();
        argv.extend(["list-units", "--all"]);
        for l in cx.lines(&argv) {
            let p: Vec<&str> = l.split_whitespace().collect();
            if p.len() >= 4 {
                let desc = format!("{} — {}", p[3], p[4..].join(" "));
                out.push(sug(p[0]).desc(desc).priority(if p[3] == "running" { 60.0 } else { 50.0 }));
            }
        }
        let mut argv = base;
        argv.push("list-unit-files");
        for l in cx.lines(&argv) {
            let p: Vec<&str> = l.split_whitespace().collect();
            if !p.is_empty() && !out.iter().any(|s| s.meta.names[0] == p[0]) {
                out.push(sug(p[0]).desc(p.get(1).copied().unwrap_or("")));
            }
        }
        out
    })
    .ttl(10_000)
}

pub fn apt_installed() -> Generator {
    gen_fn("apt:installed", |cx| cx.lines(&["dpkg-query", "-W", "-f", "${Package}\n"]).into_iter().map(|p| sug(p).desc("installed")).collect())
        .ttl(60_000)
}

/// Package names starting with the first two typed characters.
pub fn apt_available() -> Generator {
    gen_fn("apt:available", |cx| {
        let prefix: String = cx.current().chars().take(2).collect();
        if prefix.chars().count() < 2 {
            return vec![];
        }
        cx.lines(&["apt-cache", "pkgnames", &prefix]).into_iter().map(sug).collect()
    })
    .trigger(Trigger::Prefix(2))
    .ttl(300_000)
}

pub fn ssh_hosts() -> Generator {
    gen_fn("ssh:hosts", |_| ssh_host_list()).ttl(30_000)
}

/// Hosts from `~/.ssh/config` (following `Include`), the system config, and `known_hosts`.
pub fn ssh_host_list() -> Vec<SuggestionDef> {
    {
        let home = dirs::home_dir().unwrap_or_default();
        let mut hosts: Vec<SuggestionDef> = vec![];
        let mut add = |h: &str, desc: &str| {
            if !h.contains(['*', '?', '!']) && !hosts.iter().any(|s| s.meta.names[0] == h) {
                hosts.push(sug(h).desc(desc));
            }
        };
        let mut configs = vec![home.join(".ssh/config"), PathBuf::from("/etc/ssh/ssh_config")];
        let mut i = 0;
        while i < configs.len() && i < 32 {
            let text = std::fs::read_to_string(&configs[i]).unwrap_or_default();
            for line in text.lines() {
                let mut p = line.split_whitespace();
                match p.next().map(str::to_ascii_lowercase).as_deref() {
                    Some("host") => p.for_each(|h| add(h, "ssh config")),
                    Some("include") => {
                        for inc in p {
                            let inc = inc.replace('~', &home.to_string_lossy());
                            let path = if inc.starts_with('/') { PathBuf::from(&inc) } else { home.join(".ssh").join(&inc) };
                            if let Some(parent) = path.parent().filter(|_| inc.contains('*')) {
                                configs.extend(std::fs::read_dir(parent).into_iter().flatten().flatten().map(|e| e.path()));
                            } else {
                                configs.push(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        let known = std::fs::read_to_string(home.join(".ssh/known_hosts")).unwrap_or_default();
        for line in known.lines() {
            if let Some(h) = line.split_whitespace().next().filter(|h| !h.starts_with('|')) {
                for h in h.split(',') {
                    add(h.trim_start_matches('[').split(']').next().unwrap_or(h), "known host");
                }
            }
        }
        hosts
    }
}

pub fn tmux_sessions() -> Generator {
    gen_fn("tmux:sessions", |cx| {
        cx.lines(&["tmux", "list-sessions", "-F", "#S\t#{session_windows} windows#{?session_attached, (attached),}"])
            .iter()
            .map(|l| {
                let (n, d) = l.split_once('\t').unwrap_or((l, ""));
                sug(n).desc(d)
            })
            .collect()
    })
}

pub fn pip_installed() -> Generator {
    gen_fn("pip:installed", |cx| {
        let prog = cx.tokens.first().map_or("pip", String::as_str);
        cx.lines(&[prog, "list", "--format=freeze", "--disable-pip-version-check"])
            .iter()
            .filter_map(|l| {
                let (n, v) = l.split_once("==")?;
                Some(sug(n).desc(v))
            })
            .collect()
    })
    .ttl(30_000)
}

pub fn rustup_toolchains() -> Generator {
    gen_fn("rustup:toolchains", |cx| {
        cx.lines(&["rustup", "toolchain", "list"])
            .iter()
            .map(|l| {
                let name = l.split_whitespace().next().unwrap_or(l);
                sug(name).desc(l.strip_prefix(name).unwrap_or("").trim())
            })
            .collect()
    })
}

pub fn rustup_list(id: &'static str, what: &'static str, installed: bool) -> Generator {
    gen_fn(id, move |cx| {
        let mut argv = vec!["rustup", what, "list"];
        if installed {
            argv.push("--installed");
        }
        cx.lines(&argv).into_iter().map(|t| sug(t.split_whitespace().next().unwrap_or("").to_string())).collect()
    })
    .ttl(60_000)
}

// ---------------------------------------------------------------- kubectl

pub const KUBE_RESOURCES: &[&str] = &[
    "pods", "deployments", "services", "nodes", "namespaces", "configmaps", "secrets", "ingresses", "statefulsets",
    "daemonsets", "replicasets", "jobs", "cronjobs", "persistentvolumeclaims", "persistentvolumes", "serviceaccounts",
    "events", "endpoints", "horizontalpodautoscalers", "networkpolicies", "roles", "rolebindings", "clusterroles",
    "clusterrolebindings", "storageclasses", "customresourcedefinitions", "all",
];

fn kube_base(cx: &GenCtx) -> Vec<String> {
    let mut v = vec!["kubectl".to_string()];
    if let Some(ns) = cx.flag_value(&["-n", "--namespace"]) {
        v.push(format!("--namespace={ns}"));
    }
    if let Some(c) = cx.flag_value(&["--context"]) {
        v.push(format!("--context={c}"));
    }
    v
}

/// Names of the resource type typed before (`kubectl get deploy <here>`), or pods.
pub fn kube_names(default_type: Option<&'static str>) -> Generator {
    gen_fn(&format!("kube:names:{}", default_type.unwrap_or("")), move |cx| {
        let ty = match default_type {
            Some(t) => t.to_string(),
            None => match cx.positionals().get(1) {
                Some(t) => t.to_string(),
                None => return vec![],
            },
        };
        let mut argv = kube_base(cx);
        argv.extend(["get".into(), ty.clone(), "-o".into(), "name".into()]);
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        cx.lines(&argv).iter().map(|l| sug(l.rsplit('/').next().unwrap_or(l)).desc(ty.clone())).collect()
    })
    .ttl(5000)
}

pub fn kube_contexts() -> Generator {
    gen_fn("kube:contexts", |cx| cx.lines(&["kubectl", "config", "get-contexts", "-o", "name"]).into_iter().map(sug).collect())
}

pub fn kube_namespaces() -> Generator {
    gen_fn("kube:namespaces", |cx| {
        cx.lines(&["kubectl", "get", "namespaces", "-o", "name"]).iter().map(|l| sug(l.trim_start_matches("namespace/"))).collect()
    })
    .ttl(30_000)
}

#[cfg(test)]
mod tests {
    use super::relative_to;

    #[test]
    fn git_paths_relative_to_cwd() {
        assert_eq!(relative_to("src/main.rs", ""), "src/main.rs");
        assert_eq!(relative_to("src/main.rs", "src/"), "main.rs");
        assert_eq!(relative_to("README.md", "src/specs/"), "../../README.md");
    }
}

/// `<prefix>` followed by local paths (`file://…`, `fileb://…`).
pub fn prefixed_files(prefix: String) -> Generator {
    gen_fn(&format!("files:{prefix}"), move |cx| {
        let cur = cx.current();
        let Some(rest) = cur.strip_prefix(prefix.as_str()) else {
            return vec![sug(prefix.clone()).desc("local file").insert(prefix.clone())];
        };
        let dir_part = rest.rfind('/').map_or("", |i| &rest[..=i]);
        let dir = if let Some(r) = dir_part.strip_prefix("~/") {
            dirs::home_dir().unwrap_or_default().join(r)
        } else {
            cx.cwd.join(dir_part)
        };
        let mut v: Vec<SuggestionDef> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.path().is_dir() { sug(format!("{name}/")).kind(Kind::Folder) } else { sug(name).kind(Kind::File) }
            })
            .collect();
        v.sort_by(|a, b| a.meta.names[0].cmp(&b.meta.names[0]));
        v
    })
    .trigger(Trigger::Chars("/".into()))
    .query_after("/")
}

/// Native generators referenced by id from spec files (`{"builtin": "git:branches"}`).
pub fn builtin(id: &str) -> Option<Generator> {
    if let Some(prefix) = id.strip_prefix("files:") {
        return Some(prefixed_files(prefix.to_string()));
    }
    Some(match id {
        "git:branches" => git_branches(),
        "git:local-branches" => git_local_branches(),
        "git:remotes" => git_remotes(),
        "git:tags" => git_tags(),
        "git:commits" => git_commits(),
        "git:stashes" => git_stashes(),
        "git:unstaged" => git_changed(Changes::Unstaged),
        "git:staged" => git_changed(Changes::Staged),
        "git:modified" => git_changed(Changes::Modified),
        "docker:containers" => docker_containers(true),
        "docker:running" => docker_containers(false),
        "docker:images" => docker_images(),
        "docker:networks" => docker_list("docker:networks", "network"),
        "docker:volumes" => docker_list("docker:volumes", "volume"),
        "docker:compose-services" => compose_services(),
        "npm:scripts" => npm_scripts(),
        "npm:deps" => npm_deps(),
        "make:targets" => make_targets(),
        "sys:pids" => processes(),
        "sys:process-names" => process_names(),
        "systemd:units" => systemd_units(),
        "apt:installed" => apt_installed(),
        "apt:available" => apt_available(),
        "ssh:hosts" => ssh_hosts(),
        "tmux:sessions" => tmux_sessions(),
        "pip:installed" => pip_installed(),
        "rustup:toolchains" => rustup_toolchains(),
        "kube:contexts" => kube_contexts(),
        "kube:namespaces" => kube_namespaces(),
        "kube:pods" => kube_names(Some("pods")),
        "cargo:packages" => cargo_packages(),
        "cargo:installed" => cargo_installed(),
        "cargo:features" => cargo_features(),
        "zoxide:dirs" => zoxide_dirs(),
        _ => return None,
    })
}

fn home_abbrev(p: &str) -> String {
    match dirs::home_dir().map(|h| h.to_string_lossy().into_owned()) {
        Some(h) if p == h => "~".into(),
        Some(h) => p.strip_prefix(&format!("{h}/")).map_or_else(|| p.to_string(), |r| format!("~/{r}")),
        None => p.to_string(),
    }
}

/// Directories from zoxide (or rupa/z's `~/.z`), ranked and matched by the
/// tool itself using every keyword typed so far.
pub fn zoxide_dirs() -> Generator {
    gen_fn("zoxide:dirs", |cx| {
        let words: Vec<&str> = cx.tokens[1..].iter().map(String::as_str).filter(|t| !t.is_empty() && !t.starts_with('-')).collect();
        let mut argv = vec!["zoxide", "query", "--list", "--"];
        argv.extend(&words);
        let mut paths = cx.lines(&argv);
        if paths.is_empty() && cx.run(&["zoxide", "--version"]).is_none() {
            // rupa/z: `path|rank|time` lines.
            let data = std::env::var("_Z_DATA").map(PathBuf::from).unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".z"));
            let mut entries: Vec<(f64, String)> = std::fs::read_to_string(data)
                .unwrap_or_default()
                .lines()
                .filter_map(|l| {
                    let mut p = l.split('|');
                    let path = p.next()?.to_string();
                    let rank = p.next()?.parse().ok()?;
                    let lower = path.to_lowercase();
                    words.iter().all(|w| lower.contains(&w.to_lowercase())).then_some((rank, path))
                })
                .collect();
            entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            paths = entries.into_iter().map(|e| e.1).collect();
        }
        paths.iter().take(40).map(|p| sug(home_abbrev(p)).desc("frecent directory")).collect()
    })
    .trigger(Trigger::Change)
    .unfiltered()
    .ttl(10_000)
}
