use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use url::Url;

use super::html::{attr, head, json_ld, link, meta, tags};
use super::{Finding, Level};
use crate::js::re;

const LINK_TIMEOUT: Duration = Duration::from_secs(10);
const LINK_CONCURRENCY: usize = 8;
const LINK_REDIRECTS: u32 = 20;
const LINK_USER_AGENT: &str = "Mozilla/5.0 (compatible; beedle-audit)";
const WEB_URL: &str = r"^https?://";
const NOT_CHECKED: &str = r"^https?://((www\.)?schema\.org|localhost|127\.0\.0\.1)\b";

#[derive(Debug, PartialEq)]
pub struct Link {
    pub url: String,
    pub pages: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Status(u16),
    Failure(String),
}

struct Target {
    url: String,
    pages: Vec<String>,
    own: bool,
}

fn hostname(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_string)
}

fn urls_in(value: &Value, key: &str, found: &mut Vec<String>) {
    match value {
        Value::String(text) if key != "@context" && re(WEB_URL, "i").is_match(text) => {
            found.push(text.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| urls_in(item, key, found)),
        Value::Object(entries) => entries
            .iter()
            .for_each(|(name, item)| urls_in(item, name, found)),
        _ => {}
    }
}

fn urls_on_page(html: &str, head: &str, canonical: Option<&str>) -> Vec<String> {
    let anchors = tags(html, "a").into_iter().map(|a| attr(a, "href"));
    let mut urls: Vec<String> = anchors
        .chain([canonical, meta(head, "og:url"), meta(head, "og:image")])
        .flatten()
        .map(str::to_string)
        .collect();
    for item in json_ld(html).items {
        urls_in(&item, "", &mut urls);
    }
    urls
}

pub fn collect_links(pages: &[(String, String)]) -> (Vec<Link>, Vec<String>) {
    let mut links: Vec<Link> = Vec::new();
    let mut known: HashMap<String, usize> = HashMap::new();
    let mut own_hosts: Vec<String> = Vec::new();
    for (page, html) in pages {
        let head = head(html);
        let canonical = link(head, "canonical");
        let host = canonical
            .filter(|url| re(WEB_URL, "i").is_match(url))
            .and_then(hostname);
        if let Some(host) = host.filter(|host| !own_hosts.contains(host)) {
            own_hosts.push(host);
        }
        for url in urls_on_page(html, head, canonical) {
            if !re(WEB_URL, "i").is_match(&url) || re(NOT_CHECKED, "i").is_match(&url) {
                continue;
            }
            let replaced = url.replace("&amp;", "&");
            let clean = replaced.split('#').next().unwrap_or("").to_string();
            let index = *known.entry(clean.clone()).or_insert_with(|| {
                links.push(Link {
                    url: clean,
                    pages: Vec::new(),
                });
                links.len() - 1
            });
            if !links[index].pages.contains(page) {
                links[index].pages.push(page.clone());
            }
        }
    }
    (links, own_hosts)
}

fn lookup_failure(text: &str) -> Option<&'static str> {
    if text.contains("not known") || text.contains("No address associated") {
        Some("ENOTFOUND")
    } else if text.contains("Temporary failure") {
        Some("EAI_AGAIN")
    } else {
        None
    }
}

fn io_failure(error: &io::Error) -> String {
    let text = error.to_string();
    let code = match error.kind() {
        io::ErrorKind::ConnectionRefused => Some("ECONNREFUSED"),
        io::ErrorKind::ConnectionReset => Some("ECONNRESET"),
        io::ErrorKind::TimedOut => Some("ETIMEDOUT"),
        _ => lookup_failure(&text),
    };
    code.map_or(text, str::to_string)
}

fn failure(error: &ureq::Error) -> String {
    match error {
        ureq::Error::HostNotFound => "ENOTFOUND".to_string(),
        ureq::Error::Timeout(_) => "TimeoutError".to_string(),
        ureq::Error::TooManyRedirects => "TypeError".to_string(),
        ureq::Error::Io(error) => io_failure(error),
        other => other.to_string(),
    }
}

fn request(agent: &Agent, url: &str) -> Outcome {
    let Ok(parsed) = Url::parse(url) else {
        return Outcome::Failure("ERR_INVALID_URL".to_string());
    };
    match agent.get(parsed.as_str()).call() {
        Ok(response) => Outcome::Status(response.status().as_u16()),
        Err(error) => Outcome::Failure(failure(&error)),
    }
}

fn finding(target: &Target, outcome: &Outcome) -> Option<Finding> {
    let url = &target.url;
    let place = match target.pages.as_slice() {
        [only] => only.clone(),
        pages => format!("{} (+{} more)", pages[0], pages.len() - 1),
    };
    let (reason, dead, failed) = match outcome {
        Outcome::Status(200..=399) => return None,
        Outcome::Status(status) => (status.to_string(), matches!(status, 404 | 410), false),
        Outcome::Failure(code) if code == "ENOTFOUND" => {
            ("domain does not resolve".to_string(), true, true)
        }
        Outcome::Failure(code) => (code.clone(), false, true),
    };
    let (level, place, message) = if target.own && (dead || failed) {
        (
            Level::Warn,
            "site".to_string(),
            format!("The site's own domain is not live ({reason}): {url}"),
        )
    } else if dead {
        (Level::Error, place, format!("Dead link ({reason}): {url}"))
    } else if failed {
        (
            Level::Warn,
            place,
            format!("Link did not answer ({reason}): {url}"),
        )
    } else {
        (
            Level::Info,
            place,
            format!("Could not confirm link, the server refused the check ({reason}): {url}"),
        )
    };
    Some(Finding {
        level,
        place,
        message,
    })
}

fn request_all(
    targets: &[Target],
    get: &(dyn Fn(&str) -> Outcome + Sync),
    concurrency: usize,
) -> Vec<Outcome> {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut done = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::SeqCst);
            let Some(target) = targets.get(index) else {
                return done;
            };
            done.push((index, get(&target.url)));
        }
    };
    let mut outcomes: Vec<(usize, Outcome)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..concurrency.min(targets.len()))
            .map(|_| scope.spawn(worker))
            .collect();
        workers
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    outcomes.sort_by_key(|(index, _)| *index);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

// Own-domain pages may not be deployed yet, so that host is asked only whether it is live.
pub fn check_links(
    links: &[Link],
    own_hosts: &[String],
    get: &(dyn Fn(&str) -> Outcome + Sync),
    concurrency: usize,
) -> Vec<Finding> {
    let is_own = |url: &str| hostname(url).is_some_and(|host| own_hosts.contains(&host));
    let external = links.iter().filter(|link| !is_own(&link.url));
    let targets: Vec<Target> = external
        .map(|link| Target {
            url: link.url.clone(),
            pages: link.pages.clone(),
            own: false,
        })
        .chain(own_hosts.iter().map(|host| Target {
            url: format!("https://{host}/"),
            pages: vec!["site".to_string()],
            own: true,
        }))
        .collect();
    let outcomes = request_all(&targets, get, concurrency);
    let has_external = targets.iter().any(|target| !target.own);
    let all_failed = outcomes
        .iter()
        .all(|outcome| matches!(outcome, Outcome::Failure(_)));
    if has_external && all_failed {
        let count = targets.len();
        return vec![Finding {
            level: Level::Warn,
            place: "site".to_string(),
            message: format!(
                "None of {count} external links answered, so none were checked: is this machine offline?"
            ),
        }];
    }
    targets
        .iter()
        .zip(&outcomes)
        .filter_map(|(target, outcome)| finding(target, outcome))
        .collect()
}

pub fn check_live(links: &[Link], own_hosts: &[String]) -> Vec<Finding> {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(LINK_TIMEOUT))
        .http_status_as_error(false)
        .max_redirects(LINK_REDIRECTS)
        .user_agent(LINK_USER_AGENT)
        .build()
        .new_agent();
    check_links(
        links,
        own_hosts,
        &|url| request(&agent, url),
        LINK_CONCURRENCY,
    )
}
