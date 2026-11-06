use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use regex::Match;
use serde_json::Value;
use url::Url;

use super::html::{attr, filled, head, json_ld, link, meta, tags, types_of, visible_text};
use super::{Findings, Level, images, local_path, read_text};
use crate::js::{self, re};

const ANALYTICS: &[(&str, &str)] = &[
    ("PostHog", "posthog"),
    ("Amplitude", "amplitude"),
    (
        "Google Analytics",
        r"googletagmanager\.com|gtag\(|google-analytics\.com",
    ),
    ("Plausible", r"plausible\.io"),
    ("Umami", "umami"),
    ("Fathom", "usefathom"),
    ("Meta Pixel", r"fbq\(|connect\.facebook\.net"),
    ("Clarity", r"clarity\.ms"),
    ("Segment", r"segment\.(com|io)/analytics|analytics\.js"),
    ("Mixpanel", "mixpanel"),
];
const NO_TRACKING: &str = concat!(
    r"\b(no analytics|no tracking|no trackers|not use (any )?(analytics|tracking|cookies)",
    r"|(don['’]t|do not) use (analytics|cookies)",
    r"|(don['’]t|do not) track (you|visitors|anyone)( on| across)? (this|our) (site|website))\b"
);
const PLACEHOLDERS: &str = r"lorem ipsum|\bTODO\b|\bTBD\b|example\.com|your-domain|yourcompany|_TODO\b";
const COPY_TELLS: &[&str] = &[
    r"it['’]s not just\b",
    r"\bdelve\b",
    r"\bpivotal\b",
    r"\bseamless(ly)?\b",
    r"\bunlock\b",
    r"\belevate\b",
    r"\bgame[- ]chang",
    r"\bserves as\b",
    r"\brevolutioni[sz]e",
    r"\bsupercharge",
];
const SEARCH_BOTS: &[&str] = &[
    "googlebot",
    "bingbot",
    "oai-searchbot",
    "claude-searchbot",
    "claude-user",
    "perplexitybot",
];
const JS_BUDGET: usize = 50 * 1024;
const QUOTE_MARGIN: usize = 50;
const OG_MIN_WIDTH: u32 = 1200;
const OG_RATIO: f64 = 1.91;
const OG_RATIO_SLACK: f64 = 0.1;
const HOME: &str = "index.html";

fn is_present(item: &Value, key: &str) -> bool {
    item.get(key).is_some_and(js::truthy)
}

fn structured_data(page: &str, html: &str, findings: &mut Findings) {
    let found = json_ld(html);
    if found.invalid > 0 {
        findings.add(Level::Error, page, "JSON-LD does not parse");
    }
    let business = re(r"Business|Store|Service$", "");
    let application = re(r"Application|VideoGame", "");
    for item in &found.items {
        let types = types_of(item);
        if types == ["VideoGame"] {
            findings.add(
                Level::Error,
                page,
                r#"VideoGame must be co-typed, such as ["VideoGame","MobileApplication"]"#,
            );
        }
        let is_business = types.iter().any(|name| business.is_match(name));
        if is_business && !is_present(item, "address") && !is_present(item, "areaServed") {
            let name = &types[0];
            findings.add(Level::Error, page, format!("{name} has no address or areaServed"));
        }
        if is_present(item, "aggregateRating") || is_present(item, "review") {
            findings.add(
                Level::Warn,
                page,
                "Structured data claims ratings or reviews: confirm they are real",
            );
        }
        let is_application = types.iter().any(|name| application.is_match(name));
        if is_application && !is_present(item, "offers") {
            let name = &types[0];
            findings.add(Level::Warn, page, format!("{name} has no offers"));
        }
    }
    let types: Vec<String> = found.items.iter().flat_map(types_of).collect();
    for name in types.iter().filter(|name| *name == "FAQPage" || *name == "HowTo") {
        findings.add(Level::Info, page, format!("{name} no longer earns a rich result"));
    }
    if page != HOME {
        return;
    }
    if !types.iter().any(|name| name == "WebSite") {
        findings.add(
            Level::Warn,
            page,
            "Home page has no WebSite JSON-LD (sets the site name in search)",
        );
    }
    let owner = re("Business", "");
    let names_owner = |name: &String| {
        name == "Organization" || name == "Person" || owner.is_match(name)
    };
    if !types.iter().any(names_owner) {
        findings.add(
            Level::Warn,
            page,
            "Home page has no Organization, Person or LocalBusiness JSON-LD",
        );
    }
}

fn page_basics(page: &str, html: &str, head: &str, findings: &mut Findings) {
    if !re(r"<html\b[^>]*\blang=", "i").is_match(html) {
        findings.add(Level::Warn, page, "No lang attribute on <html>");
    }
    if filled(meta(head, "viewport")).is_none() {
        findings.add(Level::Error, page, "No viewport meta tag");
    }
    if !re(r"<title\b[^>]*>[^<]*\S", "i").is_match(head) {
        findings.add(Level::Error, page, "No title");
    }
    if filled(meta(head, "description")).is_none() {
        findings.add(Level::Error, page, "No meta description");
    }
    match filled(link(head, "canonical")) {
        None => findings.add(Level::Error, page, "No canonical link in the initial HTML"),
        Some(canonical) if !canonical.starts_with("https://") => findings.add(
            Level::Error,
            page,
            format!("Canonical is not absolute https: {canonical}"),
        ),
        Some(_) => {}
    }
}

fn page_shape(page: &str, html: &str, text: &str, findings: &mut Findings) {
    let h1s = tags(html, "h1").len();
    if h1s != 1 {
        findings.add(Level::Error, page, format!("Expected one h1, found {h1s}"));
    }
    if re(r"flutter_bootstrap\.js|main\.dart\.js", "").is_match(html) {
        findings.add(
            Level::Error,
            page,
            "Flutter web page: marketing content must be static HTML",
        );
    }
    let words = if text.is_empty() {
        0
    } else {
        text.split(' ').count()
    };
    if page == HOME && words < 100 {
        findings.add(
            Level::Warn,
            page,
            format!("Only {words} words in the initial HTML: is the content client-rendered?"),
        );
    }
}

fn share_tags(dir: &Path, page: &str, head: &str, findings: &mut Findings) {
    for key in ["og:title", "og:description", "og:type", "og:url", "og:image"] {
        if filled(meta(head, key)).is_none() {
            findings.add(Level::Error, page, format!("No {key}"));
        }
    }
    let card = meta(head, "twitter:card");
    if card != Some("summary_large_image") {
        let card = card.unwrap_or("missing");
        findings.add(
            Level::Warn,
            page,
            format!("twitter:card is {card}, expected summary_large_image"),
        );
    }
    let image = meta(head, "og:image");
    if filled(image).is_some_and(|image| !re(r"^https?://", "").is_match(image)) {
        findings.add(Level::Error, page, "og:image must be an absolute URL");
    }
    let file = local_path(dir, image).filter(|file| file.to_string_lossy().ends_with(".png"));
    if let Some((width, height)) = file.and_then(|file| images::png_size(&file)) {
        let ratio = f64::from(width) / f64::from(height);
        if width < OG_MIN_WIDTH || (ratio - OG_RATIO).abs() > OG_RATIO_SLACK {
            findings.add(
                Level::Warn,
                page,
                format!("og:image is {width}x{height}, expected 1200x630"),
            );
        }
    }
    let has_icon = filled(link(head, "icon")).is_some() || filled(link(head, "shortcut")).is_some();
    if !has_icon && !dir.join("favicon.ico").exists() {
        findings.add(Level::Warn, page, "No favicon");
    }
}

fn path_on_site(page: &str, src: &str) -> Option<String> {
    let base = Url::parse(&format!("https://site.invalid/{page}")).ok()?;
    Some(base.join(src).ok()?.path().to_string())
}

fn page_images(dir: &Path, page: &str, html: &str, head: &str, findings: &mut Findings) {
    for (index, img) in tags(html, "img").into_iter().enumerate() {
        let name = attr(img, "src").unwrap_or("image");
        if attr(img, "alt").is_none() {
            findings.add(Level::Error, page, format!("Image has no alt: {name}"));
        }
        if filled(attr(img, "width")).is_none() || filled(attr(img, "height")).is_none() {
            findings.add(
                Level::Warn,
                page,
                format!("Image has no width and height: {name}"),
            );
        }
        if index == 0 && attr(img, "loading") == Some("lazy") {
            findings.add(
                Level::Warn,
                page,
                format!("First image is lazy-loaded and may be the LCP element: {name}"),
            );
        }
    }
    for src in images::sources(html, meta(head, "og:image")) {
        let path = path_on_site(page, src);
        let file = local_path(dir, path.as_deref());
        if let Some(blank) = file.and_then(|file| images::blank_image(&file)) {
            let detail = blank.detail;
            findings.add(blank.level, page, format!("Image looks blank: {src} {detail}"));
        }
    }
}

fn gzipped_length(bytes: &[u8]) -> usize {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(bytes).is_err() {
        return 0;
    }
    encoder.finish().map_or(0, |compressed| compressed.len())
}

fn page_scripts(dir: &Path, page: &str, html: &str, findings: &mut Findings) {
    let mut total = 0;
    for script in tags(html, "script") {
        if let Some(file) = local_path(dir, attr(script, "src")) {
            total += fs::read(file).map_or(0, |bytes| gzipped_length(&bytes));
        }
    }
    if total > JS_BUDGET {
        let kilobytes = js::round(total as f64 / 1024.0);
        findings.add(
            Level::Warn,
            page,
            format!("{kilobytes}KB of gzipped JS is over the 50KB starting budget"),
        );
    }
}

fn page_wording(page: &str, text: &str, head: &str, findings: &mut Findings) {
    let placeholders = re(PLACEHOLDERS, "");
    if let Some(found) = placeholders.find(text).or_else(|| placeholders.find(head)) {
        let found = found.as_str();
        findings.add(Level::Error, page, format!("Placeholder text: {found}"));
    }
    let tells: Vec<&str> = COPY_TELLS
        .iter()
        .filter_map(|pattern| re(pattern, "i").find(text))
        .map(|found| found.as_str())
        .collect();
    if !tells.is_empty() {
        let tells = tells.join(", ");
        findings.add(
            Level::Info,
            page,
            format!("Generated-sounding wording: {tells}"),
        );
    }
}

pub fn audit_page(dir: &Path, page: &str, html: &str, findings: &mut Findings) {
    let head = head(html);
    let text = visible_text(html);
    page_basics(page, html, head, findings);
    page_shape(page, html, &text, findings);
    share_tags(dir, page, head, findings);
    page_images(dir, page, html, head, findings);
    for anchor in tags(html, "a") {
        if attr(anchor, "href") == Some("#") {
            findings.add(Level::Error, page, "Link goes nowhere (href=\"#\")");
        }
    }
    page_scripts(dir, page, html, findings);
    page_wording(page, &text, head, findings);
    structured_data(page, html, findings);
}

fn loaded_analytics(pages: &[(String, String)]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for (_, html) in pages {
        let scripts: Vec<&str> = re(r"<script\b[\s\S]*?</script>", "i")
            .find_iter(html)
            .map(|found| found.as_str())
            .collect();
        let scripts = scripts.join("\n");
        for (name, pattern) in ANALYTICS {
            if re(pattern, "i").is_match(&scripts) && seen.insert(*name) {
                names.push((*name).to_string());
            }
        }
    }
    names
}

fn tracking_denial(text: &str) -> Option<Match<'_>> {
    let pattern = re(NO_TRACKING, "i");
    let mut at = 0;
    while let Some(found) = pattern.find_at(text, at) {
        let after = text.as_bytes().get(found.end()..found.end() + 6);
        let about_a_pixel = found.as_str().eq_ignore_ascii_case("no tracking")
            && after.is_some_and(|next| next.eq_ignore_ascii_case(b" pixel"));
        if !about_a_pixel {
            return Some(found);
        }
        at = found.start() + 1;
    }
    None
}

fn quote_around(text: &str, found: &Match<'_>) -> String {
    let from = js::utf16_len(&text[..found.start()]);
    let to = from + js::utf16_len(found.as_str());
    js::utf16_slice(text, from.saturating_sub(QUOTE_MARGIN), to + QUOTE_MARGIN)
}

pub fn audit_privacy(pages: &[(String, String)], findings: &mut Findings) -> Vec<String> {
    let names = loaded_analytics(pages);
    let privacy: Vec<&(String, String)> = pages
        .iter()
        .filter(|(page, _)| re("privacy", "i").is_match(page))
        .collect();
    if names.is_empty() {
        return names;
    }
    let loaded = names.join(" and ");
    if privacy.is_empty() {
        findings.add(
            Level::Error,
            "site",
            format!("{loaded} load but there is no privacy page"),
        );
    }
    for (page, html) in privacy {
        let text = visible_text(html);
        if let Some(claim) = tracking_denial(&text) {
            let quote = quote_around(&text, &claim);
            findings.add(
                Level::Error,
                page,
                format!("Says \"...{quote}...\" but {loaded} load"),
            );
            continue;
        }
        let lowered = text.to_lowercase();
        let named = names.iter().any(|name| lowered.contains(&name.to_lowercase()));
        if !named && !re("analytics", "i").is_match(&text) {
            let list = names.join(" or ");
            findings.add(Level::Warn, page, format!("Does not mention {list}"));
        }
    }
    names
}

#[derive(Default)]
struct RobotGroup {
    agents: Vec<String>,
    rules: Vec<String>,
}

fn robot_groups(text: &str) -> Vec<RobotGroup> {
    let mut groups: Vec<RobotGroup> = Vec::new();
    let comment = re(r"#[^\n\r\x{2028}\x{2029}]*", "");
    for raw in text.split('\n') {
        let uncommented = comment.replace(raw, "");
        let line = js::trim(&uncommented);
        let (key, rest) = line.split_once(':').unwrap_or((line, ""));
        let value = js::trim(rest);
        if key.eq_ignore_ascii_case("user-agent") {
            if groups.last().is_none_or(|group| !group.rules.is_empty()) {
                groups.push(RobotGroup::default());
            }
            if let Some(group) = groups.last_mut() {
                group.agents.push(value.to_lowercase());
            }
        } else if key.eq_ignore_ascii_case("allow") || key.eq_ignore_ascii_case("disallow") {
            if let Some(group) = groups.last_mut() {
                let key = key.to_lowercase();
                group.rules.push(format!("{key}:{value}"));
            }
        }
    }
    groups
}

pub fn audit_robots(dir: &Path, findings: &mut Findings) {
    let file = dir.join("robots.txt");
    if !file.exists() {
        findings.add(Level::Warn, "robots.txt", "Missing");
        return;
    }
    let Some(text) = read_text(&file) else {
        return;
    };
    if !re(r"^sitemap:", "im").is_match(&text) {
        findings.add(Level::Warn, "robots.txt", "No Sitemap line");
    }
    let groups = robot_groups(&text);
    let for_agent = |agent: &str| {
        groups
            .iter()
            .find(|group| group.agents.iter().any(|name| name == agent))
    };
    let wildcard = for_agent("*");
    for bot in SEARCH_BOTS {
        let rules = for_agent(bot).or(wildcard).map(|group| &group.rules);
        let blocked = rules.is_some_and(|rules| {
            rules.iter().any(|rule| rule == "disallow:/" || rule == "disallow:/*")
        });
        if blocked {
            findings.add(Level::Error, "robots.txt", format!("Blocks {bot}"));
        }
    }
}

fn without_slash(url: &str) -> &str {
    url.strip_suffix('/').unwrap_or(url)
}

pub fn audit_sitemap(
    dir: &Path,
    indexable: &[(String, Option<String>)],
    findings: &mut Findings,
) {
    let file = dir.join("sitemap.xml");
    if !file.exists() && !dir.join("sitemap-index.xml").exists() {
        findings.add(Level::Warn, "sitemap.xml", "Missing");
        return;
    }
    let Some(text) = file.exists().then(|| read_text(&file)).flatten() else {
        return;
    };
    if re(r"<(priority|changefreq)>", "").is_match(&text) {
        findings.add(
            Level::Info,
            "sitemap.xml",
            "Google ignores priority and changefreq",
        );
    }
    let urls: Vec<String> = re(r"<loc>[^<]*</loc>", "")
        .find_iter(&text)
        .map(|found| {
            let bare = found.as_str().replace("<loc>", "").replace("</loc>", "");
            without_slash(js::trim(&bare)).to_string()
        })
        .collect();
    for (page, canonical) in indexable {
        let listed = |canonical: &str| urls.iter().any(|url| url == without_slash(canonical));
        if filled(canonical.as_deref()).is_some_and(|canonical| !listed(canonical)) {
            findings.add(Level::Warn, "sitemap.xml", format!("Does not list {page}"));
        }
    }
}
