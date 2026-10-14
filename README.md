# lig

IDE-style autocomplete for your Linux terminal, written entirely in Rust. It works
like Fig / Kiro CLI autocomplete and [easy-complete](https://github.com/chen86860/easy-complete),
but runs on Linux, with no JavaScript, no npm and no downloads.

```
% git ch█
     ▸ checkout      Switch branches or restore working tree files
     ▸ cherry-pick   Apply the changes introduced by some existing commits
```

Everything runs locally. There is no daemon, no account and no telemetry.

## Where suggestions come from

1. **All ~730 commands from [withfig/autocomplete](https://github.com/withfig/autocomplete/tree/master/src)**
   (1,478 spec files, including git, docker, aws, gcloud, az, kubectl, gh, terraform,
   brew, npm, cargo, ffmpeg, heroku, deno, go…). They are converted to plain JSON in
   `specs/fig/`, compressed into the binary at build time, and inflated one at a time
   on first use. lig contains no JavaScript; see [Converted specs](#converted-specs).
2. **Hand-written specs in Rust** (`src/specs/`): `git`, `docker` (+ `compose`),
   `cargo`, `rustup`, `npm`, `npx`, `pnpm`, `yarn`, `bun`, `pip`, `systemctl`,
   `journalctl`, `apt`, `apt-get`, `kill`, `killall`, `pkill`, `ssh`, `scp`, `make`,
   `tmux`, `kubectl`, `sudo`, `cd`, `which`, `man`, `time`, `watch`, `xargs`, ….
   Their dynamic values are native Rust generators: git branches, tags, commits,
   stashes and changed files, docker containers, images and compose services, systemd
   units, PIDs and signals, ssh hosts, Makefile targets, `package.json` scripts, cargo
   bins/examples/features, apt packages, kube resources, and more. When a command also
   has a converted spec, the two are merged: the Rust spec wins, and the converted one
   adds the options and subcommands it's missing.
3. **Every other command**: lig parses its `--help` output or man page and caches the
   result per executable version. This covers GNU/uutils tools, clap, cobra, click and
   argparse CLIs, and anything with a man page. Subcommands are expanded lazily
   (`foo bar --help`).
4. **Your own specs** in `~/.config/lig/specs/` (JSON or TOML) override all of the above.

Your shell's **aliases and functions** are known too: the zsh hook reports them when
they change. Typing `gco ` completes like `git checkout `, and `ls -` gives `lsd`'s
options if `ls` is aliased to `lsd`. Both appear in the command-name suggestions.
zoxide's `z`/`zi` complete with zoxide's own frecent directory ranking (rupa/z's `~/.z`
also works).

**Package registries are searched as you type**, the way Fig did it:

| Command | Source |
| --- | --- |
| `npm i`, `pnpm add`/`i`, `yarn add`, `bun add`, `pnpm dlx` | npms.io prefix suggestions (fallback: npm registry search); `pkg@` → dist-tags and versions |
| `cargo add`, `cargo install` | crates.io search; `crate@` → versions |
| `docker pull`, `docker run` | your local images, then Docker Hub; `image:` → tags |
| `pip install`, `pip download` | the 15,000 most downloaded PyPI packages ([top-pypi-packages](https://hugovk.github.io/top-pypi-packages/), cached for a week; offline: Fig's built-in list) since PyPI has no search API; `pkg==` → versions from PyPI |
| `snap install`, `snap find`, `snap info` | the Snap Store (`snap find`) |
| `apt install` / `apt-get install` | the local apt index (`apt-cache pkgnames`) |
| `brew install` | Homebrew's own formula and cask lists (`brew formulae` / `brew casks`) |

Requests go through `curl` and are cached for 30 minutes. While a new search runs, the
previous results stay visible, filtered to what you've typed. If there's nothing to show
yet, the popup says what it's waiting for (`Searching npm…`, `Searching crates.io…`,
`Reading foo --help…`). This only appears when results take more than about 150 ms, so
fast local suggestions never flash it. Turn this off with
`online = false`. With it on, the package name you're typing is sent to these
services.

Order in the popup: subcommands and values first (branches, scripts, units…), then
options, then files. When an argument is just a file path, the options are listed too,
so `java ` offers `-jar`, `-cp`… before files. Type a letter and only matching files
remain. Set `files_first = true` to put files first.

`lig specs list` shows every command with a spec and where it comes from.

## How it works

Wayland (GNOME) doesn't let an app put a floating window at the terminal cursor, so
lig draws the popup **inside the terminal**, the way an editor draws a completion menu:

```
Ghostty ⇄ lig shell (PTY proxy + vt100 screen model + popup) ⇄ zsh (+ ZLE hooks)
                         │
                         └─ worker thread: completion engine (specs, --help parser, generators)
```

* `lig shell` runs zsh in a pseudo-terminal and passes everything through. It keeps a
  vt100 model of the screen, so after it draws the popup it can restore the exact
  cells that were underneath. Redraws use synchronized output (`?2026`) to avoid flicker.
* The zsh integration reports the edit buffer and cursor on every redraw
  (`zle-line-pre-redraw` → a private `OSC 6973`). lig removes these messages from the
  stream before they reach the terminal.
* When you accept a suggestion, lig sends a private key sequence bound to a ZLE widget.
  The widget edits `$LBUFFER` directly, so the insert is exact in emacs and vi modes.
* Anything that runs a command (generators, `--help`) is done in a second pass.
  Static suggestions appear immediately, and the popup updates when those results arrive.

### Is running `--help` safe?

lig only runs `<cmd> --help` automatically when `<cmd>` is an ELF binary or a
python/node/ruby script. Their argument parsers handle `--help` before doing anything.
Shell scripts and other files only get their man page read.
`--help` runs with stdin closed, an invalid `DISPLAY`/`WAYLAND_DISPLAY` (so GUI apps
can't open windows), and a 2 s timeout. It runs once per executable version.
To opt a command out, add it to `help_skip`, or turn the feature off with
`help_parsing = false`.

## Install

Requirements: Rust 1.85+ and zsh ≥ 5.3.

```bash
cargo install --path .
```

```bash
lig install
```

`lig install` adds a small block at the top and at the bottom of `~/.zshrc`, and saves
the original as `~/.zshrc.lig-backup`. Then open a new terminal. To check the setup,
run `lig doctor`.

If you'd rather edit `.zshrc` yourself, put the first line at the very top and the second at the very bottom:

```zsh
eval "$(lig init zsh pre)"    # top: re-execs the shell inside lig
eval "$(lig init zsh post)"   # bottom: ZLE hooks
```

### Ghostty

Nothing extra is needed. Ghostty only injects its shell integration into the first
shell it starts, and lig re-sources it in the shell it wraps, so prompt marks, cursor
shape and titles keep working.

### Disable temporarily

```bash
LIG_DISABLE=1 zsh
```

## Look

The popup is a rounded box by default. When the list scrolls, the
scrollbar runs along its right edge and the bottom edge shows the position (`3/27`).
Change it in `[ui]`:

* `border`: `rounded`, `square` or `none`
* `density`:
  * `compact`: tight padding, narrow descriptions, most rows on screen
  * `normal`: the default
  * `relaxed`: wide padding and gaps, and a blank row above and below the list
* `theme`: `default`, `catppuccin`, `nord`, `dracula`, `gruvbox`, `tokyonight`,
  `light`, or `terminal` (follows your terminal's color scheme)
* `[ui.colors]`: override any single color of the theme
* `width`: `"auto"` fits the content (between `min_width` and `max_width`), or set a
  number of columns (`60`) or a share of the terminal (`"50%"`). With a fixed width the
  description column takes the extra space. When space is short the description
  shrinks first and is dropped before the name gets cut.
* `description_position`: `"right"` shows a description column next to each name.
  `"bottom"` lists only the names and shows the **selected** item's full description,
  wrapped, in a separate frame under the list. The frame is up to `description_lines`
  tall and keeps its size while you move through the list.
* `height`: the most suggestions listed at once (`8`), or a share of the terminal
  height (`"30%"`). The list scrolls beyond that. `min_height` keeps the popup at least
  that tall even with fewer suggestions.

Try them without editing anything:

```bash
lig preview                          # your current settings
lig preview --theme nord --density compact --border square
lig preview --all                    # every theme
lig preview --densities              # compact / normal / relaxed side by side
lig preview --width 60 --height 5    # try sizes ("50%" works too)
lig preview --description bottom     # description frame under the list
```

## Icons

With `icons = "auto"` (the default), lig uses [Nerd Font](https://www.nerdfonts.com)
glyphs when they're available. Ghostty and WezTerm bundle them, and otherwise any
installed Nerd Font is detected with `fc-list`. Without Nerd Fonts it falls back to plain
Unicode symbols. The icon depends on what the suggestion is:

* commands, aliases, functions, builtins, subcommands, flags
* folders, and files by type (Rust, Python, JS/TS, JSON, Markdown, configs, shell,
  images, archives, lockfiles…)
* packages by ecosystem (npm, crates.io, PyPI, Docker, apt/snap/brew)
* git branches, tags, commits, remotes and stashes
* containers, processes, services, hosts, users, versions, scripts and make targets

Run `lig icons` to see every icon lig uses, with its name and Unicode fallback.

Spec files can set one with `"icon"`: `package`, `npm`, `rust`, `python`, `docker`,
`container`, `git-branch`, `git-tag`, `git-commit`, `git-remote`, `git-stash`,
`process`, `service`, `host`, `user`, `version`, `script`, `folder`, `volume`,
`network`, `cloud`, `terminal`, `alias`, `function` or `builtin`.

## Keys

| Key | Action |
| --- | --- |
| `Ctrl+Space` | show the popup now / hide it |
| `Tab` / `↓` / `Ctrl-N` | next suggestion (with a single suggestion, `Tab` accepts it) |
| `Shift-Tab` / `↑` / `Ctrl-P` | previous suggestion |
| `Enter` | accept the selected suggestion |
| `Esc` | dismiss (until the line changes) |

The popup doesn't open by itself while you type a command name, either the first word
or the command after `sudo`, `time`, `env`, `watch`, `xargs`, …. So `ls`, `htop`,
`exit` or `sudo ls` can be typed and run with one `Enter`. Press `Ctrl+Space` to see command names, aliases,
functions and builtins. (Set `complete_commands = true` to get them automatically.)

`Enter` still runs the line, unless you moved the selection, when nothing is typed for
the current word (`ls ` + `Enter`) or when the word is already complete (`git log` +
`Enter`). Set `enter_runs_on_empty_word = false` to always accept.

When the popup isn't showing, keys go straight to zsh, so normal Tab completion still
works. `Ctrl+Space` is taken from zsh's `set-mark-command` while you edit a command line.

## Configuration

`~/.config/lig/config.toml`. Print the defaults with `lig config`:

```toml
[ui]
width = "auto"           # "auto" (fit content), columns (60), or "50%" of the terminal
min_width = 30           # bounds for width = "auto"
max_width = 100          #   (columns or "80%")
height = 8               # suggestion rows at most, or "30%" of the terminal height
min_height = 0           # keep at least this many rows (padded) for a steadier popup
descriptions = true
description_position = "right"   # or "bottom": the selected item's description in a frame below
description_lines = 3            # height of that frame (at most)
icons = "auto"           # "nerd", "unicode" or "none"
border = "rounded"       # "square" or "none"
density = "normal"       # "compact", "normal" or "relaxed"
theme = "default"        # catppuccin, nord, dracula, gruvbox, tokyonight, light, terminal

[ui.colors]              # optional: override any theme color
# bg = "#1e1e2e"         # 0-255 palette index, "#rrggbb", or "default" (terminal's own)
# fg, selected_bg, selected_fg, description_fg, selected_description_fg,
# match_fg, border_fg, border_bg, scrollbar_fg, scrollbar_thumb, info_fg

[keys]
accept = ["enter"]
next = ["tab", "down", "ctrl-n"]
prev = ["shift-tab", "up", "ctrl-p"]
dismiss = ["esc"]
toggle = ["ctrl-space"]

[behavior]
fuzzy = true                 # subsequence matching when nothing matches by prefix
enter_runs_on_empty_word = true   # `ls ` + Enter runs instead of accepting
files_first = false          # list files before options/subcommands
show_on_empty_token = true   # suggest right after a space
complete_commands = false    # pop up for the first word too (Ctrl+Space always does)
online = true                # search package registries (npm, crates.io, Docker Hub, …)
generator_timeout_ms = 5000
generator_cache_ms = 5000
disabled_commands = []
help_parsing = true          # specs from --help / man for other commands
help_timeout_ms = 2000
help_skip = []               # never run `<cmd> --help` for these
```

## Writing specs

### As a file (no rebuild)

Put `~/.config/lig/specs/<command>.toml` (or `.json`) there:

```toml
name = "deploy"
description = "Deploy the app"

[[options]]
name = ["-e", "--env"]
description = "Target environment"
args = { name = "env", suggestions = ["staging", "production"] }

[[subcommands]]
name = ["status", "st"]
description = "Show deployment status"

[[subcommands]]
name = "logs"
description = "Tail logs of a service"
args = { name = "service", generators = { script = ["deploy", "services"] } }
```

Supported fields: `name` (string or list), `description`, `subcommands`, `options`,
`args`, `insertValue` (with an optional `{cursor}`), `priority`, `hidden`,
`isPersistent`, `isRepeatable`, `requiresSeparator`, `exclusiveOn`, `isOptional`,
`isVariadic`, `isCommand`, `suggestions`, `template` (`"filepaths"` / `"folders"`),
`generators` (`script` as an argv list or a shell string, plus `splitOn`), and
`loadSpec`.

### In Rust (built in)

Add a module under `src/specs/` and register it in `src/specs/mod.rs`:

```rust
use crate::spec::{Subcommand, arg, cmd, gen_fn, opt, sug};

pub fn spec() -> Subcommand {
    cmd("deploy", "Deploy the app")
        .opt(opt("-e, --env", "Target environment").arg(arg("env").suggest(&["staging", "production"])))
        .sub(cmd("logs", "Tail logs").arg(arg("service").generator(gen_fn("deploy:services", |cx| {
            cx.lines(&["deploy", "services"]).into_iter().map(|s| sug(s).desc("service")).collect()
        }))))
}
```

### Debugging

```bash
lig complete "git checkout "     # what the engine suggests
lig complete --alias "gco=git checkout" "gco "
lig specs clear-cache            # forget cached --help results
LIG_LOG=1 zsh                    # log errors to ~/.local/state/lig/lig.log
```

## Converted specs

`specs/fig/` is generated from the withfig/autocomplete TypeScript sources by
`tools/update-fig-specs.sh`:

```bash
tools/update-fig-specs.sh            # or: tools/update-fig-specs.sh <git ref>
```

The script compiles the sources with esbuild, then runs `tools/figconv`, a Rust dev
tool that isn't part of lig. figconv evaluates each spec once, in an embedded QuickJS,
and writes static JSON. Fig specs contain JavaScript functions, and lig doesn't run
JavaScript, so each generator is translated by probing what it does:

| Generator in the Fig spec | In lig |
| --- | --- |
| runs a command lig has a native generator for (git branches, docker ps, systemctl units, …) | `{"builtin": "git:branches"}` (Rust) |
| `postProcess` that turns lines into suggestions | `{"script": […]}` |
| `postProcess` that picks a column, optionally skipping a header | `{"script": […], "field": 0, "skipLines": 1}` |
| `postProcess` / `custom` that reads JSON output (learned by running it against a recording proxy instead of `JSON.parse`) | `{"script": […], "json": {"name": "…", "description": "…"}}` |
| file listings | `{"template": "filepaths"}` |
| a fixed list | `{"suggestions": […]}` |

At revision `aef52ac` this converts 3,908 of 5,094 generators. The other 1,186 depend
on token-specific logic, and only those values are lost: the options and subcommands
around them are kept. Descriptions are shortened to their first line.
The specs are © Hercules Labs Inc. and the withfig contributors, MIT licensed; see
`specs/fig/LICENSE`.

## Limitations

* zsh only for now. The PTY side doesn't depend on the shell, so bash or fish only need
  their own hook scripts.
* The popup is drawn into the terminal grid. It hides inside full-screen apps
  (alternate screen) and while a command is running.
* Specs parsed from `--help` contain options and subcommands, but they can't know about
  dynamic values; arguments fall back to file paths.
* Some converted generators could not be translated (see above).

## Layout

```
src/
  main.rs      CLI (shell, init, install, specs, complete, doctor, config)
  session.rs   PTY proxy, event loop, key handling
  popup.rs     popup layout/drawing and screen restore
  osc.rs       private OSC message filter
  keys.rs      input key parsing and bindings
  parser.rs    shell command-line tokenizer
  engine.rs    spec walker, generators, ranking, insertion text
  spec.rs      spec model + builder DSL + JSON/TOML loading
  specs/       hand-written specs (git, docker, cargo, node, system), generators,
               fig.rs (reader for the embedded converted specs)
  help.rs      --help / man page parser and cache
  exec.rs      running commands with timeouts
  store.rs     user spec files
  config.rs    configuration
shell/         zsh integration (pre/post)
specs/fig/     converted withfig/autocomplete specs (generated, JSON)
build.rs       packs specs/fig into a compressed archive embedded in the binary
tools/         figconv (spec converter) and update-fig-specs.sh
```
