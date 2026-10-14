//! Embedded QuickJS host for Fig completion specs.
//!
//! Specs are compiled ES modules (`@withfig/autocomplete/build/*.js`). They are
//! evaluated inside a function scope and serialized to JSON; every JS function
//! inside a spec (generators, postProcess, loadSpec, …) is kept alive in a
//! registry and replaced by `{"__fn": id}` so Rust can call it back later.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use rquickjs::{Context, Ctx, Function, Runtime};
use serde_json::Value;

use crate::exec;

const PRELUDE: &str = r#"
globalThis.__fns = [];
globalThis.__ser = function (v) {
  return JSON.stringify(v === undefined ? null : v, function (k, x) {
    if (typeof x === "function") { __fns.push(x); return { __fn: __fns.length - 1 }; }
    if (x instanceof RegExp) return String(x);
    return x;
  });
};
globalThis.__exec = function (x) {
  const r = JSON.parse(__lig_exec(JSON.stringify(x)));
  return Promise.resolve(typeof x === "string" ? r.stdout : r);
};
globalThis.__call = function (id, args) {
  globalThis.__done = false; globalThis.__out = null; globalThis.__err = null;
  const a = JSON.parse(args).map(function (x) { return x === "\u0000EXEC" ? __exec : x; });
  Promise.resolve()
    .then(function () { return __fns[id].apply(null, a); })
    .then(function (v) { __out = __ser(v); __done = true; },
          function (e) { __err = String((e && e.stack) || e); __done = true; });
};
globalThis.console = { log() {}, warn() {}, error() {}, info() {}, debug() {} };
"#;

/// Sentinel argument replaced by `executeShellCommand` on the JS side.
pub const EXEC: &str = "\u{0}EXEC";

pub type FnId = u32;

/// Where and with which extra variables `executeShellCommand` runs.
#[derive(Debug, Clone, Default)]
pub struct ExecEnv {
    pub cwd: PathBuf,
    pub vars: Vec<(String, String)>,
}

pub struct Js {
    rt: Runtime,
    ctx: Context,
    env: Rc<RefCell<ExecEnv>>,
    deadline: Rc<Cell<Instant>>,
    budget: Duration,
}

impl Js {
    pub fn new(exec_timeout: Duration) -> Result<Self> {
        let rt = Runtime::new()?;
        rt.set_memory_limit(512 * 1024 * 1024);
        let deadline = Rc::new(Cell::new(Instant::now() + Duration::from_secs(3600)));
        let d = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > d.get())));
        let ctx = Context::full(&rt)?;
        let env = Rc::new(RefCell::new(ExecEnv::default()));
        let js = Js { rt, ctx, env, deadline, budget: Duration::from_secs(10) };
        let env = js.env.clone();
        js.ctx.with(|ctx| -> Result<()> {
            let f = Function::new(ctx.clone(), move |req: String| -> String {
                let env = env.borrow();
                exec::run_json(&req, &env.cwd, &env.vars, exec_timeout)
            })?;
            ctx.globals().set("__lig_exec", f)?;
            ctx.eval::<(), _>(PRELUDE).map_err(|e| js_err(&ctx, e))?;
            Ok(())
        })?;
        Ok(js)
    }

    pub fn set_env(&self, env: ExecEnv) {
        *self.env.borrow_mut() = env;
    }

    fn arm(&self) {
        self.deadline.set(Instant::now() + self.budget);
    }

    /// Evaluate a compiled spec module and return its serialized exports
    /// (`{"default": …, "getVersionCommand": …}`).
    pub fn load_module(&self, source: &str) -> Result<Value> {
        let wrapped = wrap_module(source).ok_or_else(|| anyhow!("not an ES module spec"))?;
        self.arm();
        let json = self.ctx.with(|ctx| -> Result<String> {
            ctx.eval::<String, _>(format!("__ser({wrapped})")).map_err(|e| js_err(&ctx, e))
        })?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Call a registered spec function; `args` may contain [`EXEC`].
    /// Promises are awaited.
    pub fn call(&self, f: FnId, args: &[Value]) -> Result<Value> {
        let args = serde_json::to_string(args)?;
        self.arm();
        self.ctx.with(|ctx| -> Result<()> {
            let call: Function = ctx.globals().get("__call")?;
            call.call::<_, ()>((f, args)).map_err(|e| js_err(&ctx, e))
        })?;
        while Instant::now() < self.deadline.get() {
            match self.rt.execute_pending_job() {
                Ok(true) | Err(_) => continue,
                Ok(false) => break,
            }
        }
        self.ctx.with(|ctx| -> Result<Value> {
            let g = ctx.globals();
            if !g.get::<_, bool>("__done")? {
                bail!("spec function did not settle");
            }
            if let Ok(err) = g.get::<_, String>("__err") {
                bail!("spec function threw: {err}");
            }
            let out: String = g.get("__out")?;
            Ok(serde_json::from_str(&out)?)
        })
    }
}

fn js_err(ctx: &Ctx<'_>, e: rquickjs::Error) -> anyhow::Error {
    if matches!(e, rquickjs::Error::Exception) {
        let v = ctx.catch();
        if let Some(ex) = v.as_exception() {
            return anyhow!("js: {}", ex.message().unwrap_or_default());
        }
        return anyhow!("js: {v:?}");
    }
    anyhow!("js: {e}")
}

/// Turn esbuild ESM output (`…;export{a as default,b as getVersionCommand};`)
/// into an expression evaluating to the exports object, in its own scope.
pub fn wrap_module(src: &str) -> Option<String> {
    let start = src.rfind("export{")?;
    let end = start + src[start..].find('}')?;
    let mut obj = String::new();
    for entry in src[start + 7..end].split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (local, exported) = match entry.split_once(" as ") {
            Some((l, e)) => (l.trim(), e.trim()),
            None => (entry, entry),
        };
        obj.push_str(&format!("{exported:?}:{local},"));
    }
    Some(format!("(function(){{\n{}\n;return {{{obj}}};\n}})()", &src[..start]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_roundtrip_and_calls() {
        let js = Js::new(Duration::from_secs(5)).unwrap();
        let src = r#"var e={name:"x",args:{generators:{script:["echo","hi"],postProcess:o=>o.trim().split(" ").map(n=>({name:n}))}}},t=async(x)=>"v"+x;export{e as default,t as getVersionCommand};"#;
        let v = js.load_module(src).unwrap();
        assert_eq!(v["default"]["name"], "x");
        let pp = v["default"]["args"]["generators"]["postProcess"]["__fn"].as_u64().unwrap() as FnId;
        let out = js.call(pp, &[Value::from(" a b "), Value::Array(vec![])]).unwrap();
        assert_eq!(out, serde_json::json!([{"name": "a"}, {"name": "b"}]));
        let vc = v["getVersionCommand"]["__fn"].as_u64().unwrap() as FnId;
        assert_eq!(js.call(vc, &[Value::from(1)]).unwrap(), Value::from("v1"));
    }

    #[test]
    fn exec_from_js() {
        let js = Js::new(Duration::from_secs(5)).unwrap();
        let src = r#"var c=async(t,x)=>{let r=await x({command:"echo",args:["hello"]});let s=await x("echo world");return [r.stdout.trim(),s.trim()]};export{c as default};"#;
        let v = js.load_module(src).unwrap();
        let id = v["default"]["__fn"].as_u64().unwrap() as FnId;
        let out = js.call(id, &[Value::Array(vec![]), Value::from(EXEC)]).unwrap();
        assert_eq!(out, serde_json::json!(["hello", "world"]));
    }
}
