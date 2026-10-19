<p align="center">
  <img src="assets/logo.svg" width="140" alt="Lig logo: a terminal prompt with a suggestion popup under the cursor">
</p>

<h1 align="center">Lig</h1>

<p align="center">
  <b>IDE-style autocomplete for your Linux terminal.</b><br>
  Subcommands, options, branches, packages and paths in a popup as you type.<br>
  For zsh, bash, and fish, in any terminal. Written in Rust.
</p>

```
❯ git ch
   ╭──────────────────────────────────────────────────────────╮
   │  ▸ checkout       Switch branches or restore working tree │
   │  ▸ cherry-pick    Apply the changes introduced by some …  │
   ╰──────────────────────────────────────────────────────────╯
```

## What is Lig?

Your shell already completes commands when you press `Tab`, but it shows a flat list
of words and only knows what each command's completion script tells it. Lig shows
a popup under the cursor while you type, like the autocomplete in a code editor: what
each subcommand and option does, values that come from your system (branches,
containers, packages), and the next thing that fits on the line.

It brings the experience of [Fig](https://fig.io) (now Kiro CLI autocomplete) and
[easy-complete](https://github.com/chen86860/easy-complete) to Linux, as a single
binary that works in any terminal: it draws the popup inside the terminal itself.

- **~730 commands** out of the box: git, docker, kubectl, cargo, npm, aws, gh, terraform,
  systemctl, apt… (the [withfig/autocomplete](https://github.com/withfig/autocomplete)
  specs, converted to JSON and embedded in the binary).
- **Live values**: git branches, docker containers, systemd units, ssh hosts, make
  targets, npm scripts, zoxide directories, PIDs…
- **Package search as you type**: npm, crates.io, Docker Hub, PyPI, Snap Store, apt.
- **Any other command** gets options and subcommands parsed from its `--help` or man page.
- Knows your **aliases, functions and builtins** (`gco ` completes like `git checkout `).
- Nerd Font icons, themes, configurable size and keys.
- Runs locally: no daemon, no account, no telemetry.

## Install

Requirements: Linux, [Rust](https://rustup.rs) 1.85+, zsh ≥ 5.3, bash ≥ 4.4, or fish ≥ 4.2.1.
Optional: `curl` (online package search), a [Nerd Font](https://www.nerdfonts.com) (icons).

```bash
git clone https://github.com/KevenGoncalves/lig lig
cd lig
cargo install --path .
lig install            # sets up your $SHELL; or: lig install zsh / lig install bash / lig install fish
```

No Rust? Download a prebuilt binary from [Releases](https://github.com/KevenGoncalves/lig/releases)
instead. The `x86_64-unknown-linux-musl` (or `aarch64-…-musl`) archive runs on any Linux:

```bash
tar xzf lig-*-x86_64-unknown-linux-musl.tar.gz
install -Dm755 lig-*/lig ~/.local/bin/lig
lig install
```

Open a new terminal, then check the setup:

```bash
lig doctor
```

`lig install` adds a pre block at the top of your shell config and a post block at
the bottom. It keeps the original in `<config>.lig-backup`. Fish uses
`${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish`.

Manual setup:

```bash
# first line of ~/.zshrc                 # last line of ~/.zshrc
eval "$(lig init zsh pre)"               eval "$(lig init zsh post)"

# first line of ~/.bashrc                # last line of ~/.bashrc
eval "$(lig init bash pre)"              eval "$(lig init bash post)"
```

```fish
# First line of config.fish
eval (lig init fish pre | string collect)
# Last line of config.fish
eval (lig init fish post | string collect)
```

- Remove it: `lig uninstall` (or `lig uninstall fish` / `lig uninstall bash`).
- Skip it for one shell: `LIG_DISABLE=1 zsh`.
- Update it: `lig update` pulls the latest changes into the folder you cloned and
  reinstalls it (`--source <dir>` for another checkout). Then open a new terminal tab:
  a tab keeps the version it started with.

## Usage

Type as usual and the popup appears with suggestions for what you're typing.

| Key | Action |
| --- | --- |
| `Tab` / `↓` | next suggestion |
| `Shift-Tab` / `↑` | previous suggestion (on the first one: close the popup) |
| `Enter` | accept the suggestion |
| `Esc` | close the popup |
| `Ctrl+Space` | open / close the popup on demand |

What to expect:

- **Command names don't pop up by themselves** (`ls`, `sudo ls`…), so simple commands
  run with a single `Enter`. Press `Ctrl+Space` to see command names, or set
  `complete_commands = true` under `[behavior]` for automatic suggestions.
- **`Enter`** completes the word you're typing (`go bui` → `go build`), or picks the
  first suggestion after a space (`git checkout ` → `git checkout main`). If the word is
  already complete (`git status`), it runs the command. `Enter` `Enter` means "pick,
  then run".
- **No space is added** after a pick, except after options (`--force `). Type the space
  yourself to see what comes next.
- **Folders** (`cd`, `mkdir`, …): picking one keeps the popup open with its subfolders.
  The first entries are **↵ Enter** (run the command) and **Back** (one folder up).
- **History and the shell's own completion stay yours**: while you browse history with
  the arrows or use `Tab` after closing the popup, lig stays out of the way. Typing
  brings it back.

## Configuration

Settings live in `~/.config/lig/config.toml`. `lig config` prints every option with its
default value. The ones you're most likely to change:

```toml
[ui]
theme = "default"          # catppuccin, nord, dracula, gruvbox, tokyonight, light, terminal
border = "rounded"         # square, none
density = "normal"         # compact, relaxed
icons = "auto"             # nerd, unicode, none
width = "auto"             # or columns (60), or "50%" of the terminal
height = 8                 # max rows, or "30%"
description_position = "right"   # or "bottom": a frame under the list

[ui.colors]                # override single theme colors: "#rrggbb", 0-255, or "default"
# bg = "#1e1e2e"

[keys]                     # any key: run `lig keys` to see a key's name
accept = ["enter"]
next = ["tab", "down", "ctrl-n"]
prev = ["shift-tab", "up", "ctrl-p"]
dismiss = ["esc"]
toggle = ["ctrl-space"]

[behavior]
enter_accepts = "auto"     # "after-space", "always", "never"
insert_space = false       # add a space after every pick
complete_commands = false  # pop up for command names too
online = true              # search package registries as you type
disabled_commands = []     # never show the popup for these
```

Keys are a key name with optional modifiers: `ctrl-n`, `alt-j`, `shift-up`, `ctrl-alt-x`,
`f5`, `pagedown`, `ctrl-space`. For anything else your terminal sends, use its escape
sequence (`"\\x1b[1;5C"`). Run `lig keys` and press a key to see its name; keys your terminal
keeps for itself (often Ctrl+Tab) never reach the shell.

Try styles without editing the file:

```bash
lig preview --all                                   # every theme
lig preview --theme nord --density compact --border square
lig preview --description bottom --width 60
lig icons                                           # every icon lig uses
```

Changes apply to new terminal tabs.

## Where suggestions come from

Looked up in this order:

1. **Your specs** in `~/.config/lig/specs/<command>.json` or `.toml`.
2. **Hand-written specs** in [`specs/lig/`](specs/lig) (git, docker, cargo, npm, systemctl,
   apt, ssh, kubectl, cd, z, …), merged with the converted one for the same command when
   there is one.
3. **Converted Fig specs** in [`specs/fig/`](specs/fig).
4. **The command's `--help` or man page**, parsed and cached per executable version.
   `--help` is only run for real binaries and Python/Node/Ruby scripts, with no input,
   no display and a 2 s timeout. Other files only get their man page read. Turn it off
   with `help_parsing = false`.

`lig specs list` shows which source each command uses.

**Online search** (`npm i`, `pnpm add`, `cargo add`, `docker pull`, `pip install`,
`snap install`) sends the package name you're typing to that registry, through `curl`,
with results cached for 30 minutes. Turn it off with `online = false`.

### Writing a spec

A spec describes a command's subcommands, options and arguments, in the
[Fig spec format](https://fig.io/docs/reference) (static parts only). Save it as
`~/.config/lig/specs/deploy.toml`:

```toml
name = "deploy"
description = "Deploy the app"

[[options]]
name = ["-e", "--env"]
description = "Target environment"
args = { name = "env", suggestions = ["staging", "production"] }

[[subcommands]]
name = "logs"
description = "Tail the logs of a service"
args = { name = "service", generators = { script = ["deploy", "list-services"] } }
```

Arguments can list fixed `suggestions`, use `template = "filepaths"` or `"folders"`, or
run a command whose output lines become suggestions (`generators = { script = [...] }`).
lig also has native generators (git branches, docker containers, …) usable with
`generators = [{ builtin = "git:branches" }]`. `lig specs generators` lists them.

To check a spec:

```bash
lig complete "deploy -e "
```

## How it works

lig runs your shell inside a pseudo-terminal (`lig shell`) and passes everything
through. It keeps a model of the screen, draws the popup into the terminal under the
cursor, and restores what was underneath when the popup closes. This is why it works
in any terminal and on Wayland, where apps can't place floating windows.

Small shell hooks tell lig what you're typing. zsh reports the line on every redraw.
For bash, lig reads the line from the screen after an invisible prompt marker.
Fish reports its edit buffer after input or redraws; there is no idle polling. To
accept a suggestion, lig sends a private key that a shell function binds to edit the
line. Slow sources (commands, network) run in the background, so the popup never
blocks typing.

## Troubleshooting

| Problem | Try |
| --- | --- |
| No popup at all | `lig doctor`. "running inside lig" must be ok; open a new tab after installing or updating. |
| Wrong or missing suggestions | `lig complete "<the line>"` shows what lig would suggest. |
| A key binding does nothing | `lig keys` shows whether the key reaches lig, and its name. |
| A command's suggestions look outdated | `lig specs clear-cache` (re-reads `--help` output). |
| Something else | Start a shell with `LIG_LOG=1 zsh` (or `bash` / `fish`) and check `~/.local/state/lig/lig.log`. |

## Limitations

- Fish reserves `SIGUSR1` for buffer reports and Ctrl+Alt+Shift+F12 for insertion.
- Fish `complete` definitions and alias bodies are not imported into lig.
- Starship has not been tested with the fish integration.
- In bash the line is read from the screen. A cursor inside a long wrapped line at the
  bottom of a full screen can occasionally confuse it, until the next key.
- Specs parsed from `--help` contain options and subcommands, but no dynamic values.
- About a quarter of the Fig specs' dynamic generators couldn't be converted (they
  relied on JavaScript logic); the options and subcommands around them are kept.

## Development

```bash
cargo test                      # unit tests, including every spec in specs/lig/
cargo build --release
python3 tests/fish_pty.py
python3 tests/shell_smoke.py
python3 tests/tmux_placement.py
python3 tests/trial_launcher.py
tools/update-fig-specs.sh       # regenerate specs/fig/ from withfig/autocomplete
```

The PTY tests require Python 3 and the relevant shells. The tmux tests use a
private server. All scripts test `target/release/lig`; set `LIG_TEST_BIN` to use
another binary.

```
src/            the lig binary (session.rs: PTY + popup, engine.rs: completion,
                popup.rs: drawing, help.rs: --help parser, specs/gens.rs: native generators)
shell/          zsh, bash, and fish hooks
specs/lig/      hand-written specs (JSON)
specs/fig/      converted withfig/autocomplete specs (generated)
build.rs        embeds both spec folders, compressed, into the binary
tools/figconv/  the Fig → JSON converter (development only, not part of lig)
assets/         the logo
```

To add a built-in spec, add `specs/lig/<command>.json` and rebuild. To add a new kind of
live value, write a generator in `src/specs/gens.rs` and register its id in `builtin()`.

## Credits and license

The specs in `specs/fig/` come from [withfig/autocomplete](https://github.com/withfig/autocomplete),
© Hercules Labs Inc. and contributors, MIT licensed (see [`specs/fig/LICENSE`](specs/fig/LICENSE)).

lig is MIT licensed, see [`LICENSE`](LICENSE).
