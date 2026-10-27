//! Golden regression: `badge-golden.osd` is `examples/badge.json` saved as a project on
//! 2026-10-18 (format version 1); it must keep compiling to the statistics recorded then.
//! It is read with the project decoder, which migrates version-1 direction angles.

use openstitch_core::Command;
use openstitch_design::Compiler;

#[test]
fn golden_project_compiles_to_recorded_statistics() {
    let text = include_str!("fixtures/badge-golden.osd");
    let envelope: serde_json::Value = serde_json::from_str(text).expect("json");
    assert_eq!(envelope["format"], "openstitch.design");
    assert_eq!(envelope["version"], 1);
    let design =
        openstitch_design::project::decode(text.as_bytes()).expect("golden design decodes");
    let p = Compiler::default().compile(&design);
    // 3055 until 2026-10-27: one zero-length stitch less where each of the 4 jumps lands
    // (compat pes-default-v6), then the satin's under sewing travels inside the region to the
    // top satin instead of trimming and jumping there (4 jumps and 4 trims before), 3 fewer
    // because plain fill rows run edge to edge, as PE-DESIGN places them (each row moves half
    // a row spacing; the ellipse's end rows change), then 6 more because the satin is
    // PE-DESIGN's sawtooth with a tie-in and end lock (3068 before), then 121 fewer because
    // the satin's 20 mm crossings stay whole, as PE-DESIGN writes them (3074 before), 4 more
    // because the running outline ties in and locks as PE-DESIGN's lines do (2953 before), then
    // 17 fewer when the satin started at its outline's first point with PE's opening row
    // (2957 before).
    assert_eq!(p.count(Command::Stitch), 2940);
    assert_eq!(p.count(Command::Jump), 3);
    assert_eq!(p.count(Command::Trim), 3);
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
