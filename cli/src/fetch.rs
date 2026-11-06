use std::io::{self, Write};
use std::time::Duration;

use clap::Args as ClapArgs;
use ureq::Agent;
use ureq::http::Response;
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = "Mozilla/5.0 (compatible; beedle-fetch)";
const BODY_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(ClapArgs)]
pub struct Args {
    /// The http or https address to request. Redirects are shown, not followed
    url: String,
    /// Print the status and headers without the body
    #[arg(long)]
    head: bool,
}

fn web_url(text: &str) -> Result<Url, String> {
    let url = Url::parse(text).map_err(|error| format!("{text} is not a URL: {error}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(format!("{scheme} isn't http or https")),
    }
}

fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .max_redirects(0)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

fn headers_of<T>(response: &Response<T>) -> String {
    let mut text = format!("{:?} {}\n", response.version(), response.status());
    for (name, value) in response.headers() {
        let value = String::from_utf8_lossy(value.as_bytes());
        text.push_str(&format!("{name}: {value}\n"));
    }
    text
}

fn fetch(args: &Args) -> Result<Vec<u8>, String> {
    let url = web_url(&args.url)?;
    let agent = agent();
    let failed = |error: ureq::Error| format!("{url}: {error}");
    if args.head {
        let response = agent.head(url.as_str()).call().map_err(failed)?;
        return Ok(headers_of(&response).into_bytes());
    }
    let mut response = agent.get(url.as_str()).call().map_err(failed)?;
    let mut out = headers_of(&response).into_bytes();
    out.push(b'\n');
    let body = response
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_vec()
        .map_err(failed)?;
    out.extend(body);
    Ok(out)
}

pub fn run(args: Args) -> u8 {
    match fetch(&args) {
        Ok(text) => match io::stdout().write_all(&text) {
            Ok(()) => 0,
            Err(_) => 1,
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
