//! Locating, listing and downloading completion specs.
//!
//! Lookup order:
//!   1. `~/.config/lig/specs/`   — your own specs (`name.js` or `name.json`)
//!   2. `~/.local/share/lig/specs/` — `@withfig/autocomplete` from npm

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

pub const NPM_PACKAGE: &str = "@withfig/autocomplete";

pub fn user_dir() -> PathBuf {
    crate::config::config_dir().join("specs")
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share")).join("lig").join("specs")
}

pub enum Source {
    Js(String),
    Json(String),
}

#[derive(Clone)]
pub struct Store {
    dirs: Vec<PathBuf>,
}

impl Default for Store {
    fn default() -> Self {
        Store { dirs: vec![user_dir(), data_dir()] }
    }
}

fn safe(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name).components().all(|c| matches!(c, Component::Normal(_)))
}

impl Store {
    #[cfg(test)]
    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        Store { dirs }
    }

    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if !safe(name) {
            return None;
        }
        for d in &self.dirs {
            for cand in [d.join(format!("{name}.js")), d.join(format!("{name}.json")), d.join(name).join("index.js")] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
        None
    }

    pub fn read(&self, name: &str) -> Option<Source> {
        let path = self.find(name)?;
        let text = fs::read_to_string(&path).ok()?;
        Some(if path.extension().is_some_and(|e| e == "json") { Source::Json(text) } else { Source::Js(text) })
    }

    /// Top-level command names that have a spec.
    pub fn commands(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for d in &self.dirs {
            if let Ok(text) = fs::read_to_string(d.join("index.json"))
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    for n in v["completions"].as_array().into_iter().flatten() {
                        if let Some(n) = n.as_str().filter(|n| !n.contains('/')) {
                            out.insert(n.to_string());
                        }
                    }
                }
            for e in fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if let (Some(stem), Some(ext)) = (p.file_stem(), p.extension())
                    && (ext == "js" || ext == "json") && stem != "index" {
                        out.insert(stem.to_string_lossy().into_owned());
                    }
            }
        }
        out
    }

    pub fn installed_version(&self) -> Option<String> {
        fs::read_to_string(data_dir().join("VERSION")).ok().map(|s| s.trim().to_string())
    }
}

fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>> {
    ureq::get(url).header("User-Agent", concat!("lig/", env!("CARGO_PKG_VERSION"))).call().with_context(|| format!("GET {url}"))
}

/// Download `@withfig/autocomplete` (latest or `version`) from the npm
/// registry and install its `build/` directory as the spec data dir.
pub fn update(version: Option<&str>) -> Result<String> {
    let meta_url = format!("https://registry.npmjs.org/{NPM_PACKAGE}/{}", version.unwrap_or("latest"));
    let meta: serde_json::Value = serde_json::from_str(&get(&meta_url)?.body_mut().read_to_string()?)?;
    let version = meta["version"].as_str().context("no version in npm metadata")?.to_string();
    let tarball = meta["dist"]["tarball"].as_str().context("no tarball in npm metadata")?;
    if !tarball.starts_with("https://registry.npmjs.org/") {
        bail!("unexpected tarball location {tarball}");
    }
    eprintln!("lig: downloading {NPM_PACKAGE}@{version} …");
    let resp = get(tarball)?;
    let reader = resp.into_body().into_with_config().limit(512 * 1024 * 1024).reader();

    let dest = data_dir();
    let parent = dest.parent().unwrap().to_path_buf();
    fs::create_dir_all(&parent)?;
    let staging = parent.join("specs.new");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut count = 0usize;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?.into_owned();
        let Ok(rel) = path.strip_prefix("package/build") else { continue };
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            continue;
        }
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "js" && ext != "json" {
            continue;
        }
        let out = staging.join(rel);
        fs::create_dir_all(out.parent().unwrap())?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        fs::write(&out, buf)?;
        count += 1;
    }
    if count == 0 {
        bail!("the package contained no specs");
    }
    fs::write(staging.join("VERSION"), &version)?;
    let old = parent.join("specs.old");
    let _ = fs::remove_dir_all(&old);
    if dest.exists() {
        fs::rename(&dest, &old)?;
    }
    fs::rename(&staging, &dest)?;
    let _ = fs::remove_dir_all(&old);
    eprintln!("lig: installed {count} specs into {}", dest.display());
    Ok(version)
}
