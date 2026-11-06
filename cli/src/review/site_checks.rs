use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::child_env::Vars;
use super::launcher::{self, Outcome};
use super::serve::Serving;
use crate::js::{self, re};

const BUILT_SITE: &str = r"^Built site:[ \t]*(.*?)[ \t]*$";
const TOOL_LIMIT: Duration = Duration::from_secs(5 * 60);
const OUTPUT_LIMIT: usize = 8000;
const REASON_LIMIT: usize = 300;
const PASSING_SCORE: f64 = 0.9;
const PA11Y_FOUND_ISSUES: i32 = 2;
const BROKEN_LINKS: &str = r"Detected \d+ broken links";

pub const SKIP_FLAG: &str = "--skip-site-checks";

pub fn site_to_check(brief: &str, base: &Path, skip: bool) -> Result<PathBuf, String> {
    if skip {
        return Err(format!("skipped with {SKIP_FLAG}"));
    }
    let named = re(BUILT_SITE, "m")
        .captures(brief)
        .map(|caps| caps[1].to_string())
        .filter(|text| !text.is_empty())
        .ok_or("the brief names no built site")?;
    let site = base.join(&named);
    if site.is_dir() {
        Ok(site)
    } else {
        Err(format!("the brief's built site, {named}, isn't a folder"))
    }
}

struct Tools<'a> {
    cwd: &'a Path,
    env: &'a Vars,
}

impl Tools<'_> {
    fn npx(&self, args: &[OsString]) -> Finished {
        let started = Instant::now();
        let mut command: Vec<OsString> = vec!["--yes".into()];
        command.extend_from_slice(args);
        let outcome = launcher::run(
            OsStr::new("npx"),
            &command,
            self.cwd,
            b"",
            TOOL_LIMIT,
            self.env,
        );
        Finished {
            outcome,
            timed_out: started.elapsed() >= TOOL_LIMIT,
        }
    }
}

struct Finished {
    outcome: Outcome,
    timed_out: bool,
}

impl Finished {
    fn code(&self) -> i32 {
        self.outcome.code
    }

    fn output(&self) -> String {
        let mut text = self.outcome.lines.join("\n");
        text.push('\n');
        text.push_str(&self.outcome.stderr);
        text
    }

    fn failure(&self) -> String {
        if self.timed_out {
            return format!("stopped after {} seconds", TOOL_LIMIT.as_secs());
        }
        let text = self.output();
        let reason = js::utf16_slice(js::trim(&text), 0, REASON_LIMIT);
        format!("exit {}: {reason}", self.code())
    }
}

fn limited(text: &str) -> String {
    let text = js::trim(text);
    if js::utf16_len(text) <= OUTPUT_LIMIT {
        return text.to_string();
    }
    format!("{}\n(cut here)", js::utf16_slice(text, 0, OUTPUT_LIMIT))
}

fn failed_audits(report: &Value) -> Vec<String> {
    let audits = report["audits"].as_object().into_iter().flatten();
    audits
        .filter(|(_, audit)| {
            audit["score"]
                .as_f64()
                .is_some_and(|score| score < PASSING_SCORE)
        })
        .map(|(id, audit)| {
            let title = audit["title"].as_str().unwrap_or_default();
            match audit["displayValue"].as_str() {
                Some(value) => format!("- {id} · {title} · {value}"),
                None => format!("- {id} · {title}"),
            }
        })
        .collect()
}

fn read_lighthouse(report: &Value, path: &Path) -> Result<String, String> {
    let categories = report["categories"].as_object();
    let scored = categories.is_some_and(|categories| {
        !categories.is_empty() && categories.values().all(|found| found["score"].is_number())
    });
    if !scored {
        let reason = report["runtimeError"]["message"].as_str();
        return Err(format!(
            "it couldn't score the page: {}",
            reason.unwrap_or("no scores in its report")
        ));
    }
    let failed = failed_audits(report);
    let list = if failed.is_empty() {
        "No failed audits.".to_string()
    } else {
        format!("Failed audits:\n{}", limited(&failed.join("\n")))
    };
    Ok(format!("{list}\n\nFull report: {}", path.display()))
}

fn lighthouse(tools: &Tools, url: &str, report: &Path) -> Result<String, String> {
    let mut output_path = OsString::from("--output-path=");
    output_path.push(report);
    let finished = tools.npx(&[
        "lighthouse".into(),
        url.into(),
        "--quiet".into(),
        "--chrome-flags=--headless".into(),
        "--only-categories=performance,accessibility,best-practices,seo".into(),
        "--output=json".into(),
        output_path,
    ]);
    if finished.code() != 0 {
        return Err(finished.failure());
    }
    let text = fs::read(report).map_err(|error| format!("it wrote no report: {error}"))?;
    let parsed =
        serde_json::from_slice(&text).map_err(|error| format!("its report isn't JSON: {error}"))?;
    read_lighthouse(&parsed, report)
}

fn pa11y(tools: &Tools, url: &str) -> Result<String, String> {
    let finished = tools.npx(&["pa11y".into(), url.into()]);
    match finished.code() {
        0 | PA11Y_FOUND_ISSUES => Ok(limited(&finished.output())),
        _ => Err(finished.failure()),
    }
}

fn without_good_links(output: &str) -> String {
    let lines = output.lines().filter(|line| !line.contains("[200]"));
    lines.collect::<Vec<_>>().join("\n")
}

fn linkinator(tools: &Tools, url: &str) -> Result<String, String> {
    let finished = tools.npx(&[
        "linkinator".into(),
        url.into(),
        "--recurse".into(),
        "--skip".into(),
        "^https?://(?!localhost)".into(),
    ]);
    let output = finished.output();
    if finished.code() == 0 || re(BROKEN_LINKS, "").is_match(&output) {
        Ok(limited(&without_good_links(&output)))
    } else {
        Err(finished.failure())
    }
}

fn section(name: &str, outcome: &Result<String, String>) -> String {
    match outcome {
        Ok(text) => format!("## {name}\n\nRan.\n\n{text}\n"),
        Err(reason) => format!("## {name}\n\nNot run: {reason}\n"),
    }
}

fn all_not_run(reason: &str) -> [(&'static str, Result<String, String>); 3] {
    ["lighthouse", "pa11y", "linkinator"].map(|name| (name, Err(reason.to_string())))
}

fn run_tools(
    site: &Path,
    tools: &Tools,
    report: &Path,
) -> (String, [(&'static str, Result<String, String>); 3]) {
    let serving = match Serving::start(site) {
        Ok(serving) => serving,
        Err(error) => {
            let reason = format!("the site couldn't be served: {error}");
            return (String::new(), all_not_run(&reason));
        }
    };
    let url = serving.url.as_str();
    let served = format!("Served {} at {url}.\n\n", site.display());
    let results = [
        ("lighthouse", lighthouse(tools, url, report)),
        ("pa11y", pa11y(tools, url)),
        ("linkinator", linkinator(tools, url)),
    ];
    (served, results)
}

pub fn write_results(
    site: Result<PathBuf, String>,
    results: &Path,
    report: &Path,
    env: &Vars,
) -> Result<(), String> {
    let cwd = results.parent().unwrap_or(Path::new("/"));
    let tools = Tools { cwd, env };
    let (served, outcomes) = match site {
        Ok(site) => run_tools(&site, &tools, report),
        Err(reason) => (String::new(), all_not_run(&reason)),
    };
    let sections: Vec<String> = outcomes
        .iter()
        .map(|(name, outcome)| section(name, outcome))
        .collect();
    let text = format!("# Pass 8 results\n\n{served}{}", sections.join("\n"));
    fs::write(results, text).map_err(|error| format!("can't write {}: {error}", results.display()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::{read_lighthouse, site_to_check, without_good_links, write_results};

    #[test]
    fn the_built_site_is_read_from_the_brief() {
        let site = tempfile::tempdir().unwrap();
        let brief = format!(
            "Project: /p\nBuilt site: {}  \nScope: all\n",
            site.path().display()
        );
        assert_eq!(
            site_to_check(&brief, Path::new("/"), false).unwrap(),
            site.path()
        );
    }

    #[test]
    fn a_relative_built_site_sits_beside_the_brief() {
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir(base.path().join("out")).unwrap();
        let found = site_to_check("Built site: out\n", base.path(), false).unwrap();
        assert_eq!(found, base.path().join("out"));
    }

    #[test]
    fn no_site_to_check_says_why() {
        let base = Path::new("/");
        let reason = |brief, skip| site_to_check(brief, base, skip).unwrap_err();
        assert_eq!(
            reason("Scope: all\n", false),
            "the brief names no built site"
        );
        assert_eq!(
            reason("Built site:\n", false),
            "the brief names no built site"
        );
        assert!(reason("Built site: none\n", false).contains("isn't a folder"));
        assert!(reason("Built site: /\n", true).contains("--skip-site-checks"));
    }

    #[test]
    fn only_failed_audits_are_listed() {
        let report = json!({
            "categories": { "seo": { "score": 0.92 } },
            "audits": {
                "image-alt": { "score": 0, "title": "Images lack alt", "displayValue": "2 found" },
                "viewport": { "score": 1, "title": "Has viewport" },
                "nearly": { "score": 0.95, "title": "Good enough" },
                "info": { "score": null, "title": "Informative" },
            },
        });
        let text = read_lighthouse(&report, Path::new("/r/lh.json")).unwrap();
        assert!(
            text.contains("- image-alt · Images lack alt · 2 found"),
            "{text}"
        );
        assert!(!text.contains("viewport") && !text.contains("nearly") && !text.contains("info"));
        assert!(text.contains("/r/lh.json"));
    }

    #[test]
    fn a_report_with_no_scores_counts_as_not_run() {
        let report = json!({
            "runtimeError": { "message": "Chrome didn't load the page" },
            "categories": { "seo": { "score": null } },
        });
        let error = read_lighthouse(&report, Path::new("/r")).unwrap_err();
        assert!(error.contains("Chrome didn't load the page"), "{error}");
    }

    #[test]
    fn a_missing_npx_is_written_down_as_not_run() {
        let run = tempfile::tempdir().unwrap();
        let (results, report) = (run.path().join("checks.md"), run.path().join("lh.json"));
        let env = vec![("PATH".into(), "/no/such/folder".into())];
        write_results(Ok(run.path().to_path_buf()), &results, &report, &env).unwrap();
        let text = std::fs::read_to_string(results).unwrap();
        assert_eq!(text.matches("Not run: exit 127").count(), 3, "{text}");
        assert!(text.contains("spawn npx ENOENT"), "{text}");
    }

    #[test]
    fn links_that_work_are_left_out() {
        let output =
            "[200] http://localhost/\n[404] http://localhost/x\n  [200] http://localhost/\nERROR";
        assert_eq!(
            without_good_links(output),
            "[404] http://localhost/x\nERROR"
        );
    }
}
