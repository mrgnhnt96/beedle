use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::child_env::filtered;
use super::report::{handed_over, read_report, summarise};
use super::{ALLOWED, FILES, TOOLS, claude_args, default_out, system_prompt};

const BRIEF: &str = "Scope: the whole site\n\
Handed to a person: B1 · Blocker · Dead links · Owner: publish it\n\
B4 · Major · Domain · Owner: buy it\n\
\n\
Return the skill's report. B9 · is not here.";

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap()
}

#[test]
fn the_reviewer_gets_the_method_checklist_and_rules() {
    let prompt = system_prompt(repo()).unwrap();
    for text in [
        "review/method.md",
        "review/checklist.md",
        "reference/seo.md",
        "reference/message.md",
    ] {
        assert!(prompt.contains(text), "{text}");
    }
}

#[test]
fn the_system_prompt_holds_the_method_text() {
    let prompt = system_prompt(repo()).unwrap();
    for text in [
        "## 1. Get the evidence",
        "You can't build anything.",
        "beedle audit --dir <built site>",
        "## 4. Rate each finding",
        "## 6. Report",
    ] {
        assert!(prompt.contains(text), "{text}");
    }
}

#[test]
fn the_reviewer_is_not_sent_the_launcher() {
    let prompt = system_prompt(repo()).unwrap();
    assert!(!prompt.contains("review/SKILL.md"));
    assert!(!prompt.contains("name: beedle-review"));
    assert!(!prompt.contains("beedle review --brief"));
}

#[test]
fn the_system_prompt_names_neither_old_script() {
    let prompt = system_prompt(repo()).unwrap();
    for script in ["audit.mjs", "run.mjs"] {
        assert!(!prompt.contains(script), "{script}");
    }
}

#[test]
fn the_callers_files_name_neither_old_script() {
    for rel in ["SKILL.md", "review/SKILL.md", "review/round.md"] {
        let text = read(rel);
        for script in ["audit.mjs", "run.mjs"] {
            assert!(!text.contains(script), "{rel}: {script}");
        }
    }
}

#[test]
fn the_launcher_holds_none_of_the_method() {
    let launcher = read("review/SKILL.md");
    assert!(launcher.contains("beedle review --brief"));
    for text in [
        "Rate each finding",
        "Verdict: Ready to ship",
        "beedle audit",
    ] {
        assert!(!launcher.contains(text), "{text}");
    }
}

#[test]
fn the_fallback_reads_the_method_and_never_loads_the_skill() {
    let round = read("review/round.md");
    assert!(round.contains("Read method.md and checklist.md"));
    assert!(!round.contains("Load the beedle-review skill"));
    assert!(!round.contains("review it yourself"));
    assert!(!round.contains("with the beedle-review skill"));
}

#[test]
fn the_rule_files_keep_their_order() {
    assert_eq!(
        FILES,
        [
            "review/method.md",
            "review/checklist.md",
            "reference/page-types.md",
            "reference/message.md",
            "reference/seo.md",
        ]
    );
    let prompt = system_prompt(repo()).unwrap();
    let places: Vec<usize> = FILES
        .iter()
        .map(|rel| prompt.find(&format!("<file path=\"{rel}\">")).unwrap())
        .collect();
    assert!(places.is_sorted());
}

#[test]
fn a_missing_rule_file_is_named() {
    let empty = tempfile::tempdir().unwrap();
    let error = system_prompt(empty.path()).unwrap_err();
    assert!(error.contains("review/method.md"), "{error}");
}

#[test]
fn the_reviewer_starts_with_no_settings_mcp_servers_or_session() {
    let args = claude_args(
        Path::new("s"),
        "opus",
        "",
        &[PathBuf::from("/p")],
        Path::new("/root"),
    );
    for flag in [
        "--setting-sources",
        "--strict-mcp-config",
        "--no-session-persistence",
    ] {
        assert!(args.iter().any(|arg| arg == flag), "{flag}");
    }
    let tools = args.iter().position(|arg| arg == "--tools").unwrap();
    assert_eq!(args[tools + 1], TOOLS);
    assert_eq!(TOOLS, "Read,Glob,Grep,Bash");
    assert!(args.iter().any(|arg| arg == "/p"));
}

#[test]
fn the_reviewer_can_only_read_audit_and_fetch() {
    let args = claude_args(Path::new("s"), "opus", "", &[], Path::new("/root"));
    let allowed = args.iter().position(|arg| arg == "--allowedTools").unwrap();
    assert_eq!(args[allowed + 1], ALLOWED);
    assert_eq!(
        ALLOWED,
        "Read,Glob,Grep,Bash(beedle audit *),Bash(beedle fetch *)"
    );
    let mode = args
        .iter()
        .position(|arg| arg == "--permission-mode")
        .unwrap();
    assert_eq!(args[mode + 1], "dontAsk");
    for tool in ["Write", "Edit", "NotebookEdit"] {
        assert!(!TOOLS.contains(tool) && !ALLOWED.contains(tool), "{tool}");
    }
}

#[test]
fn the_reviewer_is_told_what_its_bash_can_run() {
    let prompt = system_prompt(repo()).unwrap();
    for text in [
        "You can't edit, build or serve anything",
        "beedle audit --dir <built site>",
        "beedle fetch <url>",
        "*Not checked*, with the reason",
    ] {
        assert!(prompt.contains(text), "{text}");
    }
}

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(name, value)| (name.into(), value.into()))
        .collect()
}

#[test]
fn session_and_messaging_variables_stay_out_of_the_child() {
    let kept = filtered(
        vars(&[
            ("HOME", "/h"),
            ("PATH", "/bin"),
            ("ANTHROPIC_API_KEY", "k"),
            ("CLAUDE_CODE_OAUTH_TOKEN", "t"),
            ("CLAUDE_CONFIG_DIR", "/c"),
            ("CLAUDE_CODE_USE_BEDROCK", "1"),
            ("AWS_PROFILE", "p"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_SESSION_ID", "s"),
            ("CLAUDE_CODE_MESSAGING_SOCKET", "/m"),
            ("CLAUDE_CODE_MESSAGING_TOKEN", "m"),
            ("CLAUDE_CODE_CHILD_SESSION", "1"),
            ("MIDNA_SESSION", "s"),
            ("MIDNA_SOCKET", "/s"),
            ("SSH_AUTH_SOCK", "/a"),
            ("GITHUB_TOKEN", "g"),
        ])
        .into_iter(),
        None,
    );
    let names: Vec<_> = kept
        .iter()
        .map(|(name, _)| name.to_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "HOME",
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_USE_BEDROCK",
            "AWS_PROFILE",
            "PATH",
        ]
    );
}

#[test]
fn the_running_beedle_comes_first_on_the_childs_path() {
    let kept = filtered(
        vars(&[("PATH", "/usr/bin:/bin")]).into_iter(),
        Some(Path::new("/installed")),
    );
    assert_eq!(kept, vars(&[("PATH", "/installed:/usr/bin:/bin")]));
}

#[test]
fn the_effort_is_only_passed_when_given() {
    let root = Path::new("/root");
    let without = claude_args(Path::new("s"), "opus", "", &[], root);
    assert!(!without.iter().any(|arg| arg == "--effort"));
    let with = claude_args(Path::new("s"), "opus", "high", &[], root);
    let effort = with.iter().position(|arg| arg == "--effort").unwrap();
    assert_eq!(with[effort + 1], "high");
}

#[test]
fn the_report_comes_from_the_result_event() {
    let event = json!({
        "type": "result",
        "subtype": "success",
        "result": "report",
        "num_turns": 2,
        "total_cost_usd": 1.5,
    });
    let summary = summarise(&[event.to_string()]);
    assert!(summary.ok);
    assert_eq!(summary.report, "report");
    assert_eq!(summary.turns, json!(2));
    assert_eq!(summary.cost_usd, json!(1.5));
}

#[test]
fn the_last_assistant_text_stands_in_for_an_empty_result() {
    let assistant = json!({
        "type": "assistant",
        "message": { "content": [{ "type": "text", "text": "from the stream" }] },
    });
    let result = json!({ "type": "result", "subtype": "success", "result": "  " });
    let summary = summarise(&[assistant.to_string(), result.to_string()]);
    assert_eq!(summary.report, "from the stream");
}

#[test]
fn a_failed_result_keeps_its_subtype_as_the_error() {
    let result = json!({ "type": "result", "subtype": "error_max_turns", "is_error": true });
    let summary = summarise(&[result.to_string()]);
    assert!(!summary.ok);
    assert_eq!(summary.error, json!("error_max_turns"));
}

#[test]
fn lines_that_are_not_json_are_skipped() {
    let lines = [
        "not json".to_string(),
        "null".to_string(),
        json!({ "type": "result", "subtype": "success", "result": "report" }).to_string(),
    ];
    assert_eq!(summarise(&lines).report, "report");
}

#[test]
fn the_output_sits_beside_the_brief() {
    assert_eq!(
        default_out(Path::new("/r/brief-2.md")),
        Path::new("/r/review-2")
    );
    assert_eq!(
        default_out(Path::new("/r/notes.md")),
        Path::new("/r/notes.review")
    );
}

#[test]
fn the_reviewer_is_told_it_never_fixes_anything() {
    let prompt = system_prompt(repo()).unwrap();
    for text in [
        "This review never fixes anything",
        "**Suggested fix:**",
        "Verdict: Can't ship until a person acts",
        "Handed to a person",
    ] {
        assert!(prompt.contains(text), "{text}");
    }
    assert!(!prompt.contains("**Fix:**"));
}

#[test]
fn every_handed_over_item_is_read_from_the_brief() {
    assert_eq!(handed_over(BRIEF), ["B1", "B4"]);
    assert!(handed_over("Handed to a person: none\n\nReturn").is_empty());
    assert!(handed_over("Scope: all\n").is_empty());
}

#[test]
fn the_verdict_is_read_whatever_the_emphasis_or_apostrophe() {
    let read = |report| read_report(report, "").verdict;
    assert_eq!(
        read("Verdict: Ready to ship\n\nText").as_deref(),
        Some("Ready to ship")
    );
    assert_eq!(read("**Verdict: Fix first**").as_deref(), Some("Fix first"));
    assert_eq!(
        read("**Verdict:** Can’t ship until a person acts").as_deref(),
        Some("Can't ship until a person acts")
    );
    assert_eq!(read("No verdict here"), None);
}

#[test]
fn a_handed_over_item_reported_as_a_finding_counts_as_repeated() {
    let report = "Verdict: Fix first\n\n### B1 · Blocker · Dead links\n\n### B15 · Minor · New\n\n## Waiting on a person\n- B4 · Major · Domain · Owner: buy it\n";
    assert_eq!(read_report(report, BRIEF).repeated, ["B1"]);
}

#[test]
fn the_waiting_list_on_its_own_is_not_a_repeat() {
    let report = "Verdict: Can't ship until a person acts\n\n## Waiting on a person\n- B1 · Blocker · Dead links · Owner: publish it\n";
    assert!(read_report(report, BRIEF).repeated.is_empty());
}
