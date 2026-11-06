//! Runs beedle against the recorded output of the two scripts it replaces, and against the
//! scripts themselves while they are still in the repo.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const SPOTTER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/spotter/site");
const GOLDEN: &str = include_str!("golden/spotter-audit.json");
const STREAM: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"working"}]}}
not json
{"type":"result","subtype":"success","result":"Verdict: Can’t ship until a person acts\n\n### B1 · Blocker · Dead links\n\n### B15 · Minor · New\n","num_turns":7,"duration_ms":61499,"total_cost_usd":1.125}
"#;

fn beedle() -> Command {
    Command::new(env!("CARGO_BIN_EXE_beedle"))
}

fn script(name: &str) -> Option<Command> {
    let path = Path::new(REPO).join("review").join(name);
    let found = Command::new("node").arg("--version").output().is_ok();
    if !path.exists() || !found {
        eprintln!("skipped: no node, or review/{name} is gone");
        return None;
    }
    let mut command = Command::new("node");
    command.arg(path);
    Some(command)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn same_run(script: &Output, beedle: &Output) {
    assert_eq!(text(&beedle.stdout), text(&script.stdout));
    assert_eq!(beedle.status.code(), script.status.code());
}

#[test]
fn the_spotter_audit_matches_the_recorded_output() {
    let run = beedle()
        .args(["audit", "--dir", SPOTTER, "--offline", "--json"])
        .output()
        .unwrap();
    assert_eq!(text(&run.stdout), GOLDEN);
    assert_eq!(run.status.code(), Some(1));
}

#[test]
fn the_spotter_audit_matches_the_script() {
    for args in [
        vec!["--dir", SPOTTER, "--offline", "--json"],
        vec!["--dir", SPOTTER, "--offline"],
        vec!["--dir", SPOTTER, "--offline", "--json", "--exclude", "privacy"],
    ] {
        let Some(mut node) = script("audit.mjs") else {
            return;
        };
        let node = node.args(&args).output().unwrap();
        let rust = beedle().arg("audit").args(&args).output().unwrap();
        same_run(&node, &rust);
    }
}

fn write_site(dir: &Path, files: &[(&str, &str)]) {
    for (name, body) in files {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
}

#[test]
fn a_site_with_many_faults_gets_the_same_findings_from_both() {
    let head = r#"<title>Tool</title><meta name="description" content="Same"><link rel="canonical" href="/x">
<meta property="og:image" content="/missing.png"><script type="application/ld+json">{"@type":"VideoGame","sameAs":["https://x.dev/me#a"]}</script>"#;
    let sites: [&[(&str, String)]; 3] = [
        &[
            ("index.html", format!(r##"<html><head>{head}</head><body><img src="a.png"><a href="#">x</a><p>lorem ipsum</p><script src="https://cdn.posthog.com/a.js"></script></body></html>"##)),
            ("about/index.html", format!(r#"<html lang="en"><head>{head}</head><body><h1>A</h1><h1>B</h1><a href="https://x.dev/me">m</a></body></html>"#)),
            ("privacy/index.html", format!("<html><head>{head}</head><body><p>We do not use any analytics, cookies or tracking.</p></body></html>")),
            ("robots.txt", "User-agent: *\nDisallow: /\n".to_string()),
            ("sitemap.xml", "<urlset><url><loc>https://b.dev/gone</loc></url></urlset>".to_string()),
        ],
        &[("page.html", "<html><body>😀 no index</body></html>".to_string())],
        &[
            ("index.html", "<!doctype html><title></title><body><script src=\"flutter_bootstrap.js\"></script></body>".to_string()),
            ("500.html", "<html><body></body></html>".to_string()),
            ("robots.txt", "User-agent: GPTBot\nDisallow: /\n\nUser-agent: Googlebot\nDisallow: /\n".to_string()),
        ],
    ];
    for files in sites {
        let Some(mut node) = script("audit.mjs") else {
            return;
        };
        let dir = TempDir::new().unwrap();
        let files: Vec<(&str, &str)> = files.iter().map(|(n, b)| (*n, b.as_str())).collect();
        write_site(dir.path(), &files);
        let args = ["--dir", dir.path().to_str().unwrap(), "--offline", "--json"];
        let node = node.args(args).output().unwrap();
        let rust = beedle().arg("audit").args(args).output().unwrap();
        same_run(&node, &rust);
    }
}

#[test]
fn a_big_script_gets_the_same_gzip_size_from_both() {
    let Some(mut node) = script("audit.mjs") else {
        return;
    };
    let mut state = 1u64;
    let mut app = String::new();
    for _ in 0..40_000 {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        app.push_str(&format!("var a{:x}=b({});\n", state >> 44, state >> 60));
    }
    let dir = TempDir::new().unwrap();
    let page = r#"<html lang="en"><head><title>x</title></head><body><script src="/app.js"></script></body></html>"#;
    write_site(dir.path(), &[("index.html", page), ("app.js", &app)]);
    let args = ["--dir", dir.path().to_str().unwrap(), "--offline", "--json"];
    let node = node.args(args).output().unwrap();
    let rust = beedle().arg("audit").args(args).output().unwrap();
    assert!(text(&node.stdout).contains("KB of gzipped JS is over"));
    same_run(&node, &rust);
}

struct Run {
    args: Vec<String>,
    prompt: String,
    stdin: String,
    report: String,
    record: String,
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn fake_claude(dir: &Path) -> PathBuf {
    let path = dir.join("fake claude");
    fs::write(dir.join("stream"), STREAM).unwrap();
    let body = r#"#!/bin/sh
dir=$(dirname "$0")
next=0
for arg in "$@"; do
  printf '%s\n' "$arg" >> "$dir/args"
  [ "$next" = 1 ] && cp "$arg" "$dir/prompt" && next=0
  [ "$arg" = --system-prompt-file ] && next=1
done
cat > "$dir/stdin"
cat "$dir/stream"
"#;
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn review(mut command: Command, dir: &Path, skill_dir: &str) -> Run {
    let fake = fake_claude(dir);
    let brief = dir.join("brief-1.md");
    fs::write(
        &brief,
        "Scope: all\nHanded to a person: B1 · Blocker · Dead links · Owner: publish it\n\nReturn\n",
    )
    .unwrap();
    let extra = dir.join("the site");
    let output = command
        .env("BEEDLE_CLAUDE", fake)
        .arg("--brief")
        .arg(&brief)
        .arg("--effort")
        .arg("high")
        .arg("--add-dir")
        .arg(&extra)
        .output()
        .unwrap();
    let read = |name: &str| {
        let text = fs::read_to_string(dir.join(name)).unwrap();
        text.replace(dir.to_str().unwrap(), "<dir>")
            .replace(skill_dir, "<skill>")
    };
    let mut args: Vec<String> = read("args").lines().map(str::to_string).collect();
    args[4] = "<prompt file>".to_string();
    Run {
        args,
        prompt: read("prompt"),
        stdin: read("stdin"),
        report: read("review-1.md"),
        record: read("review-1.json"),
        stdout: text(&output.stdout).replace(dir.to_str().unwrap(), "<dir>"),
        stderr: text(&output.stderr).replace(dir.to_str().unwrap(), "<dir>"),
        code: output.status.code(),
    }
}

#[test]
fn a_review_with_a_fake_claude_matches_the_script() {
    let Some(node) = script("run.mjs") else {
        return;
    };
    let root = fs::canonicalize(REPO).unwrap();
    let root = root.to_str().unwrap();
    let (first, second) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let script = review(node, first.path(), root);

    let mut rust = beedle();
    rust.args(["review", "--skill-dir", root]);
    let rust = review(rust, second.path(), root);

    assert_eq!(rust.args, script.args);
    assert_eq!(rust.prompt, script.prompt);
    assert_eq!(rust.stdin, script.stdin);
    assert_eq!(rust.report, script.report);
    assert_eq!(rust.record, script.record);
    assert_eq!(rust.stdout, script.stdout);
    assert_eq!(rust.stderr, script.stderr);
    assert_eq!(rust.code, script.code);
    assert_eq!(rust.code, Some(0));
}
