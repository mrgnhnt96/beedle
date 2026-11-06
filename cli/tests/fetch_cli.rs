use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::thread::{self, JoinHandle};

const OK: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
const MOVED: &str = "HTTP/1.1 301 Moved Permanently\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const MISSING: &str =
    "HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nConnection: close\r\n\r\ngone";

fn answer_once(response: &'static str) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/page", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 2048];
        let read = stream.read(&mut request).unwrap();
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8_lossy(&request[..read]).into_owned()
    });
    (url, server)
}

fn fetch(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_beedle"))
        .arg("fetch")
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn it_prints_the_status_headers_and_body() {
    let (url, server) = answer_once(OK);
    let run = fetch(&[&url]);
    assert_eq!(run.status.code(), Some(0));
    let text = stdout(&run);
    assert!(text.starts_with("HTTP/1.1 200 OK\n"), "{text}");
    assert!(text.contains("content-type: text/html\n"), "{text}");
    assert!(text.ends_with("\n\nhello"), "{text}");
    assert!(server.join().unwrap().starts_with("GET /page "));
}

#[test]
fn head_leaves_out_the_body() {
    let (url, server) = answer_once("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
    let run = fetch(&["--head", &url]);
    assert_eq!(run.status.code(), Some(0));
    assert!(stdout(&run).starts_with("HTTP/1.1 200 OK\n"));
    assert!(server.join().unwrap().starts_with("HEAD /page "));
}

#[test]
fn a_redirect_is_shown_rather_than_followed() {
    let (url, server) = answer_once(MOVED);
    let run = fetch(&[&url]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = stdout(&run);
    assert!(text.starts_with("HTTP/1.1 301"), "{text}");
    assert!(text.contains("location: /elsewhere\n"), "{text}");
    server.join().unwrap();
}

#[test]
fn an_error_status_is_still_an_answer() {
    let (url, server) = answer_once(MISSING);
    let run = fetch(&[&url]);
    assert_eq!(run.status.code(), Some(0));
    assert!(stdout(&run).starts_with("HTTP/1.1 404"));
    server.join().unwrap();
}

#[test]
fn only_web_addresses_are_accepted() {
    let run = fetch(&["file:///etc/hosts"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("isn't http or https"));
}

#[test]
fn nothing_answering_exits_one() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    drop(listener);
    assert_eq!(fetch(&[&url]).status.code(), Some(1));
}
