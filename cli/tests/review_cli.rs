use std::fs;
use std::net::TcpStream;
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
env > "$dir/env"
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
            .env("BEEDLE_CLAUDE", &self.fake)
            .env("XDG_DATA_HOME", self.path("data"));
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

const FAKE_NPX: &str = r#"#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/npx-calls"
case "$2" in
  lighthouse)
    beedle fetch "$3" > "$dir/served"
    for arg in "$@"; do
      case "$arg" in
        --output-path=*)
          printf '%s' '{"categories":{"seo":{"score":0.92}},"audits":{"image-alt":{"score":0,"title":"Images lack alt"}}}' > "${arg#--output-path=}"
          ;;
      esac
    done
    ;;
  pa11y)
    echo 'Error: a heading is empty'
    exit 2
    ;;
  linkinator)
    echo '[200] http://localhost/'
    echo '[404] http://localhost/gone'
    echo 'ERROR: Detected 1 broken links.' >&2
    exit 1
    ;;
esac
"#;
const CRASHING_NPX: &str = "#!/bin/sh\necho 'npm ERR! it broke' >&2\nexit 1\n";

impl Review {
    fn with_site(script: &str, npx: &str) -> Self {
        let review = Self::new(script);
        let site = review.path("my site");
        fs::create_dir(&site).unwrap();
        fs::write(site.join("index.html"), "<h1>Served</h1>").unwrap();
        fs::write(
            review.path("brief-1.md"),
            format!("Built site: {}\n\nReturn\n", site.display()),
        )
        .unwrap();
        let tool = review.path("npx");
        fs::write(&tool, npx).unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        review
    }

    fn run_with_npx(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.home.path().display(),
            std::env::var("PATH").unwrap()
        );
        self.command(args).env("PATH", path).output().unwrap()
    }

    fn results(&self) -> String {
        self.read("review-1.site-checks.md")
    }
}

#[test]
fn the_site_checks_run_before_the_reviewer_with_the_checklists_arguments() {
    let review = Review::with_site(SUCCESS, FAKE_NPX);
    let run = review.run_with_npx(&[]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));

    let calls = review.read("npx-calls");
    let calls: Vec<&str> = calls.lines().collect();
    let url = calls[0].split(' ').nth(2).unwrap();
    let report = review.path("review-1.lighthouse.json");
    assert_eq!(
        calls,
        [
            format!(
                "--yes lighthouse {url} --quiet --chrome-flags=--headless --only-categories=performance,accessibility,best-practices,seo --output=json --output-path={}",
                report.display()
            ),
            format!("--yes pa11y {url}"),
            format!("--yes linkinator {url} --recurse --skip ^https?://(?!localhost)"),
        ]
    );
    assert!(url.starts_with("http://localhost:"), "{url}");
    assert!(review.read("served").contains("<h1>Served</h1>"));

    let results = review.results();
    assert!(
        results.contains("- image-alt · Images lack alt"),
        "{results}"
    );
    assert!(results.contains("Error: a heading is empty"), "{results}");
    assert!(results.contains("[404] http://localhost/gone"), "{results}");
    assert!(!results.contains("[200]"), "{results}");
    assert!(!results.contains("Not run"), "{results}");
    assert!(review.read("stdin").contains("review-1.site-checks.md"));

    let port = url.rsplit(':').next().unwrap();
    assert!(TcpStream::connect(format!("127.0.0.1:{port}")).is_err());
}

#[test]
fn a_tool_that_fails_is_written_down_as_not_run_and_the_review_goes_on() {
    let review = Review::with_site(SUCCESS, CRASHING_NPX);
    let run = review.run_with_npx(&[]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    let results = review.results();
    assert_eq!(results.matches("Not run: exit 1").count(), 3, "{results}");
    assert!(results.contains("npm ERR! it broke"), "{results}");
    assert_eq!(review.record()["verdict"], "Fix first");
}

#[test]
fn the_skip_flag_runs_no_tool_and_says_so() {
    let review = Review::with_site(SUCCESS, FAKE_NPX);
    let run = review.run_with_npx(&["--skip-site-checks"]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(!review.path("npx-calls").exists());
    let results = review.results();
    let reason = "Not run: skipped with --skip-site-checks";
    assert_eq!(results.matches(reason).count(), 3, "{results}");
    assert!(review.read("stdin").contains("review-1.site-checks.md"));
}

#[test]
fn a_brief_with_no_built_site_runs_no_tool_and_says_so() {
    let review = Review::with_site(SUCCESS, FAKE_NPX);
    fs::write(review.path("brief-1.md"), "Return\n").unwrap();
    review.run_with_npx(&[]);
    assert!(!review.path("npx-calls").exists());
    let reason = "Not run: the brief names no built site";
    assert_eq!(review.results().matches(reason).count(), 3);
}

#[test]
fn the_child_keeps_auth_and_drops_session_and_messaging_variables() {
    let review = Review::new(SUCCESS);
    let run = review
        .command(&[])
        .env("ANTHROPIC_API_KEY", "kept")
        .env("MIDNA_SESSION", "dropped")
        .env("CLAUDE_CODE_SESSION_ID", "dropped")
        .env("CLAUDE_CODE_MESSAGING_TOKEN", "dropped")
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    let env = review.read("env");
    assert!(env.contains("ANTHROPIC_API_KEY=kept"), "{env}");
    assert!(env.contains("HOME="), "{env}");
    assert!(!env.contains("dropped"), "{env}");
}

#[test]
fn the_reviewer_uses_the_five_minute_cache() {
    let review = Review::new(SUCCESS);
    review.run(&[]);
    assert!(review.read("env").contains("FORCE_PROMPT_CACHING_5M=1\n"));
}

#[test]
fn each_review_adds_a_line_to_the_usage_log() {
    let usage = r#"printf '%s\n' '{"type":"result","subtype":"success","result":"Verdict: Fix first\n","num_turns":3,"duration_ms":2500,"total_cost_usd":0.125,"usage":{"input_tokens":10,"output_tokens":200,"cache_read_input_tokens":3000,"cache_creation":{"ephemeral_5m_input_tokens":400,"ephemeral_1h_input_tokens":0}},"modelUsage":{"opus":{"thinkingTokens":50}}}'"#;
    let review = Review::new(usage);
    review.run(&["--effort", "low"]);
    review.run(&[]);

    let log = review.read("data/beedle/usage.jsonl");
    let lines: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "{log}");
    assert!(lines[0]["at"].as_u64().unwrap() > 0);
    assert_eq!(lines[0]["effort"], "low");
    assert_eq!(lines[0]["brief"], review.path("brief-1.md").to_str().unwrap());
    assert_eq!(lines[0]["verdict"], "Fix first");
    assert_eq!(lines[0]["costUsd"], 0.125);
    assert_eq!(
        lines[0]["tokens"],
        json!({
            "input": 10,
            "output": 200,
            "thinking": 50,
            "cacheRead": 3000,
            "cacheWrite5m": 400,
            "cacheWrite1h": 0,
        })
    );
}

#[test]
fn the_usage_log_defaults_to_the_home_data_folder() {
    let review = Review::new(SUCCESS);
    let run = review
        .command(&[])
        .env_remove("XDG_DATA_HOME")
        .env("HOME", review.home.path())
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(review.path(".local/share/beedle/usage.jsonl").is_file());
}

#[test]
fn a_usage_log_that_cannot_be_written_does_not_fail_the_review() {
    let review = Review::new(SUCCESS);
    fs::write(review.path("data"), "a file, not a folder").unwrap();
    let run = review.run(&[]);
    assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
    assert!(stderr(&run).contains("can't log usage to"));
    assert!(review.path("review-1.md").exists());
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
    let results = review.path("review-1.site-checks.md");
    assert_eq!(
        review.read("stdin"),
        format!(
            "{}\n\nPass 8 results: {}\n",
            review.read("brief-1.md"),
            results.display()
        )
    );
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
        "Read,Glob,Grep,Bash(beedle audit *),Bash(beedle fetch *)",
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
    assert!(review.read("stdin").starts_with("caf\u{FFFD}\n"));
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
        .env_remove("XDG_DATA_HOME")
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
    assert!(stderr(&run).contains("review/method.md"));
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
