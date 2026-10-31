//! Artwork imports: SVG paths as shapes, raster background images.
#![cfg(all(feature = "formats", feature = "digitize"))]

use openstitch_design::ObjectKind;
use openstitch_digitize::Raster;
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-imports-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

const ARTWORK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="20mm" viewBox="0 0 40 20">
  <rect x="0" y="0" width="20" height="20" fill="#e01010"/>
  <circle cx="30" cy="10" r="8" fill="#1040c0" stroke="#000000"/>
  <path d="M 2 18 C 10 10 30 30 38 18" fill="none" stroke="#00a000"/>
</svg>"##;

#[test]
fn svg_paths_become_grouped_editable_shapes_in_one_undo_step() {
    let dir = temp_dir();
    std::fs::write(dir.join("art.svg"), ARTWORK).expect("svg");
    let mut s = Session::new(&dir);
    let out = run(
        &mut s,
        "import.svg",
        json!({"path": "art.svg", "x": 10, "y": 15}),
    );
    let ids = out["ids"].as_array().expect("ids");
    assert_eq!(ids.len(), 3);
    let d = s.design();
    assert!(d.objects.iter().all(|o| o.kind == ObjectKind::Shape));
    let group = d.objects[0].group.expect("grouped");
    assert!(d.objects.iter().all(|o| o.group == Some(group)));
    // Fill → fill region in the nearest Brother chart colour, as PE-DESIGN imports it
    // (#e01010 → 800 RED, PEC 5); stroke → running line; open path → line only.
    let fill = d.objects[0].region.as_ref().map(|r| r.thread.clone());
    assert_eq!(fill, openstitch_core::PEC_CHART[5].clone());
    assert_eq!(
        d.objects[1]
            .line
            .as_ref()
            .map(|l| l.thread.hex())
            .as_deref(),
        Some("#000000")
    );
    assert!(d.objects[0].line.is_none());
    assert!(d.objects[1].region.is_some() && d.objects[1].line.is_some());
    assert!(d.objects[2].region.is_none() && d.objects[2].line.is_some());
    assert!(d.objects[1].contours[0].has_curves());
    // Physical size kept (40 × 20 mm) and placed at (10, 15) mm.
    let xs = d
        .objects
        .iter()
        .flat_map(|o| o.contours.iter())
        .flat_map(|c| c.points.iter());
    let (min_x, max_x) = xs.fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    assert!((min_x - 100.0).abs() < 1e-3, "{min_x}");
    assert!((max_x - 480.0).abs() < 1e-3, "{max_x}"); // circle edge at 38 mm; usvg works in f32
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64().unwrap_or(0) > 0, "{stats}");

    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty());

    // Scaled to a width.
    run(
        &mut s,
        "import.svg",
        json!({"path": "art.svg", "width": 80}),
    );
    let b = s
        .design()
        .objects
        .iter()
        .flat_map(|o| o.contours.iter())
        .flat_map(|c| c.points.iter());
    let (lo, hi) = b.fold((f64::MAX, f64::MIN), |(a, z), p| (a.min(p.x), z.max(p.x)));
    assert!((hi - lo - 800.0).abs() < 1e-3, "{}", hi - lo);
}

#[test]
fn svg_import_rejects_bad_files_and_never_reads_linked_images() {
    let dir = temp_dir();
    std::fs::write(dir.join("junk.svg"), "<svg").expect("junk");
    std::fs::write(
        dir.join("empty.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="/etc/hosts" width="9" height="9"/></svg>"#,
    )
    .expect("empty");
    let mut s = Session::new(&dir);
    for (params, why) in [
        (json!({"path": "junk.svg"}), "parse"),
        (json!({"path": "empty.svg"}), "nothing to import"),
        (json!({"path": "missing.svg"}), "missing"),
        (json!({"path": "../outside.svg"}), "sandbox"),
        (json!({"path": "art.svg", "width": -3}), "width"),
    ] {
        assert!(s.execute("import.svg", Some(&params)).is_err(), "{why}");
    }
    assert!(s.design().objects.is_empty());
}

#[test]
fn background_image_is_placed_shown_saved_and_undone() {
    let dir = temp_dir();
    let img = Raster::from_fn(200, 100, |x, _| [x as u8, 0, 0, 255]).expect("raster");
    std::fs::write(dir.join("ref.png"), img.encode_png().expect("png")).expect("write");
    let mut s = Session::new(&dir);
    let out = run(
        &mut s,
        "background.set",
        json!({"path": "ref.png", "width": 50}),
    );
    assert_eq!(out["pixels"], json!([200, 100]));
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.width, b.height), (500.0, 250.0));
    assert_eq!(b.opacity, 0.5);
    assert!(b.path.ends_with("ref.png"));

    run(
        &mut s,
        "background.show",
        json!({"visible": false, "opacity": 0.8}),
    );
    let b = s.design().background.clone().expect("background");
    assert!(!b.visible && b.opacity == 0.8);

    // Saved with the project, and never sewn.
    run(&mut s, "project.save", json!({"path": "p.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "p.osd"}));
    assert_eq!(t.design().background, s.design().background);

    run(&mut s, "background.clear", json!({}));
    assert!(s.design().background.is_none());
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().background.is_some());

    std::fs::write(dir.join("bad.png"), b"\x89PNG\r\n\x1a\nnope").expect("bad");
    for (id, params) in [
        ("background.set", json!({"path": "bad.png"})),
        ("background.set", json!({"path": "ref.png", "opacity": 2})),
        ("background.set", json!({"path": "ref.png", "width": 0})),
        ("background.show", json!({"opacity": -1})),
    ] {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
    }
    run(&mut s, "background.clear", json!({}));
    assert!(s.execute("background.clear", Some(&json!({}))).is_err());
    assert!(s.execute("background.show", Some(&json!({}))).is_err());
}

#[test]
fn background_image_turns_crops_adjusts_and_saves_as_shown() {
    let dir = temp_dir();
    // 200 × 100 px: red ramp left to right, blue in the bottom half.
    let img = Raster::from_fn(200, 100, |x, y| {
        [x as u8, 0, if y >= 50 { 255 } else { 0 }, 255]
    })
    .expect("raster");
    std::fs::write(dir.join("ref.png"), img.encode_png().expect("png")).expect("write");
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "background.set",
        json!({"path": "ref.png", "width": 40, "x": 10, "y": 20}),
    );
    // 40 × 20 mm at (10, 20): centre (30, 30).
    run(&mut s, "background.rotate", json!({"direction": "right"}));
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.x, b.y, b.width, b.height), (200.0, 100.0, 200.0, 400.0));
    assert_eq!(b.adjust.rotation, 1);
    run(&mut s, "background.rotate", json!({"direction": "left"}));
    run(&mut s, "background.rotate", json!({"direction": "left"}));
    assert_eq!(
        s.design().background.as_ref().map(|b| b.adjust.rotation),
        Some(3)
    );
    run(&mut s, "background.rotate", json!({"direction": "right"}));
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.x, b.y, b.width, b.height), (100.0, 200.0, 400.0, 200.0));

    // Cropping the left quarter keeps the rest where it was, at the same scale.
    let out = run(
        &mut s,
        "background.modify",
        json!({"crop": [25, 0, 0, 50], "brightness": 20, "saturation": -100}),
    );
    assert_eq!(out["widthMM"], json!(30.0));
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.x, b.y, b.width, b.height), (200.0, 200.0, 300.0, 100.0));
    assert_eq!(b.adjust.crop, [0.25, 0.0, 0.0, 0.5]);
    // Turning carries the crop with the image.
    run(&mut s, "background.rotate", json!({"direction": "right"}));
    let turned = s.design().background.clone().expect("background");
    assert_eq!(turned.adjust.crop, [0.5, 0.25, 0.0, 0.0]);
    run(&mut s, "history.undo", json!({}));
    // Uncropping restores the whole image's place.
    run(&mut s, "background.modify", json!({"crop": [0, 0, 0, 0]}));
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.x, b.y, b.width, b.height), (100.0, 200.0, 400.0, 200.0));
    run(&mut s, "history.undo", json!({}));
    run(
        &mut s,
        "background.modify",
        json!({"x": 0, "y": 5, "width": 60}),
    );
    let b = s.design().background.clone().expect("background");
    assert_eq!((b.x, b.y, b.width, b.height), (0.0, 50.0, 600.0, 200.0));

    // Saved as shown: 150 × 50 px of the top half, grey.
    let out = run(&mut s, "background.save", json!({"path": "shown.bmp"}));
    assert_eq!(out["pixels"], json!([150, 50]));
    let saved = Raster::decode(&std::fs::read(dir.join("shown.bmp")).expect("read")).expect("bmp");
    assert_eq!((saved.width(), saved.height()), (150, 50));
    let [r, g, b, _] = saved.pixel(10, 10);
    assert!(r == g && g == b, "desaturated");
    for ext in ["png", "jpg"] {
        run(
            &mut s,
            "background.save",
            json!({"path": format!("shown.{ext}")}),
        );
    }
    // Adjustments travel with the project and the file is never changed.
    run(&mut s, "project.save", json!({"path": "p.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "p.osd"}));
    assert_eq!(t.design().background, s.design().background);
    assert_eq!(
        Raster::decode(&std::fs::read(dir.join("ref.png")).expect("read")).expect("png"),
        img
    );

    for (id, params) in [
        ("background.rotate", json!({"direction": "up"})),
        ("background.rotate", json!({})),
        ("background.modify", json!({"crop": [50, 0, 50, 0]})),
        ("background.modify", json!({"crop": [95, 0, 0, 0]})),
        ("background.modify", json!({"crop": [0, 0, 0]})),
        ("background.modify", json!({"crop": "all"})),
        ("background.modify", json!({"brightness": 101})),
        ("background.modify", json!({"width": 0})),
        ("background.modify", json!({"x": 1e9})),
        ("background.save", json!({"path": "shown.gif"})),
        ("background.save", json!({})),
    ] {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
    }
    run(&mut s, "background.clear", json!({}));
    for id in ["background.rotate", "background.modify", "background.save"] {
        let params = json!({"direction": "right", "path": "x.png"});
        assert!(
            s.execute(id, Some(&params)).is_err(),
            "{id} without an image"
        );
    }
}

/// PE-DESIGN 11.50 gives every imported colour the nearest BROTHER EMBROIDERY chart thread
/// (compat thread-match-swatches); `exactColors: true` keeps the SVG's own RGB instead.
#[test]
fn svg_colours_snap_to_the_brother_chart_unless_exact() {
    let dir = temp_dir();
    std::fs::write(
        dir.join("o.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect x="0" y="0" width="20" height="20" fill="rgb(255,128,0)" stroke="none"/></svg>"##,
    )
    .expect("write");
    let thread = |exact: bool| {
        let mut s = Session::new(&dir);
        run(
            &mut s,
            "import.svg",
            json!({"path": "o.svg", "exactColors": exact}),
        );
        s.design().objects[0]
            .region
            .as_ref()
            .map(|r| r.thread.clone())
            .expect("region")
    };
    // PE-DESIGN picked 214 DEEP GOLD (PEC 11) for this orange, not 62 Orange.
    assert_eq!(Some(thread(false)), openstitch_core::PEC_CHART[11].clone());
    assert_eq!(thread(true).hex(), "#ff8000");
}

#[test]
fn pe_designs_convert_to_blocks_imports_as_grouped_blocks_in_one_undo_step() {
    // PE-DESIGN 11.50's Convert to Blocks of Lato Bold H (owner file stroke-blocks-H): five
    // Manual Punch blocks and three running pieces, one group.
    let dir = temp_dir();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../openstitch-formats/tests/fixtures/pe11/stroke-blocks-H.pes"
    );
    std::fs::copy(fixture, dir.join("h.pes")).expect("fixture");
    let mut s = Session::new(&dir);
    let out = run(&mut s, "import.pes", json!({"path": "h.pes"}));
    assert_eq!(out["ids"].as_array().map(Vec::len), Some(8));
    assert_eq!(out["textsNotImported"], 0);
    let d = s.design();
    let lines: Vec<usize> = d
        .objects
        .iter()
        .filter_map(|o| o.region.as_ref())
        .map(|r| r.direction_lines.len())
        .collect();
    assert_eq!(lines, [3, 4, 2, 4, 7]);
    let group = d.objects[0].group.expect("grouped");
    assert!(d.objects.iter().all(|o| o.group == Some(group)));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().objects.is_empty());
    // A PES without PE-DESIGN 11's objects is refused with the command to use instead.
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20}),
    );
    run(&mut s, "design.export", json!({"path": "v6.pes"}));
    let e = s
        .execute("import.pes", Some(&json!({"path": "v6.pes"})))
        .expect_err("v6 has no editable objects");
    assert!(e.to_string().contains("stitches.import"), "{e}");
}
