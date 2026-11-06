use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const SPOTTER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/spotter/site");

fn beedle(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_beedle"))
        .arg("audit")
        .args(args)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn offline_makes_no_requests_and_still_fails_on_the_blank_screenshots() {
    let run = beedle(&["--dir", SPOTTER, "--offline", "--json"]);
    assert_eq!(run.status.code(), Some(1));
    let report = json(&run);
    let findings = report["findings"].as_array().unwrap();
    assert!(!findings.is_empty());
    for finding in findings {
        let message = finding["message"].as_str().unwrap().to_lowercase();
        assert!(!message.contains("link"), "{message}");
    }
}

#[test]
fn the_text_report_ends_with_the_totals() {
    let run = beedle(&["--dir", SPOTTER, "--offline"]);
    assert_eq!(run.status.code(), Some(1));
    let text = String::from_utf8(run.stdout).unwrap();
    assert!(text.contains("3 pages, 2 indexable, analytics: none, external links not checked\n"));
    assert!(text.contains("2 errors, "));
}

#[test]
fn every_exclude_is_kept_and_a_site_without_errors_exits_zero() {
    let site = TempDir::with_prefix("beedle site ").unwrap();
    write(site.path(), "index.html", "<html><body>home</body></html>");
    write(site.path(), "docs/index.html", "<html><body>docs</body></html>");
    write(site.path(), "blog/post/index.html", "<html><body>blog</body></html>");
    let dir = site.path().to_str().unwrap();
    let run = beedle(&[
        "--dir", dir, "--exclude", "/docs/", "--exclude", "blog", "--offline", "--json",
    ]);
    assert_eq!(json(&run)["pages"], 1);

    let clean = TempDir::new().unwrap();
    let hidden = r#"<html><head><meta name="robots" content="noindex"></head></html>"#;
    write(clean.path(), "index.html", hidden);
    write(clean.path(), "404.html", "");
    write(clean.path(), "robots.txt", "User-agent: *\nAllow: /\nSitemap: https://a.dev/s.xml\n");
    let run = beedle(&["--dir", clean.path().to_str().unwrap(), "--offline", "--json"]);
    let findings = json(&run)["findings"].clone();
    let errors = findings.as_array().unwrap().iter().filter(|f| f["level"] == "error");
    assert_eq!(errors.count(), 0, "{findings}");
    assert_eq!(run.status.code(), Some(0));
}

#[test]
fn a_missing_dir_prints_the_usage_and_exits_two() {
    for args in [vec![], vec!["--dir", "/no/such/folder"]] {
        let run = beedle(&args);
        assert_eq!(run.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&run.stderr).contains("usage: beedle audit --dir"));
    }
}

#[test]
fn an_unknown_option_exits_two() {
    assert_eq!(beedle(&["--nope"]).status.code(), Some(2));
}

#[test]
fn the_last_dir_wins_when_it_is_given_twice() {
    let run = beedle(&["--dir", "/no/such/folder", "--dir", SPOTTER, "--offline", "--json"]);
    assert_eq!(json(&run)["pages"], 3);
}
