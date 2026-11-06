use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
const SUCCESS: &str = r#"printf '%s\n' '{"type":"result","subtype":"success","result":"Verdict: Fix first\n\n### B1 · Blocker · Dead links\n","num_turns":3,"duration_ms":2500,"total_cost_usd":0.125}'"#;

struct Review {
    home: TempDir,
    fake: PathBuf,
}

impl Review {
    fn new(script: &str) -> Self {
        let home = TempDir::with_prefix("beedle review ").unwrap();
        let review = Self {
            fake: home.path().join("fake claude"),
            home,
        };
        let body = format!(
            r#"#!/bin/sh
dir=$(dirname "$0")
for arg in "$@"; do printf '%s\n' "$arg"; done > "$dir/args"
: > ran-here
cat > "$dir/stdin"
{script}
"#
        );
        fs::write(&review.fake, body).unwrap();
        fs::set_permissions(&review.fake, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            review.path("brief-1.md"),
            "Handed to a person: B1 · Blocker · Dead links · Owner: publish it\n\nReturn\n",
        )
        .unwrap();
        review
    }

    fn path(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_beedle"));
        command
            .arg("review")
            .arg("--brief")
            .arg(self.path("brief-1.md"))
            .arg("--skill-dir")
            .arg(REPO)
            .args(args)
            .env("BEEDLE_CLAUDE", &self.fake);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    fn record(&self) -> Value {
        serde_json::from_str(&self.read("review-1.json")).unwrap()
    }

    fn args(&self) -> Vec<String> {
        self.read("args").lines().map(str::to_string).collect()
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn is_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "timed out waiting for {what}");
        thread::sleep(POLL);
    }
}

fn pid_in(path: &Path) -> i32 {
    fs::read_to_string(path).unwrap().trim().parse().unwrap()
}

#[test]
fn a_clean_run_writes_both_files_and_one_summary_line() {
    let review = Review::new(SUCCESS);
    let run = review.run(&[]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));

    let out = review.path("review-1");
    let out = out.display();
    assert_eq!(
        String::from_utf8(run.stdout.clone()).unwrap(),
        format!("review: {out}.md · Fix first · 3 turns · 3s · $0.13\n")
    );
    assert_eq!(
        review.read("review-1.md"),
        "Verdict: Fix first\n\n### B1 · Blocker · Dead links\n"
    );
    assert_eq!(
        review.record(),
        json!({
            "brief": review.path("brief-1.md").to_str().unwrap(),
            "model": "opus",
            "ok": true,
            "turns": 3,
            "seconds": 3,
            "costUsd": 0.125,
            "error": null,
            "verdict": "Fix first",
            "repeated": ["B1"],
        })
    );
    assert_eq!(
        stderr(&run),
        "review repeated items handed to a person: B1\n"
    );
    assert_eq!(review.read("stdin"), review.read("brief-1.md"));
    assert!(review.path("ran-here").exists());
}

#[test]
fn the_launch_flags_stay_lean_and_every_add_dir_is_kept() {
    let review = Review::new(SUCCESS);
    let (first, second) = (review.path("first dir"), review.path("second dir"));
    review.run(&[
        "--model",
        "sonnet",
        "--effort",
        "high",
        "--add-dir",
        first.to_str().unwrap(),
        "--add-dir",
        second.to_str().unwrap(),
    ]);
    let args = review.args();
    let root = fs::canonicalize(REPO).unwrap();
    let mut expected = vec![
        "-p",
        "--model",
        "sonnet",
        "--system-prompt-file",
        &args[4],
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--output-format",
        "stream-json",
        "--verbose",
        "--tools",
        "Read,Glob,Grep,Bash",
        "--allowedTools",
        "Read,Glob,Grep,Bash",
        "--permission-mode",
        "dontAsk",
        "--add-dir",
        root.to_str().unwrap(),
        "--effort",
        "high",
        "--add-dir",
    ];
    expected.push(first.to_str().unwrap());
    expected.push("--add-dir");
    expected.push(second.to_str().unwrap());
    assert_eq!(args, expected);
}

#[test]
fn the_prompt_file_is_there_during_the_run_and_gone_after() {
    let review = Review::new(&format!(
        r#"next=0
for arg in "$@"; do
  [ "$next" = 1 ] && cp "$arg" "$dir/prompt" && next=0
  [ "$arg" = --system-prompt-file ] && next=1
done
{SUCCESS}"#
    ));
    review.run(&[]);
    assert!(review.read("prompt").starts_with("# Beedle reviewer\n"));
    assert!(!Path::new(&review.args()[4]).exists());
}

#[test]
fn a_missing_claude_binary_fails_with_a_record() {
    let review = Review::new("");
    let missing = review.path("no such claude");
    let run = review
        .command(&[])
        .env("BEEDLE_CLAUDE", &missing)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
    let record = review.record();
    assert_eq!(record["ok"], false);
    let error = record["error"].as_str().unwrap();
    assert!(error.ends_with("ENOENT"), "{error}");
    assert!(stderr(&run).starts_with("review failed: Error: spawn "));
    assert!(!review.path("review-1.md").exists());
}

#[test]
fn a_child_that_exits_zero_with_no_result_event_is_a_failure() {
    let review = Review::new("echo '{\"type\":\"system\"}'");
    let run = review.run(&[]);
    assert_eq!(run.status.code(), Some(1));
    let record = review.record();
    assert_eq!(record["ok"], false);
    assert_eq!(record["error"], "claude exited 0");
    assert!(run.stdout.is_empty());
}

#[test]
fn a_child_that_fails_after_a_result_is_a_failure_and_stderr_is_the_reason() {
    let review = Review::new(&format!("{SUCCESS}\necho '  it broke  ' >&2\nexit 3"));
    let run = review.run(&[]);
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(review.record()["error"], "it broke");
    assert!(review.path("review-1.md").exists());
}

#[test]
fn a_line_that_is_not_utf8_does_not_lose_the_result() {
    let review = Review::new(&format!(r"printf 'caf\351 \377\n'{}{SUCCESS}", "\n"));
    let run = review.run(&[]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert_eq!(review.record()["verdict"], "Fix first");
}

#[test]
fn a_brief_that_is_not_utf8_still_runs() {
    let review = Review::new(SUCCESS);
    fs::write(review.path("brief-1.md"), b"caf\xe9\n").unwrap();
    assert_eq!(review.run(&[]).status.code(), Some(0));
    assert_eq!(review.read("stdin"), "caf\u{FFFD}\n");
}

#[test]
fn the_out_folder_is_made_and_spaces_in_every_path_are_kept() {
    let review = Review::new(SUCCESS);
    let out = review.path("new folder/my review");
    let run = review.run(&["--out", out.to_str().unwrap()]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(review.path("new folder/my review.md").exists());
    assert!(review.path("new folder/my review.json").exists());
}

#[test]
fn timing_out_kills_the_whole_group_even_a_child_that_ignores_term() {
    let review = Review::new(
        r#"(trap '' TERM; echo $$ > /dev/null; sh -c 'trap "" TERM; echo $$ > "$0"; sleep 300' "$dir/stubborn") &
sleep 300"#,
    );
    let start = Instant::now();
    let run = review.run(&["--timeout", "0.05"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(start.elapsed() < WAIT);
    assert_eq!(review.record()["error"], "claude exited 1");
    let stubborn = pid_in(&review.path("stubborn"));
    wait_for("the stubborn child to die", || !is_alive(stubborn));
}

#[test]
fn a_timed_out_child_keeps_what_it_had_written() {
    let review = Review::new(&format!("{SUCCESS}\nsleep 300"));
    let run = review.run(&["--timeout", "0.05"]);
    assert_eq!(run.status.code(), Some(1));
    let record = review.record();
    assert_eq!(record["ok"], false);
    assert_eq!(record["verdict"], "Fix first");
}

#[test]
fn a_signal_to_beedle_stops_the_group() {
    let review = Review::new(
        r#"(echo $$ > /dev/null; sh -c 'echo $$ > "$0"; sleep 300' "$dir/child") &
sleep 300"#,
    );
    let mut beedle = review
        .command(&[])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for("the child to start", || {
        fs::read_to_string(review.path("child")).is_ok_and(|pid| !pid.trim().is_empty())
    });
    let child = pid_in(&review.path("child"));
    unsafe { libc::kill(beedle.id() as i32, libc::SIGTERM) };
    beedle.wait().unwrap();
    wait_for("the child to die", || !is_alive(child));
}

#[test]
fn the_skill_folder_defaults_to_the_one_under_home() {
    let review = Review::new(SUCCESS);
    let skills = review.path(".claude/skills");
    fs::create_dir_all(&skills).unwrap();
    std::os::unix::fs::symlink(fs::canonicalize(REPO).unwrap(), skills.join("beedle")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_beedle"));
    let run = command
        .arg("review")
        .arg("--brief")
        .arg(review.path("brief-1.md"))
        .env("BEEDLE_CLAUDE", &review.fake)
        .env("HOME", review.home.path())
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
}

#[test]
fn a_missing_skill_folder_is_said_plainly() {
    let review = Review::new(SUCCESS);
    let run = review.run(&["--skill-dir", "/no/such/skill"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr(&run).contains("no skill folder at /no/such/skill"));
    assert!(!review.path("args").exists());
}

#[test]
fn a_skill_folder_without_a_rule_file_is_said_plainly() {
    let review = Review::new(SUCCESS);
    let empty = review.path("empty skill");
    fs::create_dir(&empty).unwrap();
    let run = review.run(&["--skill-dir", empty.to_str().unwrap()]);
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr(&run).contains("review/SKILL.md"));
    assert!(!review.path("args").exists());
}

#[test]
fn no_brief_prints_the_usage_and_exits_two() {
    let run = Command::new(env!("CARGO_BIN_EXE_beedle"))
        .arg("review")
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(2));
    assert!(stderr(&run).contains("usage: beedle review --brief"));
}

#[test]
fn a_brief_that_cannot_be_read_exits_one() {
    let review = Review::new(SUCCESS);
    fs::remove_file(review.path("brief-1.md")).unwrap();
    assert_eq!(review.run(&[]).status.code(), Some(1));
    assert!(!review.path("args").exists());
}
