//! Fig completion spec model, built from the JSON the JS host produces.
//! Field names follow https://fig.io/docs/reference. A few fields are parsed
//! for completeness of the model but not used by the engine yet.
#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use crate::js::FnId;

#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub names: Vec<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub insert_value: Option<String>,
    pub icon: Option<String>,
    pub priority: Option<f64>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SuggestionDef {
    pub meta: Meta,
    /// Fig `type` field (`"file"`, `"folder"`, `"subcommand"`, …).
    pub kind: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LoadSpec {
    Name(String),
    Func(FnId),
    Inline(Arc<Subcommand>),
}

#[derive(Debug, Clone, Default)]
pub struct Directives {
    pub flags_are_posix_noncompliant: bool,
    pub options_must_precede_arguments: bool,
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
    pub generate_spec: Option<FnId>,
    pub filter_strategy: Option<String>,
    pub directives: Option<Directives>,
}

#[derive(Debug, Clone, Default)]
pub struct Opt {
    pub meta: Meta,
    pub args: Vec<Arc<Arg>>,
    pub is_persistent: bool,
    pub is_repeatable: bool,
    /// `requiresSeparator` / `requiresEquals`: the separator to insert.
    pub separator: Option<String>,
    pub exclusive_on: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Arg {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_optional: bool,
    pub is_variadic: bool,
    pub options_can_break_variadic: bool,
    pub is_command: bool,
    pub suggestions: Vec<SuggestionDef>,
    pub template: Vec<Template>,
    pub generators: Vec<Arc<Generator>>,
    pub load_spec: Option<LoadSpec>,
    pub filter_strategy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Filepaths,
    Folders,
    History,
    Help,
}

#[derive(Debug, Clone)]
pub enum Script {
    Shell(String),
    Argv(Vec<String>),
    Func(FnId),
}

#[derive(Debug, Clone)]
pub enum Trigger {
    /// Re-run when the token gains/loses any of these characters.
    Chars(String),
    Func(FnId),
    Change,
    Threshold(usize),
    Match(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum QueryTerm {
    Chars(String),
    Func(FnId),
}

#[derive(Debug, Clone, Default)]
pub struct Generator {
    /// Unique id, used as part of the result cache key.
    pub id: u64,
    pub script: Option<Script>,
    pub post_process: Option<FnId>,
    pub split_on: Option<String>,
    pub template: Vec<Template>,
    pub filter_template_suggestions: Option<FnId>,
    pub trigger: Option<Trigger>,
    pub get_query_term: Option<QueryTerm>,
    pub custom: Option<FnId>,
    pub cache_ttl_ms: Option<u64>,
    pub script_timeout_ms: Option<u64>,
}

static NEXT_GEN_ID: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------- parsing

pub fn func(v: &Value) -> Option<FnId> {
    v.get("__fn")?.as_u64().map(|n| n as FnId)
}

fn string(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
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
        icon: string(v, "icon"),
        priority: v.get("priority").and_then(Value::as_f64),
        hidden: flag(v, "hidden"),
    }
}

pub fn suggestion(v: &Value) -> Option<SuggestionDef> {
    match v {
        Value::String(s) => Some(SuggestionDef {
            meta: Meta { names: vec![s.clone()], ..Default::default() },
            kind: None,
        }),
        Value::Object(_) => {
            let meta = meta(v);
            if meta.names.is_empty() && meta.insert_value.is_none() && meta.display_name.is_none() {
                return None;
            }
            Some(SuggestionDef { meta, kind: string(v, "type") })
        }
        _ => None,
    }
}

pub fn suggestions(v: Option<&Value>) -> Vec<SuggestionDef> {
    one_or_many(v).into_iter().filter_map(suggestion).collect()
}

fn load_spec(v: Option<&Value>) -> Option<LoadSpec> {
    match v? {
        Value::String(s) => Some(LoadSpec::Name(s.clone())),
        o @ Value::Object(_) => match func(o) {
            Some(f) => Some(LoadSpec::Func(f)),
            None => Some(LoadSpec::Inline(Arc::new(subcommand(o)))),
        },
        _ => None,
    }
}

fn templates(v: Option<&Value>) -> Vec<Template> {
    names(v)
        .iter()
        .filter_map(|t| match t.as_str() {
            "filepaths" => Some(Template::Filepaths),
            "folders" => Some(Template::Folders),
            "history" => Some(Template::History),
            "help" => Some(Template::Help),
            _ => None,
        })
        .collect()
}

pub fn subcommand(v: &Value) -> Subcommand {
    let directives = v.get("parserDirectives").map(|d| Directives {
        flags_are_posix_noncompliant: flag(d, "flagsArePosixNoncompliant"),
        options_must_precede_arguments: flag(d, "optionsMustPrecedeArguments"),
        option_arg_separators: names(d.get("optionArgSeparators")),
    });
    Subcommand {
        meta: meta(v),
        subcommands: one_or_many(v.get("subcommands")).into_iter().map(|s| Arc::new(subcommand(s))).collect(),
        options: one_or_many(v.get("options")).into_iter().map(|o| Arc::new(opt(o))).collect(),
        args: one_or_many(v.get("args")).into_iter().map(|a| Arc::new(arg(a))).collect(),
        additional_suggestions: suggestions(v.get("additionalSuggestions")),
        load_spec: load_spec(v.get("loadSpec")),
        generate_spec: v.get("generateSpec").and_then(func),
        filter_strategy: string(v, "filterStrategy"),
        directives,
    }
}

fn opt(v: &Value) -> Opt {
    let separator = match (v.get("requiresSeparator"), v.get("requiresEquals")) {
        (Some(Value::String(s)), _) => Some(s.clone()),
        (Some(Value::Bool(true)), _) | (_, Some(Value::Bool(true))) => Some("=".into()),
        _ => None,
    };
    Opt {
        meta: meta(v),
        args: one_or_many(v.get("args")).into_iter().map(|a| Arc::new(arg(a))).collect(),
        is_persistent: flag(v, "isPersistent"),
        is_repeatable: matches!(v.get("isRepeatable"), Some(Value::Bool(true) | Value::Number(_))),
        separator,
        exclusive_on: names(v.get("exclusiveOn")),
    }
}

fn arg(v: &Value) -> Arg {
    Arg {
        name: string(v, "name"),
        description: string(v, "description"),
        is_optional: flag(v, "isOptional"),
        is_variadic: flag(v, "isVariadic"),
        options_can_break_variadic: v.get("optionsCanBreakVariadicArg").and_then(Value::as_bool).unwrap_or(true),
        is_command: flag(v, "isCommand"),
        suggestions: suggestions(v.get("suggestions")),
        template: templates(v.get("template")),
        generators: one_or_many(v.get("generators")).into_iter().map(|g| Arc::new(generator(g))).collect(),
        load_spec: load_spec(v.get("loadSpec")),
        filter_strategy: string(v, "filterStrategy"),
    }
}

fn generator(v: &Value) -> Generator {
    let script = match v.get("script") {
        Some(Value::String(s)) => Some(Script::Shell(s.clone())),
        Some(Value::Array(a)) => Some(Script::Argv(a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())),
        Some(o @ Value::Object(_)) => func(o).map(Script::Func),
        _ => None,
    };
    let trigger = match v.get("trigger") {
        Some(Value::String(s)) => Some(Trigger::Chars(s.clone())),
        Some(o @ Value::Object(_)) => match func(o) {
            Some(f) => Some(Trigger::Func(f)),
            None => match o.get("on").and_then(Value::as_str) {
                Some("threshold") => {
                    Some(Trigger::Threshold(o.get("length").and_then(Value::as_u64).unwrap_or(0) as usize))
                }
                Some("match") => Some(Trigger::Match(names(o.get("string")))),
                _ => Some(Trigger::Change),
            },
        },
        _ => None,
    };
    let get_query_term = match v.get("getQueryTerm") {
        Some(Value::String(s)) => Some(QueryTerm::Chars(s.clone())),
        Some(o @ Value::Object(_)) => func(o).map(QueryTerm::Func),
        _ => None,
    };
    Generator {
        id: NEXT_GEN_ID.fetch_add(1, Ordering::Relaxed),
        script,
        post_process: v.get("postProcess").and_then(func),
        split_on: string(v, "splitOn"),
        template: templates(v.get("template")),
        filter_template_suggestions: v.get("filterTemplateSuggestions").and_then(func),
        trigger,
        get_query_term,
        custom: v.get("custom").and_then(func),
        cache_ttl_ms: v.get("cache").and_then(|c| c.get("ttl")).and_then(Value::as_u64),
        script_timeout_ms: v.get("scriptTimeout").and_then(Value::as_u64),
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
