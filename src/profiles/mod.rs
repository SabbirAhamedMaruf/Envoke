pub mod view;

use anyhow::{Result, bail};
use regex::Regex;
use std::{collections::BTreeMap, fs, path::Path, sync::LazyLock, time::SystemTime};

// dotenv 17's LINE pattern (lib/main.js), so a profile gives exactly the values dotenv, and so Vite and Next, would read.
static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:export\s+)?([A-Za-z0-9_.-]+)(?:\s*=\s*?|:\s+?)(\s*'(?:\\'|[^'])*'|\s*"(?:\\"|[^"])*"|\s*`(?:\\`|[^`])*`|[^#\r\n]+)?\s*(?:#.*)?$"#,
    )
    .expect("dotenv line pattern")
});

// Like dotenv.parse: never rejects a line it cannot read (it skips it), keeps the last of duplicate keys, expands nothing.
pub fn parse_env(text: &str) -> Result<BTreeMap<String, String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut vars = BTreeMap::new();
    for found in LINE.captures_iter(&text) {
        let key = found[1].to_owned();
        let raw = found.get(2).map_or("", |m| m.as_str()).trim();
        let mut value = match raw.chars().next() {
            Some(q @ ('\'' | '"' | '`')) if raw.len() >= 2 && raw.ends_with(q) => {
                raw[1..raw.len() - 1].to_owned()
            }
            _ => raw.to_owned(),
        };
        if raw.starts_with('"') {
            value = value.replace("\\n", "\n").replace("\\r", "\r");
        }
        if key.contains('\0') || value.contains('\0') {
            bail!("Environment variables cannot contain NUL bytes");
        }
        vars.insert(key, value);
    }
    Ok(vars)
}

// Size + modification time: a cheap way to notice an external edit without reading the file every second.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    pub fn of(path: &Path) -> Option<Self> {
        fs::metadata(path).ok().map(|m| Self {
            len: m.len(),
            modified: m.modified().ok(),
        })
    }
}
