//! The completion engine: walks a command line through its Fig spec and
//! produces ranked suggestions.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::Behavior;
use crate::exec::{self, Cmd};
use crate::js::{EXEC, ExecEnv, Js};
use crate::parser::{self, CommandLine, Token};
use crate::spec::{self, Arg, Generator, LoadSpec, Opt, QueryTerm, Script, Subcommand, SuggestionDef, Template, Trigger};
use crate::store::{Source, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    Subcommand,
    Option,
    Arg,
    File,
    Folder,
    Shortcut,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub label: String,
    pub description: String,
    pub kind: Kind,
    /// Text to insert after deleting `delete` characters left of the cursor.
    pub insert: String,
    pub delete: usize,
    /// Move the cursor this many characters back after inserting.
    pub back: usize,
    /// The text the suggestion was matched against (for highlighting).
    pub query: String,
}

#[derive(Debug, Default)]
pub struct Completion {
    pub items: Vec<Suggestion>,
    /// Some generators were skipped and still need to run.
    pub incomplete: bool,
}

/// What the shell told us about its environment.
#[derive(Debug, Clone, Default)]
pub struct ShellEnv {
    pub cwd: PathBuf,
    pub path: Option<String>,
}

struct Cand {
    names: Vec<String>,
    label: String,
    description: Option<String>,
    insert_value: Option<String>,
    priority: f64,
    hidden: bool,
    kind: Kind,
    query: String,
    delete: usize,
    suffix: String,
}

struct Cx<'a> {
    cl: &'a CommandLine,
    env: &'a ShellEnv,
    tokens: Vec<String>,
    cands: Vec<Cand>,
    run_generators: bool,
    incomplete: bool,
}

impl Cx<'_> {
    fn default_query(&self) -> (String, usize) {
        (self.cl.current.value.clone(), self.cl.current.raw.chars().count())
    }

    fn push_def(&mut self, d: &SuggestionDef, fallback: Kind, query: &str, delete: usize) {
        let kind = match d.kind.as_deref() {
            Some("file") => Kind::File,
            Some("folder") => Kind::Folder,
            Some("subcommand") => Kind::Subcommand,
            Some("option") => Kind::Option,
            Some("special" | "shortcut" | "auto-execute" | "mixin") => Kind::Shortcut,
            Some("arg") => Kind::Arg,
            _ => fallback,
        };
        let mut c = cand(&d.meta, kind, " ");
        c.query = query.to_string();
        c.delete = delete;
        self.cands.push(c);
    }
}

fn cand(m: &spec::Meta, kind: Kind, suffix: &str) -> Cand {
    let label = m.display_name.clone().unwrap_or_else(|| match kind {
        Kind::Option => m.names.join(", "),
        _ => m.names.first().cloned().unwrap_or_default(),
    });
    let names = if m.names.is_empty() { vec![label.clone()] } else { m.names.clone() };
    Cand {
        names,
        label,
        description: m.description.clone(),
        insert_value: m.insert_value.clone(),
        priority: m.priority.unwrap_or(50.0),
        hidden: m.hidden,
        kind,
        query: String::new(),
        delete: 0,
        suffix: suffix.to_string(),
    }
}

pub struct Engine {
    js: Js,
    store: Store,
    cfg: Behavior,
    specs: HashMap<String, Option<Arc<Subcommand>>>,
    generated: HashMap<String, (Instant, Arc<Subcommand>)>,
    gen_cache: HashMap<String, (Instant, Vec<SuggestionDef>)>,
    path_cmds: Option<(Instant, String, Arc<HashSet<String>>)>,
    spec_cmds: Option<Arc<BTreeSet<String>>>,
}

impl Engine {
    pub fn new(store: Store, cfg: Behavior) -> anyhow::Result<Self> {
        let js = Js::new(Duration::from_millis(cfg.generator_timeout_ms))?;
        Ok(Engine {
            js,
            store,
            cfg,
            specs: HashMap::new(),
            generated: HashMap::new(),
            gen_cache: HashMap::new(),
            path_cmds: None,
            spec_cmds: None,
        })
    }

    fn exec_vars(env: &ShellEnv) -> Vec<(String, String)> {
        env.path.iter().map(|p| ("PATH".to_string(), p.clone())).collect()
    }

    /// Complete `line` (the text left of the cursor). With
    /// `run_generators == false`, only cached generator results are used and
    /// `incomplete` tells whether running them could add more.
    pub fn complete(&mut self, line: &str, env: &ShellEnv, run_generators: bool) -> Completion {
        self.js.set_env(ExecEnv { cwd: env.cwd.clone(), vars: Self::exec_vars(env) });
        let cl = parser::parse(line);
        let mut cx = Cx { tokens: cl.values(), cl: &cl, env, cands: vec![], run_generators, incomplete: false };

        if cl.current.redirect {
            self.files(&mut cx, false);
        } else if cl.tokens.is_empty() {
            let v = &cl.current.value;
            if self.cfg.complete_commands && !v.is_empty() {
                if v.contains('/') {
                    self.files(&mut cx, false);
                } else {
                    self.commands(&mut cx);
                }
            }
        } else if self.cfg.show_on_empty_token || !cl.current.raw.is_empty() {
            let cmd = basename(&cl.tokens[0].value);
            if !self.cfg.disabled_commands.iter().any(|d| d == cmd)
                && let Some(root) = self.spec(cmd) {
                    let root = self.resolve(&root, &cx, 1);
                    self.walk(root, &mut cx);
                }
        }
        let incomplete = cx.incomplete;
        Completion { items: self.finish(cx), incomplete }
    }

    // ------------------------------------------------------------ walking

    fn walk(&mut self, root: Arc<Subcommand>, cx: &mut Cx<'_>) {
        let tokens: Vec<Token> = cx.cl.tokens[1..].to_vec();
        let mut node = root;
        let mut persistent: Vec<Arc<Opt>> = vec![];
        let mut used: Vec<Arc<Opt>> = vec![];
        let mut arg_idx = 0usize;
        let mut saw_positional = false;
        let mut no_more_opts = false;
        let mut pending: Option<(Arc<Opt>, usize)> = None;
        let mut directives = node.directives();

        for (i, tok) in tokens.iter().enumerate() {
            let t = tok.value.as_str();
            if let Some((opt, k)) = pending.take() {
                let a = &opt.args[k];
                let breaks = a.is_variadic && a.options_can_break_variadic && is_flag(t);
                if !breaks {
                    if a.is_variadic {
                        pending = Some((opt, k));
                    } else if opt.args.get(k + 1).is_some_and(|n| !n.is_optional) {
                        pending = Some((opt.clone(), k + 1));
                    }
                    continue;
                }
            }
            if !no_more_opts && t == "--" {
                no_more_opts = true;
                continue;
            }
            if !no_more_opts && is_flag(t) {
                if let Some((opt, inline)) = match_option(&node, &persistent, t, &directives) {
                    if !inline && opt.takes_arg() {
                        pending = Some((opt.clone(), 0));
                    }
                    used.push(opt);
                }
                continue;
            }
            if !saw_positional
                && let Some(sub) = node.find_subcommand(t).cloned() {
                    persistent.extend(node.options.iter().filter(|o| o.is_persistent).cloned());
                    node = self.resolve(&sub, cx, i + 2);
                    if let Some(d) = &node.directives {
                        directives = d.clone();
                    }
                    arg_idx = 0;
                    continue;
                }
            saw_positional = true;
            let Some(arg) = current_arg(&node, arg_idx) else { continue };
            let next_spec = if arg.is_command {
                Some(self.spec(basename(t)))
            } else {
                arg.load_spec.clone().map(|ls| self.load(&ls, t))
            };
            match next_spec {
                // The rest of the line belongs to another command.
                Some(Some(s)) => {
                    node = self.resolve(&s, cx, i + 2);
                    persistent.clear();
                    used.clear();
                    arg_idx = 0;
                    saw_positional = false;
                    no_more_opts = false;
                    directives = node.directives();
                }
                Some(None) => return,
                None if !arg.is_variadic => arg_idx += 1,
                None => {}
            }
        }

        // --- the token under the cursor
        let cur = cx.cl.current.clone();
        if let Some((opt, k)) = pending {
            let a = opt.args[k].clone();
            if !(a.is_variadic && a.options_can_break_variadic && is_flag(&cur.value)) {
                let (q, d) = cx.default_query();
                self.arg_suggestions(&a, cx, &q, d);
                return;
            }
        }
        if !no_more_opts && cur.value.starts_with('-') {
            // `--opt=value`
            let seps: Vec<String> = if directives.option_arg_separators.is_empty() {
                vec!["=".into()]
            } else {
                directives.option_arg_separators.clone()
            };
            for sep in &seps {
                if let Some((name, value)) = cur.value.split_once(sep.as_str())
                    && let Some(opt) = find_option(&node, &persistent, name) {
                        if let Some(a) = opt.args.first().cloned() {
                            let delete = cur.raw.rsplit_once(sep.as_str()).map_or(0, |(_, r)| r.chars().count());
                            self.arg_suggestions(&a, cx, value, delete);
                        }
                        return;
                    }
            }
            self.options(&node, &persistent, &used, cx);
            return;
        }
        if !saw_positional {
            let (q, d) = cx.default_query();
            for s in node.subcommands.clone() {
                let mut c = cand(&s.meta, Kind::Subcommand, " ");
                c.query = q.clone();
                c.delete = d;
                cx.cands.push(c);
            }
            for s in node.additional_suggestions.clone() {
                cx.push_def(&s, Kind::Shortcut, &q, d);
            }
        }
        let arg = current_arg(&node, arg_idx);
        if let Some(a) = &arg {
            if a.is_command {
                self.commands(cx);
            } else {
                let (q, d) = cx.default_query();
                self.arg_suggestions(a, cx, &q, d);
            }
        }
        if cx.cands.is_empty() && !cx.incomplete && arg.is_none_or(|a| a.is_optional) {
            self.options(&node, &persistent, &used, cx);
        }
    }

    /// Apply `loadSpec` / `generateSpec` of a subcommand.
    fn resolve(&mut self, sub: &Arc<Subcommand>, cx: &Cx<'_>, upto: usize) -> Arc<Subcommand> {
        let mut node = sub.clone();
        if let Some(ls) = sub.load_spec.clone() {
            let token = cx.tokens.get(upto.saturating_sub(1)).cloned().unwrap_or_default();
            if let Some(loaded) = self.load(&ls, &token) {
                let mut merged = (*loaded).clone();
                merged.meta = sub.meta.clone();
                merged.options.extend(sub.options.iter().cloned());
                merged.load_spec = None;
                node = Arc::new(merged);
            }
        }
        if let Some(f) = node.generate_spec {
            let before = &cx.tokens[..upto.min(cx.tokens.len())];
            let key = format!("{f}\0{}\0{}", cx.env.cwd.display(), before.join("\x1f"));
            let generated = match self.generated.get(&key) {
                Some((t, s)) if t.elapsed() < Duration::from_secs(30) => Some(s.clone()),
                _ => {
                    let v = self.js.call(f, &[json!(before), Value::from(EXEC)]);
                    let s = v.ok().filter(Value::is_object).map(|v| Arc::new(spec::subcommand(&v)));
                    if let Some(s) = &s {
                        self.generated.insert(key, (Instant::now(), s.clone()));
                    }
                    s
                }
            };
            if let Some(g) = generated {
                let mut merged = (*node).clone();
                merged.generate_spec = None;
                merged.subcommands.extend(g.subcommands.iter().cloned());
                merged.options.extend(g.options.iter().cloned());
                if merged.args.is_empty() {
                    merged.args = g.args.clone();
                }
                node = Arc::new(merged);
            }
        }
        node
    }

    fn load(&mut self, ls: &LoadSpec, token: &str) -> Option<Arc<Subcommand>> {
        match ls {
            LoadSpec::Name(n) => self.spec(n),
            LoadSpec::Inline(s) => Some(s.clone()),
            LoadSpec::Func(f) => match self.js.call(*f, &[Value::from(token), Value::from(EXEC)]).ok()? {
                Value::String(n) => self.spec(&n),
                v @ Value::Object(_) => Some(Arc::new(spec::subcommand(&v))),
                _ => None,
            },
        }
    }

    pub fn spec(&mut self, name: &str) -> Option<Arc<Subcommand>> {
        if let Some(s) = self.specs.get(name) {
            return s.clone();
        }
        let s = self.load_spec_file(name, 0);
        self.specs.insert(name.to_string(), s.clone());
        s
    }

    fn load_spec_file(&mut self, name: &str, depth: u32) -> Option<Arc<Subcommand>> {
        if depth > 3 {
            return None;
        }
        let v = match self.store.read(name)? {
            Source::Json(text) => serde_json::from_str::<Value>(&text).ok()?,
            Source::Js(text) => {
                let exports = match self.js.load_module(&text) {
                    Ok(e) => e,
                    Err(e) => {
                        crate::log(&format!("spec {name}: {e}"));
                        return None;
                    }
                };
                let def = exports.get("default")?;
                match spec::func(def) {
                    // A versioned spec: `default(version)` → `{versionedSpecPath}`.
                    Some(f) => {
                        let version = exports
                            .get("getVersionCommand")
                            .and_then(spec::func)
                            .and_then(|vc| self.js.call(vc, &[Value::from(EXEC)]).ok())
                            .unwrap_or(Value::Null);
                        let res = self.js.call(f, &[version]).ok()?;
                        if let Some(p) = res.get("versionedSpecPath").and_then(Value::as_str) {
                            return self.load_spec_file(p, depth + 1);
                        }
                        res
                    }
                    None => def.clone(),
                }
            }
        };
        v.is_object().then(|| Arc::new(spec::subcommand(&v)))
    }

    // ------------------------------------------------------------ sources

    fn options(&mut self, node: &Subcommand, persistent: &[Arc<Opt>], used: &[Arc<Opt>], cx: &mut Cx<'_>) {
        let (q, d) = cx.default_query();
        for o in node.options.iter().chain(persistent) {
            if !o.is_repeatable && used.iter().any(|u| Arc::ptr_eq(u, o)) {
                continue;
            }
            if used.iter().any(|u| u.meta.names.iter().any(|n| o.exclusive_on.contains(n))) {
                continue;
            }
            let suffix = match &o.separator {
                Some(sep) if o.takes_arg() => sep.as_str(),
                _ => " ",
            };
            let mut c = cand(&o.meta, Kind::Option, suffix);
            c.query = q.clone();
            c.delete = d;
            cx.cands.push(c);
        }
    }

    fn arg_suggestions(&mut self, a: &Arg, cx: &mut Cx<'_>, query: &str, delete: usize) {
        for s in &a.suggestions {
            cx.push_def(s, Kind::Arg, query, delete);
        }
        for t in &a.template {
            match t {
                Template::Filepaths => self.files(cx, false),
                Template::Folders => self.files(cx, true),
                _ => {}
            }
        }
        for g in &a.generators {
            self.generator(g, cx, query, delete);
        }
    }

    fn generator(&mut self, g: &Arc<Generator>, cx: &mut Cx<'_>, query: &str, delete: usize) {
        if !g.template.is_empty() {
            let start = cx.cands.len();
            for t in &g.template {
                match t {
                    Template::Filepaths => self.files(cx, false),
                    Template::Folders => self.files(cx, true),
                    _ => {}
                }
            }
            if let Some(f) = g.filter_template_suggestions {
                let list: Vec<Value> = cx.cands[start..]
                    .iter()
                    .map(|c| {
                        let ty = if c.kind == Kind::Folder { "folder" } else { "file" };
                        json!({"name": c.names[0], "type": ty, "insertValue": c.names[0]})
                    })
                    .collect();
                if let Ok(Value::Array(kept)) = self.js.call(f, &[Value::Array(list)]) {
                    let keep: HashSet<&str> = kept.iter().filter_map(|k| k.get("name")?.as_str()).collect();
                    let mut i = 0;
                    cx.cands.retain(|c| {
                        i += 1;
                        i <= start || keep.contains(c.names[0].as_str())
                    });
                }
            }
            return;
        }

        let cur = &cx.cl.current;
        let (query, delete) = match &g.get_query_term {
            Some(QueryTerm::Chars(chars)) => match cur.value.rfind(|c| chars.contains(c)) {
                Some(i) => {
                    let q = cur.value[i + 1..].to_string();
                    let d = q.chars().count();
                    (q, d)
                }
                None => (query.to_string(), delete),
            },
            Some(QueryTerm::Func(f)) => match self.js.call(*f, &[Value::from(cur.value.clone())]) {
                Ok(Value::String(q)) => {
                    let d = q.chars().count();
                    (q, d)
                }
                _ => (query.to_string(), delete),
            },
            None => (query.to_string(), delete),
        };

        let v = &cur.value;
        let trigger_key = match &g.trigger {
            None => String::new(),
            Some(Trigger::Chars(chars)) => v.rfind(|c| chars.contains(c)).map_or(String::new(), |i| v[..=i].to_string()),
            Some(Trigger::Func(_)) => v.rfind('/').map_or(String::new(), |i| v[..=i].to_string()),
            Some(Trigger::Change) => v.clone(),
            Some(Trigger::Threshold(n)) => (v.chars().count() > *n).to_string(),
            Some(Trigger::Match(list)) => if list.contains(v) { v.clone() } else { String::new() },
        };
        let before = &cx.tokens[..cx.tokens.len() - 1];
        let key = format!("{}\0{}\0{}\0{}", g.id, cx.env.cwd.display(), before.join("\x1f"), trigger_key);
        let ttl = Duration::from_millis(g.cache_ttl_ms.unwrap_or(self.cfg.generator_cache_ms));

        let defs = match self.gen_cache.get(&key) {
            Some((t, defs)) if t.elapsed() < ttl => defs.clone(),
            _ if !cx.run_generators => {
                cx.incomplete = true;
                return;
            }
            _ => {
                let defs = match self.run_generator(g, cx) {
                    Ok(d) => d,
                    Err(e) => {
                        crate::log(&format!("generator: {e}"));
                        vec![]
                    }
                };
                self.gen_cache.insert(key, (Instant::now(), defs.clone()));
                defs
            }
        };
        for d in &defs {
            cx.push_def(d, Kind::Arg, &query, delete);
        }
    }

    fn run_generator(&mut self, g: &Generator, cx: &Cx<'_>) -> anyhow::Result<Vec<SuggestionDef>> {
        let tokens = json!(cx.tokens);
        if let Some(f) = g.custom {
            let gctx = json!({
                "currentWorkingDirectory": cx.env.cwd,
                "currentProcess": "zsh",
                "sshPrefix": "",
                "searchTerm": cx.cl.current.value,
                "environmentVariables": {},
                "isDangerous": false,
            });
            let v = self.js.call(f, &[tokens, Value::from(EXEC), gctx])?;
            return Ok(spec::suggestions(Some(&v)));
        }
        let Some(script) = &g.script else { return Ok(vec![]) };
        let (shell, argv): (Option<String>, Vec<String>) = match script {
            Script::Shell(s) => (Some(s.clone()), vec![]),
            Script::Argv(a) => (None, a.clone()),
            Script::Func(f) => match self.js.call(*f, std::slice::from_ref(&tokens))? {
                Value::String(s) => (Some(s), vec![]),
                Value::Array(a) => (None, a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()),
                Value::Object(o) => {
                    let mut argv = vec![o.get("command").and_then(Value::as_str).unwrap_or("").to_string()];
                    if let Some(Value::Array(a)) = o.get("args") {
                        argv.extend(a.iter().filter_map(|x| x.as_str().map(str::to_string)));
                    }
                    (None, argv)
                }
                _ => return Ok(vec![]),
            },
        };
        let timeout = Duration::from_millis(g.script_timeout_ms.unwrap_or(self.cfg.generator_timeout_ms));
        let vars = Self::exec_vars(cx.env);
        let out = match &shell {
            Some(s) => exec::run(Cmd::Shell(s), &cx.env.cwd, &vars, timeout),
            None => exec::run(Cmd::Argv(&argv), &cx.env.cwd, &vars, timeout),
        };
        let stdout = out.stdout.trim_end();
        if let Some(pp) = g.post_process {
            let v = self.js.call(pp, &[Value::from(stdout), tokens])?;
            return Ok(spec::suggestions(Some(&v)));
        }
        let sep = g.split_on.as_deref().unwrap_or("\n");
        Ok(stdout
            .split(sep)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|s| spec::suggestion(&Value::from(s)))
            .collect())
    }

    fn files(&mut self, cx: &mut Cx<'_>, folders_only: bool) {
        let cur = &cx.cl.current;
        let (dir_part, base) = match cur.value.rfind('/') {
            Some(i) => (&cur.value[..=i], &cur.value[i + 1..]),
            None => ("", cur.value.as_str()),
        };
        let delete = match cur.raw.rfind('/') {
            Some(i) => cur.raw[i + 1..].chars().count(),
            None => cur.raw.chars().count(),
        };
        let dir = if dir_part.is_empty() {
            cx.env.cwd.clone()
        } else if dir_part == "~/" || dir_part.starts_with("~/") {
            dirs::home_dir().unwrap_or_default().join(&dir_part[2..])
        } else if dir_part.starts_with('/') {
            PathBuf::from(dir_part)
        } else {
            cx.env.cwd.join(dir_part)
        };
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        let mut entries: Vec<(bool, String)> = rd
            .flatten()
            .take(5000)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') && !base.starts_with('.') {
                    return None;
                }
                let is_dir = std::fs::metadata(e.path()).map(|m| m.is_dir()).unwrap_or(false);
                (is_dir || !folders_only).then_some((is_dir, name))
            })
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        for (is_dir, name) in entries {
            let (kind, label) = if is_dir { (Kind::Folder, format!("{name}/")) } else { (Kind::File, name.clone()) };
            let meta = spec::Meta { names: vec![label], ..Default::default() };
            let mut c = cand(&meta, kind, if is_dir { "" } else { " " });
            c.query = base.to_string();
            c.delete = delete;
            cx.cands.push(c);
        }
    }

    fn commands(&mut self, cx: &mut Cx<'_>) {
        let specs = self.spec_cmds.get_or_insert_with(|| Arc::new(self.store.commands())).clone();
        let path = self.path_commands(cx.env);
        let (q, d) = cx.default_query();
        for name in specs.iter().filter(|n| path.contains(*n)) {
            let meta = spec::Meta { names: vec![name.clone()], ..Default::default() };
            let mut c = cand(&meta, Kind::Command, " ");
            c.query = q.clone();
            c.delete = d;
            cx.cands.push(c);
        }
    }

    fn path_commands(&mut self, env: &ShellEnv) -> Arc<HashSet<String>> {
        let path = env.path.clone().or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
        if let Some((t, p, set)) = &self.path_cmds
            && *p == path && t.elapsed() < Duration::from_secs(30) {
                return set.clone();
            }
        let mut set = HashSet::new();
        for dir in path.split(':').filter(|d| !d.is_empty()) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                if let Ok(m) = std::fs::metadata(e.path())
                    && m.is_file() && m.permissions().mode() & 0o111 != 0 {
                        set.insert(e.file_name().to_string_lossy().into_owned());
                    }
            }
        }
        let set = Arc::new(set);
        self.path_cmds = Some((Instant::now(), path, set.clone()));
        set
    }

    // ------------------------------------------------------------ ranking

    fn finish(&self, cx: Cx<'_>) -> Vec<Suggestion> {
        let cur = &cx.cl.current;
        let mut scored: Vec<(u8, f64, usize, String)> = vec![];
        for (idx, c) in cx.cands.iter().enumerate() {
            if c.names.iter().all(|n| n.trim().is_empty()) && c.insert_value.is_none() {
                continue;
            }
            // Rank by the best-matching name, but insert the first name (in
            // spec order) that matches as a prefix: `b` → `build`, not `b`.
            let tiers: Vec<Option<u8>> = c.names.iter().map(|n| match_tier(n, &c.query, self.cfg.fuzzy)).collect();
            let Some(tier) = tiers.iter().flatten().min().copied() else { continue };
            let pick = if tier <= 1 {
                tiers.iter().position(|t| matches!(t, Some(0 | 1)))
            } else {
                tiers.iter().position(Option::is_some)
            };
            let pick = pick.unwrap_or(0);
            let name = &c.names[pick];
            if c.hidden && tier != 0 {
                continue;
            }
            scored.push((tier, c.priority, idx, name.clone()));
        }
        // Fuzzy matches only when nothing matches by prefix.
        if scored.iter().any(|s| s.0 < 2) {
            scored.retain(|s| s.0 < 2);
        }
        scored.sort_by(|a, b| {
            let bucket = |t: u8| (t == 2) as u8;
            bucket(a.0)
                .cmp(&bucket(b.0))
                .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.0.cmp(&b.0))
                .then(a.2.cmp(&b.2))
        });

        let mut seen = HashSet::new();
        let mut out = vec![];
        for (_, _, idx, name) in scored {
            let c = &cx.cands[idx];
            let (insert, back) = insertion(c, &name, cur);
            if !seen.insert(insert.clone()) {
                continue;
            }
            out.push(Suggestion {
                label: c.label.clone(),
                description: c.description.as_deref().unwrap_or("").lines().next().unwrap_or("").trim().to_string(),
                kind: c.kind,
                insert,
                delete: c.delete,
                back,
                query: c.query.clone(),
            });
            if out.len() >= 500 {
                break;
            }
        }
        // Nothing to offer if the only suggestion is exactly what's typed.
        if out.len() == 1 && out[0].kind != Kind::Folder && out[0].insert.trim_end() == cur.raw {
            out.clear();
        }
        out
    }
}

fn insertion(c: &Cand, name: &str, cur: &Token) -> (String, usize) {
    if let Some(iv) = &c.insert_value {
        return match iv.split_once("{cursor}") {
            Some((a, b)) => (format!("{a}{b}"), b.chars().count()),
            None => (iv.clone(), 0),
        };
    }
    let is_dir = c.kind == Kind::Folder || name.ends_with('/');
    let bare = if is_dir { name.trim_end_matches('/') } else { name };
    let suffix = if is_dir { "/" } else { c.suffix.as_str() };
    match cur.open_quote {
        Some(q) if c.kind != Kind::Option => {
            let body = match q {
                '\'' => bare.replace('\'', r"'\''"),
                _ => bare.chars().fold(String::new(), |mut s, ch| {
                    if matches!(ch, '"' | '\\' | '$' | '`') {
                        s.push('\\');
                    }
                    s.push(ch);
                    s
                }),
            };
            let open = if c.delete >= cur.raw.chars().count() { q.to_string() } else { String::new() };
            if is_dir {
                (format!("{open}{body}/"), 0)
            } else {
                (format!("{open}{body}{q}{suffix}"), 0)
            }
        }
        _ if matches!(c.kind, Kind::File | Kind::Folder) => (format!("{}{suffix}", parser::escape(bare)), 0),
        // Values from specs may be globs or `~/…`; only escape what would break the word.
        _ => (format!("{}{suffix}", parser::escape_minimal(bare)), 0),
    }
}

fn is_flag(t: &str) -> bool {
    t.starts_with('-') && t.len() > 1
}

fn basename(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}

fn current_arg(node: &Subcommand, idx: usize) -> Option<Arc<Arg>> {
    node.args.get(idx).or_else(|| node.args.last().filter(|a| a.is_variadic)).cloned()
}

fn find_option(node: &Subcommand, persistent: &[Arc<Opt>], name: &str) -> Option<Arc<Opt>> {
    node.options.iter().chain(persistent).find(|o| o.meta.names.iter().any(|n| n == name)).cloned()
}

/// Returns the option a flag token refers to and whether its argument is
/// already part of the token (`--foo=bar`, `-ovalue`, `-abc`).
fn match_option(
    node: &Subcommand,
    persistent: &[Arc<Opt>],
    t: &str,
    d: &spec::Directives,
) -> Option<(Arc<Opt>, bool)> {
    if let Some(o) = find_option(node, persistent, t) {
        return Some((o, false));
    }
    if let Some((name, _)) = t.split_once('=')
        && let Some(o) = find_option(node, persistent, name) {
            return Some((o, true));
        }
    for sep in &d.option_arg_separators {
        if let Some((name, _)) = t.split_once(sep.as_str())
            && let Some(o) = find_option(node, persistent, name) {
                return Some((o, true));
            }
    }
    if !t.starts_with("--") && !d.flags_are_posix_noncompliant {
        let chars: Vec<char> = t[1..].chars().collect();
        let mut last = None;
        for (j, ch) in chars.iter().enumerate() {
            match find_option(node, persistent, &format!("-{ch}")) {
                Some(o) if o.takes_arg() => return Some((o, j + 1 < chars.len())),
                Some(o) => last = Some(o),
                None => break,
            }
        }
        return last.map(|o| (o, true));
    }
    None
}

/// 0 = exact, 1 = prefix, 2 = fuzzy (subsequence); smart-case.
pub fn match_tier(name: &str, query: &str, fuzzy: bool) -> Option<u8> {
    if query.is_empty() {
        return Some(1);
    }
    let sensitive = query.chars().any(char::is_uppercase);
    let (n, q) = if sensitive { (name.to_string(), query.to_string()) } else { (name.to_lowercase(), query.to_lowercase()) };
    if n == q {
        return Some(0);
    }
    if n.starts_with(&q) {
        return Some(1);
    }
    // Fuzzy: subsequence, anchored on the first significant character.
    let (n, q) = (n.trim_start_matches('-'), q.trim_start_matches('-'));
    if fuzzy && q.chars().count() >= 2 && n.chars().next() == q.chars().next() {
        let mut it = n.chars();
        if q.chars().all(|qc| it.by_ref().any(|nc| nc == qc)) {
            return Some(2);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"var s={name:"tool",options:[{name:["-v","--verbose"],description:"Loud",isPersistent:true},
      {name:"--color",requiresSeparator:true,args:{name:"when",suggestions:["always","never","auto"]}},
      {name:["-o","--output"],args:{name:"file",template:"filepaths"}}],
      subcommands:[{name:["build","b"],description:"Build it",args:{name:"target",generators:{script:["printf","alpha\\nbeta\\n"],
        postProcess:o=>o.split("\n").map(n=>({name:n,description:"from gen"}))}}},
        {name:"bench"},{name:"commit",options:[{name:"-m",insertValue:"-m '{cursor}'",args:{name:"msg"}}]},
        {name:"hidden-cmd",hidden:true}]};export{s as default};"#;

    fn engine() -> (Engine, ShellEnv) {
        let dir = std::env::temp_dir().join(format!("lig-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("specs")).unwrap();
        std::fs::create_dir_all(dir.join("work/sub dir")).unwrap();
        std::fs::write(dir.join("work/file.txt"), "").unwrap();
        std::fs::write(dir.join("specs/tool.js"), SPEC).unwrap();
        let e = Engine::new(Store::with_dirs(vec![dir.join("specs")]), Behavior::default()).unwrap();
        (e, ShellEnv { cwd: dir.join("work"), path: None })
    }

    fn labels(e: &mut Engine, env: &ShellEnv, line: &str) -> Vec<(String, String, usize)> {
        e.complete(line, env, true).items.into_iter().map(|s| (s.label, s.insert, s.delete)).collect()
    }

    #[test]
    fn subcommands_and_aliases() {
        let (mut e, env) = engine();
        let r = labels(&mut e, &env, "tool b");
        assert_eq!(r[0], ("build".into(), "build ".into(), 1));
        assert_eq!(r[1].0, "bench");
        assert!(!labels(&mut e, &env, "tool ").iter().any(|r| r.0 == "hidden-cmd"));
        assert_eq!(labels(&mut e, &env, "tool hidden-c").len(), 0);
    }

    #[test]
    fn options_args_and_generators() {
        let (mut e, env) = engine();
        let r = labels(&mut e, &env, "tool --c");
        assert_eq!(r, vec![("--color".into(), "--color=".into(), 3)]);
        let r = labels(&mut e, &env, "tool --color=a");
        assert_eq!(r, vec![("always".into(), "always ".into(), 1), ("auto".into(), "auto ".into(), 1)]);
        // Persistent option offered under a subcommand; generator output.
        let r = labels(&mut e, &env, "tool build -");
        assert!(r.iter().any(|x| x.0 == "-v, --verbose"));
        let r = labels(&mut e, &env, "tool -v build ");
        assert_eq!(r.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["alpha", "beta"]);
        // insertValue with {cursor}.
        let c = e.complete("tool commit -", &env, true);
        assert_eq!((c.items[0].insert.as_str(), c.items[0].back), ("-m ''", 1));
    }

    #[test]
    fn files_and_option_args() {
        let (mut e, env) = engine();
        let r = labels(&mut e, &env, "tool -o ");
        assert_eq!(r, vec![("sub dir/".into(), "sub\\ dir/".into(), 0), ("file.txt".into(), "file.txt ".into(), 0)]);
        let r = labels(&mut e, &env, "tool -o \"su");
        assert_eq!(r, vec![("sub dir/".into(), "\"sub dir/".into(), 3)]);
    }

    #[test]
    fn generators_are_deferred() {
        let (mut e, env) = engine();
        let c = e.complete("tool build ", &env, false);
        assert!(c.incomplete && c.items.is_empty());
        assert_eq!(e.complete("tool build ", &env, true).items.len(), 2);
        let c = e.complete("tool build a", &env, false);
        assert!(!c.incomplete);
        assert_eq!(c.items[0].label, "alpha");
    }
}
