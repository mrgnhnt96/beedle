mod checks;
mod html;
mod images;
mod links;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use serde_json::{Value, json};
use url::Url;

use crate::js::{self, re};
use html::{attr, filled, head, link, meta, tags};
use links::Link;

const USAGE: &str =
    "usage: beedle audit --dir <built site> [--exclude <prefix>]... [--offline] [--json]";
const NOT_INDEXED: &str = r"^(404|500)(/index)?\.html$";

#[derive(ClapArgs)]
#[command(args_override_self = true)]
pub struct Args {
    /// The built site to check
    #[arg(long, allow_hyphen_values = true)]
    dir: Option<PathBuf>,
    /// A path prefix to leave out. Repeat for more
    #[arg(long, allow_hyphen_values = true)]
    exclude: Vec<String>,
    /// Don't request external links
    #[arg(long)]
    offline: bool,
    /// Print the full result as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub level: Level,
    pub place: String,
    pub message: String,
}

#[derive(Default)]
pub struct Findings(Vec<Finding>);

impl Findings {
    pub fn add(&mut self, level: Level, place: &str, message: impl Into<String>) {
        let finding = Finding {
            level,
            place: place.to_string(),
            message: message.into(),
        };
        if !self.0.contains(&finding) {
            self.0.push(finding);
        }
    }
}

pub struct Report {
    pages: usize,
    indexable: usize,
    analytics: Vec<String>,
    findings: Vec<Finding>,
    links: Vec<Link>,
    own_hosts: Vec<String>,
}

impl Report {
    fn to_json(&self) -> Value {
        let findings: Vec<Value> = self
            .findings
            .iter()
            .map(|finding| {
                json!({
                    "level": finding.level.name(),
                    "where": finding.place,
                    "message": finding.message,
                })
            })
            .collect();
        let links: Vec<Value> = self
            .links
            .iter()
            .map(|link| json!({ "url": link.url, "pages": link.pages }))
            .collect();
        json!({
            "pages": self.pages,
            "indexable": self.indexable,
            "analytics": self.analytics,
            "findings": findings,
            "links": links,
            "ownHosts": self.own_hosts,
        })
    }

    fn to_text(&self, offline: bool) -> String {
        let mut text = String::new();
        for finding in &self.findings {
            let (level, place, message) =
                (finding.level.name(), &finding.place, &finding.message);
            text.push_str(&format!("{level:<5}  {place}: {message}\n"));
        }
        let count = |level| self.findings.iter().filter(|f| f.level == level).count();
        let analytics = if self.analytics.is_empty() {
            "none".to_string()
        } else {
            self.analytics.join(", ")
        };
        let links = if offline {
            "external links not checked".to_string()
        } else {
            format!("{} external links checked", self.links.len())
        };
        let (pages, indexable) = (self.pages, self.indexable);
        text.push_str(&format!(
            "\n{pages} pages, {indexable} indexable, analytics: {analytics}, {links}\n"
        ));
        let (errors, warnings, notes) =
            (count(Level::Error), count(Level::Warn), count(Level::Info));
        text.push_str(&format!("{errors} errors, {warnings} warnings, {notes} notes\n"));
        text
    }
}

pub fn read_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn local_path(dir: &Path, url: Option<&str>) -> Option<PathBuf> {
    let url = filled(url)?;
    let parsed = Url::parse(url);
    let path = parsed.as_ref().map_or(url, |parsed| parsed.path());
    let path = path.split(['?', '#']).next().unwrap_or("");
    let file = js::join(dir, &js::decode_uri_component(path)?);
    file.is_file().then_some(file)
}

fn walk(dir: &Path, root: &Path, pages: &mut Vec<String>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(&path, root, pages)?;
        } else if entry.file_name().to_string_lossy().ends_with(".html") {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            pages.push(relative.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

fn is_excluded(page: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|prefix| {
        let prefix = prefix.strip_prefix('/').unwrap_or(prefix);
        let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
        page.starts_with(&format!("{prefix}/"))
    })
}

fn read_pages(dir: &Path, exclude: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut names = Vec::new();
    walk(dir, dir, &mut names).map_err(|error| format!("can't read {}: {error}", dir.display()))?;
    names.retain(|page| !is_excluded(page, exclude));
    names
        .into_iter()
        .map(|page| {
            let path = dir.join(&page);
            let html = read_text(&path).ok_or(format!("can't read {}", path.display()))?;
            Ok((page, html))
        })
        .collect()
}

fn note_repeat(
    seen: &mut HashMap<String, String>,
    key: &str,
    value: Option<&str>,
    page: &str,
    findings: &mut Findings,
) {
    let Some(value) = filled(value) else {
        return;
    };
    match seen.get(value) {
        Some(first) => findings.add(Level::Warn, page, format!("Same {key} as {first}")),
        None => {
            seen.insert(value.to_string(), page.to_string());
        }
    }
}

fn is_left_out(page: &str, head: &str) -> bool {
    let noindex = re("noindex", "i").is_match(meta(head, "robots").unwrap_or(""));
    let redirects = tags(head, "meta").into_iter().any(|tag| {
        attr(tag, "http-equiv").is_some_and(|value| value.to_lowercase() == "refresh")
    });
    noindex || redirects || re(NOT_INDEXED, "").is_match(page)
}

pub fn audit(dir: &Path, exclude: &[String]) -> Result<Report, String> {
    let mut findings = Findings::default();
    let pages = read_pages(dir, exclude)?;
    let has_page = |name: &str| pages.iter().any(|(page, _)| page == name);
    if pages.is_empty() {
        let dir = dir.display();
        findings.add(Level::Error, "site", format!("No HTML files in {dir}"));
    }
    if !has_page("index.html") {
        findings.add(Level::Error, "site", "No index.html");
    }

    let mut indexable = Vec::new();
    let mut titles = HashMap::new();
    let mut descriptions = HashMap::new();
    for (page, html) in &pages {
        let head = head(html);
        if is_left_out(page, head) {
            continue;
        }
        checks::audit_page(dir, page, html, &mut findings);
        indexable.push((page.clone(), link(head, "canonical").map(str::to_string)));
        let title = re(r"<title\b[^>]*>([^<]*)", "i")
            .captures(head)
            .and_then(|caps| caps.get(1))
            .map(|found| js::trim(found.as_str()));
        note_repeat(&mut titles, "title", title, page, &mut findings);
        let description = meta(head, "description");
        note_repeat(&mut descriptions, "description", description, page, &mut findings);
    }
    if !has_page("404.html") && !has_page("404/index.html") {
        findings.add(Level::Warn, "site", "No 404 page");
    }

    let analytics = checks::audit_privacy(&pages, &mut findings);
    checks::audit_robots(dir, &mut findings);
    checks::audit_sitemap(dir, &indexable, &mut findings);
    let mut findings = findings.0;
    findings.sort_by_key(|finding| finding.level);
    let (links, own_hosts) = links::collect_links(&pages);
    Ok(Report {
        pages: pages.len(),
        indexable: indexable.len(),
        analytics,
        findings,
        links,
        own_hosts,
    })
}

pub fn run(args: Args) -> u8 {
    let Some(dir) = args.dir.filter(|dir| dir.exists()) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let mut report = match audit(&js::resolve(&dir), &args.exclude) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    if !args.offline {
        report
            .findings
            .extend(links::check_live(&report.links, &report.own_hosts));
        report.findings.sort_by_key(|finding| finding.level);
    }
    let output = if args.json {
        let json = serde_json::to_string_pretty(&report.to_json()).unwrap_or_default();
        json + "\n"
    } else {
        report.to_text(args.offline)
    };
    let _ = io::stdout().write_all(output.as_bytes());
    let failed = report.findings.iter().any(|f| f.level == Level::Error);
    u8::from(failed)
}
