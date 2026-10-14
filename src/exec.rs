//! Running generator commands with a timeout.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[derive(Debug, Default, Clone)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

pub enum Cmd<'a> {
    /// Run through `bash -c`.
    Shell(&'a str),
    /// Run directly.
    Argv(&'a [String]),
}

pub fn run(cmd: Cmd<'_>, cwd: &Path, env: &[(String, String)], timeout: Duration) -> Output {
    let mut c = match cmd {
        Cmd::Shell(s) => {
            let mut c = Command::new(if Path::new("/bin/bash").exists() { "/bin/bash" } else { "/bin/sh" });
            c.arg("-c").arg(s);
            c
        }
        Cmd::Argv(argv) => {
            let Some((prog, args)) = argv.split_first() else {
                return Output { status: 127, ..Default::default() };
            };
            let mut c = Command::new(prog);
            c.args(args);
            c
        }
    };
    if cwd.is_dir() {
        c.current_dir(cwd);
    }
    c.envs(env.iter().map(|(k, v)| (k, v)))
        .env("LIG_GENERATOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => return Output { stderr: e.to_string(), status: 127, ..Default::default() },
    };
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(1),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.wait();
                break 124;
            }
        }
    };
    let stdout = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
    Output { stdout, stderr, status }
}

/// Backend of Fig's `executeShellCommand`: accepts either a shell string or
/// `{command, args, cwd, env}` and answers `{stdout, stderr, status}`.
pub fn run_json(req: &str, cwd: &Path, base_env: &[(String, String)], timeout: Duration) -> String {
    let req: Value = serde_json::from_str(req).unwrap_or(Value::Null);
    let out = match &req {
        Value::String(s) => run(Cmd::Shell(s), cwd, base_env, timeout),
        Value::Object(o) => {
            let mut argv: Vec<String> = vec![o.get("command").and_then(Value::as_str).unwrap_or("").to_string()];
            if let Some(Value::Array(a)) = o.get("args") {
                argv.extend(a.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())));
            }
            let dir = o.get("cwd").and_then(Value::as_str).map(Path::new).unwrap_or(cwd);
            let mut env = base_env.to_vec();
            if let Some(Value::Object(e)) = o.get("env") {
                env.extend(e.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))));
            }
            run(Cmd::Argv(&argv), dir, &env, timeout)
        }
        _ => Output { status: 1, ..Default::default() },
    };
    json!({"stdout": out.stdout, "stderr": out.stderr, "status": out.status}).to_string()
}
