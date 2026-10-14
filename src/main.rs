mod config;
mod engine;
mod exec;
mod js;
mod keys;
mod osc;
mod parser;
mod popup;
mod session;
mod spec;
mod store;

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

const PRE_ZSH: &str = include_str!("../shell/pre.zsh");
const POST_ZSH: &str = include_str!("../shell/post.zsh");

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
    /// Print shell integration code: eval "$(lig init zsh)"
    Init {
        #[arg(value_enum)]
        shell: ShellKind,
        /// `pre` goes at the top of .zshrc, `post` at the bottom; `all` does both
        #[arg(value_enum, default_value_t = Part::All)]
        part: Part,
    },
    /// Add lig to ~/.zshrc and download the completion specs
    Install {
        /// Don't download specs
        #[arg(long)]
        no_specs: bool,
    },
    /// Remove lig from ~/.zshrc
    Uninstall,
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
        /// Text left of the cursor, e.g. "git checkout "
        line: String,
    },
    /// Check the installation
    Doctor,
    /// Print the default configuration (save it to `lig config path`)
    Config {
        #[arg(value_enum, default_value_t = ConfigCmd::Default)]
        what: ConfigCmd,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ShellKind {
    Zsh,
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
    /// Download the latest @withfig/autocomplete specs from npm
    Update {
        /// A specific package version
        #[arg(long)]
        version: Option<String>,
    },
    /// List commands that have a spec
    List,
    /// Show where specs are read from
    Path,
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

fn init_script(part: Part) -> String {
    let mut s = String::new();
    if part != Part::Post {
        s.push_str(PRE_ZSH);
    }
    if part != Part::Pre {
        s.push_str(POST_ZSH);
    }
    s.replace("@LIG_BIN@", &exe())
}

fn zshrc() -> PathBuf {
    let dir = std::env::var_os("ZDOTDIR").map(PathBuf::from).or_else(dirs::home_dir).unwrap_or_default();
    dir.join(".zshrc")
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

fn install(no_specs: bool) -> Result<()> {
    let rc = zshrc();
    let old = std::fs::read_to_string(&rc).unwrap_or_default();
    let backup = rc.with_file_name(".zshrc.lig-backup");
    if !old.is_empty() && !backup.exists() {
        std::fs::write(&backup, &old)?;
    }
    let body = strip_block(&strip_block(&old, PRE_BEGIN, PRE_END), POST_BEGIN, POST_END);
    let body = body.trim_end();
    let bin = exe();
    let new = format!(
        "{PRE_BEGIN}\neval \"$(\"{bin}\" init zsh pre)\"\n{PRE_END}\n{body}{}{POST_BEGIN}\neval \"$(\"{bin}\" init zsh post)\"\n{POST_END}\n",
        if body.is_empty() { "" } else { "\n\n" },
    );
    std::fs::write(&rc, new).with_context(|| format!("writing {}", rc.display()))?;
    println!("lig: updated {}", rc.display());
    if !no_specs && store::Store::default().installed_version().is_none() {
        store::update(None)?;
    }
    println!("lig: done. Open a new terminal (or run `exec zsh`) to start using it.");
    Ok(())
}

fn uninstall() -> Result<()> {
    let rc = zshrc();
    let old = std::fs::read_to_string(&rc)?;
    let new = strip_block(&strip_block(&old, PRE_BEGIN, PRE_END), POST_BEGIN, POST_END);
    std::fs::write(&rc, format!("{}\n", new.trim_end()))?;
    println!("lig: removed from {}. Specs remain in {}", rc.display(), store::data_dir().display());
    Ok(())
}

fn doctor() -> Result<()> {
    let ok = |b: bool| if b { "ok  " } else { "FAIL" };
    let st = store::Store::default();
    let ver = st.installed_version();
    println!("[{}] specs: {}", ok(ver.is_some()), ver.as_deref().map_or("not installed — run `lig specs update`".to_string(), |v| format!("{} {v} in {}", store::NPM_PACKAGE, store::data_dir().display())));
    let rc = std::fs::read_to_string(zshrc()).unwrap_or_default();
    let has = rc.contains(PRE_BEGIN) && rc.contains(POST_BEGIN);
    println!("[{}] {} integration", ok(has), zshrc().display());
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
    match engine::Engine::new(st, config::Config::load().behavior) {
        Ok(mut e) => {
            let env = engine::ShellEnv { cwd: std::env::current_dir()?, path: std::env::var("PATH").ok() };
            let n = e.complete("git ", &env, false).items.len();
            println!("[{}] engine: `git ` → {n} suggestions", ok(n > 0));
        }
        Err(e) => println!("[FAIL] engine: {e}"),
    }
    Ok(())
}

fn complete(line: &str, cwd: Option<PathBuf>) -> Result<()> {
    let cwd = match cwd {
        Some(c) => c,
        None => std::env::current_dir()?,
    };
    let mut e = engine::Engine::new(store::Store::default(), config::Config::load().behavior)?;
    let env = engine::ShellEnv { cwd, path: std::env::var("PATH").ok() };
    let t = std::time::Instant::now();
    let c = e.complete(line, &env, true);
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
            let code = session::run(shell.unwrap_or_else(default_shell), login)?;
            std::process::exit(code);
        }
        Cmd::Init { shell: ShellKind::Zsh, part } => print!("{}", init_script(part)),
        Cmd::Install { no_specs } => install(no_specs)?,
        Cmd::Uninstall => uninstall()?,
        Cmd::Specs { cmd: SpecsCmd::Update { version } } => {
            store::update(version.as_deref())?;
        }
        Cmd::Specs { cmd: SpecsCmd::List } => {
            for c in store::Store::default().commands() {
                println!("{c}");
            }
        }
        Cmd::Specs { cmd: SpecsCmd::Path } => {
            println!("{}", store::user_dir().display());
            println!("{}", store::data_dir().display());
        }
        Cmd::Complete { cwd, line } => complete(&line, cwd)?,
        Cmd::Doctor => doctor()?,
        Cmd::Config { what: ConfigCmd::Path } => println!("{}", config::config_path().display()),
        Cmd::Config { what: ConfigCmd::Default } => {
            print!("{}", toml::to_string_pretty(&config::Config::default())?);
        }
    }
    Ok(())
}
