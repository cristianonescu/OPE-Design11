//! Engine commands for the line and region sew types beyond running/triple/zigzag/fill/satin.

use openstitch_core::Command;
use openstitch_design::{Arrange, ChainShape, Compiler, LineType};
use openstitch_engine::{Session, project};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-stitch-types-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn session_with_circle() -> Session {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 30, "ry": 20, "outline": "Black"}),
    );
    s
}

#[test]
fn patterns_are_listed_by_kind() {
    let mut s = Session::new(temp_dir());
    let list = run(&mut s, "patterns.list", json!({}));
    let count = |kind: &str| {
        list.as_array()
            .expect("array")
            .iter()
            .filter(|p| p["kind"] == kind)
            .count()
    };
    assert!(count("motif") >= 6);
    assert!(count("fill") >= 6);
    assert!(count("decorative") >= 6);
    assert_eq!(count("net"), 6);
}

#[test]
fn every_line_type_is_set_sewn_saved_and_undone() {
    let mut s = session_with_circle();
    for kind in LineType::ALL {
        run(
            &mut s,
            "object.setLine",
            json!({"id": 1, "type": kind.id()}),
        );
        assert_eq!(
            s.design().objects[0].line.as_ref().map(|l| l.kind),
            Some(kind)
        );
        let p = Compiler::default().compile(s.design());
        assert!(p.count(Command::Stitch) > 20, "{kind:?}");
    }
    run(
        &mut s,
        "object.setLine",
        json!({
            "id": 1, "type": "motif", "motif": "heart", "width": 5, "height": 4,
            "spacing": 1, "arrange": "alternate", "pitch": 1.5
        }),
    );
    let l = s.design().objects[0].line.clone().expect("line");
    assert_eq!(l.motif.as_deref(), Some("heart"));
    assert_eq!((l.width, l.height, l.spacing), (5.0, Some(4.0), Some(1.0)));
    assert_eq!(l.arrange, Arrange::Alternate);
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "chain", "chainShape": "diamond", "runTimes": 6}),
    );
    let l = s.design().objects[0].line.clone().expect("line");
    assert_eq!((l.chain_shape, l.run_times), (ChainShape::Diamond, 6));
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "stem", "angle": 120, "single": true, "spacing": 2}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "eStitch", "strokeTimes": 3}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "zigzag", "halfStitch": true}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "cutting", "leaveUncut": true}),
    );
    // Round trip through .osd keeps every attribute.
    let bytes = project::encode(s.design()).expect("encode");
    assert_eq!(&project::decode(&bytes).expect("decode"), s.design());
    run(&mut s, "history.undo", json!({}));
    assert_eq!(
        s.design().objects[0].line.as_ref().map(|l| l.kind),
        Some(LineType::Zigzag)
    );
}

#[test]
fn bad_line_parameters_fail_cleanly() {
    let mut s = session_with_circle();
    let before = s.design().clone();
    for params in [
        json!({"id": 1, "type": "sparkle"}),
        json!({"id": 1, "type": "motif", "motif": "nope"}),
        json!({"id": 1, "motif": 3}),
        json!({"id": 1, "height": 0}),
        json!({"id": 1, "spacing": 50}),
        json!({"id": 1, "type": "stem", "spacing": 0}),
        json!({"id": 1, "angle": 10}),
        json!({"id": 1, "strokeTimes": 4}),
        json!({"id": 1, "arrange": "sideways"}),
        json!({"id": 1, "chainShape": "circle"}),
        json!({"id": 1, "leaveUncut": "yes"}),
        json!({"id": 1, "halfStitch": 1}),
        json!({"id": 1, "type": "running", "runTimes": 6}),
        json!({"id": 1, "type": "chain", "runTimes": 7}),
    ] {
        assert!(
            s.execute("object.setLine", Some(&params)).is_err(),
            "{params} should fail"
        );
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}

#[test]
fn satin_and_fill_options_are_set_and_undone() {
    use openstitch_design::{FeatherSide, RegionType, UnderlayStyle};
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 30, "ry": 10, "fill": "Red"}),
    );
    let plain = Compiler::default().compile(s.design());
    // Gradation and Feathered edge exclude each other and Auto direction (the guide), so they
    // are set one after another.
    run(
        &mut s,
        "object.setRegion",
        json!({
            "id": 1, "type": "satin", "underlay": true, "underlayStyle": "double",
            "underlaySpacing": 3, "gradation": 40, "splitLength": 4, "travel": true
        }),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.kind, RegionType::Satin);
    assert!(r.running_stitch_path.is_some() && r.underlay);
    assert_eq!(r.underlay_style, UnderlayStyle::Double);
    assert_eq!(
        (r.underlay_spacing, r.gradation, r.split_length),
        (
            Some(3.0),
            Some(openstitch_design::Gradation::continuous(40.0)),
            Some(4.0)
        )
    );
    let graded = Compiler::default().compile(s.design());
    assert_ne!(graded, plain);
    assert!(graded.count(Command::Stitch) > 50);
    // 100 % turns gradation off; a feathered edge can then be set.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradation": 100, "featherSide": "both", "featherWidth": 1.5}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.gradation, None);
    assert_eq!(
        r.feather.map(|f| (f.side, f.width)),
        Some((FeatherSide::Both, 1.5))
    );
    assert_ne!(Compiler::default().compile(s.design()), graded);
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "autoDirection": true}),
    );
    assert!(
        s.design().objects[0]
            .region
            .as_ref()
            .expect("r")
            .auto_direction
    );
    // Zero feather width and zero split length turn the options off.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "autoDirection": false, "featherWidth": 0, "splitLength": 0}),
    );
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!((r.gradation, r.feather, r.split_length), (None, None, None));
    let bytes = project::encode(s.design()).expect("encode");
    assert_eq!(&project::decode(&bytes).expect("decode"), s.design());
    for _ in 0..4 {
        run(&mut s, "history.undo", json!({}));
    }
    assert_eq!(Compiler::default().compile(s.design()), plain);
}

#[test]
fn gradation_pattern_options() {
    use openstitch_design::Gradation;
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 30, "height": 30, "fill": "Red"}),
    );
    let gradation = |s: &Session| s.design().objects[0].region.as_ref().expect("r").gradation;
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationPattern": true}),
    );
    assert_eq!(gradation(&s), Some(Gradation::PE_DEFAULT));
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationMax": 80, "gradationSteps": 4}),
    );
    assert_eq!(
        gradation(&s),
        Some(Gradation {
            max: 80.0,
            min: 5.0,
            steps: Some(4)
        })
    );
    // Turning the pattern on again keeps it; off clears it.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationPattern": true}),
    );
    assert_eq!(gradation(&s).and_then(|g| g.steps), Some(4));
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationPattern": false}),
    );
    assert_eq!(gradation(&s), None);
    // A continuous (old) gradation becomes PE's default pattern when a pattern option is set.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradation": 40}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationMin": 10}),
    );
    assert_eq!(
        gradation(&s),
        Some(Gradation {
            max: 53.0,
            min: 10.0,
            steps: Some(9)
        })
    );
    for params in [
        json!({"id": 1, "gradationMax": 4}),
        json!({"id": 1, "gradationMin": 101}),
        json!({"id": 1, "gradationSteps": 1}),
        json!({"id": 1, "gradationSteps": 21}),
    ] {
        let reply = s.handle(&json!({"command": "object.setRegion", "params": params}));
        assert_eq!(reply["ok"], json!(false), "{params}");
    }
}

#[test]
fn gradation_and_feathered_edge_follow_the_guide_exclusions() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 30, "height": 10, "fill": "Red"}),
    );
    run(
        &mut s,
        "text.add",
        json!({"text": "A", "x": 0, "y": 30, "size": 10}),
    );
    let refused = |s: &mut Session, params: Value, why: &str| {
        let before = s.design().clone();
        let reply = s.handle(&json!({"command": "object.setRegion", "params": params}));
        assert_eq!(reply["ok"], json!(false), "{params}");
        assert!(reply.to_string().contains(why), "{params}: {reply}");
        assert_eq!(s.design(), &before, "{params}");
    };
    // Gradation and Feathered edge exclude each other.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationPattern": true}),
    );
    refused(
        &mut s,
        json!({"id": 1, "featherWidth": 2}),
        "with gradation",
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "gradationPattern": false}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "featherWidth": 2}),
    );
    refused(
        &mut s,
        json!({"id": 1, "gradationPattern": true}),
        "feathered edge",
    );
    // Both in one call: whichever comes second is refused.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "featherWidth": 0}),
    );
    refused(
        &mut s,
        json!({"id": 1, "gradation": 40, "featherWidth": 2}),
        "not available",
    );
    // Not with Auto direction, not on text, not for other sew types (yet).
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "autoDirection": true}),
    );
    refused(
        &mut s,
        json!({"id": 1, "gradationPattern": true}),
        "Auto direction",
    );
    refused(
        &mut s,
        json!({"id": 1, "featherWidth": 2}),
        "Auto direction",
    );
    refused(
        &mut s,
        json!({"id": 2, "gradationPattern": true}),
        "for text",
    );
    refused(&mut s, json!({"id": 2, "featherWidth": 2}), "for text");
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "programmableFill", "autoDirection": false}),
    );
    refused(
        &mut s,
        json!({"id": 1, "gradationPattern": true}),
        "fill and satin",
    );
}

#[test]
fn unavailable_gradation_and_feather_are_kept_but_not_sewn() {
    use openstitch_design::{Feather, FeatherSide, Gradation, RegionType};
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 30, "height": 10, "fill": "Red"}),
    );
    // A file may hold options that are unavailable now (an older version, a changed type).
    let mut d = s.design().clone();
    let r = d.objects[0].region.as_mut().expect("region");
    r.auto_direction = true;
    r.gradation = Some(Gradation::PE_DEFAULT);
    let auto_plain = {
        let mut a = s.design().clone();
        a.objects[0].region.as_mut().expect("r").auto_direction = true;
        Compiler::default().compile(&a)
    };
    assert_eq!(Compiler::default().compile(&d), auto_plain);
    let r = d.objects[0].region.as_mut().expect("region");
    r.auto_direction = false;
    r.feather = Some(Feather {
        side: FeatherSide::Both,
        width: 3.0,
    });
    // Gradation wins over a feathered edge stored with it.
    let mut graded = s.design().clone();
    graded.objects[0].region.as_mut().expect("r").gradation = Some(Gradation::PE_DEFAULT);
    assert_eq!(
        Compiler::default().compile(&d),
        Compiler::default().compile(&graded)
    );
    let r = d.objects[0].region.as_mut().expect("region");
    r.gradation = None;
    r.kind = RegionType::ProgrammableFill;
    assert_eq!(r.sewn_feather(), None);
    assert_ne!(
        Compiler::default().compile(s.design()),
        Compiler::default().compile(&graded)
    );
}

#[test]
fn bad_region_options_fail_cleanly() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    for params in [
        json!({"id": 1, "underlayStyle": "zag"}),
        json!({"id": 1, "underlaySpacing": 0.5}),
        json!({"id": 1, "autoDirection": "on"}),
        json!({"id": 1, "gradation": 2}),
        json!({"id": 1, "gradation": 101}),
        json!({"id": 1, "featherSide": "top"}),
        json!({"id": 1, "featherWidth": -1}),
        json!({"id": 1, "splitLength": 1}),
        json!({"id": 1, "splitLength": 13}),
        json!({"id": 1, "travel": 1}),
    ] {
        assert!(
            s.execute("object.setRegion", Some(&params)).is_err(),
            "{params} should fail"
        );
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}

#[test]
fn every_region_type_is_set_sewn_saved_and_undone() {
    use openstitch_design::RegionType;
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 50, "cy": 50, "rx": 25, "ry": 18, "fill": "Red"}),
    );
    for kind in RegionType::ALL {
        run(
            &mut s,
            "object.setRegion",
            json!({"id": 1, "type": kind.id()}),
        );
        assert_eq!(
            s.design().objects[0].region.as_ref().map(|r| r.kind),
            Some(kind)
        );
        let p = Compiler::default().compile(s.design());
        assert!(p.count(Command::Stitch) > 50, "{kind:?}");
    }
    let cases = [
        json!({"type": "programmableFill", "pattern": "bricks", "patternWidth": 6,
               "patternHeight": 4, "rowOffset": 50, "columnOffset": 25, "frequency": 20,
               "baseSewing": true}),
        json!({"type": "motifFill", "pattern": "leaf", "spacing": 0, "rowOffset": 30}),
        json!({"type": "piping", "runPitch": 3, "frequency": 50}),
        json!({"type": "crossStitch", "spacing": 2, "triple": true}),
        json!({"type": "concentricCircle", "startInside": true, "density": 1}),
        json!({"type": "spiral", "counterclockwise": true}),
        json!({"type": "flexibleSpiral", "spacing": 1.5, "startInside": true}),
        json!({"type": "stippling", "spacing": 1.5, "runPitch": 1.5}),
        json!({"type": "netFill", "pattern": "honeycomb", "spacing": 5}),
        json!({"type": "zigzagNetFill", "pattern": "triangle", "patternWidth": 1.2,
               "underlay": true}),
        json!({"type": "decorativeFill", "pattern": "scales", "randomShift": 20,
               "travel": false}),
    ];
    for mut c in cases {
        c["id"] = json!(1);
        run(&mut s, "object.setRegion", c.clone());
        let p = Compiler::default().compile(s.design());
        assert!(p.count(Command::Stitch) > 50, "{c}");
    }
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.pattern.as_deref(), Some("scales"));
    assert_eq!((r.random_shift, r.running_stitch_path), (Some(20.0), None));
    let bytes = project::encode(s.design()).expect("encode");
    assert_eq!(&project::decode(&bytes).expect("decode"), s.design());
    run(&mut s, "history.undo", json!({}));
    let r = s.design().objects[0].region.clone().expect("region");
    assert_eq!(r.kind, RegionType::ZigzagNetFill);
    assert!(r.underlay);
}

#[test]
fn bad_region_type_parameters_fail_cleanly() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "netFill"}),
    );
    let before = s.design().clone();
    for params in [
        json!({"id": 1, "type": "sparkle"}),
        json!({"id": 1, "pattern": "heart"}),
        json!({"id": 1, "type": "zigzagNetFill", "pattern": "waves"}),
        json!({"id": 1, "type": "fill", "pattern": "square"}),
        json!({"id": 1, "type": "programmableFill", "pattern": "nope"}),
        json!({"id": 1, "pattern": 5}),
        json!({"id": 1, "patternWidth": 0}),
        json!({"id": 1, "patternHeight": 60}),
        json!({"id": 1, "spacing": 0.2}),
        json!({"id": 1, "spacing": 25}),
        json!({"id": 1, "runPitch": 0.5}),
        json!({"id": 1, "frequency": 150}),
        json!({"id": 1, "rowOffset": -1}),
        json!({"id": 1, "columnOffset": 101}),
        json!({"id": 1, "startInside": "yes"}),
        json!({"id": 1, "counterclockwise": 0}),
        json!({"id": 1, "triple": "no"}),
        json!({"id": 1, "randomShift": 60}),
        json!({"id": 1, "baseSewing": null, "spacing": "wide"}),
    ] {
        assert!(
            s.execute("object.setRegion", Some(&params)).is_err(),
            "{params} should fail"
        );
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
}
