mod child_env;
mod launcher;
mod report;
mod serve;
mod site_checks;
#[cfg(test)]
mod tests;
mod usage_log;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Args as ClapArgs;
use serde_json::{Value, json};

use crate::js;
use report::{Summary, read_report, summarise};

const TOOLS: &str = "Read,Glob,Grep,Bash";
const ALLOWED: &str = "Read,Glob,Grep,Bash(beedle audit *),Bash(beedle fetch *)";
const LEAN_ARGS: &[&str] = &[
    "--setting-sources",
    "",
    "--strict-mcp-config",
    "--disable-slash-commands",
    "--no-session-persistence",
    "--output-format",
    "stream-json",
    "--verbose",
    "--tools",
    TOOLS,
    "--allowedTools",
    ALLOWED,
    "--permission-mode",
    "dontAsk",
];
const FILES: &[&str] = &[
    "review/method.md",
    "review/checklist.md",
    "reference/page-types.md",
    "reference/message.md",
    "reference/seo.md",
];
const USAGE: &str =
    "usage: beedle review --brief <file> [--out <path>] [--add-dir <dir>]... [--skip-site-checks]";

#[derive(ClapArgs)]
#[command(args_override_self = true)]
pub struct Args {
    /// The brief, which becomes the reviewer's user message
    #[arg(long, allow_hyphen_values = true)]
    brief: Option<PathBuf>,
    /// Where to write <out>.md and <out>.json. Defaults to beside the brief
    #[arg(long, allow_hyphen_values = true)]
    out: Option<PathBuf>,
    /// A folder the reviewer may read. Repeat for more
    #[arg(long = "add-dir", allow_hyphen_values = true)]
    add_dirs: Vec<PathBuf>,
    /// The model to review with
    #[arg(long, default_value = "opus", allow_hyphen_values = true)]
    model: String,
    /// The reasoning effort to review at
    #[arg(long, default_value = "", allow_hyphen_values = true)]
    effort: String,
    /// Minutes before the review is stopped
    #[arg(long, default_value = "20", value_parser = minutes)]
    timeout: Duration,
    /// The beedle skill folder. Defaults to ~/.claude/skills/beedle
    #[arg(long, allow_hyphen_values = true)]
    skill_dir: Option<PathBuf>,
    /// Don't run lighthouse, pa11y and linkinator. The reviewer lists them under Not checked
    #[arg(long)]
    skip_site_checks: bool,
}

fn minutes(text: &str) -> Result<Duration, String> {
    let value: f64 = text
        .parse()
        .map_err(|_| format!("{text} is not a number"))?;
    Duration::try_from_secs_f64(value * 60.0).map_err(|error| error.to_string())
}

fn skill_dir(given: Option<&Path>) -> Result<PathBuf, String> {
    let dir = match given {
        Some(dir) => dir.to_path_buf(),
        None => env::home_dir()
            .ok_or("no home folder")?
            .join(".claude/skills/beedle"),
    };
    dir.canonicalize()
        .map_err(|error| format!("no skill folder at {}: {error}", dir.display()))
}

fn system_prompt(root: &Path) -> Result<String, String> {
    let mut parts = vec![
        "# Beedle reviewer".to_string(),
        String::new(),
        "You review marketing pages, with no conversation before this one. The review method,".to_string(),
        "its checklist and the rules are below: follow the method. Wherever they say".to_string(),
        format!(
            "~/.claude/skills/beedle/, use {}/ instead. Skills can't be loaded here; this is the",
            root.display()
        ),
        "whole of it. The user message is the brief. Answer with the report only.".to_string(),
        String::new(),
        "You can't edit, build or serve anything. There's no Write or Edit tool, and Bash runs only".to_string(),
        "`beedle audit` and `beedle fetch`, each as one plain command with no pipes, redirects or".to_string(),
        "`&&`. Everything else is denied, so don't retry it. Whatever the rules below show, run the".to_string(),
        "audit as `beedle audit --dir <built site>`, and check a live URL with `beedle fetch <url>`,".to_string(),
        "or `beedle fetch --head <url>` for just the status and headers. It shows redirects without".to_string(),
        "following them. A check that needs anything else goes under *Not checked*, with the reason.".to_string(),
    ];
    for rel in FILES {
        let path = root.join(rel);
        let bytes =
            fs::read(&path).map_err(|error| format!("can't read {}: {error}", path.display()))?;
        parts.push(String::new());
        parts.push(format!("<file path=\"{rel}\">"));
        parts.push(js::trim_end(&String::from_utf8_lossy(&bytes)).to_string());
        parts.push("</file>".to_string());
    }
    Ok(parts.join("\n") + "\n")
}

fn claude_args(
    system: &Path,
    model: &str,
    effort: &str,
    add_dirs: &[PathBuf],
    root: &Path,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "-p".into(),
        "--model".into(),
        model.into(),
        "--system-prompt-file".into(),
        system.into(),
    ];
    args.extend(LEAN_ARGS.iter().map(OsString::from));
    args.extend(["--add-dir".into(), root.into()]);
    if !effort.is_empty() {
        args.extend(["--effort".into(), effort.into()]);
    }
    for dir in add_dirs {
        args.extend(["--add-dir".into(), dir.into()]);
    }
    args
}

fn default_out(brief: &Path) -> PathBuf {
    let file = brief.file_name().unwrap_or_default().to_string_lossy();
    let name = file.strip_suffix(".md").unwrap_or(&file);
    let out = match name.strip_prefix("brief") {
        Some(rest) => format!("review{rest}"),
        None => format!("{name}.review"),
    };
    brief.parent().unwrap_or(Path::new("")).join(out)
}

fn program() -> OsString {
    env::var_os("BEEDLE_CLAUDE")
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "claude".into())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn text_of(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

fn write_results(
    out: &Path,
    brief_path: &Path,
    model: &str,
    summary: &Summary,
    verdict: Option<&str>,
    repeated: &[String],
) -> Result<(), String> {
    let failed = |path: &Path, error: io::Error| format!("can't write {}: {error}", path.display());
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|error| failed(parent, error))?;
    }
    if !summary.report.is_empty() {
        let path = with_suffix(out, ".md");
        let text = format!("{}\n", js::trim_end(&summary.report));
        fs::write(&path, text).map_err(|error| failed(&path, error))?;
    }
    let record = json!({
        "brief": js::resolve(brief_path).to_string_lossy(),
        "model": model,
        "ok": summary.ok,
        "turns": summary.turns,
        "seconds": summary.seconds,
        "costUsd": summary.cost_usd,
        "error": summary.error,
        "verdict": verdict,
        "repeated": repeated,
    });
    let path = with_suffix(out, ".json");
    let text = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
    fs::write(&path, text + "\n").map_err(|error| failed(&path, error))
}

fn run_site_checks(
    brief: &str,
    base: &Path,
    skip: bool,
    out: &Path,
    results: &Path,
    env: &child_env::Vars,
) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("can't write {}: {error}", parent.display()))?;
    }
    let site = site_checks::site_to_check(brief, base, skip);
    site_checks::write_results(site, results, &with_suffix(out, ".lighthouse.json"), env)
}

fn review(args: &Args, brief_path: &Path) -> Result<u8, String> {
    let brief_bytes = fs::read(brief_path)
        .map_err(|error| format!("can't read {}: {error}", brief_path.display()))?;
    let brief = String::from_utf8_lossy(&brief_bytes);
    let out = js::resolve(&args.out.clone().unwrap_or_else(|| default_out(brief_path)));
    let root = skill_dir(args.skill_dir.as_deref())?;
    let add_dirs: Vec<PathBuf> = args.add_dirs.iter().map(|dir| js::resolve(dir)).collect();

    let mut system = tempfile::Builder::new()
        .prefix("beedle-review-")
        .suffix(".md")
        .tempfile()
        .map_err(|error| format!("can't make the prompt file: {error}"))?;
    system
        .write_all(system_prompt(&root)?.as_bytes())
        .map_err(|error| format!("can't write the prompt file: {error}"))?;
    let claude = claude_args(system.path(), &args.model, &args.effort, &add_dirs, &root);

    let cwd = js::resolve(brief_path);
    let cwd = cwd.parent().unwrap_or(Path::new("/"));
    let env = child_env::current();
    let results = with_suffix(&out, ".site-checks.md");
    run_site_checks(&brief, cwd, args.skip_site_checks, &out, &results, &env)?;
    let message = format!("{brief}\n\nPass 8 results: {}\n", results.display());
    let launched = launcher::run(
        &program(),
        &claude,
        cwd,
        message.as_bytes(),
        args.timeout,
        &child_env::for_claude(&env),
    );

    let mut summary = summarise(&launched.lines);
    if launched.code != 0 {
        summary.ok = false;
    }
    if !summary.ok && !js::truthy(&summary.error) {
        let fallback = format!("claude exited {}", launched.code);
        let text = if launched.stderr.is_empty() {
            &fallback
        } else {
            &launched.stderr
        };
        summary.error = Value::from(js::utf16_slice(js::trim(text), 0, 500));
    }
    let read = read_report(&summary.report, &brief);
    usage_log::record(
        brief_path,
        &args.model,
        &args.effort,
        &summary,
        read.verdict.as_deref(),
    );
    write_results(
        &out,
        brief_path,
        &args.model,
        &summary,
        read.verdict.as_deref(),
        &read.repeated,
    )?;
    report_to_terminal(&summary, read.verdict.as_deref(), &read.repeated, &out);
    Ok(if summary.ok { 0 } else { 1 })
}

fn report_to_terminal(summary: &Summary, verdict: Option<&str>, repeated: &[String], out: &Path) {
    let mut stderr = io::stderr();
    if summary.ok && verdict.is_none() {
        let _ = writeln!(stderr, "review has no verdict line");
    }
    if !repeated.is_empty() {
        let items = repeated.join(", ");
        let _ = writeln!(stderr, "review repeated items handed to a person: {items}");
    }
    let out = out.display();
    if summary.ok {
        let verdict = verdict.unwrap_or("no verdict");
        let turns = text_of(&summary.turns);
        let seconds = summary.seconds;
        let cost = js::to_fixed_2(summary.cost_usd.as_f64().unwrap_or(0.0));
        let _ = writeln!(
            io::stdout(),
            "review: {out}.md · {verdict} · {turns} turns · {seconds}s · ${cost}"
        );
    } else {
        let error = text_of(&summary.error);
        let _ = writeln!(stderr, "review failed: {error} ({out}.json)");
    }
}

pub fn run(args: Args) -> u8 {
    let Some(brief) = args.brief.clone() else {
        eprintln!("{USAGE}");
        return 2;
    };
    review(&args, &brief).unwrap_or_else(|error| {
        eprintln!("{error}");
        1
    })
}
