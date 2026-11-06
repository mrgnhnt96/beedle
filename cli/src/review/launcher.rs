use std::ffi::{OsStr, OsString};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use libc::c_int;

const EXIT_POLL: Duration = Duration::from_millis(20);
const TERM_GRACE: Duration = Duration::from_secs(2);
const KILL_GRACE: Duration = Duration::from_secs(1);
const SPAWN_FAILED: i32 = 127;

static GROUP: AtomicI32 = AtomicI32::new(0);

pub struct Outcome {
    pub code: i32,
    pub lines: Vec<String>,
    pub stderr: String,
}

struct Capture {
    bytes: Arc<Mutex<Vec<u8>>>,
    closed: Arc<AtomicBool>,
}

impl Capture {
    fn start(mut stream: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(AtomicBool::new(false));
        let (sink, done) = (bytes.clone(), closed.clone());
        thread::spawn(move || {
            let mut chunk = [0; 8192];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => sink.lock().unwrap().extend_from_slice(&chunk[..read]),
                    Err(error) if error.kind() == ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            done.store(true, Ordering::SeqCst);
        });
        Self { bytes, closed }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.bytes.lock().unwrap())
    }
}

struct Running {
    child: Child,
    status: Option<ExitStatus>,
    stdout: Capture,
    stderr: Capture,
}

impl Running {
    fn finished(&mut self) -> bool {
        if self.status.is_none() {
            self.status = self.child.try_wait().ok().flatten();
        }
        self.status.is_some() && self.stdout.is_closed() && self.stderr.is_closed()
    }

    fn wait_until_finished(&mut self, limit: Duration) -> bool {
        let deadline = Instant::now().checked_add(limit);
        while !self.finished() {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return false;
            }
            thread::sleep(EXIT_POLL);
        }
        true
    }

    fn stop_group(&mut self) {
        let group = self.child.id() as i32;
        unsafe { libc::killpg(group, libc::SIGTERM) };
        self.wait_until_finished(TERM_GRACE);
        unsafe { libc::killpg(group, libc::SIGKILL) };
        self.wait_until_finished(KILL_GRACE);
    }
}

extern "C" fn stop_group_then_die(signal: c_int) {
    let group = GROUP.load(Ordering::SeqCst);
    unsafe {
        if group > 0 {
            libc::kill(-group, libc::SIGTERM);
        }
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

fn forward_signals() {
    let handler = stop_group_then_die as extern "C" fn(c_int) as libc::sighandler_t;
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            if libc::signal(signal, handler) == libc::SIG_IGN {
                libc::signal(signal, libc::SIG_IGN);
            }
        }
    }
}

fn spawn_error(program: &OsStr, error: &std::io::Error) -> String {
    let name = program.to_string_lossy();
    match error.kind() {
        ErrorKind::NotFound => format!("Error: spawn {name} ENOENT"),
        ErrorKind::PermissionDenied => format!("Error: spawn {name} EACCES"),
        _ => format!("Error: spawn {name}: {error}"),
    }
}

fn split_lines(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| String::from_utf8_lossy(line).into_owned())
        .collect()
}

pub fn run(
    program: &OsStr,
    args: &[OsString],
    cwd: &Path,
    input: &[u8],
    timeout: Duration,
    env: &[(OsString, OsString)],
) -> Outcome {
    forward_signals();
    let spawned = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return Outcome {
                code: SPAWN_FAILED,
                lines: Vec::new(),
                stderr: spawn_error(program, &error),
            };
        }
    };
    GROUP.store(child.id() as i32, Ordering::SeqCst);

    if let Some(mut stdin) = child.stdin.take() {
        let input = input.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        unreachable!("both pipes were asked for");
    };
    let mut running = Running {
        child,
        status: None,
        stdout: Capture::start(stdout),
        stderr: Capture::start(stderr),
    };

    if !running.wait_until_finished(timeout) {
        running.stop_group();
    }
    GROUP.store(0, Ordering::SeqCst);

    Outcome {
        code: running.status.and_then(|status| status.code()).unwrap_or(1),
        lines: split_lines(&running.stdout.take()),
        stderr: String::from_utf8_lossy(&running.stderr.take()).into_owned(),
    }
}
