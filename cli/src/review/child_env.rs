use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub type Vars = Vec<(OsString, OsString)>;

const KEPT: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "TERM",
    "__CF_USER_TEXT_ENCODING",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLOUD_ML_REGION",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "NODE_EXTRA_CA_CERTS",
    "SSL_CERT_FILE",
    "CHROME_PATH",
];
const KEPT_PREFIXES: &[&str] = &[
    "ANTHROPIC_",
    "AWS_",
    "CLAUDE_CODE_USE_",
    "LC_",
    "VERTEX_REGION_",
];

fn is_kept(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    KEPT.contains(&name.as_ref()) || KEPT_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

fn path_with(first: Option<&Path>, path: Option<&OsStr>) -> Option<OsString> {
    let rest = path.map(env::split_paths).into_iter().flatten();
    let all: Vec<PathBuf> = first
        .map(Path::to_path_buf)
        .into_iter()
        .chain(rest)
        .collect();
    env::join_paths(all).ok()
}

pub fn filtered(
    vars: impl Iterator<Item = (OsString, OsString)>,
    first_on_path: Option<&Path>,
) -> Vars {
    let vars: Vars = vars.collect();
    let path = vars
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.as_os_str());
    let path = path_with(first_on_path, path).or_else(|| path.map(OsStr::to_owned));
    let mut kept: Vars = vars
        .iter()
        .filter(|(name, _)| is_kept(name))
        .cloned()
        .collect();
    kept.extend(path.map(|value| ("PATH".into(), value)));
    kept
}

pub fn current() -> Vars {
    let exe = env::current_exe().ok();
    filtered(env::vars_os(), exe.as_deref().and_then(Path::parent))
}
