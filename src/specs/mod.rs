//! Built-in completion specs, written in Rust.
//!
//! Commands not listed here get a spec parsed from their `--help` output or
//! man page (see `help.rs`); users can add or override specs with JSON/TOML
//! files in `~/.config/lig/specs/`.

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
