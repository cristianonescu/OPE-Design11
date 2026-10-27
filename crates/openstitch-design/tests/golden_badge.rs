//! Golden regression: `badge-golden.osd` is `examples/badge.json` saved as a project on
//! 2026-10-18; it must keep compiling to the statistics recorded then.

use openstitch_core::Command;
use openstitch_design::{Compiler, Design};

#[test]
fn golden_project_compiles_to_recorded_statistics() {
    let text = include_str!("fixtures/badge-golden.osd");
    let envelope: serde_json::Value = serde_json::from_str(text).expect("json");
    assert_eq!(envelope["format"], "openstitch.design");
    let design: Design =
        serde_json::from_value(envelope["design"].clone()).expect("golden design decodes");
    let p = Compiler::default().compile(&design);
    // 3055 until 2026-10-27: one zero-length stitch less where each of the 4 jumps lands
    // (compat pes-default-v6).
    assert_eq!(p.count(Command::Stitch), 3051);
    assert_eq!(p.count(Command::Jump), 4);
    assert_eq!(p.count(Command::Trim), 4);
    assert_eq!(p.count(Command::ColorChange), 2);
    let hex: Vec<String> = p.threads.iter().map(|t| t.hex()).collect();
    assert_eq!(hex, ["#ffff00", "#000000", "#ed171f"]);
    let b = p.bounds();
    assert_eq!(
        (
            (b.width() / 10.0 * 10.0).round() / 10.0,
            (b.height() / 10.0 * 10.0).round() / 10.0
        ),
        (80.0, 60.0)
    );
}
