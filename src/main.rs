mod config;
mod engine;
mod exec;
mod help;
mod icons;
mod theme;
mod keys;
mod osc;
mod parser;
mod popup;
mod session;
mod spec;
mod specs;
mod store;

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

const PRE_ZSH: &str = include_str!("../shell/pre.zsh");
const POST_ZSH: &str = include_str!("../shell/post.zsh");
const PRE_BASH: &str = include_str!("../shell/pre.bash");
const POST_BASH: &str = include_str!("../shell/post.bash");

#[derive(Parser)]
#[command(name = "lig", version, about = "Fig-style IDE autocomplete for your Linux terminal")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a shell inside lig (the zsh integration does this for you)
    Shell {
        /// Shell to run (default: $SHELL)
        #[arg(long)]
        shell: Option<PathBuf>,
        /// Start it as a login shell
        #[arg(long)]
        login: bool,
    },
    /// Print shell integration code: eval "$(lig init zsh)" / eval "$(lig init bash)"
    Init {
        #[arg(value_enum)]
        shell: ShellKind,
        /// `pre` goes at the top of the rc file, `post` at the bottom; `all` does both
        #[arg(value_enum, default_value_t = Part::All)]
        part: Part,
    },
    /// Add lig to ~/.zshrc or ~/.bashrc (default: your $SHELL)
    Install {
        #[arg(value_enum)]
        shell: Option<ShellKind>,
    },
    /// Remove lig from ~/.zshrc or ~/.bashrc (default: your $SHELL)
    Uninstall {
        #[arg(value_enum)]
        shell: Option<ShellKind>,
    },
    /// Manage completion specs
    Specs {
        #[command(subcommand)]
        cmd: SpecsCmd,
    },
    /// Print the suggestions for a command line (for debugging specs)
    Complete {
        /// Working directory to complete in
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Pretend this shell alias exists (`--alias gco='git checkout'`)
        #[arg(long = "alias", value_name = "NAME=VALUE")]
        aliases: Vec<String>,
        /// Text left of the cursor, e.g. "git checkout "
        line: String,
    },
    /// Check the installation
    Doctor,
    /// List every icon (in the mode you use, next to its Unicode fallback)
    Icons,
    /// Show a sample popup with your settings (or try others)
    Preview {
        /// Theme: default, catppuccin, nord, dracula, gruvbox, tokyonight, light, terminal
        #[arg(long)]
        theme: Option<String>,
        /// rounded, square or none
        #[arg(long)]
        border: Option<String>,
        /// compact, normal or relaxed
        #[arg(long)]
        density: Option<String>,
        /// auto, nerd, unicode or none
        #[arg(long)]
        icons: Option<String>,
        /// auto, columns (60) or percent ("50%")
        #[arg(long)]
        width: Option<String>,
        /// rows (8) or percent ("30%")
        #[arg(long)]
        height: Option<String>,
        /// right or bottom
        #[arg(long)]
        description: Option<String>,
        /// Show every theme
        #[arg(long)]
        all: bool,
        /// Show every density
        #[arg(long)]
        densities: bool,
    },
    /// Print the default configuration (save it to `lig config path`)
    Config {
        #[arg(value_enum, default_value_t = ConfigCmd::Default)]
        what: ConfigCmd,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ShellKind {
    Zsh,
    Bash,
}

impl ShellKind {
    /// From `$SHELL`, zsh unless it's bash.
    fn current() -> ShellKind {
        let shell = std::env::var("SHELL").unwrap_or_default();
        if shell.ends_with("/bash") || shell == "bash" { ShellKind::Bash } else { ShellKind::Zsh }
    }

    fn name(self) -> &'static str {
        match self {
            ShellKind::Zsh => "zsh",
            ShellKind::Bash => "bash",
        }
    }

    fn rc(self) -> PathBuf {
        match self {
            ShellKind::Zsh => {
                let dir = std::env::var_os("ZDOTDIR").map(PathBuf::from).or_else(dirs::home_dir).unwrap_or_default();
                dir.join(".zshrc")
            }
            ShellKind::Bash => dirs::home_dir().unwrap_or_default().join(".bashrc"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Part {
    Pre,
    Post,
    All,
}

#[derive(Clone, Copy, ValueEnum)]
enum ConfigCmd {
    Default,
    Path,
}

#[derive(Subcommand)]
enum SpecsCmd {
    /// List commands with a built-in or user spec
    List,
    /// Show where user specs and the --help cache live
    Path,
    /// Forget cached --help parsing results
    ClearCache,
    /// List the native generators spec files can use (`{"builtin": "<id>"}`)
    Generators,
}

/// Append a line to the debug log when `LIG_LOG` is set.
pub fn log(msg: &str) {
    if std::env::var_os("LIG_LOG").is_none() {
        return;
    }
    let dir = dirs::state_dir().unwrap_or_else(std::env::temp_dir).join("lig");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("lig.log")) {
        let _ = writeln!(f, "{msg}");
    }
}

fn exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "lig".into())
}

fn init_script(shell: ShellKind, part: Part) -> String {
    let (pre, post) = match shell {
        ShellKind::Zsh => (PRE_ZSH, POST_ZSH),
        ShellKind::Bash => (PRE_BASH, POST_BASH),
    };
    let mut s = String::new();
    if part != Part::Post {
        s.push_str(pre);
    }
    if part != Part::Pre {
        s.push_str(post);
    }
    s.replace("@LIG_BIN@", &exe())
}

const PRE_BEGIN: &str = "# >>> lig pre >>>";
const PRE_END: &str = "# <<< lig pre <<<";
const POST_BEGIN: &str = "# >>> lig post >>>";
const POST_END: &str = "# <<< lig post <<<";

fn strip_block(text: &str, begin: &str, end: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.split_inclusive('\n') {
        let t = line.trim_end();
        if t == begin {
            skipping = true;
        } else if t == end && skipping {
            skipping = false;
        } else if !skipping {
            out.push_str(line);
        }
    }
    out
}

fn install(shell: ShellKind) -> Result<()> {
    let rc = shell.rc();
    let sh = shell.name();
    let old = std::fs::read_to_string(&rc).unwrap_or_default();
    let backup = rc.with_file_name(format!(".{sh}rc.lig-backup"));
    if !old.is_empty() && !backup.exists() {
        std::fs::write(&backup, &old)?;
    }
    let body = strip_block(&strip_block(&old, PRE_BEGIN, PRE_END), POST_BEGIN, POST_END);
    let body = body.trim_end();
    let bin = exe();
    let new = format!(
        "{PRE_BEGIN}\neval \"$(\"{bin}\" init {sh} pre)\"\n{PRE_END}\n{body}{}{POST_BEGIN}\neval \"$(\"{bin}\" init {sh} post)\"\n{POST_END}\n",
        if body.is_empty() { "" } else { "\n\n" },
    );
    std::fs::write(&rc, new).with_context(|| format!("writing {}", rc.display()))?;
    println!("lig: updated {}", rc.display());
    println!("lig: done. Open a new terminal (or run `exec {sh}`) to start using it.");
    Ok(())
}

fn uninstall(shell: ShellKind) -> Result<()> {
    let rc = shell.rc();
    let old = std::fs::read_to_string(&rc)?;
    let new = strip_block(&strip_block(&old, PRE_BEGIN, PRE_END), POST_BEGIN, POST_END);
    std::fs::write(&rc, format!("{}\n", new.trim_end()))?;
    println!("lig: removed from {}", rc.display());
    Ok(())
}

fn doctor() -> Result<()> {
    let ok = |b: bool| if b { "ok  " } else { "FAIL" };
    for shell in [ShellKind::Zsh, ShellKind::Bash] {
        let path = shell.rc();
        let rc = std::fs::read_to_string(&path).unwrap_or_default();
        let has = rc.contains(PRE_BEGIN) && rc.contains(POST_BEGIN);
        if has || shell.name() == ShellKind::current().name() {
            println!("[{}] {} integration", ok(has), path.display());
        }
    }
    let tty = unsafe {
        let p = libc::ttyname(0);
        (!p.is_null()).then(|| std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
    };
    let inside = tty.is_some() && std::env::var("LIG_TTY").ok() == tty;
    println!("[{}] this terminal is running inside lig", ok(inside));
    let cfg = config::config_path();
    if cfg.exists() {
        let valid = std::fs::read_to_string(&cfg).ok().and_then(|t| toml::from_str::<config::Config>(&t).ok()).is_some();
        println!("[{}] config {}", ok(valid), cfg.display());
    } else {
        println!("[ok  ] config: defaults ({} not present)", cfg.display());
    }
    let mut e = engine::Engine::new(store::Store::default(), config::Config::load().behavior);
    let env = engine::ShellEnv { cwd: std::env::current_dir()?, path: std::env::var("PATH").ok(), ..Default::default() };
    let n = e.complete("git ", &env, false, false).items.len();
    println!("[{}] engine: `git ` → {n} suggestions", ok(n > 0));
    let n = e.complete("ls --", &env, true, false).items.len();
    println!("[{}] --help parsing: `ls --` → {n} suggestions", ok(n > 0));
    Ok(())
}

fn complete(line: &str, cwd: Option<PathBuf>, aliases: Vec<String>) -> Result<()> {
    let cwd = match cwd {
        Some(c) => c,
        None => std::env::current_dir()?,
    };
    let mut e = engine::Engine::new(store::Store::default(), config::Config::load().behavior);
    let env = engine::ShellEnv { cwd, path: std::env::var("PATH").ok(), aliases: std::sync::Arc::new(aliases.into_iter().filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect()), ..Default::default() };
    let t = std::time::Instant::now();
    let c = e.complete(line, &env, true, true);
    let elapsed = t.elapsed();
    for s in &c.items {
        println!("{:<11} {:<32} {:<28} {}", format!("{:?}", s.kind), s.label, format!("{:?}-{}", s.insert, s.delete), s.description);
    }
    eprintln!("{} suggestions in {:.1?}", c.items.len(), elapsed);
    Ok(())
}

fn default_shell() -> PathBuf {
    std::env::var_os("SHELL").map(PathBuf::from).filter(|p| Path::new(p).exists()).unwrap_or_else(|| "/bin/zsh".into())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Shell { shell, login } => {
            icons::detect_in_background();
            let code = session::run(shell.unwrap_or_else(default_shell), login)?;
            std::process::exit(code);
        }
        Cmd::Init { shell, part } => print!("{}", init_script(shell, part)),
        Cmd::Install { shell } => install(shell.unwrap_or_else(ShellKind::current))?,
        Cmd::Uninstall { shell } => uninstall(shell.unwrap_or_else(ShellKind::current))?,
        Cmd::Specs { cmd: SpecsCmd::List } => {
            let user = store::Store::default().commands();
            let mut all: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
            for c in specs::FIG.names() {
                all.insert(c.to_string(), "fig");
            }
            for c in specs::lig_names() {
                let src = if all.contains_key(c) { "lig + fig" } else { "lig" };
                all.insert(c.to_string(), src);
            }
            for c in &user {
                all.insert(c.clone(), "user");
            }
            for (c, src) in &all {
                println!("{c:<32} {src}");
            }
            eprintln!("{} commands ({} lig + {} fig spec files embedded)", all.len(), specs::LIG.count(), specs::FIG.count());
        }
        Cmd::Specs { cmd: SpecsCmd::Path } => {
            println!("user specs: {}", store::user_dir().display());
            println!("--help cache: {}", help::cache_dir().display());
        }
        Cmd::Specs { cmd: SpecsCmd::Generators } => {
            for (id, what) in specs::gens::BUILTIN_IDS {
                println!("{id:<30} {what}");
            }
            println!("{:<30} <prefix> then local paths (e.g. files:file://)", "files:<prefix>");
            println!("{:<30} names of a kubernetes resource type", "kube:names:<type>");
        }
        Cmd::Specs { cmd: SpecsCmd::ClearCache } => {
            let dir = help::cache_dir();
            if dir.exists() {
                std::fs::remove_dir_all(&dir)?;
            }
            println!("lig: cleared {}", dir.display());
        }
        Cmd::Complete { cwd, aliases, line } => complete(&line, cwd, aliases)?,
        Cmd::Doctor => doctor()?,
        Cmd::Icons => {
            icons::detect();
            let style = config::Config::load().ui.icons;
            let nerd = icons::use_nerd(&style);
            println!("icons = \"{style}\" → using {}\n", if nerd { "Nerd Font glyphs" } else { "Unicode symbols" });
            let fallback = icons::legend(false);
            for (i, (g, c, what)) in icons::legend(nerd).into_iter().enumerate() {
                let alt = fallback.get(i).filter(|_| nerd).map_or(String::new(), |(ug, _, _)| format!("   (unicode: {ug})"));
                println!("  \x1b[{c}m{g}\x1b[0m  {what}{alt}");
            }
        }
        Cmd::Preview { theme, border, density, icons: icon_style, width, height, description, all, densities: density_all } => {
            icons::detect();
            let mut ui = config::Config::load().ui;
            ui.theme = theme.unwrap_or(ui.theme);
            ui.border = border.unwrap_or(ui.border);
            ui.density = density.unwrap_or(ui.density);
            ui.icons = icon_style.unwrap_or(ui.icons);
            if let Some(w) = width {
                ui.width = config::Size::Text(w);
            }
            if let Some(h) = height {
                ui.height = config::Size::Text(h);
            }
            if let Some(d) = description {
                ui.description_position = d;
            }
            let cols = unsafe {
                let mut ws: libc::winsize = std::mem::zeroed();
                if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 { ws.ws_col } else { 100 }
            };
            if density_all {
                for d in ["compact", "normal", "relaxed"] {
                    ui.density = d.into();
                    println!("\x1b[1m{d}\x1b[0m");
                    println!("{}", popup::preview(&ui, cols));
                }
                return Ok(());
            }
            let names: Vec<String> = if all { theme::NAMES.iter().map(|s| s.to_string()).collect() } else { vec![ui.theme.clone()] };
            for name in names {
                ui.theme = name.clone();
                println!("\x1b[1m{name}\x1b[0m  (border = {}, density = {}, icons = {})", ui.border, ui.density, ui.icons);
                println!("{}", popup::preview(&ui, cols));
            }
        }
        Cmd::Config { what: ConfigCmd::Path } => println!("{}", config::config_path().display()),
        Cmd::Config { what: ConfigCmd::Default } => {
            print!("{}", toml::to_string_pretty(&config::Config::default())?);
        }
    }
    Ok(())
}
