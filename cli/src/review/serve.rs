use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::js;

pub struct Serving {
    pub url: String,
    server: Arc<Server>,
    worker: Option<JoinHandle<()>>,
}

impl Serving {
    pub fn start(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("{}: {error}", root.display()))?;
        let server = Server::http("127.0.0.1:0").map_err(|error| error.to_string())?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or("the server has no port")?
            .port();
        let server = Arc::new(server);
        let listener = server.clone();
        let worker = thread::spawn(move || {
            for request in listener.incoming_requests() {
                respond(&root, request);
            }
        });
        Ok(Self {
            url: format!("http://localhost:{port}"),
            server,
            worker: Some(worker),
        })
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug, PartialEq)]
enum Found {
    File(PathBuf),
    Redirect(String),
    Missing,
}

fn find(root: &Path, target: &str) -> Found {
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let Some(decoded) = js::decode_uri_component(path) else {
        return Found::Missing;
    };
    let Ok(file) = root.join(decoded.trim_start_matches('/')).canonicalize() else {
        return Found::Missing;
    };
    if !file.starts_with(root) {
        return Found::Missing;
    }
    if !file.is_dir() {
        return Found::File(file);
    }
    if !path.ends_with('/') {
        return Found::Redirect(format!("{path}/"));
    }
    let index = file.join("index.html");
    if index.is_file() {
        Found::File(index)
    } else {
        Found::Missing
    }
}

fn content_type(file: &Path) -> &'static str {
    let extension = file.extension().unwrap_or_default().to_string_lossy();
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

fn respond(root: &Path, request: Request) {
    let found = find(root, request.url());
    let _ = match found {
        Found::File(file) => match File::open(&file) {
            Ok(opened) => {
                let mut response = Response::from_file(opened);
                if let Some(kind) = header("Content-Type", content_type(&file)) {
                    response.add_header(kind);
                }
                request.respond(response)
            }
            Err(_) => request.respond(Response::empty(StatusCode(404))),
        },
        Found::Redirect(location) => {
            let mut response = Response::empty(StatusCode(301));
            if let Some(location) = header("Location", &location) {
                response.add_header(location);
            }
            request.respond(response)
        }
        Found::Missing => request.respond(Response::empty(StatusCode(404))),
    };
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Found, find};

    #[test]
    fn folders_serve_their_index_and_files_serve_themselves() {
        let site = tempfile::tempdir().unwrap();
        let root = site.path().canonicalize().unwrap();
        fs::create_dir(root.join("a b")).unwrap();
        fs::write(root.join("a b/index.html"), "").unwrap();
        fs::write(root.join("index.html"), "").unwrap();
        assert_eq!(find(&root, "/"), Found::File(root.join("index.html")));
        assert_eq!(
            find(&root, "/a%20b/?v=1"),
            Found::File(root.join("a b/index.html"))
        );
        assert_eq!(
            find(&root, "/a%20b"),
            Found::Redirect("/a%20b/".to_string())
        );
        assert_eq!(find(&root, "/nope"), Found::Missing);
    }

    #[test]
    fn nothing_outside_the_site_is_served() {
        let parent = tempfile::tempdir().unwrap();
        let parent = parent.path().canonicalize().unwrap();
        let root = parent.join("site");
        fs::create_dir(&root).unwrap();
        fs::write(parent.join("secret"), "").unwrap();
        assert_eq!(find(&root, "/../secret"), Found::Missing);
        assert_eq!(find(&root, "/%2e%2e/secret"), Found::Missing);
    }
}
