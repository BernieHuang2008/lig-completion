//! `~/.config/lig/config.toml`

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config")).join("lig")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[derive(Default)]
pub struct Config {
    pub ui: Ui,
    pub keys: Keys,
    pub behavior: Behavior,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    /// Maximum number of rows in the popup.
    pub max_rows: u16,
    /// Maximum width of the popup, in columns.
    pub max_width: u16,
    /// Show descriptions next to suggestions.
    pub descriptions: bool,
    /// `"unicode"`, `"nerd"` (Nerd Font glyphs, built into Ghostty) or `"none"`.
    pub icons: String,
    /// Colors: 0-255 palette index or `"#rrggbb"`.
    pub bg: String,
    pub fg: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub description_fg: String,
    pub match_fg: String,
    pub scrollbar_fg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keys {
    pub accept: Vec<String>,
    pub next: Vec<String>,
    pub prev: Vec<String>,
    pub dismiss: Vec<String>,
    /// Show the popup now / hide it.
    pub toggle: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Behavior {
    /// Also match suggestions by subsequence (`gco` → `git-checkout`).
    pub fuzzy: bool,
    /// Enter (when bound to accept) right after a space, with the selection
    /// untouched, runs the line instead of accepting.
    pub enter_runs_on_empty_word: bool,
    /// List files before options and subcommands.
    pub files_first: bool,
    /// Show suggestions right after a space (empty current token).
    pub show_on_empty_token: bool,
    /// Pop up automatically for the first word (command names). Off by
    /// default so short commands can be typed and run with one Enter; the
    /// toggle key (Ctrl+Space) always shows them.
    pub complete_commands: bool,
    /// Kill generator commands after this many milliseconds.
    pub generator_timeout_ms: u64,
    /// Cache generator results for this long when the spec doesn't say.
    pub generator_cache_ms: u64,
    /// Commands for which lig never shows suggestions.
    pub disabled_commands: Vec<String>,
    /// Build specs for other commands from their `--help` output / man page.
    pub help_parsing: bool,
    /// Kill `--help` / `man` after this many milliseconds.
    pub help_timeout_ms: u64,
    /// Never run `<command> --help` for these.
    pub help_skip: Vec<String>,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            max_rows: 8,
            max_width: 90,
            descriptions: true,
            icons: "unicode".into(),
            bg: "236".into(),
            fg: "252".into(),
            selected_bg: "25".into(),
            selected_fg: "231".into(),
            description_fg: "244".into(),
            match_fg: "117".into(),
            scrollbar_fg: "240".into(),
        }
    }
}

impl Default for Keys {
    fn default() -> Self {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
        Keys {
            accept: v(&["enter"]),
            next: v(&["tab", "down", "ctrl-n"]),
            prev: v(&["shift-tab", "up", "ctrl-p"]),
            dismiss: v(&["esc"]),
            toggle: v(&["ctrl-space"]),
        }
    }
}

impl Default for Behavior {
    fn default() -> Self {
        Behavior {
            fuzzy: true,
            enter_runs_on_empty_word: true,
            files_first: false,
            show_on_empty_token: true,
            complete_commands: false,
            generator_timeout_ms: 3000,
            generator_cache_ms: 5000,
            disabled_commands: vec![],
            help_parsing: true,
            help_timeout_ms: 2000,
            help_skip: vec![],
        }
    }
}


impl Config {
    /// Load the config file; problems are reported and defaults are used.
    pub fn load() -> Config {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("lig: ignoring {}: {e}", path.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }
}

/// SGR parameters for a color spec, as foreground (`38`) or background (`48`).
pub fn sgr_color(spec: &str, base: u8) -> String {
    let s = spec.trim();
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
            && let Ok(n) = u32::from_str_radix(hex, 16) {
                return format!("{base};2;{};{};{}", n >> 16, (n >> 8) & 0xff, n & 0xff);
            }
    match s.parse::<u8>() {
        Ok(n) => format!("{base};5;{n}"),
        Err(_) => format!("{}", base + 1), // default color
    }
}
