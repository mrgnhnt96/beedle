use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::report::Summary;
use crate::js;

const FILE: &str = "beedle/usage.jsonl";

/// `$XDG_DATA_HOME/beedle/usage.jsonl`, or the same under `~/.local/share`. Never in a project.
fn path() -> Option<PathBuf> {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| Some(env::home_dir()?.join(".local/share")))?;
    Some(data.join(FILE))
}

fn entry(
    brief: &Path,
    model: &str,
    effort: &str,
    summary: &Summary,
    verdict: Option<&str>,
) -> Value {
    let at = SystemTime::now().duration_since(UNIX_EPOCH);
    json!({
        "at": at.map_or(0, |since| since.as_secs()),
        "brief": js::resolve(brief).to_string_lossy(),
        "model": model,
        "effort": effort,
        "ok": summary.ok,
        "verdict": verdict,
        "turns": summary.turns,
        "seconds": summary.seconds,
        "costUsd": summary.cost_usd,
        "tokens": summary.tokens,
    })
}

fn append(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())
}

/// Adds one line per review. A review that ran still counts if this can't be written.
pub fn record(brief: &Path, model: &str, effort: &str, summary: &Summary, verdict: Option<&str>) {
    let Some(path) = path() else {
        eprintln!("can't log usage: no home folder");
        return;
    };
    let line = entry(brief, model, effort, summary, verdict).to_string() + "\n";
    if let Err(error) = append(&path, &line) {
        eprintln!("can't log usage to {}: {error}", path.display());
    }
}
