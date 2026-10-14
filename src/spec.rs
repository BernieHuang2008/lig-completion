//! Completion spec model.
//!
//! The shape follows Fig's spec format (subcommands / options / args /
//! generators), but everything is plain Rust: built-in specs are written with
//! the small builder DSL below (see `src/specs/`), generators are Rust
//! closures, and user specs are static JSON/TOML files.

// The builder methods form the spec-authoring API; not all are used by the
// built-in specs.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::exec::{self, Cmd};

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

#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub names: Vec<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub insert_value: Option<String>,
    pub priority: Option<f64>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SuggestionDef {
    pub meta: Meta,
    pub kind: Option<Kind>,
}

#[derive(Debug, Clone)]
pub enum LoadSpec {
    /// Another spec by command name (built-in, user, or `--help`).
    Name(String),
    /// Parse `<argv> --help` when this subcommand is entered.
    Help(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct Directives {
    /// `-abc` is one flag, not `-a -b -c` (e.g. `find -name`, `kill -9`).
    pub flags_are_posix_noncompliant: bool,
    pub option_arg_separators: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Subcommand {
    pub meta: Meta,
    pub subcommands: Vec<Arc<Subcommand>>,
    pub options: Vec<Arc<Opt>>,
    pub args: Vec<Arc<Arg>>,
    pub additional_suggestions: Vec<SuggestionDef>,
    pub load_spec: Option<LoadSpec>,
    pub directives: Option<Directives>,
}

#[derive(Debug, Clone, Default)]
pub struct Opt {
    pub meta: Meta,
    pub args: Vec<Arc<Arg>>,
    pub is_persistent: bool,
    pub is_repeatable: bool,
    /// The separator to insert between the flag and its value (`=`).
    pub separator: Option<String>,
    pub exclusive_on: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Arg {
    pub name: String,
    pub is_optional: bool,
    pub is_variadic: bool,
    pub options_can_break_variadic: bool,
    /// The rest of the line is another command (`sudo`, `time`, …).
    pub is_command: bool,
    /// Suggest command names, without switching to their spec (`which`, `man`).
    pub command_names: bool,
    pub suggestions: Vec<SuggestionDef>,
    pub template: Vec<Template>,
    pub generators: Vec<Arc<Generator>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Filepaths,
    Folders,
}

/// When cached generator results must be recomputed while typing a token.
#[derive(Debug, Clone)]
pub enum Trigger {
    /// When the part of the token up to the last of these characters changes.
    Chars(String),
    /// On every keystroke.
    Change,
    /// When the first `n` characters change (and only once there are `n`).
    Prefix(usize),
}

/// Context handed to generator functions.
pub struct GenCtx<'a> {
    pub cwd: &'a Path,
    pub env: &'a [(String, String)],
    /// All tokens of the command, the one being completed last.
    pub tokens: &'a [String],
    pub timeout: Duration,
}

impl GenCtx<'_> {
    pub fn current(&self) -> &str {
        self.tokens.last().map_or("", String::as_str)
    }

    /// Run a command in the shell's directory; stdout on success.
    pub fn run(&self, argv: &[&str]) -> Option<String> {
        let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        let out = exec::run(Cmd::Argv(&argv), self.cwd, self.env, self.timeout);
        (out.status == 0).then_some(out.stdout)
    }

    pub fn lines(&self, argv: &[&str]) -> Vec<String> {
        self.run(argv)
            .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Whether one of these flags was typed.
    pub fn has_flag(&self, names: &[&str]) -> bool {
        self.tokens.iter().any(|t| names.contains(&t.as_str()))
    }

    /// The value given to one of these flags (`-n ns`, `--namespace=ns`).
    pub fn flag_value(&self, names: &[&str]) -> Option<String> {
        let toks = &self.tokens[..self.tokens.len().saturating_sub(1)];
        for (i, t) in toks.iter().enumerate() {
            if names.contains(&t.as_str())
                && let Some(v) = toks.get(i + 1) {
                    return Some(v.clone());
                }
            if let Some((n, v)) = t.split_once('=')
                && names.contains(&n) {
                    return Some(v.to_string());
                }
        }
        None
    }

    /// Positional (non-flag) tokens after the command, excluding the current one.
    pub fn positionals(&self) -> Vec<&str> {
        let toks = &self.tokens[..self.tokens.len().saturating_sub(1)];
        toks.iter().skip(1).filter(|t| !t.starts_with('-')).map(String::as_str).collect()
    }
}

pub type GenFn = dyn Fn(&GenCtx) -> Vec<SuggestionDef> + Send + Sync;

pub enum GenSource {
    Rust(Box<GenFn>),
    /// From user specs: run a command and split its output.
    Script { argv: Vec<String>, shell: Option<String>, split_on: String },
}

pub struct Generator {
    /// Stable id, part of the result cache key.
    pub id: String,
    pub source: GenSource,
    pub trigger: Option<Trigger>,
    /// Match only the text after the last of these characters (`/`, `,`, `:`).
    pub query_term: Option<String>,
    pub cache_ttl: Option<Duration>,
}

impl std::fmt::Debug for Generator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Generator({})", self.id)
    }
}

impl Subcommand {
    pub fn find_subcommand(&self, name: &str) -> Option<&Arc<Subcommand>> {
        self.subcommands.iter().find(|s| s.meta.names.iter().any(|n| n == name))
    }

    pub fn directives(&self) -> Directives {
        self.directives.clone().unwrap_or_default()
    }
}

impl Opt {
    pub fn takes_arg(&self) -> bool {
        self.args.first().is_some_and(|a| !a.is_optional)
    }
}

// ---------------------------------------------------------------- DSL

fn split_names(names: &str) -> Vec<String> {
    names.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// A (sub)command. `names` is comma separated: `"remove, rm"`.
pub fn cmd(names: &str, desc: &str) -> Subcommand {
    Subcommand {
        meta: Meta { names: split_names(names), description: (!desc.is_empty()).then(|| desc.into()), ..Default::default() },
        ..Default::default()
    }
}

/// An option. `names` is comma separated: `"-v, --verbose"`.
pub fn opt(names: &str, desc: &str) -> Opt {
    Opt {
        meta: Meta { names: split_names(names), description: (!desc.is_empty()).then(|| desc.into()), ..Default::default() },
        ..Default::default()
    }
}

pub fn arg(name: &str) -> Arg {
    Arg { name: name.into(), options_can_break_variadic: true, ..Default::default() }
}

/// A suggestion for generators and static lists.
pub fn sug(name: impl Into<String>) -> SuggestionDef {
    SuggestionDef { meta: Meta { names: vec![name.into()], ..Default::default() }, kind: None }
}

/// A generator backed by a Rust function.
pub fn gen_fn(id: &str, f: impl Fn(&GenCtx) -> Vec<SuggestionDef> + Send + Sync + 'static) -> Generator {
    Generator { id: id.into(), source: GenSource::Rust(Box::new(f)), trigger: None, query_term: None, cache_ttl: None }
}

impl SuggestionDef {
    pub fn desc(mut self, d: impl Into<String>) -> Self {
        let d = d.into();
        self.meta.description = (!d.is_empty()).then_some(d);
        self
    }
    pub fn kind(mut self, k: Kind) -> Self {
        self.kind = Some(k);
        self
    }
    pub fn priority(mut self, p: f64) -> Self {
        self.meta.priority = Some(p);
        self
    }
    pub fn insert(mut self, v: impl Into<String>) -> Self {
        self.meta.insert_value = Some(v.into());
        self
    }
}

impl Subcommand {
    pub fn sub(mut self, s: Subcommand) -> Self {
        self.subcommands.push(Arc::new(s));
        self
    }
    pub fn subs(mut self, s: impl IntoIterator<Item = Subcommand>) -> Self {
        self.subcommands.extend(s.into_iter().map(Arc::new));
        self
    }
    pub fn opt(mut self, o: Opt) -> Self {
        self.options.push(Arc::new(o));
        self
    }
    pub fn opts(mut self, o: impl IntoIterator<Item = Opt>) -> Self {
        self.options.extend(o.into_iter().map(Arc::new));
        self
    }
    pub fn arg(mut self, a: Arg) -> Self {
        self.args.push(Arc::new(a));
        self
    }
    pub fn hidden(mut self) -> Self {
        self.meta.hidden = true;
        self
    }
    pub fn priority(mut self, p: f64) -> Self {
        self.meta.priority = Some(p);
        self
    }
    pub fn posix_noncompliant(mut self) -> Self {
        self.directives.get_or_insert_with(Default::default).flags_are_posix_noncompliant = true;
        self
    }
    pub fn shortcut(mut self, s: SuggestionDef) -> Self {
        self.additional_suggestions.push(s.kind(Kind::Shortcut));
        self
    }
    /// Take the details from `<argv> --help` when this subcommand is entered.
    pub fn help_from(mut self, argv: &[&str]) -> Self {
        self.load_spec = Some(LoadSpec::Help(argv.iter().map(|s| s.to_string()).collect()));
        self
    }
    pub fn load(mut self, name: &str) -> Self {
        self.load_spec = Some(LoadSpec::Name(name.into()));
        self
    }
}

impl Opt {
    pub fn arg(mut self, a: Arg) -> Self {
        self.args.push(Arc::new(a));
        self
    }
    pub fn persistent(mut self) -> Self {
        self.is_persistent = true;
        self
    }
    pub fn repeatable(mut self) -> Self {
        self.is_repeatable = true;
        self
    }
    /// The value is attached with `=` (`--color=auto`).
    pub fn eq(mut self) -> Self {
        self.separator = Some("=".into());
        self
    }
    pub fn exclusive(mut self, names: &[&str]) -> Self {
        self.exclusive_on = names.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn insert(mut self, v: &str) -> Self {
        self.meta.insert_value = Some(v.into());
        self
    }
    pub fn priority(mut self, p: f64) -> Self {
        self.meta.priority = Some(p);
        self
    }
}

impl Arg {
    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }
    pub fn variadic(mut self) -> Self {
        self.is_variadic = true;
        self
    }
    pub fn files(mut self) -> Self {
        self.template.push(Template::Filepaths);
        self
    }
    pub fn folders(mut self) -> Self {
        self.template.push(Template::Folders);
        self
    }
    pub fn command(mut self) -> Self {
        self.is_command = true;
        self
    }
    pub fn command_names(mut self) -> Self {
        self.command_names = true;
        self
    }
    pub fn generator(mut self, g: Generator) -> Self {
        self.generators.push(Arc::new(g));
        self
    }
    pub fn suggest(mut self, items: &[&str]) -> Self {
        self.suggestions.extend(items.iter().map(|s| sug(*s)));
        self
    }
    /// `(name, description)` pairs.
    pub fn suggest_desc(mut self, items: &[(&str, &str)]) -> Self {
        self.suggestions.extend(items.iter().map(|(n, d)| sug(*n).desc(*d)));
        self
    }
}

impl Generator {
    pub fn trigger(mut self, t: Trigger) -> Self {
        self.trigger = Some(t);
        self
    }
    pub fn query_after(mut self, chars: &str) -> Self {
        self.query_term = Some(chars.into());
        self
    }
    pub fn ttl(mut self, ms: u64) -> Self {
        self.cache_ttl = Some(Duration::from_millis(ms));
        self
    }
}

// ---------------------------------------------------------------- user specs (JSON/TOML)

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_string)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn one_or_many(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Null) | None => vec![],
        Some(v) => vec![v],
    }
}

fn names(v: Option<&Value>) -> Vec<String> {
    one_or_many(v).into_iter().filter_map(|n| n.as_str().map(str::to_string)).collect()
}

fn meta(v: &Value) -> Meta {
    Meta {
        names: names(v.get("name")),
        display_name: string(v, "displayName"),
        description: string(v, "description"),
        insert_value: string(v, "insertValue"),
        priority: v.get("priority").and_then(Value::as_f64),
        hidden: flag(v, "hidden"),
    }
}

fn kind_from_str(s: &str) -> Option<Kind> {
    Some(match s {
        "file" => Kind::File,
        "folder" => Kind::Folder,
        "subcommand" => Kind::Subcommand,
        "option" => Kind::Option,
        "arg" => Kind::Arg,
        "shortcut" | "special" => Kind::Shortcut,
        _ => return None,
    })
}

fn suggestion(v: &Value) -> Option<SuggestionDef> {
    match v {
        Value::String(s) => Some(sug(s.clone())),
        Value::Object(_) => {
            let meta = meta(v);
            (!meta.names.is_empty()).then(|| SuggestionDef { meta, kind: v.get("type").and_then(Value::as_str).and_then(kind_from_str) })
        }
        _ => None,
    }
}

fn templates(v: Option<&Value>) -> Vec<Template> {
    names(v)
        .iter()
        .filter_map(|t| match t.as_str() {
            "filepaths" => Some(Template::Filepaths),
            "folders" => Some(Template::Folders),
            _ => None,
        })
        .collect()
}

/// Build a spec from a JSON/TOML value in (a static subset of) Fig's format.
/// Generators may use `script` (a shell string or an argv array) and `splitOn`.
pub fn from_value(v: &Value) -> Subcommand {
    let directives = v.get("parserDirectives").map(|d| Directives {
        flags_are_posix_noncompliant: flag(d, "flagsArePosixNoncompliant"),
        option_arg_separators: names(d.get("optionArgSeparators")),
    });
    Subcommand {
        meta: meta(v),
        subcommands: one_or_many(v.get("subcommands")).into_iter().map(|s| Arc::new(from_value(s))).collect(),
        options: one_or_many(v.get("options")).into_iter().map(|o| Arc::new(opt_from(o))).collect(),
        args: one_or_many(v.get("args")).into_iter().map(|a| Arc::new(arg_from(a))).collect(),
        additional_suggestions: one_or_many(v.get("additionalSuggestions")).into_iter().filter_map(suggestion).collect(),
        load_spec: string(v, "loadSpec").map(LoadSpec::Name),
        directives,
    }
}

fn opt_from(v: &Value) -> Opt {
    let separator = match (v.get("requiresSeparator"), v.get("requiresEquals")) {
        (Some(Value::String(s)), _) => Some(s.clone()),
        (Some(Value::Bool(true)), _) | (_, Some(Value::Bool(true))) => Some("=".into()),
        _ => None,
    };
    Opt {
        meta: meta(v),
        args: one_or_many(v.get("args")).into_iter().map(|a| Arc::new(arg_from(a))).collect(),
        is_persistent: flag(v, "isPersistent"),
        is_repeatable: matches!(v.get("isRepeatable"), Some(Value::Bool(true) | Value::Number(_))),
        separator,
        exclusive_on: names(v.get("exclusiveOn")),
    }
}

fn arg_from(v: &Value) -> Arg {
    Arg {
        name: string(v, "name").unwrap_or_default(),
        is_optional: flag(v, "isOptional"),
        is_variadic: flag(v, "isVariadic"),
        options_can_break_variadic: v.get("optionsCanBreakVariadicArg").and_then(Value::as_bool).unwrap_or(true),
        is_command: flag(v, "isCommand"),
        command_names: false,
        suggestions: one_or_many(v.get("suggestions")).into_iter().filter_map(suggestion).collect(),
        template: templates(v.get("template")),
        generators: one_or_many(v.get("generators")).into_iter().filter_map(generator_from).map(Arc::new).collect(),
    }
}

fn generator_from(v: &Value) -> Option<Generator> {
    let (argv, shell) = match v.get("script")? {
        Value::String(s) => (vec![], Some(s.clone())),
        Value::Array(a) => (a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(), None),
        _ => return None,
    };
    let id = format!("user:{}", v.get("script")?);
    let split_on = string(v, "splitOn").unwrap_or_else(|| "\n".into());
    Some(Generator {
        id,
        source: GenSource::Script { argv, shell, split_on },
        trigger: v.get("trigger").and_then(Value::as_str).map(|s| Trigger::Chars(s.into())),
        query_term: string(v, "getQueryTerm"),
        cache_ttl: v.get("cache").and_then(|c| c.get("ttl")).and_then(Value::as_u64).map(Duration::from_millis),
    })
}
