//! Holds beedle to the output recorded from the script it replaced.

use std::process::Command;

const SPOTTER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/spotter/site"
);
const GOLDEN: &str = include_str!("golden/spotter-audit.json");

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_spotter_audit_matches_the_recorded_output() {
    let run = Command::new(env!("CARGO_BIN_EXE_beedle"))
        .args(["audit", "--dir", SPOTTER, "--offline", "--json"])
        .output()
        .unwrap();
    assert_eq!(text(&run.stdout), GOLDEN);
    assert_eq!(run.status.code(), Some(1));
}
