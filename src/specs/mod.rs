//! Built-in completion specs.
//!
//! * Hand-written in Rust (the modules below), with native generators.
//! * ~1500 converted from withfig/autocomplete (`specs/fig/`, see `fig.rs`).
//!   Where both exist they are merged: the Rust spec wins, the converted one
//!   fills in the options and subcommands it lacks.
//!
//! Commands with neither get a spec parsed from `--help` / man (`help.rs`);
//! user JSON/TOML specs in `~/.config/lig/specs/` override everything.

pub mod fig;
pub mod gens;
mod docker;
mod git;
mod node;
mod rust;
mod system;

use crate::spec::Subcommand;

/// Names with a built-in spec.
pub const NAMES: &[&str] = &[
    "git", "docker", "cargo", "rustup", "npm", "npx", "pnpm", "yarn", "bun", "pip", "pip3", "systemctl", "journalctl",
    "apt", "apt-get", "kill", "killall", "pkill", "pgrep", "ssh", "scp", "make", "tmux", "kubectl", "sudo", "doas",
    "cd", "pushd", "time", "nohup", "exec", "command", "builtin", "nice", "ionice", "timeout", "watch", "xargs", "env",
    "strace", "flatpak-spawn", "which", "whereis", "type", "man", "tldr",
];

pub fn builtin(name: &str) -> Option<Subcommand> {
    Some(match name {
        "git" => git::spec(),
        "docker" => docker::spec(),
        "cargo" => rust::cargo(),
        "rustup" => rust::rustup(),
        "npm" => node::npm(),
        "npx" => node::npx(),
        "pnpm" | "yarn" | "bun" => node::pm(name),
        "pip" | "pip3" => node::pip(name),
        "systemctl" => system::systemctl(),
        "journalctl" => system::journalctl(),
        "apt" | "apt-get" => system::apt(name),
        "kill" => system::kill(),
        "killall" | "pkill" | "pgrep" => system::killall(name),
        "ssh" => system::ssh(),
        "scp" => system::scp(),
        "make" => system::make(),
        "tmux" => system::tmux(),
        "kubectl" => system::kubectl(),
        "sudo" | "doas" => system::sudo(name),
        "cd" | "pushd" => system::cd(name),
        _ => return system::wrappers().into_iter().find(|s| s.meta.names[0] == name),
    })
}

/// Merge `extra` into `base`: `base` wins, `extra` adds what's missing.
pub fn merge(base: &Subcommand, extra: &Subcommand) -> Subcommand {
    let mut out = base.clone();
    if out.meta.description.is_none() {
        out.meta.description = extra.meta.description.clone();
    }
    for o in &extra.options {
        if !out.options.iter().any(|b| b.meta.names.iter().any(|n| o.meta.names.contains(n))) {
            out.options.push(o.clone());
        }
    }
    for s in &extra.subcommands {
        match out.subcommands.iter().position(|b| b.meta.names.iter().any(|n| s.meta.names.contains(n))) {
            Some(i) => out.subcommands[i] = std::sync::Arc::new(merge(&out.subcommands[i], s)),
            None => out.subcommands.push(s.clone()),
        }
    }
    if out.args.is_empty() {
        out.args = extra.args.clone();
    }
    if out.additional_suggestions.is_empty() {
        out.additional_suggestions = extra.additional_suggestions.clone();
    }
    if out.load_spec.is_none() && base.subcommands.is_empty() && base.args.is_empty() {
        out.load_spec = extra.load_spec.clone();
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_name_builds() {
        for n in super::NAMES {
            let s = super::builtin(n).unwrap_or_else(|| panic!("no builtin spec for {n}"));
            assert_eq!(s.meta.names[0], *n);
        }
    }
}
