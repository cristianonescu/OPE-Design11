use openstitch_design::{ObjectKind, v2};
use openstitch_engine::{CommandError, Session, project};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-engine-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn undo_redo_one_step_per_command() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    run(&mut s, "object.move", json!({"id": 1, "dx": 5}));
    assert_eq!(s.design().objects[0].bounds().map(|b| b.min_x), Some(50.0));
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().objects[0].bounds().map(|b| b.min_x), Some(0.0));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty());
    assert!(s.execute("history.undo", None).is_err());
    run(&mut s, "history.redo", json!({}));
    assert_eq!(s.design().objects.len(), 1);
}

#[test]
fn read_commands_make_no_history() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "design.info", json!({}));
    run(&mut s, "object.list", json!({}));
    assert!(!s.can_undo());
}

#[test]
fn transforms_use_selection_centre() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 10}),
    );
    run(&mut s, "object.rotate", json!({"id": 1, "degrees": 90}));
    let b = s.design().objects[0].bounds().expect("bounds");
    assert!((b.width() - 100.0).abs() < 1e-9 && (b.height() - 200.0).abs() < 1e-9);
    assert!((b.center().x - 100.0).abs() < 1e-9 && (b.center().y - 50.0).abs() < 1e-9);
    run(&mut s, "object.center", json!({"id": 1}));
    let c = s.design().objects[0].bounds().expect("bounds").center();
    assert!((c.x - 500.0).abs() < 1e-9 && (c.y - 500.0).abs() < 1e-9);
    assert_eq!(
        run(
            &mut s,
            "object.duplicate",
            json!({"ids": [1], "dx": 10, "dy": 0})
        )["ids"],
        json!([2])
    );
    run(&mut s, "object.reorder", json!({"id": 2, "index": 0}));
    assert_eq!(
        s.design().objects.iter().map(|o| o.id).collect::<Vec<_>>(),
        [2, 1]
    );
}

#[test]
fn shapes_and_attributes() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 20, "fill": "Red", "outline": "#000000"}),
    );
    run(
        &mut s,
        "shape.ellipse",
        json!({"cx": 70, "cy": 50, "rx": 15, "fill": 13}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "satin", "angle": 90, "underlay": true}),
    );
    let stats = run(&mut s, "design.generate", json!({}));
    assert_eq!(stats["colors"], json!(["#ed171f", "#000000", "#ffff00"]));
    assert!(stats["stitches"].as_u64().unwrap_or(0) > 100);
    let o = &s.design().objects[0];
    assert_eq!(o.region.as_ref().map(|r| r.angle), Some(90.0));
}

#[test]
fn hole_sewing_reduces_stitches_and_persists() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 20, "height": 20}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 5, "y": 5, "width": 10, "height": 10, "fill": "Red"}),
    );
    let before = run(&mut s, "design.generate", json!({}))["stitches"].as_u64();
    run(
        &mut s,
        "object.setHoleSewing",
        json!({"id": 1, "enabled": true}),
    );
    let after = run(&mut s, "design.generate", json!({}))["stitches"].as_u64();
    assert!(after < before);
    run(&mut s, "project.save", json!({"path": "h.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "h.osd"}));
    assert!(t.design().objects[0].hole_sewing);
    assert_eq!(t.design(), s.design());
    assert!(t.can_undo());
}

#[test]
fn opens_golden_project() {
    let text = include_str!("../../openstitch-design/tests/fixtures/badge-golden.osd");
    let d = project::decode(text.as_bytes()).expect("golden .osd decodes");
    assert_eq!(d.objects.len(), 2);
    assert_eq!(d.objects[0].kind, ObjectKind::Shape);
    let back = project::decode(&project::encode(&d).expect("encode")).expect("re-decode");
    assert_eq!(back, d);
}

#[test]
fn malformed_projects_rejected() {
    for text in [
        "",
        "{}",
        "[1,2]",
        r#"{"format":"other","version":1}"#,
        r#"{"format":"openstitch.design","version":99}"#,
        r#"{"format":"openstitch.design","version":1,"design":{"name":1}}"#,
    ] {
        assert!(project::decode(text.as_bytes()).is_err(), "{text}");
    }
}

#[test]
fn bad_parameters_fail_cleanly() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("nope", json!({})),
        ("shape.rectangle", json!({})),
        (
            "shape.rectangle",
            json!({"x": "1", "y": 0, "width": 10, "height": 10}),
        ),
        (
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": -10, "height": 10}),
        ),
        (
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 1e300, "height": 10}),
        ),
        (
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 10, "height": 10, "fill": "#zzzzzz"}),
        ),
        ("shape.ellipse", json!({"cx": 0, "cy": 0, "rx": 0})),
        ("shape.polygon", json!({"points": [[0, 0], [1, 1]]})),
        ("shape.polygon", json!({"points": [[0, 0], [1], [2, 2]]})),
        ("shape.polygon", json!({"points": "0,0"})),
        ("object.move", json!({"id": 999, "dx": 1})),
        ("object.move", json!({"ids": [], "dx": 1})),
        ("object.move", json!({"ids": [1.5]})),
        ("object.move", json!({"id": -1})),
        ("object.setRegion", json!({"id": 1, "type": "sparkle"})),
        ("object.setRegion", json!({"id": 1, "density": 0})),
        ("object.setRegion", json!({"id": 1, "underlayStyle": "zag"})),
        ("object.setRegion", json!({"id": 1, "splitLength": 1})),
        ("object.setRegion", json!({"id": 1, "pattern": "heart"})),
        (
            "pattern.define",
            json!({"pattern": {"format": "openstitch.pattern"}}),
        ),
        ("pattern.delete", json!({"kind": "fill", "id": "nope"})),
        (
            "sewing.define",
            json!({"stitch": {"format": "openstitch.sewing"}}),
        ),
        ("sewing.delete", json!({"id": "nope"})),
        (
            "object.setRegion",
            json!({"id": 1, "type": "netFill", "spacing": 0}),
        ),
        ("object.setLine", json!({"id": 1, "runTimes": 2.5})),
        (
            "object.setLine",
            json!({"id": 1, "type": "motif", "motif": "nope"}),
        ),
        ("object.setLine", json!({"id": 1, "arrange": "sideways"})),
        ("object.setLine", json!({"id": 1, "angle": 200})),
        ("object.flip", json!({"id": 1, "axis": "diagonal"})),
        ("object.reorder", json!({"id": 1, "index": 7})),
        ("design.export", json!({"path": "x.pes", "pesVersion": "7"})),
        ("design.export", json!({"path": "x.shv"})),
        ("design.export", json!({"path": "x.pes", "split": "yes"})),
        ("import.svg", json!({})),
        ("background.set", json!({"opacity": 0.5})),
        ("background.clear", json!({})),
        ("object.setHoleSewing", json!({"id": 1, "enabled": "yes"})),
        ("project.open", json!({"path": "/nonexistent/file.osd"})),
        ("design.new", json!({"width": 0})),
        ("design.rename", json!({"name": ""})),
        ("shape.arc", json!({"cx": 0, "cy": 0, "rx": 10, "sweep": 0})),
        (
            "shape.arc",
            json!({"cx": 0, "cy": 0, "rx": 10, "sweep": 90, "mode": "x"}),
        ),
        (
            "shape.preset",
            json!({"kind": "star", "cx": 0, "cy": 0, "width": 10, "sides": 99}),
        ),
        ("object.group", json!({"id": 1})),
        ("object.ungroup", json!({"id": 1})),
        (
            "object.arrangeCopies",
            json!({"id": 1, "mode": "circle", "count": 0}),
        ),
        (
            "object.pasteAttributes",
            json!({"from": 1, "id": 1, "attributes": [1]}),
        ),
        ("shape.merge", json!({"ids": [1]})),
        ("shape.combine", json!({"ids": [1, 1], "op": "union"})),
        (
            "shape.split",
            json!({"id": 1, "points": [[20, 20], [30, 20], [30, 30]]}),
        ),
        ("shape.offsetLines", json!({"id": 1, "count": 0})),
        ("object.setEntryExit", json!({"id": 1})),
        ("shape.floral", json!({"id": 1, "pattern": 999})),
        ("shape.decorative", json!({"kind": "laurel"})),
        ("attributes.apply", json!({"name": "missing", "id": 1})),
        ("threads.remap", json!({"chart": "missing"})),
        ("shape.column", json!({"lines": [[[0, 0], [0, 10]]]})),
        ("object.setDirectionLines", json!({"id": 1, "lines": 7})),
        ("order.move", json!({"id": 1, "to": "sideways"})),
        ("object.setJumpTrimming", json!({"id": 1, "enabled": 1})),
        ("object.setCenterPoint", json!({"id": 1, "point": [1, 1]})),
        (
            "object.setDirectionCurve",
            json!({"id": 1, "points": [[0, 0]]}),
        ),
        (
            "shape.emboss",
            json!({"id": 1, "tool": 1, "effect": "engravedLine"}),
        ),
        ("object.deleteStamp", json!({"id": 1, "index": 0})),
        ("object.setEntryExit", json!({"id": 1, "regionEntry": [1]})),
        ("fonts.loadTrueType", json!({})),
        (
            "fonts.loadTrueType",
            json!({"path": "/nonexistent/font.ttf"}),
        ),
        (
            "fonts.loadTrueType",
            json!({"path": "x.ttf", "id": "Bad Id"}),
        ),
        ("fonts.loadTrueType", json!({"path": "x.ttf", "face": -1})),
        ("fonts.loadTrueType", json!({"path": "x.ttf", "face": 1000})),
        ("fonts.remove", json!({"id": "nope"})),
        ("fonts.remove", json!({"id": "lato-bold"})),
        (
            "wizard.backgroundFill",
            json!({"ids": [1], "fillType": "plaid"}),
        ),
        ("wizard.backgroundFill", json!({"ids": [1], "spacing": 0})),
        ("wizard.backgroundFill", json!({"ids": [1], "area": 1})),
        ("wizard.applique", json!({"ids": [1], "cover": "running"})),
        ("wizard.patch", json!({"ids": [1], "width": 0})),
        ("wizard.cutwork", json!({"ids": [1], "netPattern": 6})),
        ("wizard.cutwork", json!({"ids": [1], "coverWidth": 10})),
        (
            "wizard.patch",
            json!({"ids": [1], "border": false, "cutting": "none"}),
        ),
        (
            "wizard.applique",
            json!({"ids": [1], "output": "replace", "distance": 1}),
        ),
        ("fonts.create", json!({"id": "lato-bold"})),
        ("fonts.create", json!({"id": "x", "height": 0})),
        ("fonts.create", json!({"id": "x", "family": ""})),
        (
            "fonts.mapCharacter",
            json!({"font": "nope", "char": "A", "object": 1}),
        ),
        (
            "fonts.mapCharacter",
            json!({"font": "lato-bold", "char": "A", "object": 1}),
        ),
        (
            "fonts.mapFiles",
            json!({"font": "nope", "paths": ["a.pes"]}),
        ),
        ("fonts.unmapCharacter", json!({"font": "nope", "char": "A"})),
        (
            "fonts.save",
            json!({"font": "lato-bold", "path": "x.osfont"}),
        ),
        ("fonts.open", json!({"path": "/nonexistent/x.osfont"})),
        (
            "fonts.setGlyph",
            json!({"font": "nope", "char": "A", "id": 1, "x": 0, "y": 0}),
        ),
        (
            "fonts.editGlyph",
            json!({"font": "nope", "char": "A", "x": 0, "y": 0}),
        ),
        (
            "fonts.convertTrueType",
            json!({"font": "nope", "from": "lato-bold", "chars": "A"}),
        ),
        ("fonts.setMetrics", json!({"font": "nope", "height": 5})),
        ("monogram.add", json!({"letters": "ABCD"})),
        ("monogram.edit", json!({"id": 1})),
        ("decoration.add", json!({"frame": "nope", "id": 1})),
        ("text.setNameDrop", json!({"id": 1})),
        ("nameDrop.apply", json!({"values": ["x"]})),
        ("nameDrop.export", json!({"rows": ["x"], "name": "x"})),
        ("nameDrop.readCsv", json!({"path": "/nonexistent.csv"})),
        ("template.open", json!({"template": "builtin:nope"})),
        ("template.fill", json!({"values": []})),
        ("template.save", json!({"path": "x.ostemplate"})),
    ];
    for (id, params) in bad {
        let r = s.execute(id, Some(&params));
        assert!(r.is_err(), "{id} {params} should fail");
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
    assert!(s.execute("shape.rectangle", Some(&json!([1, 2]))).is_err());
}

#[test]
fn sandbox_blocks_escapes() {
    let root = temp_dir();
    let outside = temp_dir();
    std::fs::write(outside.join("secret.osd"), "{}").expect("write");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("link")).expect("symlink");
    let mut s = Session::sandboxed(&root);
    for path in [
        "/etc/passwd",
        "../x.osd",
        "a/../../x.osd",
        "./x.osd",
        "",
        "dir\\x.osd",
        "missing-dir/x.osd",
    ] {
        let r = s.execute("project.save", Some(&json!({ "path": path })));
        assert!(
            matches!(r, Err(CommandError::Failed { .. })),
            "{path:?} should be rejected"
        );
    }
    #[cfg(unix)]
    assert!(
        s.execute("project.open", Some(&json!({"path": "link/secret.osd"})))
            .is_err()
    );
    run(&mut s, "project.save", json!({"path": "ok.osd"}));
    assert!(root.join("ok.osd").exists());
}

#[test]
fn control_reply_shape() {
    let mut s = Session::new(temp_dir());
    assert_eq!(
        s.handle(&json!({"id": 7, "command": "design.info"}))["ok"],
        json!(true)
    );
    assert_eq!(
        s.handle(&json!({"id": 8, "command": "design.info"}))["id"],
        json!(8)
    );
    assert_eq!(s.handle(&json!({"command": 3}))["ok"], json!(false));
    assert_eq!(
        s.handle(&json!({"command": "shape.rectangle", "params": [1, 2]}))["ok"],
        json!(false)
    );
}

#[test]
fn commands_documented_and_unique() {
    let mut s = Session::new(temp_dir());
    let list = run(&mut s, "commands.list", json!({}));
    let ids: Vec<&str> = list
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    let mut unique = ids.clone();
    unique.dedup();
    assert_eq!(ids.len(), unique.len());
    assert_eq!(ids.len(), s.commands().len());
    assert!(s.commands().iter().all(|c| !c.summary.is_empty()));
    let _ = v2(0.0, 0.0);
}

/// Golden regression: `examples/badge.json` exported `badge-golden.pes` (version 1, no objects)
/// on 2026-10-18, re-recorded on 2026-10-27 without the four zero-length `00 00` records at
/// jump landings (compat pes-default-v6; otherwise identical), and again when satin took
/// PE-DESIGN's defaults (4.5 lines/mm, under sewing linked to the top satin), when fill did
/// (the ellipse: 4.5 rows/mm, under sewing on, rows edge to edge), and when satin became PE's
/// sawtooth with a tie-in and end lock, and when direction angles turned counter-clockwise
/// (the script's 45° ellipse fill now rises to the right), and when satin crossings stopped
/// being split at 12 mm, and when the ellipse and rectangle took PE-DESIGN's start and end point
/// in the middle of the bottom edge, and when their outlines started sewing there too. The
/// same design
/// exported as `1t`
/// must keep writing the identical bytes; the
/// script's default export is now version 6 with objects and sews the same stitches.
#[cfg(feature = "formats")]
#[test]
fn script_output_matches_golden_pes() {
    use openstitch_formats::pes;
    let dir = temp_dir();
    let script: Value = serde_json::from_str(include_str!("fixtures/badge.json")).expect("script");
    let mut s = Session::new(&dir);
    for step in script.as_array().expect("array") {
        let reply = s.handle(step);
        assert_eq!(reply["ok"], json!(true), "{reply}");
    }
    run(
        &mut s,
        "design.export",
        json!({"path": "badge-1t.pes", "pesVersion": "1t"}),
    );
    let golden = include_bytes!("fixtures/badge-golden.pes");
    let machine_only = std::fs::read(dir.join("badge-1t.pes")).expect("exported");
    assert_eq!(machine_only, golden);
    let rust = std::fs::read(dir.join("badge.pes")).expect("exported");
    let objects = pes::read_objects(&rust).expect("objects");
    assert_eq!(objects.version, "0060");
    assert!(!objects.objects.is_empty() && !objects.threads.is_empty());
    let (v6, v1) = (
        pes::read(&rust).expect("read"),
        pes::read(golden).expect("read"),
    );
    assert_eq!(v6.stitches, v1.stitches);
}

#[cfg(feature = "formats")]
#[test]
fn import_export_round_trip_and_bad_files() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 30, "height": 20, "fill": "Red"}),
    );
    for ext in [
        "pes", "pec", "dst", "exp", "jef", "pcs", "sew", "xxx", "hus", "vip",
    ] {
        run(
            &mut s,
            "design.export",
            json!({ "path": format!("r.{ext}") }),
        );
        let mut t = Session::new(&dir);
        run(
            &mut t,
            "stitches.import",
            json!({ "path": format!("r.{ext}") }),
        );
        assert_eq!(t.design().objects[0].kind, ObjectKind::Stitches, "{ext}");
    }
    std::fs::write(dir.join("junk.pes"), b"#PES0001\xff\xff\xff\x7f").expect("write");
    assert!(
        s.execute("stitches.import", Some(&json!({"path": "junk.pes"})))
            .is_err()
    );
    assert!(
        s.execute("design.export", Some(&json!({"path": "x.svg"})))
            .is_err()
    );
    // PES version 6 carries the exact RGB threads; version 1 maps to the PEC chart.
    let mut c = Session::new(&dir);
    run(
        &mut c,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 30, "height": 20, "fill": "#123456"}),
    );
    for (version, exact) in [("6", true), ("6t", true), ("1", false)] {
        run(
            &mut c,
            "design.export",
            json!({ "path": format!("v{version}.pes"), "pesVersion": version }),
        );
        let bytes = std::fs::read(dir.join(format!("v{version}.pes"))).expect("pes");
        let back = openstitch_formats::pes::read(&bytes).expect("read");
        assert_eq!(back.threads[0].hex() == "#123456", exact, "{version}");
    }
    assert!(
        s.execute(
            "design.export",
            Some(&json!({"path": "x.pes", "pesVersion": "7"}))
        )
        .is_err()
    );
    // Read-only formats are refused before anything is written.
    let err = s
        .execute("design.export", Some(&json!({"path": "x.shv"})))
        .expect_err("read only");
    assert!(err.to_string().contains("read only"), "{err}");
    assert!(!dir.join("x.shv").exists());
}

#[test]
fn read_roots_are_read_only_write_roots_read_write() {
    use openstitch_engine::{Access, Sandbox};
    let work = temp_dir();
    let library = temp_dir().join("library");
    std::fs::create_dir_all(&library).expect("library");
    let mut s = Session::with_sandbox(Sandbox::new(&[&library], &[&work]).expect("sandbox"));
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );

    // Relative paths land in the first write root; absolute paths inside a root are fine.
    run(&mut s, "project.save", json!({"path": "a.osd"}));
    assert!(work.join("a.osd").exists());
    let abs = work.join("b.osd").display().to_string();
    run(&mut s, "project.save", json!({ "path": abs }));

    // Read roots refuse writes but allow reads.
    let in_library = library.join("a.osd");
    let r = s.execute(
        "project.save",
        Some(&json!({ "path": in_library.display().to_string() })),
    );
    assert!(
        matches!(&r, Err(CommandError::Failed { reason, .. }) if reason.contains("outside the allowed write roots")),
        "{r:?}"
    );
    assert!(!in_library.exists());
    std::fs::copy(work.join("a.osd"), &in_library).expect("copy");
    run(
        &mut s,
        "project.open",
        json!({ "path": in_library.display().to_string() }),
    );

    let sandbox = s.sandbox().expect("sandbox");
    assert!(
        sandbox
            .resolve(&in_library.display().to_string(), Access::Read)
            .is_ok()
    );
    assert!(
        sandbox
            .resolve(&in_library.display().to_string(), Access::Write)
            .is_err()
    );
    for bad in ["a/./b.osd", "a/../b.osd", "..", ".", "x\0y"] {
        assert!(sandbox.resolve(bad, Access::Read).is_err(), "{bad:?}");
    }
}

#[test]
fn sandbox_without_roots_refuses_files() {
    use openstitch_engine::{Access, Sandbox};
    let mut s = Session::with_sandbox(Sandbox::deny_all());
    let r = s.execute("project.save", Some(&json!({"path": "a.osd"})));
    assert!(
        matches!(&r, Err(CommandError::Failed { reason, .. }) if reason.contains("no write root")),
        "{r:?}"
    );
    assert!(
        Sandbox::deny_all()
            .resolve("a.osd", Access::Read)
            .is_err_and(|e| e.0.contains("no read root"))
    );
    // Missing roots are refused at construction; `sandboxed` fails closed.
    let missing = temp_dir().join("missing");
    assert!(Sandbox::new::<PathBuf, _>(&[], &[&missing]).is_err());
    let mut closed = Session::sandboxed(&missing);
    assert!(
        closed
            .execute("project.save", Some(&json!({"path": "a.osd"})))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn sandbox_resolves_symlinked_roots_and_dangling_links() {
    use openstitch_engine::{Access, Sandbox};
    let real = temp_dir();
    let alias = temp_dir().join("alias");
    std::os::unix::fs::symlink(&real, &alias).expect("symlink");
    let outside = temp_dir();
    std::os::unix::fs::symlink(outside.join("target.osd"), real.join("dangling.osd"))
        .expect("symlink");
    let sandbox = Sandbox::new::<PathBuf, _>(&[], &[&alias]).expect("sandbox");
    let canonical_root = real.canonicalize().expect("canonical");
    assert_eq!(sandbox.write_roots(), std::slice::from_ref(&canonical_root));
    let p = sandbox
        .resolve(&alias.join("x.pes").display().to_string(), Access::Write)
        .expect("inside");
    assert!(p.starts_with(&canonical_root));
    // A dangling symlink pointing outside must not be written through.
    assert!(sandbox.resolve("dangling.osd", Access::Write).is_err());
    assert!(std::fs::read_dir(&outside).expect("dir").next().is_none());
}

#[test]
fn scale_and_rotate_about_explicit_origin() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 10}),
    );
    // Corner-handle drag: scale ×2 keeping the top-left corner (10, 10) mm fixed.
    run(
        &mut s,
        "object.scale",
        json!({"id": 1, "sx": 2, "originX": 10, "originY": 10}),
    );
    let b = s.design().objects[0].bounds().expect("bounds");
    assert_eq!(
        (b.min_x, b.min_y, b.max_x, b.max_y),
        (100.0, 100.0, 500.0, 300.0)
    );
    run(
        &mut s,
        "object.rotate",
        json!({"id": 1, "degrees": 90, "originX": 10, "originY": 10}),
    );
    let b = s.design().objects[0].bounds().expect("bounds");
    assert!((b.max_x - 100.0).abs() < 1e-9 && (b.min_y - 100.0).abs() < 1e-9);
    // Half an origin is an error and changes nothing.
    let before = s.design().clone();
    assert!(
        s.execute(
            "object.scale",
            Some(&json!({"id": 1, "sx": 2, "originX": 10}))
        )
        .is_err()
    );
    assert_eq!(s.design(), &before);
}

#[test]
fn path_with_curves_and_point_editing() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    // A lens: two vertices, two curved segments.
    run(
        &mut s,
        "shape.path",
        json!({"points": [[10, 20], [50, 20]], "closed": true,
               "curves": [[[20, 5], [40, 5]], [[40, 35], [20, 35]]], "fill": "Red"}),
    );
    let b = s.design().objects[0].bounds().expect("bounds");
    assert!(
        b.min_y < 200.0 && b.max_y > 200.0,
        "curves bulge both ways: {b:?}"
    );
    let stitches = run(&mut s, "design.generate", json!({}))["stitches"]
        .as_u64()
        .unwrap_or(0);
    assert!(stitches > 50);

    let r = run(
        &mut s,
        "object.insertPoint",
        json!({"id": 1, "segment": 0, "t": 0.5}),
    );
    assert_eq!(r["index"], json!(1));
    assert_eq!(s.design().objects[0].contours[0].points.len(), 3);

    run(
        &mut s,
        "object.setPoint",
        json!({"id": 1, "index": 1, "x": 30, "y": 0}),
    );
    let c = &s.design().objects[0].contours[0];
    assert_eq!(c.points[1], v2(300.0, 0.0));

    run(
        &mut s,
        "object.setSegment",
        json!({"id": 1, "segment": 0, "straight": true}),
    );
    assert!(s.design().objects[0].contours[0].curve(0).is_none());
    run(&mut s, "object.setSegment", json!({"id": 1, "segment": 0}));
    assert!(s.design().objects[0].contours[0].curve(0).is_some());

    run(&mut s, "object.deletePoint", json!({"id": 1, "index": 1}));
    assert_eq!(s.design().objects[0].contours[0].points.len(), 2);

    // Each edit was one undo step.
    for _ in 0..5 {
        run(&mut s, "history.undo", json!({}));
    }
    assert_eq!(s.design().objects[0].contours[0].points.len(), 2);
    assert!(s.design().objects[0].contours[0].curve(0).is_some());

    run(&mut s, "project.save", json!({"path": "curves.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "curves.osd"}));
    assert_eq!(t.design(), s.design());
}

#[test]
fn path_and_point_commands_reject_bad_input() {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let bad: Vec<(&str, Value)> = vec![
        ("shape.path", json!({"points": [[0, 0], [1, 1]]})),
        (
            "shape.path",
            json!({"points": [[0, 0], [1, 1], [2, 0]], "curves": [null]}),
        ),
        (
            "shape.path",
            json!({"points": [[0, 0], [1, 1], [2, 0]], "curves": [[[0, 0]], null, null]}),
        ),
        (
            "shape.path",
            json!({"points": [[0, 0], [1, 1], [2, 0]], "curves": [[[0, 0], [1, "x"]], null, null]}),
        ),
        (
            "object.setPoint",
            json!({"id": 1, "index": 4, "x": 0, "y": 0}),
        ),
        (
            "object.setPoint",
            json!({"id": 1, "index": 0, "x": 1e9, "y": 0}),
        ),
        (
            "object.setPoint",
            json!({"id": 1, "contour": 1, "index": 0, "x": 0, "y": 0}),
        ),
        (
            "object.setPoint",
            json!({"id": 9, "index": 0, "x": 0, "y": 0}),
        ),
        ("object.insertPoint", json!({"id": 1, "segment": 4})),
        ("object.insertPoint", json!({"id": 1, "segment": 0, "t": 1})),
        ("object.deletePoint", json!({"id": 1, "index": -1})),
        (
            "object.setSegment",
            json!({"id": 1, "segment": 0, "straight": true, "curve": [[0, 0], [1, 1]]}),
        ),
        ("object.setSegment", json!({"id": 1, "segment": 9})),
    ];
    for (id, params) in bad {
        assert!(
            s.execute(id, Some(&params)).is_err(),
            "{id} {params} should fail"
        );
        assert_eq!(s.design(), &before, "{id} changed the design");
    }
    // A closed contour keeps at least three vertices.
    run(&mut s, "object.deletePoint", json!({"id": 1, "index": 0}));
    assert!(
        s.execute("object.deletePoint", Some(&json!({"id": 1, "index": 0})))
            .is_err()
    );
}

#[cfg(feature = "formats")]
#[test]
fn exports_over_a_format_limit_fail_or_split_into_numbered_files() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    // 260 colour blocks: more than PES/PEC's 255.
    for i in 0..260 {
        let (x, y) = (f64::from(i % 20) * 4.0, f64::from(i / 20) * 4.0);
        run(
            &mut s,
            "shape.rectangle",
            json!({"x": x, "y": y, "width": 3, "height": 3,
                   "fill": format!("#{:06x}", (i * 0x00_3F_17) & 0xFF_FFFF)}),
        );
    }
    let err = s
        .execute("design.export", Some(&json!({"path": "big.pes"})))
        .expect_err("too many colours");
    assert!(err.to_string().contains("split: true"), "{err}");
    assert!(!dir.join("big.pes").exists());

    let out = run(
        &mut s,
        "design.export",
        json!({"path": "big.pes", "split": true, "pesVersion": "6"}),
    );
    let paths = out["paths"].as_array().expect("paths");
    assert_eq!(paths.len(), 2, "{out}");
    let mut blocks = 0;
    for (i, p) in paths.iter().enumerate() {
        let p = p.as_str().expect("path");
        assert!(p.ends_with(&format!("big-{}.pes", i + 1)), "{p}");
        let back = openstitch_formats::pes::read(&std::fs::read(p).expect("read")).expect("pes");
        blocks += back.color_block_count();
    }
    assert_eq!(blocks, 260);
    // A design that fits keeps its single file name.
    let mut small = Session::new(&dir);
    run(
        &mut small,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let out = run(
        &mut small,
        "design.export",
        json!({"path": "small.dst", "split": true}),
    );
    assert_eq!(out["paths"].as_array().map(Vec::len), Some(1));
    assert!(dir.join("small.dst").exists());
}
