//! User-mapped text: embroidery patterns registered as characters of an embroidery font
//! (`fonts.create`, `fonts.mapCharacter`, `fonts.mapFiles`) and typed as text.

use openstitch_core::Command;
use openstitch_design::{Compiler, ObjectKind, Rect};
use openstitch_engine::Session;
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-mapped-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Writes a stitch file holding one preset shape `width` mm wide.
fn export_shape(dir: &PathBuf, file: &str, kind: &str, width: f64, fill: &str) {
    let mut s = Session::new(dir);
    run(
        &mut s,
        "shape.preset",
        json!({"kind": kind, "cx": 50, "cy": 50, "width": width, "fill": fill}),
    );
    run(
        &mut s,
        "design.export",
        json!({ "path": file, "pesVersion": "6t" }),
    );
}

fn bounds(s: &Session, id: u64) -> Rect {
    let d = s.design();
    d.objects[d.index_of(id).expect("object")]
        .bounds()
        .expect("bounds")
}

fn mapped_session() -> (Session, PathBuf) {
    let dir = temp_dir();
    export_shape(&dir, "u_A.pes", "star", 10.0, "Red");
    export_shape(&dir, "l_b.pes", "heart", 8.0, "Blue");
    export_shape(&dir, "s_qm.dst", "diamond", 2.0, "Red");
    export_shape(&dir, "readme.dst", "diamond", 10.0, "Red");
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "fonts.create",
        json!({"id": "shapes", "family": "Shapes", "height": 10}),
    );
    (s, dir)
}

#[test]
fn files_map_to_characters_by_name() {
    let (mut s, _) = mapped_session();
    let r = run(
        &mut s,
        "fonts.mapFiles",
        json!({"font": "shapes", "paths": ["u_A.pes", "l_b.pes", "s_qm.dst", "readme.dst", "none.pes"]}),
    );
    assert_eq!(r["mapped"], json!(["A", "b"]));
    let skipped = r["skipped"].as_array().expect("skipped");
    assert_eq!(skipped.len(), 3, "{r}");
    assert!(
        skipped[0]["reason"].as_str().expect("r").contains("mm"),
        "too small"
    );
    let fonts = run(&mut s, "fonts.list", json!({}));
    let row = &fonts.as_array().expect("rows")[2];
    assert_eq!(row["kind"], "embroidery");
    assert_eq!(row["glyphs"], 2);
    assert_eq!(row["stitchGlyphs"], 2);
    assert_eq!(row["heightMM"], 10.0);
}

#[test]
fn mapped_text_places_the_patterns_in_sequence() {
    let (mut s, _) = mapped_session();
    run(
        &mut s,
        "fonts.mapFiles",
        json!({"font": "shapes", "paths": ["u_A.pes", "l_b.pes"]}),
    );
    let r = run(
        &mut s,
        "text.add",
        json!({"text": "AbA?", "font": "shapes", "size": 10, "x": 10, "y": 50, "align": "left"}),
    );
    assert_eq!(r["glyphs"], 3);
    assert_eq!(r["missing"], json!(["?"]));
    let id = r["id"].as_u64().expect("id");
    let o = &s.design().objects[0];
    assert_eq!(o.kind, ObjectKind::Text);
    assert!(o.contours.is_empty(), "stitch glyphs have no outline");
    let block = o.stitch_block.as_ref().expect("stitches");
    // Two colours per glyph boundary at most: red star, blue heart, red star.
    assert_eq!(block.threads.len(), 3);
    let b = bounds(&s, id);
    assert!((b.min_x - 100.0).abs() < 2.0, "starts at x = 10 mm: {b:?}");
    assert!((b.max_y - 500.0).abs() < 2.0, "sits on the baseline: {b:?}");
    assert!(
        b.width() > 240.0 && b.width() < 300.0,
        "star + heart + star: {b:?}"
    );

    // Colours and stitches are the registered ones.
    let p = Compiler::default().compile(s.design());
    let colours: Vec<String> = p.threads.iter().map(|t| t.hex()).collect();
    assert_eq!(colours.len(), 3, "{colours:?}");
    assert_eq!(colours[0], colours[2]);
    assert_ne!(colours[0], colours[1]);
    let stitched = p.count(Command::Stitch);
    assert!(stitched > 50, "{stitched}");

    // Size scales the patterns; undo restores.
    run(&mut s, "object.setText", json!({"id": id, "size": 20}));
    let big = bounds(&s, id);
    assert!((big.width() / b.width() - 2.0).abs() < 0.02, "{big:?}");
    run(&mut s, "history.undo", json!({}));
    assert_eq!(bounds(&s, id), b);

    // Moving the text moves its stitches; it stays text.
    run(&mut s, "object.move", json!({"id": id, "dx": 5, "dy": 0}));
    assert!((bounds(&s, id).min_x - b.min_x - 50.0).abs() < 1e-6);
    run(&mut s, "object.setText", json!({"id": id, "text": "bb"}));
    assert!(
        (bounds(&s, id).min_x - b.min_x - 50.0).abs() < 1.0,
        "placement kept"
    );

    // Converting gives a stitch pattern object with the same needle points.
    let before = Compiler::default()
        .compile(s.design())
        .count(Command::Stitch);
    let r = run(&mut s, "text.toShapes", json!({"id": id}));
    assert_eq!(r["ids"].as_array().expect("ids").len(), 1);
    assert_eq!(s.design().objects[0].kind, ObjectKind::Stitches);
    assert_eq!(
        Compiler::default()
            .compile(s.design())
            .count(Command::Stitch),
        before
    );
}

#[test]
fn design_objects_and_offsets_become_glyphs() {
    let mut s = Session::new(temp_dir());
    run(&mut s, "fonts.create", json!({"id": "deco", "height": 10}));
    let heart = run(
        &mut s,
        "shape.preset",
        json!({"kind": "heart", "cx": 30, "cy": 30, "width": 10, "fill": "Red"}),
    )["id"]
        .as_u64()
        .expect("id");
    let r = run(
        &mut s,
        "fonts.mapCharacter",
        json!({"font": "deco", "char": "h", "object": heart, "dx": 1, "dy": 2, "advance": 14}),
    );
    let own = {
        let mut one = s.design().clone();
        one.objects.retain(|o| o.id == heart);
        Compiler::default().compile(&one)
    };
    assert_eq!(
        r["stitches"].as_u64(),
        Some(
            own.stitches
                .iter()
                .filter(|x| x.command != Command::End)
                .count() as u64
        )
    );
    let t = run(
        &mut s,
        "text.add",
        json!({"text": "hh", "font": "deco", "size": 10, "x": 0, "y": 50, "align": "left"}),
    )["id"]
        .as_u64()
        .expect("id");
    let b = bounds(&s, t);
    assert!((b.min_x - 10.0).abs() < 1.0, "dx: {b:?}");
    assert!((b.max_y - 520.0).abs() < 1.0, "dy: {b:?}");
    assert!(
        (b.width() - 240.0).abs() < 2.0,
        "advance 14 mm + 10 mm heart: {b:?}"
    );
    // Replacing and removing a character.
    run(
        &mut s,
        "fonts.mapCharacter",
        json!({"font": "deco", "char": "h", "object": heart}),
    );
    run(
        &mut s,
        "fonts.unmapCharacter",
        json!({"font": "deco", "char": "h"}),
    );
    assert!(
        s.execute(
            "fonts.unmapCharacter",
            Some(&json!({"font": "deco", "char": "h"}))
        )
        .is_err()
    );
}

#[test]
fn osfont_files_round_trip_and_hostile_ones_fail() {
    let (mut s, dir) = mapped_session();
    run(
        &mut s,
        "fonts.mapFiles",
        json!({"font": "shapes", "paths": ["u_A.pes", "l_b.pes"]}),
    );
    run(
        &mut s,
        "fonts.save",
        json!({"font": "shapes", "path": "shapes.osfont"}),
    );
    let mut t = Session::new(&dir);
    let r = run(&mut t, "fonts.open", json!({"path": "shapes.osfont"}));
    assert_eq!(r, json!({"id": "shapes", "glyphs": 2}));
    assert_eq!(t.design().fonts, s.design().fonts);
    assert!(
        t.execute("fonts.open", Some(&json!({"path": "shapes.osfont"})))
            .is_err(),
        "id taken"
    );
    run(
        &mut t,
        "fonts.open",
        json!({"path": "shapes.osfont", "id": "shapes-2"}),
    );

    let good: Value =
        serde_json::from_slice(&std::fs::read(dir.join("shapes.osfont")).expect("read"))
            .expect("json");
    let mutate = |f: &dyn Fn(&mut Value)| {
        let mut v = good.clone();
        f(&mut v);
        serde_json::to_vec(&v).expect("bytes")
    };
    let glyph = good["font"]["data"]["embroidery"]["glyphs"][0].clone();
    let many: Vec<Value> = (0..5000u32)
        .filter_map(char::from_u32)
        .filter(|c| !c.is_control())
        .map(|c| {
            let mut g = glyph.clone();
            g["char"] = json!(c.to_string());
            g
        })
        .collect();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("garbage", b"\x00\xffnot json".to_vec()),
        (
            "other format",
            mutate(&|v| v["format"] = json!("openstitch.design")),
        ),
        ("future", mutate(&|v| v["version"] = json!(99))),
        (
            "truetype",
            mutate(&|v| v["font"]["data"] = json!({"trueType": {"bytes": "AAAA"}})),
        ),
        (
            "unsorted",
            mutate(&|v| {
                let g = &mut v["font"]["data"]["embroidery"]["glyphs"];
                let (a, b) = (g[0].clone(), g[1].clone());
                g[0] = b;
                g[1] = a;
            }),
        ),
        (
            "cap height",
            mutate(&|v| v["font"]["data"]["embroidery"]["capHeight"] = json!(0)),
        ),
        (
            "huge coordinate",
            mutate(&|v| {
                v["font"]["data"]["embroidery"]["glyphs"][0]["stitches"]["stitches"][3]["x"] =
                    json!(1e300)
            }),
        ),
        (
            "two chars",
            mutate(&|v| v["font"]["data"]["embroidery"]["glyphs"][0]["char"] = json!("AB")),
        ),
        (
            "control char",
            mutate(&|v| v["font"]["data"]["embroidery"]["glyphs"][0]["char"] = json!("\u{7}")),
        ),
        (
            "too many glyphs",
            mutate(&|v| v["font"]["data"]["embroidery"]["glyphs"] = json!(many.clone())),
        ),
        ("bad id", mutate(&|v| v["font"]["id"] = json!("Bad Id"))),
        (
            "empty family",
            mutate(&|v| v["font"]["family"] = json!(" ")),
        ),
    ];
    let mut u = Session::new(&dir);
    for (what, bytes) in cases {
        std::fs::write(dir.join("bad.osfont"), bytes).expect("write");
        assert!(
            u.execute("fonts.open", Some(&json!({"path": "bad.osfont"})))
                .is_err(),
            "{what} opened"
        );
        assert!(u.design().fonts.is_empty(), "{what}");
    }
}

#[test]
fn mapped_fonts_travel_in_the_osd() {
    let (mut s, dir) = mapped_session();
    run(
        &mut s,
        "fonts.mapFiles",
        json!({"font": "shapes", "paths": ["u_A.pes"]}),
    );
    run(&mut s, "text.add", json!({"text": "AA", "font": "shapes"}));
    run(&mut s, "project.save", json!({"path": "m.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "m.osd"}));
    assert_eq!(t.design(), s.design());
    assert_eq!(
        Compiler::default().compile(t.design()),
        Compiler::default().compile(s.design())
    );
}
