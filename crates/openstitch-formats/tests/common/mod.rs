//! Fixtures in `tests/fixtures` were produced by pyembroidery 1.5.1, an independent open-source
//! implementation, via `tools/genfixtures.py` and `tools/gen_tables.py`. They act as a second
//! opinion, not as ground truth from PE-DESIGN.
#![allow(dead_code)]

use openstitch_core::{Command, Stitch};
use std::path::PathBuf;

pub const FIXTURE_FILES: [&str; 13] = [
    "oracle.pes",
    "oracle.pec",
    "oracle.dst",
    "oracle.exp",
    "oracle.jef",
    "oracle.pcs",
    "oracle.sew",
    "oracle.xxx",
    "oracle.hus",
    "openstitch.hus",
    "openstitch.vip",
    "oracle.shv",
    "oracle.phc",
];

pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn data(name: &str) -> Vec<u8> {
    let path = fixture_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

pub struct Decoded {
    pub threads: Vec<String>,
    pub stitches: Vec<Stitch>,
}

pub fn stitches(name: &str) -> Decoded {
    let v: serde_json::Value = serde_json::from_slice(&data(name)).expect("fixture JSON");
    let threads = v["threads"]
        .as_array()
        .expect("threads")
        .iter()
        .map(|t| t.as_str().expect("hex").to_owned())
        .collect();
    let stitches = v["stitches"]
        .as_array()
        .expect("stitches")
        .iter()
        .map(|row| {
            let command: Command = serde_json::from_value(row[2].clone()).expect("command");
            Stitch::new(
                row[0].as_f64().expect("x"),
                row[1].as_f64().expect("y"),
                command,
            )
        })
        .collect();
    Decoded { threads, stitches }
}

pub fn hexes(threads: &[openstitch_core::Thread]) -> Vec<String> {
    threads.iter().map(|t| t.hex()).collect()
}

/// Compares `bytes` with a committed fixture that OpenStitch itself writes, or rewrites the
/// fixture when `OPENSTITCH_REGEN=1` (then re-run `tools/genfixtures.py` for its oracle read).
pub fn assert_golden(name: &str, bytes: &[u8]) {
    let path = fixture_dir().join(name);
    if std::env::var_os("OPENSTITCH_REGEN").is_some() {
        std::fs::write(&path, bytes).expect("write fixture");
    }
    assert!(
        data(name) == bytes,
        "{name} differs from the committed fixture"
    );
}
