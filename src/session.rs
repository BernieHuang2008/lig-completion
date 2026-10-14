//! `lig shell`: runs the shell inside a PTY, mirrors its screen with vt100,
//! listens to the zsh integration and draws the suggestion popup.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use nix::sys::termios::{SetArg, Termios, cfmakeraw, tcgetattr, tcsetattr};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use unicode_width::UnicodeWidthStr;

use crate::config::Config;
use crate::engine::{Engine, Kind, ShellEnv, Suggestion};
use crate::keys::{self, Action, Bindings, Key};
use crate::osc::{Msg, OscFilter};
use crate::popup::{self, Rect};
use crate::store::Store;

/// Wait for output to settle this long before drawing.
const DEBOUNCE: Duration = Duration::from_millis(4);
/// …but never longer than this.
const MAX_DELAY: Duration = Duration::from_millis(40);
/// How long Enter waits for suggestions that are still loading.
const ENTER_WAIT: Duration = Duration::from_millis(1500);
/// Show "Searching…" only when results take longer than this.
const LOADING_DELAY: Duration = Duration::from_millis(150);
/// Key sequence bound to the `_lig_insert` ZLE widget.
const INSERT_KEY: &str = "\x1b[9147~";

enum Event {
    Input(Vec<u8>),
    Output(Vec<u8>),
    Exit,
    Resize,
    Done { line: String, items: Vec<Suggestion>, loading: Option<String> },
}

struct Request {
    line: String,
    env: ShellEnv,
    /// Asked for with the toggle key.
    manual: bool,
}

fn winsize(fd: i32) -> Option<(u16, u16)> {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0;
    (ok && ws.ws_row > 0 && ws.ws_col > 0).then_some((ws.ws_row, ws.ws_col))
}

/// Replace this process with the plain shell (used when lig can't run).
fn exec_shell(shell: &Path, login: bool) -> ! {
    let mut c = std::process::Command::new(shell);
    if login {
        c.arg("-l");
    }
    let err = c.env("LIG_DISABLE", "1").exec();
    eprintln!("lig: cannot run {}: {err}", shell.display());
    std::process::exit(127);
}

struct RawMode(Termios);

impl RawMode {
    fn enable() -> Result<RawMode> {
        let orig = tcgetattr(std::io::stdin())?;
        let mut raw = orig.clone();
        cfmakeraw(&mut raw);
        tcsetattr(std::io::stdin(), SetArg::TCSANOW, &raw)?;
        Ok(RawMode(orig))
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = tcsetattr(std::io::stdin(), SetArg::TCSANOW, &self.0);
    }
}

pub fn run(shell: PathBuf, login: bool) -> Result<i32> {
    if unsafe { libc::isatty(0) == 0 || libc::isatty(1) == 0 } {
        exec_shell(&shell, login);
    }
    let (rows, cols) = winsize(0).unwrap_or((24, 80));
    let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
    let pair = match native_pty_system().openpty(size) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("lig: openpty failed: {e}");
            exec_shell(&shell, login);
        }
    };
    let mut cmd = CommandBuilder::new(&shell);
    if login {
        cmd.arg("-l");
    }
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd);
    }
    if let Some(tty) = pair.master.tty_name() {
        cmd.env("LIG_TTY", tty);
    }
    cmd.env("LIG_PID", std::process::id().to_string());
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lig: spawning {} failed: {e}", shell.display());
            exec_shell(&shell, login);
        }
    };
    drop(pair.slave);
    let shell_pid = child.process_id();

    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let raw = RawMode::enable()?;
    let cfg = Config::load();

    let (tx, rx) = unbounded::<Event>();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
                if n <= 0 {
                    if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    let _ = tx.send(Event::Exit);
                    break;
                }
                if tx.send(Event::Input(buf[..n as usize].to_vec())).is_err() {
                    break;
                }
            }
        });
    }
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = tx.send(Event::Exit);
                        break;
                    }
                    Ok(n) => {
                        if tx.send(Event::Output(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
    {
        let tx = tx.clone();
        let mut signals = signal_hook::iterator::Signals::new([libc::SIGWINCH])?;
        std::thread::spawn(move || {
            for _ in signals.forever() {
                if tx.send(Event::Resize).is_err() {
                    break;
                }
            }
        });
    }
    let (req_tx, req_rx) = unbounded::<Request>();
    {
        let tx = tx.clone();
        let behavior = cfg.behavior.clone();
        std::thread::spawn(move || worker(req_rx, tx, behavior));
    }

    let mut s = Session {
        cwd_fallback: std::env::current_dir().unwrap_or_else(|_| "/".into()),
        bindings: Bindings::new(&cfg.keys),
        cfg,
        out: std::io::stdout(),
        pty: writer,
        master: pair.master,
        parser: vt100::Parser::new(rows, cols, 0),
        osc: OscFilter::default(),
        editing: false,
        hidden: false,
        line: String::new(),
        env: ShellEnv::default(),
        shell_pid,
        req_tx,
        items: vec![],
        items_line: String::new(),
        selected: 0,
        navigated: false,
        just_accepted: false,
        manual: false,
        loading: None,
        pending_enter: None,
        deferred: vec![],
        scroll: 0,
        dismissed: None,
        drawn: None,
        sync_open: false,
        dirty: false,
        dirty_since: Instant::now(),
        last_output: Instant::now(),
    };
    s.event_loop(&rx);

    s.clear_popup();
    drop(raw);
    let code = child.wait().map(|st| st.exit_code() as i32).unwrap_or(0);
    Ok(code)
}

/// Whether Enter picks the selected suggestion (instead of running the line).
///
/// `"auto"`: pick, unless the word typed is already complete (equals a
/// suggestion: `git status`); `"after-space"`: only when the line ends in a
/// space; `"always"`; `"never"`. A selection moved with Tab/arrows is always
/// picked, and the Enter right after a pick runs the line.
fn enter_accepts(mode: &str, word_empty: bool, complete: bool, navigated: bool, just_accepted: bool) -> bool {
    match mode {
        "never" => false,
        _ if navigated => true,
        _ if just_accepted => false,
        "always" => true,
        "after-space" => word_empty,
        _ => word_empty || !complete,
    }
}

fn worker(rx: Receiver<Request>, tx: Sender<Event>, behavior: crate::config::Behavior) {
    let mut engine = Engine::new(Store::default(), behavior);
    while let Ok(mut req) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            req = newer;
        }
        let c = engine.complete(&req.line, &req.env, false, req.manual);
        let _ = tx.send(Event::Done { line: req.line.clone(), items: c.items, loading: c.loading });
        if c.incomplete && rx.is_empty() {
            let c = engine.complete(&req.line, &req.env, true, req.manual);
            let _ = tx.send(Event::Done { line: req.line, items: c.items, loading: None });
        }
    }
}

struct Session {
    cfg: Config,
    bindings: Bindings,
    out: std::io::Stdout,
    pty: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    parser: vt100::Parser,
    osc: OscFilter,

    // Shell state reported by the integration.
    editing: bool,
    hidden: bool,
    /// Edit buffer left of the cursor.
    line: String,
    env: ShellEnv,
    shell_pid: Option<u32>,
    cwd_fallback: PathBuf,
    req_tx: Sender<Request>,

    // Popup state.
    items: Vec<Suggestion>,
    items_line: String,
    selected: usize,
    /// The user moved the selection on the current line.
    navigated: bool,
    /// A suggestion was just accepted and nothing typed since: Enter runs.
    just_accepted: bool,
    /// The popup was asked for with the toggle key (until the line is run).
    manual: bool,
    /// (line, message, since) while slower suggestions are being fetched.
    loading: Option<(String, String, Instant)>,
    /// Enter pressed while loading: when, and the input from it on.
    pending_enter: Option<Instant>,
    deferred: Vec<u8>,
    scroll: usize,
    dismissed: Option<String>,
    drawn: Option<Rect>,
    sync_open: bool,
    dirty: bool,
    dirty_since: Instant,
    last_output: Instant,
}

impl Session {
    fn event_loop(&mut self, rx: &Receiver<Event>) {
        loop {
            let mut deadline = self.dirty.then(|| (self.last_output + DEBOUNCE).min(self.dirty_since + MAX_DELAY));
            if let Some(w) = self.loading_wake() {
                deadline = Some(deadline.map_or(w, |d| d.min(w)));
            }
            if let Some(t) = self.pending_enter {
                let w = t + ENTER_WAIT;
                deadline = Some(deadline.map_or(w, |d| d.min(w)));
            }
            let ev = if let Some(due) = deadline {
                match rx.recv_deadline(due) {
                    Ok(ev) => ev,
                    Err(RecvTimeoutError::Timeout) => {
                        if self.pending_enter.is_some_and(|t| t.elapsed() >= ENTER_WAIT) {
                            self.resolve_pending_enter();
                        }
                        self.render();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match rx.recv() {
                    Ok(ev) => ev,
                    Err(_) => return,
                }
            };
            match ev {
                Event::Output(data) => self.on_output(&data),
                Event::Input(data) => self.on_input(&data),
                Event::Done { line, items, loading } => self.on_done(line, items, loading),
                Event::Resize => self.on_resize(),
                Event::Exit => return,
            }
        }
    }

    fn write_out(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut out = self.out.lock();
        let _ = out.write_all(bytes);
        let _ = out.flush();
    }

    fn write_pty(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let _ = self.pty.write_all(bytes);
        let _ = self.pty.flush();
    }

    fn mark_dirty(&mut self) {
        if !self.dirty {
            self.dirty = true;
            self.dirty_since = Instant::now();
        }
    }

    fn begin_sync(&mut self, w: &mut Vec<u8>) {
        if !self.sync_open {
            // Synchronized output: the terminal shows erase+redraw atomically.
            w.extend_from_slice(b"\x1b[?2026h\x1b[?25l");
            self.sync_open = true;
        }
    }

    fn end_sync(&mut self, w: &mut Vec<u8>) {
        if self.sync_open {
            w.extend_from_slice(b"\x1b[?2026l");
            self.sync_open = false;
        }
    }

    fn erase(&mut self, w: &mut Vec<u8>) {
        if let Some(r) = self.drawn.take() {
            self.begin_sync(w);
            popup::erase(w, self.parser.screen(), r);
            popup::restore_cursor(w, self.parser.screen());
        }
    }

    fn clear_popup(&mut self) {
        let mut w = vec![];
        self.erase(&mut w);
        self.end_sync(&mut w);
        self.write_out(&w);
    }

    fn visible(&self) -> bool {
        self.editing
            && !self.hidden
            && !self.items.is_empty()
            && self.items_line == self.line
            && self.dismissed.as_deref() != Some(self.line.as_str())
            && !self.parser.screen().alternate_screen()
    }

    // ------------------------------------------------------------ events

    fn on_output(&mut self, data: &[u8]) {
        let mut fwd = Vec::with_capacity(data.len());
        let mut msgs = vec![];
        self.osc.feed(data, &mut fwd, &mut msgs);
        let mut w = vec![];
        if !fwd.is_empty() {
            // Take the popup off the screen before the shell draws, so the
            // screen is exactly what the shell (and our vt100 model) expects.
            self.erase(&mut w);
            w.extend_from_slice(&fwd);
            self.parser.process(&fwd);
        }
        self.write_out(&w);
        for m in msgs {
            self.on_msg(m);
        }
        self.last_output = Instant::now();
        self.mark_dirty();
    }

    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Buffer { cursor, text } => {
                self.editing = true;
                self.hidden = false;
                let line: String = text.chars().take(cursor).collect();
                if line != self.line || self.items_line != line {
                    if self.dismissed.as_deref() != Some(line.as_str()) {
                        self.dismissed = None;
                    }
                    self.line = line;
                    self.request();
                }
            }
            Msg::Hide => self.hidden = true,
            Msg::Finish => {
                self.editing = false;
                self.just_accepted = false;
                self.manual = false;
                self.loading = None;
                if self.pending_enter.take().is_some() {
                    let data = std::mem::take(&mut self.deferred);
                    self.write_pty(&data);
                }
                self.items.clear();
                self.items_line.clear();
                self.line.clear();
                self.dismissed = None;
            }
            Msg::Path(p) => self.env.path = Some(p),
            Msg::Cwd(c) => self.env.cwd = PathBuf::from(c),
            Msg::Aliases(a) => self.env.aliases = std::sync::Arc::new(a.into_iter().collect()),
            Msg::Functions(f) => self.env.functions = std::sync::Arc::new(f),
            Msg::Builtins(b) => self.env.builtins = std::sync::Arc::new(b),
        }
        self.mark_dirty();
    }

    /// The "Searching npm…" message, once loading has taken a moment.
    fn loading_message(&self) -> Option<&str> {
        let (line, label, since) = self.loading.as_ref()?;
        let show = *line == self.line
            && since.elapsed() >= LOADING_DELAY
            && self.editing
            && !self.hidden
            && !self.visible()
            && self.dismissed.as_deref() != Some(self.line.as_str())
            && !self.parser.screen().alternate_screen();
        show.then_some(label.as_str())
    }

    /// When to wake up to show the loading message.
    fn loading_wake(&self) -> Option<Instant> {
        let (line, _, since) = self.loading.as_ref()?;
        let at = *since + LOADING_DELAY;
        (*line == self.line && Instant::now() < at).then_some(at)
    }

    fn request(&mut self) {
        let mut env = self.env.clone();
        if env.cwd.as_os_str().is_empty() {
            env.cwd = self
                .shell_pid
                .and_then(|p| std::fs::read_link(format!("/proc/{p}/cwd")).ok())
                .unwrap_or_else(|| self.cwd_fallback.clone());
        }
        let _ = self.req_tx.send(Request { line: self.line.clone(), env, manual: self.manual });
        // Until the answer arrives this line counts as loading (the worker may
        // still be busy with an earlier, slower request). The first word only
        // pops up on demand, so it doesn't get a message.
        let first_word = !self.line.trim_start().contains(char::is_whitespace);
        if !first_word || self.manual {
            if !self.loading.as_ref().is_some_and(|(l, _, _)| *l == self.line) {
                self.loading = Some((self.line.clone(), "Loading suggestions".into(), Instant::now()));
            }
        } else {
            self.loading = None;
        }
    }

    /// Nothing typed for the current word: the line ends in a space.
    fn word_empty(&self) -> bool {
        self.line.is_empty() || self.line.ends_with(char::is_whitespace)
    }

    /// The word being typed already equals one of the suggestions (`git status`).
    fn word_complete(&self) -> bool {
        let chars: Vec<char> = self.line.chars().collect();
        self.items.iter().any(|s| {
            let typed: String = chars[chars.len().saturating_sub(s.delete)..].iter().collect();
            !typed.is_empty() && s.insert.trim_end().trim_end_matches('/') == typed.trim_end_matches('/')
        })
    }

    fn enter_picks(&self) -> bool {
        enter_accepts(
            &self.cfg.behavior.enter_accepts,
            self.word_empty(),
            self.word_complete(),
            self.navigated,
            self.just_accepted,
        )
    }

    /// Replay a deferred Enter (and the keys after it) now that the
    /// suggestions are final, or loading took too long.
    fn resolve_pending_enter(&mut self) {
        if self.pending_enter.take().is_none() {
            return;
        }
        self.loading = None;
        let data = std::mem::take(&mut self.deferred);
        self.on_input(&data);
    }

    fn on_done(&mut self, line: String, items: Vec<Suggestion>, loading: Option<String>) {
        if line != self.line {
            return; // stale
        }
        let fin = loading.is_none();
        self.apply_done(line, items, loading);
        if fin && self.pending_enter.is_some() {
            self.render();
            self.resolve_pending_enter();
        }
    }

    fn apply_done(&mut self, line: String, items: Vec<Suggestion>, loading: Option<String>) {
        self.loading = match (loading, self.loading.take()) {
            // Still loading the same line: keep the original start time.
            (Some(label), Some((l, _, since))) if l == line => Some((line.clone(), label, since)),
            (Some(label), _) => Some((line.clone(), label, Instant::now())),
            (None, _) => None,
        };
        let same_line = self.items_line == line;
        let keep = self.items.get(self.selected).map(|s| s.label.clone());
        self.items = items;
        self.items_line = line;
        // When slower results arrive for the same line, keep the selection
        // only if the user moved it; otherwise start from the top again.
        self.selected = if same_line && self.navigated {
            keep.and_then(|l| self.items.iter().position(|s| s.label == l)).unwrap_or(0)
        } else {
            0
        };
        if !same_line {
            self.navigated = false;
        }
        if self.selected == 0 {
            self.scroll = 0;
        }
        self.mark_dirty();
    }

    fn on_input(&mut self, data: &[u8]) {
        // Keys typed while a deferred Enter waits go after it, in order.
        if self.pending_enter.is_some() {
            self.deferred.extend_from_slice(data);
            return;
        }
        let mut fwd: Vec<u8> = vec![];
        let mut redraw = false;
        let keys = keys::split(data);
        // Once a key in this chunk has gone to the shell, the popup state is
        // stale until the shell redraws: the rest of the chunk goes through as is
        // (pastes, fast macros).
        let mut passthrough = false;
        for (n, (raw, key)) in keys.iter().enumerate() {
            let (raw, key) = (*raw, key.clone());
            if passthrough {
                fwd.extend_from_slice(raw);
                continue;
            }
            // Enter while this line's suggestions are still loading: wait for
            // them (briefly) so Enter acts on the final list, not a partial one.
            if key == Key::Enter
                && self.pending_enter.is_none()
                && self.bindings.action(&Key::Enter) == Some(Action::Accept)
                && self.cfg.behavior.enter_accepts != "never"
                && !self.just_accepted
                && self.editing
                && !self.hidden
                && self.dismissed.as_deref() != Some(self.line.as_str())
                && self.loading.as_ref().is_some_and(|(l, _, _)| *l == self.line)
            {
                self.pending_enter = Some(Instant::now());
                self.deferred = data[data.len() - keys[n..].iter().map(|(r, _)| r.len()).sum::<usize>()..].to_vec();
                break;
            }
            let toggle = self.bindings.action(&key) == Some(Action::Toggle);
            if toggle && self.editing && !self.hidden && !self.parser.screen().alternate_screen() {
                if self.visible() {
                    self.dismissed = Some(self.line.clone());
                    self.manual = false;
                } else {
                    self.manual = true;
                    self.dismissed = None;
                    self.navigated = false;
                    self.items_line.clear();
                    self.request();
                }
                redraw = true;
                continue;
            }
            if !self.visible() {
                if key != Key::Enter {
                    self.just_accepted = false;
                }
                fwd.extend_from_slice(raw);
                passthrough = true;
                continue;
            }
            let mut action = self.bindings.action(&key);
            // Next with a single suggestion: nothing to cycle through, take it.
            if action == Some(Action::Next) && self.items.len() == 1 {
                action = Some(Action::Accept);
            }
            // Enter completes (`go bui` → build) or picks after a space, but
            // runs a word that's already complete; the Enter after a pick runs.
            if action == Some(Action::Accept) && key == Key::Enter && !self.enter_picks() {
                action = None;
            }
            match action {
                Some(Action::Next) => {
                    self.navigated = true;
                    self.selected = (self.selected + 1) % self.items.len();
                    redraw = true;
                }
                Some(Action::Prev) => {
                    self.navigated = true;
                    self.selected = (self.selected + self.items.len() - 1) % self.items.len();
                    redraw = true;
                }
                Some(Action::Dismiss) | Some(Action::Toggle) => {
                    self.dismissed = Some(self.line.clone());
                    self.manual = false;
                    redraw = true;
                }
                Some(Action::Accept) => {
                    self.just_accepted = true;
                    passthrough = true;
                    let s = &self.items[self.selected];
                    let payload = format!("{INSERT_KEY}{};{};{}\x07", s.delete, s.back, s.insert);
                    fwd.extend_from_slice(payload.as_bytes());
                    self.items.clear();
                    self.items_line.clear();
                    redraw = true;
                }
                None => {
                    if key != Key::Enter {
                        self.just_accepted = false;
                    }
                    passthrough = true;
                    if key == Key::Enter {
                        self.dismissed = Some(self.line.clone());
                        redraw = true;
                    }
                    fwd.extend_from_slice(raw);
                }
            }
        }
        if redraw {
            self.render();
        }
        self.write_pty(&fwd);
    }

    fn on_resize(&mut self) {
        let Some((rows, cols)) = winsize(0) else { return };
        self.clear_popup();
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        self.parser.screen_mut().set_size(rows, cols);
        self.mark_dirty();
    }

    // ------------------------------------------------------------ drawing

    fn render(&mut self) {
        self.dirty = false;
        let mut w = vec![];
        let stuck = self.osc.take_pending();
        if !stuck.is_empty() {
            self.erase(&mut w);
            w.extend_from_slice(&stuck);
            self.parser.process(&stuck);
        }
        // Either the suggestions, or a one-line "Searching…" message.
        let message: Option<Vec<Suggestion>> = self.loading_message().map(|label| {
            let n = self.line.chars().count();
            let word = self.line.chars().rev().take_while(|c| !c.is_whitespace()).count();
            vec![Suggestion {
                label: label.to_string(),
                description: String::new(),
                kind: Kind::Info,
                icon: None,
                insert: String::new(),
                delete: word.min(n),
                back: 0,
                query: String::new(),
            }]
        });
        let is_message = message.is_some();
        let shown: Vec<Suggestion> = if self.visible() { self.items.clone() } else { message.unwrap_or_default() };
        let layout = if shown.is_empty() {
            None
        } else {
            let screen = self.parser.screen();
            let delete = shown[0].delete;
            let tail: String = {
                let n = self.line.chars().count();
                self.line.chars().skip(n.saturating_sub(delete)).collect()
            };
            popup::layout(&self.cfg.ui, &shown, screen.size(), screen.cursor_position(), tail.width() as u16)
        };
        match (&layout, self.drawn) {
            (Some(l), Some(old)) if l.rect == old => {}
            _ => self.erase(&mut w),
        }
        if let Some(l) = layout {
            let h = l.visible();
            if self.selected < self.scroll {
                self.scroll = self.selected;
            } else if self.selected >= self.scroll + h {
                self.scroll = self.selected + 1 - h;
            }
            self.scroll = self.scroll.min(self.items.len().saturating_sub(h));
            self.begin_sync(&mut w);
            let (selected, scroll) = if is_message { (usize::MAX, 0) } else { (self.selected, self.scroll) };
            popup::draw(&mut w, &self.cfg.ui, &l, &shown, selected, scroll);
            popup::restore_cursor(&mut w, self.parser.screen());
            self.drawn = Some(l.rect);
        }
        self.end_sync(&mut w);
        self.write_out(&w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_rule() {
        // (word_empty, complete, navigated, just_accepted)
        // `z ` → pick; `go bui` → complete it; `git status` → run.
        assert!(enter_accepts("auto", true, false, false, false));
        assert!(enter_accepts("auto", false, false, false, false));
        assert!(!enter_accepts("auto", false, true, false, false));
        // Enter, Enter: pick, then run. Tab to an item, then Enter: picked.
        assert!(!enter_accepts("auto", true, false, false, true));
        assert!(enter_accepts("auto", false, true, true, false));
        // Other modes.
        assert!(!enter_accepts("after-space", false, false, false, false));
        assert!(enter_accepts("always", false, true, false, false));
        assert!(!enter_accepts("never", true, false, true, false));
    }
}
