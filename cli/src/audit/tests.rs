use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Mutex;
use std::thread;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use regex::Regex;
use tempfile::TempDir;

use super::html::visible_text;
use super::links::{Link, Outcome, check_links, check_live};
use super::{Finding, Level, Report, audit};

const SPOTTER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/spotter/site");
const OK_LINK: &str = "https://ok.dev/";
const HEAD: &str = r#"<title>Tool for teams</title><meta name="viewport" content="width=device-width">
<meta name="description" content="A tool."><link rel="canonical" href="https://a.dev/">
<meta property="og:title" content="t"><meta property="og:description" content="d"><meta property="og:type" content="website">
<meta property="og:url" content="https://a.dev/"><meta property="og:image" content="https://a.dev/og.jpg">
<meta name="twitter:card" content="summary_large_image"><link rel="icon" href="/i.png">
<script type="application/ld+json">[{"@type":"WebSite"},{"@type":"Organization"}]</script>"#;

type Files = BTreeMap<String, Vec<u8>>;

fn words() -> String {
    "word ".repeat(120)
}

fn page_with(body: &str, head: &str) -> String {
    format!(r#"<!doctype html><html lang="en"><head>{head}</head><body>{body}</body></html>"#)
}

fn page(body: &str) -> String {
    page_with(body, HEAD)
}

fn good() -> Files {
    let words = words();
    with(
        Files::new(),
        &[
            ("index.html", &page(&format!("<h1>Tool</h1><p>{words}</p>"))),
            ("404.html", &page("<h1>Lost</h1>")),
            (
                "robots.txt",
                "User-agent: *\nAllow: /\nSitemap: https://a.dev/sitemap.xml\n",
            ),
            (
                "sitemap.xml",
                "<urlset><url><loc>https://a.dev/</loc></url></urlset>",
            ),
        ],
    )
}

fn with(mut files: Files, changes: &[(&str, &str)]) -> Files {
    for (name, text) in changes {
        files.insert(name.to_string(), text.as_bytes().to_vec());
    }
    files
}

fn site(files: &Files) -> TempDir {
    let dir = TempDir::with_prefix("beedle-").unwrap();
    for (name, bytes) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    dir
}

fn report(files: &Files) -> Report {
    audit(site(files).path(), &[]).unwrap()
}

fn messages_of(findings: &[Finding], level: Option<Level>) -> Vec<String> {
    findings
        .iter()
        .filter(|finding| level.is_none_or(|level| finding.level == level))
        .map(|finding| finding.message.clone())
        .collect()
}

fn messages(files: &Files, level: Option<Level>) -> Vec<String> {
    messages_of(&report(files).findings, level)
}

fn matches(list: &[String], pattern: &str) -> bool {
    let pattern = Regex::new(pattern).unwrap();
    list.iter().any(|message| pattern.is_match(message))
}

#[track_caller]
fn has(list: &[String], pattern: &str) {
    assert!(matches(list, pattern), "no match for {pattern} in {list:?}");
}

#[test]
fn a_complete_site_has_no_findings() {
    assert_eq!(messages(&good(), None), Vec::<String>::new());
}

#[test]
fn privacy_text_that_denies_analytics_fails_when_analytics_load() {
    let words = words();
    let script = r#"<script src="https://cdn.posthog.com/array.js"></script>"#;
    let files = with(
        good(),
        &[
            (
                "index.html",
                &page(&format!("<h1>Tool</h1><p>{words}</p>{script}")),
            ),
            (
                "privacy/index.html",
                &page("<h1>Privacy</h1><p>There is no analytics on this site.</p>"),
            ),
        ],
    );
    has(&messages(&files, Some(Level::Error)), "(?i)no analytics.*PostHog");
}

#[test]
fn no_tracking_pixel_alone_is_not_a_denial_of_analytics() {
    let words = words();
    let files = with(
        good(),
        &[
            (
                "index.html",
                &page(&format!(
                    "<h1>Tool</h1><script>posthog.init()</script><p>{words}</p>"
                )),
            ),
            (
                "privacy/index.html",
                &page("<h1>Privacy</h1><p>We use PostHog analytics but no tracking pixel.</p>"),
            ),
        ],
    );
    assert!(!matches(&messages(&files, Some(Level::Error)), "load$"));
}

#[test]
fn a_statement_about_background_location_is_not_a_denial_of_analytics() {
    let words = words();
    let files = with(
        good(),
        &[
            (
                "index.html",
                &page(&format!(
                    "<h1>Tool</h1><script>posthog.init()</script><p>{words}</p>"
                )),
            ),
            (
                "privacy/index.html",
                &page(
                    "<h1>Privacy</h1><p>We do not track you in the background. PostHog measures visits.</p>",
                ),
            ),
        ],
    );
    assert!(!matches(&messages(&files, Some(Level::Error)), "Says"));
}

#[test]
fn analytics_without_a_privacy_page_is_an_error() {
    let words = words();
    let script = r#"<script src="https://x.amplitude.com/a.js"></script>"#;
    let files = with(
        good(),
        &[(
            "index.html",
            &page(&format!("<h1>T</h1><p>{words}</p>{script}")),
        )],
    );
    has(&messages(&files, Some(Level::Error)), "no privacy page");
}

#[test]
fn flags_a_flutter_shell_and_thin_html() {
    let files = with(
        good(),
        &[(
            "index.html",
            &page(r#"<script src="flutter_bootstrap.js"></script>"#),
        )],
    );
    let list = messages(&files, None);
    has(&list, "Flutter web");
    has(&list, "Expected one h1");
    has(&list, "words in the initial HTML");
}

#[test]
fn noindex_pages_skip_page_checks() {
    let files = with(
        good(),
        &[(
            "auth/index.html",
            r#"<html><head><meta name="robots" content="noindex"></head><body></body></html>"#,
        )],
    );
    assert_eq!(messages(&files, None), Vec::<String>::new());
}

#[test]
fn excluded_sections_are_skipped() {
    let files = with(
        good(),
        &[("docs/index.html", "<html><body>docs</body></html>")],
    );
    let dir = site(&files);
    assert_eq!(audit(dir.path(), &["docs".to_string()]).unwrap().pages, 2);
}

#[test]
fn robots_that_block_a_search_crawler_fail_and_blocking_a_training_crawler_does_not() {
    let blocked = with(
        good(),
        &[(
            "robots.txt",
            "User-agent: OAI-SearchBot\nDisallow: /\n\nUser-agent: *\nAllow: /\nSitemap: https://a.dev/s.xml\n",
        )],
    );
    has(&messages(&blocked, Some(Level::Error)), "Blocks oai-searchbot");
    let training = with(
        good(),
        &[(
            "robots.txt",
            "User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nAllow: /\nSitemap: https://a.dev/sitemap.xml\n",
        )],
    );
    assert_eq!(
        messages(&training, Some(Level::Error)),
        Vec::<String>::new()
    );
}

#[test]
fn flags_missing_alt_dead_links_placeholders_and_bad_json_ld() {
    let words = words();
    let body = format!(r##"<h1>T</h1><img src="a.png"><a href="#">x</a><p>lorem ipsum {words}</p>"##);
    let head = format!(r#"{HEAD}<script type="application/ld+json">{{bad</script>"#);
    let files = with(good(), &[("index.html", &page_with(&body, &head))]);
    let list = messages(&files, Some(Level::Error));
    has(&list, "no alt");
    has(&list, "goes nowhere");
    has(&list, "Placeholder");
    has(&list, "does not parse");
}

#[test]
fn rating_claims_and_bare_video_game_are_caught() {
    let words = words();
    let ld = r#"<script type="application/ld+json">{"@type":"VideoGame","aggregateRating":{"ratingValue":5}}</script>"#;
    let body = format!("<h1>T</h1><p>{words}</p>");
    let files = with(
        good(),
        &[("index.html", &page_with(&body, &format!("{HEAD}{ld}")))],
    );
    let list = messages(&files, None);
    has(&list, "co-typed");
    has(&list, "ratings or reviews");
}

#[test]
fn visible_text_ignores_scripts_and_styles() {
    assert_eq!(
        visible_text("<body><style>a{}</style><script>x</script><p>Hi &amp; bye</p></body>"),
        "Hi & bye"
    );
}

fn chunk(name: &str, data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(&[0; 4]);
    out
}

struct Png<'a> {
    size: u32,
    colour: u8,
    depth: u8,
    filter: u8,
    palette: Option<&'a [u8]>,
}

impl Default for Png<'_> {
    fn default() -> Self {
        Self {
            size: 120,
            colour: 2,
            depth: 8,
            filter: 0,
            palette: None,
        }
    }
}

impl Png<'_> {
    fn build(&self, line: impl Fn(u32) -> Vec<u8>) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(&self.size.to_be_bytes());
        header.extend_from_slice(&self.size.to_be_bytes());
        header.extend_from_slice(&[self.depth, self.colour, 0, 0, 0]);
        let mut rows = ZlibEncoder::new(Vec::new(), Compression::default());
        for y in 0..self.size {
            rows.write_all(&[self.filter]).unwrap();
            rows.write_all(&line(y)).unwrap();
        }
        let mut out = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        out.extend(chunk("IHDR", &header));
        if let Some(palette) = self.palette {
            out.extend(chunk("PLTE", palette));
        }
        out.extend(chunk("IDAT", &rows.finish().unwrap()));
        out.extend(chunk("IEND", &[]));
        out
    }
}

fn jpeg(width: u16, height: u16, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    bytes[..7].copy_from_slice(&[0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08]);
    bytes[7..9].copy_from_slice(&height.to_be_bytes());
    bytes[9..11].copy_from_slice(&width.to_be_bytes());
    bytes
}

fn fill(size: usize, pixel: &[u8]) -> impl Fn(u32) -> Vec<u8> + use<> {
    let row = pixel.repeat(size);
    move |_| row.clone()
}

fn noise(size: u32, step: u32) -> impl Fn(u32) -> Vec<u8> {
    move |y| {
        (0..size * step)
            .map(|x| ((x * 37 + y * 101) % 256) as u8)
            .collect()
    }
}

fn with_image_html(name: &str, bytes: Vec<u8>, html: &str) -> Files {
    let words = words();
    let mut files = with(
        good(),
        &[(
            "index.html",
            &page(&format!("<h1>Tool</h1><p>{words}</p>{html}")),
        )],
    );
    files.insert(name.to_string(), bytes);
    files
}

fn with_image(name: &str, bytes: Vec<u8>) -> Files {
    let html = format!(r#"<img src="/{name}" alt="a" width="120" height="120">"#);
    with_image_html(name, bytes, &html)
}

fn blanks(files: &Files, level: Option<Level>) -> Vec<String> {
    let mut list = messages(files, level);
    list.retain(|message| message.contains("looks blank"));
    list
}

#[test]
fn a_flat_png_is_blank_and_the_report_names_the_colour() {
    let bytes = Png::default().build(fill(120, &[0x30, 0x40, 0x60]));
    has(
        &blanks(&with_image("a.png", bytes), Some(Level::Error)),
        r"a\.png is one flat colour \(#304060\)",
    );
}

#[test]
fn a_flat_16_bit_png_with_alpha_is_blank() {
    let png = Png {
        colour: 6,
        depth: 16,
        ..Png::default()
    };
    let bytes = png.build(fill(120, &[0x30, 0, 0x40, 0, 0x60, 0, 0xff, 0xff]));
    has(
        &blanks(&with_image("a.png", bytes), Some(Level::Error)),
        "#304060",
    );
}

#[test]
fn a_flat_png_stored_with_a_row_filter_is_blank() {
    let png = Png {
        filter: 1,
        ..Png::default()
    };
    let bytes = png.build(|_| {
        let mut row = vec![0x30, 0x40, 0x60];
        row.resize(360, 0);
        row
    });
    has(
        &blanks(&with_image("a.png", bytes), Some(Level::Error)),
        "#304060",
    );
}

#[test]
fn a_one_colour_palette_png_is_blank_and_a_two_colour_one_is_not() {
    let palette = [0x30, 0x40, 0x60, 0xff, 0xff, 0xff];
    let png = Png {
        colour: 3,
        depth: 1,
        palette: Some(&palette),
        ..Png::default()
    };
    has(
        &blanks(
            &with_image("a.png", png.build(|_| vec![0; 15])),
            Some(Level::Error),
        ),
        "#304060",
    );
    let striped = png.build(|_| vec![0b1010_1010; 15]);
    assert_eq!(
        blanks(&with_image("a.png", striped), None),
        Vec::<String>::new()
    );
}

#[test]
fn a_png_with_real_variation_is_not_blank_whichever_filter_it_uses() {
    for filter in 0..=4 {
        let png = Png {
            filter,
            ..Png::default()
        };
        let bytes = png.build(noise(120, 3));
        assert_eq!(
            blanks(&with_image("a.png", bytes), None),
            Vec::<String>::new(),
            "filter {filter}"
        );
    }
}

#[test]
fn an_image_too_small_to_be_a_screenshot_is_ignored() {
    let png = Png {
        size: 90,
        ..Png::default()
    };
    let bytes = png.build(fill(90, &[1, 2, 3]));
    assert_eq!(
        blanks(&with_image("a.png", bytes), None),
        Vec::<String>::new()
    );
}

#[test]
fn a_jpeg_with_almost_no_bytes_only_warns_because_a_gradient_looks_the_same() {
    has(
        &blanks(
            &with_image("a.jpg", jpeg(1600, 1000, 9_700)),
            Some(Level::Warn),
        ),
        "1600x1000 in 9700 bytes",
    );
    assert_eq!(
        blanks(&with_image("a.jpg", jpeg(1600, 1000, 30_000)), None),
        Vec::<String>::new()
    );
}

#[test]
fn srcset_candidates_picture_sources_and_the_local_share_image_are_all_checked() {
    let words = words();
    let flat = Png::default().build(fill(120, &[9, 9, 9]));
    let srcset = r#"<img src="/real.svg" srcset="/a.png 1x" alt="a" width="1" height="1">"#;
    has(
        &blanks(&with_image_html("a.png", flat.clone(), srcset), None),
        r"a\.png",
    );
    let picture = r#"<picture><source srcset="/a.png 800w"><img src="/real.svg" alt="a" width="1" height="1"></picture>"#;
    has(
        &blanks(&with_image_html("a.png", flat.clone(), picture), None),
        r"a\.png",
    );
    let share = HEAD.replace("https://a.dev/og.jpg", "https://a.dev/a.png");
    let body = format!("<h1>Tool</h1><p>{words}</p>");
    let mut files = with(good(), &[("index.html", &page_with(&body, &share))]);
    files.insert("a.png".to_string(), flat);
    has(&blanks(&files, None), r"a\.png");
}

#[test]
fn a_cut_off_png_is_not_called_blank() {
    let mut bytes = Png::default().build(noise(120, 3));
    bytes.truncate(40);
    assert_eq!(
        blanks(&with_image("a.png", bytes), None),
        Vec::<String>::new()
    );
}

#[test]
fn a_bad_percent_escape_in_an_image_path_is_not_fatal() {
    let html = r#"<img src="/100%.png" alt="a" width="1" height="1">"#;
    let files = with_image_html("a.png", Vec::new(), html);
    assert!(audit(site(&files).path(), &[]).is_ok());
}

#[test]
fn the_spotter_trial_both_blank_screenshots_are_errors_and_its_real_images_pass() {
    let report = audit(Path::new(SPOTTER), &[]).unwrap();
    let list = messages_of(&report.findings, Some(Level::Error));
    has(&list, r"menu-1\.2\.0-1600\.webp is 1600x1000 in 110 bytes");
    has(&list, r"menu-1\.2\.0-800\.webp is 800x500 in 62 bytes");
    assert_eq!(list.len(), 2);
}

#[derive(Default)]
struct FakeFetch {
    answers: HashMap<String, Outcome>,
    asked: Mutex<Vec<String>>,
}

impl FakeFetch {
    fn new(answers: &[(&str, Outcome)]) -> Self {
        let answers = answers
            .iter()
            .map(|(url, answer)| (url.to_string(), answer.clone()))
            .collect();
        Self {
            answers,
            asked: Mutex::default(),
        }
    }

    fn get(&self, url: &str) -> Outcome {
        self.asked.lock().unwrap().push(url.to_string());
        self.answers
            .get(url)
            .cloned()
            .unwrap_or(Outcome::Status(200))
    }

    fn check(&self, links: &[Link], own_hosts: &[String]) -> Vec<Finding> {
        check_links(links, own_hosts, &|url| self.get(url), 8)
    }

    fn asked(&self) -> Vec<String> {
        let mut asked = self.asked.lock().unwrap().clone();
        asked.sort();
        asked
    }
}

fn link_set(urls: &[&str]) -> Vec<Link> {
    urls.iter()
        .map(|url| Link {
            url: url.to_string(),
            pages: vec!["index.html".to_string()],
        })
        .collect()
}

fn failed(code: &str) -> Outcome {
    Outcome::Failure(code.to_string())
}

fn levels(found: &[Finding]) -> Vec<Level> {
    found.iter().map(|finding| finding.level).collect()
}

#[test]
fn the_spotter_trial_dead_github_links_are_errors_and_the_unlaunched_domain_warns() {
    let report = audit(Path::new(SPOTTER), &[]).unwrap();
    let repo = "https://github.com/spotter-demo/spotter";
    let (latest, releases) = (format!("{repo}/releases/latest"), format!("{repo}/releases"));
    let fetch = FakeFetch::new(&[
        (repo, Outcome::Status(404)),
        (&latest, Outcome::Status(404)),
        (&releases, Outcome::Status(404)),
        ("https://spotter-demo.dev/", failed("ENOTFOUND")),
    ]);
    let found = fetch.check(&report.links, &report.own_hosts);
    let mut errors = messages_of(&found, Some(Level::Error));
    errors.sort();
    assert_eq!(
        errors,
        [
            format!("Dead link (404): {repo}"),
            format!("Dead link (404): {repo}/releases"),
            format!("Dead link (404): {repo}/releases/latest"),
        ]
    );
    has(
        &messages_of(&found, Some(Level::Warn)),
        r"own domain is not live \(domain does not resolve\)",
    );
    assert_eq!(found.len(), 4);
    let own = fetch.asked();
    let own = own.iter().filter(|url| url.contains("spotter-demo.dev"));
    assert_eq!(own.count(), 1);
}

#[test]
fn a_gone_page_and_a_domain_that_does_not_exist_are_dead_links() {
    let fetch = FakeFetch::new(&[
        ("https://a.dev/x", Outcome::Status(410)),
        ("https://b.dev/", failed("ENOTFOUND")),
    ]);
    let found = fetch.check(&link_set(&["https://a.dev/x", "https://b.dev/", OK_LINK]), &[]);
    assert_eq!(levels(&found), [Level::Error, Level::Error]);
}

#[test]
fn a_network_failure_warns_and_a_refused_check_is_only_a_note() {
    let fetch = FakeFetch::new(&[
        ("https://a.dev/", failed("ECONNRESET")),
        ("https://b.dev/", Outcome::Status(403)),
        ("https://c.dev/", Outcome::Status(503)),
    ]);
    let links = link_set(&["https://a.dev/", "https://b.dev/", "https://c.dev/", OK_LINK]);
    assert_eq!(
        levels(&fetch.check(&links, &[])),
        [Level::Warn, Level::Info, Level::Info]
    );
}

#[test]
fn a_machine_with_no_network_gets_one_warning_not_a_dead_link_for_every_url() {
    let urls = ["https://a.dev/", "https://b.dev/"];
    let fetch = FakeFetch::new(&urls.map(|url| (url, failed("ENOTFOUND"))));
    let found = fetch.check(&link_set(&urls), &[]);
    assert_eq!(levels(&found), [Level::Warn]);
    has(&messages_of(&found, None), "offline");
}

#[test]
fn a_site_with_only_its_own_unlaunched_domain_is_told_that_not_that_it_is_offline() {
    let fetch = FakeFetch::new(&[("https://a.dev/", failed("ENOTFOUND"))]);
    let found = fetch.check(&[], &["a.dev".to_string()]);
    has(&messages_of(&found, None), "own domain is not live");
}

#[test]
fn pages_on_the_own_domain_are_not_requested_since_they_may_not_be_deployed_yet() {
    let fetch = FakeFetch::default();
    fetch.check(
        &link_set(&["https://a.dev/pricing", OK_LINK]),
        &["a.dev".to_string()],
    );
    assert_eq!(fetch.asked(), ["https://a.dev/", OK_LINK]);
}

#[test]
fn a_page_repeating_a_dead_link_is_counted_once_and_localhost_is_left_out() {
    let words = words();
    let links = r#"<a href="https://x.dev/a#top">a</a><a href="http://localhost:3000/">l</a><a href="mailto:a@a.dev">m</a>"#;
    let files = with(
        good(),
        &[
            (
                "index.html",
                &page(&format!("<h1>Tool</h1><p>{words}</p>{links}")),
            ),
            (
                "b/index.html",
                &page(r#"<h1>B</h1><a href="https://x.dev/a">a</a>"#),
            ),
        ],
    );
    let links = report(&files).links;
    let dead: Vec<(&str, usize)> = links
        .iter()
        .filter(|link| link.url.contains("x.dev"))
        .map(|link| (link.url.as_str(), link.pages.len()))
        .collect();
    assert_eq!(dead, [("https://x.dev/a", 2)]);
    assert!(
        !links
            .iter()
            .any(|link| link.url.contains("localhost") || link.url.starts_with("mailto"))
    );
}

#[test]
fn structured_data_links_are_collected_and_schema_org_itself_is_not() {
    let ld = r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","sameAs":["https://x.dev/me"]}</script>"#;
    let files = with(
        good(),
        &[(
            "index.html",
            &page_with("<h1>T</h1>", &format!("{HEAD}{ld}")),
        )],
    );
    let links = report(&files).links;
    assert!(links.iter().any(|link| link.url == "https://x.dev/me"));
    assert!(!links.iter().any(|link| link.url.contains("schema.org")));
}

fn answer(stream: std::net::TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap() > 2 {
        line.clear();
    }
    let status = if first.contains("/gone ") {
        "404 Not Found"
    } else {
        "200 OK"
    };
    let reply = format!("HTTP/1.1 {status}\r\nContent-Length: 4\r\nConnection: close\r\n\r\nbody");
    reader.get_mut().write_all(reply.as_bytes()).unwrap();
}

#[test]
fn the_real_request_reads_the_status_from_a_live_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            thread::spawn(move || answer(stream.unwrap()));
        }
    });
    let (gone, here) = (format!("{base}/gone"), format!("{base}/here"));
    let found = check_live(&link_set(&[&gone, &here]), &[]);
    assert_eq!(
        messages_of(&found, None),
        [format!("Dead link (404): {gone}")]
    );
}
