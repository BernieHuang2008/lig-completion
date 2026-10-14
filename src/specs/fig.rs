//! The specs converted from withfig/autocomplete (`specs/fig/`), embedded in
//! the binary by `build.rs` and inflated one at a time on first use.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use crate::spec::{self, Subcommand};

static ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fig_specs.bin"));

fn index() -> &'static HashMap<&'static str, (usize, usize)> {
    static INDEX: OnceLock<HashMap<&'static str, (usize, usize)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut map = HashMap::new();
        if !ARCHIVE.starts_with(b"LIGSPEC1") {
            return map;
        }
        let u32_at = |i: usize| u32::from_le_bytes(ARCHIVE[i..i + 4].try_into().unwrap()) as usize;
        let count = u32_at(8);
        let mut i = 12;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let len = u16::from_le_bytes([ARCHIVE[i], ARCHIVE[i + 1]]) as usize;
            let name = std::str::from_utf8(&ARCHIVE[i + 2..i + 2 + len]).unwrap_or("");
            i += 2 + len;
            entries.push((name, u32_at(i), u32_at(i + 4)));
            i += 8;
        }
        for (name, off, len) in entries {
            map.insert(name, (i + off, len));
        }
        map
    })
}

/// The spec for `name` (`git`, `aws/s3`, …).
pub fn load(name: &str) -> Option<Subcommand> {
    let &(off, len) = index().get(name)?;
    let mut json = Vec::new();
    flate2::read::DeflateDecoder::new(&ARCHIVE[off..off + len]).read_to_end(&mut json).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&json).ok()?;
    Some(spec::from_value(&v))
}

/// Top-level command names (no `aws/s3`-style sub-specs).
pub fn names() -> impl Iterator<Item = &'static str> {
    index().keys().copied().filter(|n| !n.contains('/'))
}

pub fn count() -> usize {
    index().len()
}
