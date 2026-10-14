# lig

IDE-style autocomplete for your Linux terminal, in Rust. It works like Fig / Kiro CLI
autocomplete and [easy-complete](https://github.com/chen86860/easy-complete), and it
uses the same 700+ open-source [withfig completion specs](https://github.com/withfig/autocomplete)
(git, docker, cargo, npm, kubectl, aws, systemctl, apt, …).

```
% git ch█
     ▸ checkout      Switch branches or restore working tree files
     ▸ cherry-pick   Apply the changes introduced by some existing commits
```

Everything runs locally. There is no daemon, no account and no telemetry.

## How it works

Wayland (GNOME) doesn't let an app put a floating window at the terminal cursor, so
lig draws the popup **inside the terminal**, the way an editor draws a completion menu:

```
Ghostty ⇄ lig shell (PTY proxy + vt100 screen model + popup) ⇄ zsh (+ ZLE hooks)
                         │
                         └─ worker thread: spec engine + embedded QuickJS
```

* `lig shell` runs zsh in a pseudo-terminal and passes everything through. It keeps a
  vt100 model of the screen, so after it draws the popup it can restore the exact
  cells that were underneath. Redraws use synchronized output (`?2026`) to avoid flicker.
* The zsh integration reports the edit buffer and cursor on every redraw
  (`zle-line-pre-redraw` → a private `OSC 6973`). lig removes these messages from the
  stream before they reach the terminal.
* When you accept a suggestion, lig sends a private key sequence bound to a ZLE widget.
  The widget edits `$LBUFFER` directly, so the insert is exact in emacs and vi modes.
* Specs are the compiled JS modules from the `@withfig/autocomplete` npm package. They
  run in an embedded QuickJS, so dynamic parts work as well: generators (`git branch`,
  `docker ps`, `kill` PIDs, …), `postProcess`, `custom` async generators,
  `loadSpec`, `generateSpec` and versioned specs.
  Generators run in your shell's cwd with your shell's `PATH`, and results are cached.

## Install

Requirements: Rust (1.85+), a C compiler (QuickJS is compiled by `cc`), zsh ≥ 5.3.

```bash
cargo install --path .
```

```bash
lig install
```

`lig install`:

* adds a small block at the top and at the bottom of `~/.zshrc`, and saves the original
  as `~/.zshrc.lig-backup`;
* downloads the specs from npm into `~/.local/share/lig/specs`.

Then open a new terminal. To check the setup, run `lig doctor`.

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

## Keys (while the popup is visible)

| Key | Action |
| --- | --- |
| `↓` / `Ctrl-N` | next |
| `↑` / `Ctrl-P` / `Shift-Tab` | previous |
| `Tab` | accept |
| `Esc` | dismiss (until the line changes) |
| `Enter` | run the line as usual (set `enter_accepts = true` to accept instead) |

When the popup isn't showing, every key goes straight to zsh, so normal Tab completion still works.

## Configuration

`~/.config/lig/config.toml`. Print the defaults with `lig config`:

```toml
[ui]
max_rows = 8
max_width = 90
descriptions = true
icons = "unicode"        # "nerd" (Ghostty ships Nerd Font symbols), or "none"
bg = "236"               # 0-255 palette index or "#rrggbb"
fg = "252"
selected_bg = "25"
selected_fg = "231"
description_fg = "244"
match_fg = "117"
scrollbar_fg = "240"

[keys]
accept = ["tab"]
next = ["down", "ctrl-n"]
prev = ["up", "ctrl-p", "shift-tab"]
dismiss = ["esc"]

[behavior]
fuzzy = true                 # subsequence matching when nothing matches by prefix
enter_accepts = false
show_on_empty_token = true   # suggest right after a space
complete_commands = true     # suggest command names for the first word
generator_timeout_ms = 3000
generator_cache_ms = 5000
disabled_commands = []
```

## Specs

```bash
lig specs update            # latest @withfig/autocomplete from npm
lig specs update --version 2.692.3
lig specs list
lig complete "git checkout "   # see what the engine suggests (debugging)
```

Your own specs go in `~/.config/lig/specs/` and take precedence over the downloaded ones.
Two formats work:

* `name.json`: a static Fig spec, for example
  `{"name": "deploy", "subcommands": [{"name": "staging"}, {"name": "prod"}]}`
* `name.js`: a spec compiled to an ES module, as built by `@withfig/autocomplete-tools`.
  It needs the `export{… as default}` form.

## Debugging

Set `LIG_LOG=1` before starting the shell. Spec and generator errors go to
`~/.local/state/lig/lig.log`.

## Limitations

* zsh only for now. The PTY side doesn't depend on the shell, so bash or fish only need
  their own hook scripts.
* The popup is drawn into the terminal grid. It hides inside full-screen apps
  (alternate screen) and while a command is running.
* Generators execute the commands listed in the specs (e.g. `git branch`), as Fig does.

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
  spec.rs      Fig spec model
  js.rs        QuickJS host for compiled specs
  exec.rs      running generator commands with timeouts
  store.rs     spec lookup and npm download
  config.rs    configuration
shell/         zsh integration (pre/post)
```
