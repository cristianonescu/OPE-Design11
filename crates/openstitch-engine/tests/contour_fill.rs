//! Contour fill through the engine (backlog B1, beyond PE-DESIGN): `object.setRegion
//! {type: "contour", contourStrategy}` on a circle and a shape with a paired hole; saved, reopened, undone and validated. The geometry is tested in
//! `openstitch-design/src/contour_tests.rs`.

use openstitch_core::{Command, Stitch};
use openstitch_design::{Compiler, ContourStrategy, RegionType};
use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Needle points of the compiled design, mm.
fn needles(s: &Session) -> Vec<Stitch> {
    Compiler::default()
        .compile(s.design())
        .stitches
        .into_iter()
        .filter(|st| st.command == Command::Stitch)
        .collect()
}

#[test]
fn contour_fill_is_set_sewn_saved_and_undone() {
    let dir = std::env::temp_dir().join(format!("os-contour-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 20, "ry": 20, "fill": "Red"}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "contour"}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.kind, RegionType::Contour);
    assert_eq!(
        r.contour_strategy,
        ContourStrategy::Rings,
        "rings by default"
    );
    let rings = needles(&s);
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "contourStrategy": "singleSpiral", "spacing": 2.0, "runPitch": 2.5}),
    );
    let spiral = needles(&s);
    assert!(
        spiral.len() > 100 && rings.len() > spiral.len(),
        "{} {}",
        rings.len(),
        spiral.len()
    );
    // Running stitches no longer than the run pitch, apart from the tie-in and lock.
    let long = spiral
        .windows(2)
        .filter(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y) > 25.0 + 1e-6)
        .count();
    assert!(long <= 2, "{long} stitches longer than 2.5 mm");
    // No trim or jump between the first and the last needle point.
    let all = Compiler::default().compile(s.design()).stitches;
    let first = all
        .iter()
        .position(|st| st.command == Command::Stitch)
        .expect("stitches");
    let last = all
        .iter()
        .rposition(|st| st.command == Command::Stitch)
        .expect("stitches");
    let inside = all[first..last]
        .iter()
        .filter(|st| matches!(st.command, Command::Trim | Command::Jump))
        .count();
    assert_eq!(inside, 0, "no trims or jumps inside the object");
    // Echo lines, a mitred join and the double spiral are accepted; out-of-range values are not.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "contourStrategy": "doubleSpiral", "contourJoin": "miter", "echoLines": 3}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(
        (r.echo_lines, r.contour_join),
        (3, openstitch_design::ContourJoin::Miter)
    );
    assert!(needles(&s).len() > spiral.len(), "echo rings add stitches");
    for bad in [
        json!({"id": 1, "echoLines": 31}),
        json!({"id": 1, "contourJoin": "bevel"}),
    ] {
        assert!(s.execute("object.setRegion", Some(&bad)).is_err(), "{bad}");
    }
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "contourStrategy": "singleSpiral", "contourJoin": "round", "echoLines": 0}),
    );

    run(&mut s, "project.save", json!({"path": "contour.osd"}));
    let text = std::fs::read_to_string(dir.join("contour.osd")).expect("saved");
    assert!(
        text.contains("\"contourStrategy\": \"singleSpiral\""),
        "{text}"
    );
    run(&mut s, "design.new", json!({}));
    run(&mut s, "project.open", json!({"path": "contour.osd"}));
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(
        (r.kind, r.contour_strategy),
        (RegionType::Contour, ContourStrategy::SingleSpiral)
    );
    assert_eq!(
        needles(&s).len(),
        spiral.len(),
        "reopened designs sew the same"
    );

    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "contourStrategy": "rings"}),
    );
    run(&mut s, "history.undo", json!({}));
    assert_eq!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("region")
            .contour_strategy,
        ContourStrategy::SingleSpiral
    );
    assert!(
        s.execute(
            "object.setRegion",
            Some(&json!({"id": 1, "contourStrategy": "spiral"}))
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contour_fill_keeps_out_of_a_paired_hole() {
    let mut s = Session::new(std::env::temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 40, "fill": "Blue"}),
    );
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 30, "cy": 30, "rx": 6, "ry": 6, "fill": "Red"}),
    );
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"ids": [1, 2], "enabled": true}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "contour", "contourStrategy": "singleSpiral", "echoLines": 0}),
    );
    let first: Vec<Stitch> = {
        let all = Compiler::default().compile(s.design()).stitches;
        let end = all
            .iter()
            .position(|st| st.command == Command::ColorChange)
            .unwrap_or(all.len());
        all[..end]
            .iter()
            .filter(|st| st.command == Command::Stitch)
            .cloned()
            .collect()
    };
    assert!(first.len() > 100);
    let inside_hole = first
        .iter()
        .filter(|st| ((st.x - 300.0).hypot(st.y - 300.0)) < 55.0)
        .count();
    assert_eq!(inside_hole, 0, "no needle point inside the 6 mm hole");
}
