//! PES compatibility (docs/COMPATIBILITY_AUDIT.md items 2 and 19): every user-facing PES path
//! writes version 6 with a stitch-group object and the thread table, and the Design Property
//! fields reach the file, survive `.osd` and come back on import. Internal regressions only; no
//! PE-DESIGN reference stands behind them.
#![cfg(feature = "formats")]

use openstitch_engine::Session;
use openstitch_formats::pes;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-pes-{tag}-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.canonicalize().expect("canonical")
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Version ≥ 6, at least one `CEmbOne` and a thread table.
fn assert_full_pes(path: &Path) -> pes::ObjectSection {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let objects = pes::read_objects(&bytes).expect("objects");
    let version: u32 = objects.version.parse().expect("numeric version");
    assert!(version >= 60, "{}: version {version}", path.display());
    assert!(
        !objects.objects.is_empty(),
        "{}: no CEmbOne",
        path.display()
    );
    assert!(
        !objects.threads.is_empty(),
        "{}: no thread table",
        path.display()
    );
    objects
}

fn rose(dir: &Path) -> Session {
    let mut s = Session::new(dir);
    run(
        &mut s,
        "design.new",
        json!({"name": "t1", "width": 100, "height": 100}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 20, "width": 40, "height": 30, "fill": "Red", "outline": "Black"}),
    );
    s
}

#[test]
fn design_export_without_version_writes_full_v6() {
    let dir = temp_dir("export");
    let mut s = rose(&dir);
    run(&mut s, "design.export", json!({"path": "a.pes"}));
    let objects = assert_full_pes(&dir.join("a.pes"));
    assert_eq!(objects.version, "0060");
    assert_eq!(objects.threads.len(), 2);
    // `1t` stays available as the smallest, machine-only file.
    run(
        &mut s,
        "design.export",
        json!({"path": "b.pes", "pesVersion": "1t"}),
    );
    let small = pes::read_objects(&std::fs::read(dir.join("b.pes")).expect("b")).expect("b");
    assert_eq!(small.version, "0001");
    assert!(small.objects.is_empty() && small.threads.is_empty());
}

#[test]
fn name_drop_export_writes_full_v6() {
    let dir = temp_dir("namedrop");
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "text.add",
        json!({"text": "Ann", "size": 10, "x": 50, "y": 40}),
    );
    run(&mut s, "text.setNameDrop", json!({"id": 1}));
    let r = run(
        &mut s,
        "nameDrop.export",
        json!({"rows": ["Ann", "Bo"], "name": "tag"}),
    );
    assert_eq!(r["count"], 2);
    for f in ["tag001.pes", "tag002.pes"] {
        assert_full_pes(&dir.join(f));
    }
}

#[cfg(feature = "library")]
#[test]
fn library_convert_writes_full_v6() {
    let dir = temp_dir("convert");
    std::fs::write(
        dir.join("oracle.dst"),
        include_bytes!("../../openstitch-formats/tests/fixtures/oracle.dst"),
    )
    .expect("dst");
    let mut s = Session::new(&dir);
    let r = run(
        &mut s,
        "library.convert",
        json!({"paths": ["oracle.dst"], "format": "pes"}),
    );
    assert_eq!(r["written"], 1, "{r}");
    assert_full_pes(&dir.join("oracle.pes"));
}

const PROPERTIES: [(&str, &str); 4] = [
    ("category", "Flowers"),
    ("author", "Ana Pérez"),
    ("keywords", "rose, red, spring"),
    ("comments", "Line one\nline two"),
];

fn set_properties(s: &mut Session) {
    let params: serde_json::Map<String, Value> = PROPERTIES
        .iter()
        .map(|&(k, v)| (k.to_string(), json!(v)))
        .collect();
    run(s, "design.setProperties", Value::Object(params));
}

#[test]
fn design_properties_reach_the_pes_header_byte_exact() {
    let dir = temp_dir("props");
    let mut s = Session::new(&dir);
    run(&mut s, "design.new", json!({"name": "Rose"}));
    set_properties(&mut s);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 20, "width": 40, "height": 30, "fill": "Red"}),
    );
    run(
        &mut s,
        "design.export",
        json!({"path": "rose.pes", "pesVersion": "6"}),
    );
    let objects = assert_full_pes(&dir.join("rose.pes"));
    assert_eq!(
        objects.metadata.get("name").map(String::as_str),
        Some("Rose")
    );
    for (k, v) in PROPERTIES {
        assert_eq!(objects.metadata.get(k).map(String::as_str), Some(v), "{k}");
    }
    let info = run(&mut s, "design.info", json!({}));
    assert_eq!(info["properties"]["author"], "Ana Pérez");

    // Only the fields given change; "" clears one.
    run(
        &mut s,
        "design.setProperties",
        json!({"keywords": "", "author": "Bo"}),
    );
    let p = &s.design().properties;
    assert_eq!((p.category.as_str(), p.author.as_str()), ("Flowers", "Bo"));
    assert!(p.keywords.is_empty());
}

#[test]
fn design_properties_are_validated_all_or_nothing() {
    let mut s = Session::new(temp_dir("validate"));
    set_properties(&mut s);
    let before = s.design().properties.clone();
    for bad in [
        json!({}),
        json!({"author": 3}),
        json!({"category": "ok", "author": "x".repeat(256)}),
        json!({"category": "ok", "comments": "bell\u{7}"}),
    ] {
        assert!(
            s.execute("design.setProperties", Some(&bad)).is_err(),
            "{bad}"
        );
        assert_eq!(s.design().properties, before, "{bad}");
    }
    // 255 bytes is the PES limit, counted in UTF-8 bytes.
    run(
        &mut s,
        "design.setProperties",
        json!({"comments": "é".repeat(127)}),
    );
    assert!(
        s.execute(
            "design.setProperties",
            Some(&json!({"comments": "é".repeat(128)}))
        )
        .is_err()
    );
}

#[test]
fn design_properties_survive_osd_and_come_back_on_import() {
    let dir = temp_dir("osd");
    let mut s = rose(&dir);
    set_properties(&mut s);
    run(&mut s, "project.save", json!({"path": "rose.osd"}));
    run(&mut s, "design.export", json!({"path": "rose.pes"}));
    let saved = s.design().clone();

    let mut back = Session::new(&dir);
    run(&mut back, "project.open", json!({"path": "rose.osd"}));
    assert_eq!(back.design().properties, saved.properties);

    // An empty design adopts the file's fields.
    let mut fresh = Session::new(&dir);
    let r = run(&mut fresh, "stitches.import", json!({"path": "rose.pes"}));
    assert_eq!(r["propertiesAdopted"], true);
    assert_eq!(r["properties"]["category"], "Flowers");
    assert_eq!(fresh.design().properties, saved.properties);

    // A design with its own fields keeps them unless asked; the file's are still offered.
    let mut own = Session::new(&dir);
    run(&mut own, "design.setProperties", json!({"author": "Me"}));
    let r = run(&mut own, "stitches.import", json!({"path": "rose.pes"}));
    assert_eq!(r["propertiesAdopted"], false);
    assert_eq!(r["properties"]["author"], "Ana Pérez");
    assert_eq!(own.design().properties.author, "Me");
    run(
        &mut own,
        "stitches.import",
        json!({"path": "rose.pes", "adoptProperties": true}),
    );
    assert_eq!(own.design().properties, saved.properties);

    // A design without fields writes none, and `.osd` omits the key.
    let mut plain = rose(&dir);
    run(&mut plain, "project.save", json!({"path": "plain.osd"}));
    let text = std::fs::read_to_string(dir.join("plain.osd")).expect("osd");
    assert!(!text.contains("\"properties\""));
    run(&mut plain, "design.export", json!({"path": "plain.pes"}));
    let objects = assert_full_pes(&dir.join("plain.pes"));
    assert!(
        ["category", "author", "keywords", "comments"]
            .iter()
            .all(|k| !objects.metadata.contains_key(*k))
    );
}

#[test]
fn pes_version_11_writes_editable_shapes_and_text_and_refuses_what_it_cannot_hold() {
    // docs/COMPATIBILITY_AUDIT.md item 5: a rectangle with PE-DESIGN's default attributes
    // becomes one CEmbLine in a #PES0110 file, a text one CEmbNText; what is not mapped yet is
    // refused with the object named.
    let dir = temp_dir("v11");
    let mut s = Session::new(dir.clone());
    run(
        &mut s,
        "design.new",
        json!({"name": "v11", "width": 100, "height": 100}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 10, "height": 10, "fill": 16, "outline": 5}),
    );
    run(
        &mut s,
        "object.setLine",
        json!({"ids": [1], "type": "zigzag", "color": 5}),
    );
    run(
        &mut s,
        "design.export",
        json!({"path": "a.pes", "pesVersion": "11", "split": true}),
    );
    let bytes = std::fs::read(dir.join("a.pes")).expect("a.pes");
    let objects = pes::read_objects(&bytes).expect("objects");
    assert_eq!(objects.version, "0110");
    assert_eq!(objects.unparsed, None);
    assert_eq!(objects.page_mm, Some((100, 100)));
    assert_eq!(objects.shapes.len(), 1);
    let shape = &objects.shapes[0];
    assert_eq!(shape.bounds, [600, 600, 700, 700]);
    assert_eq!(shape.region, openstitch_formats::pes11::RegionSewType::Fill);
    assert_eq!(
        shape.line_type,
        openstitch_formats::pes11::Shape::LINE_ZIGZAG
    );
    assert_eq!(objects.threads.len(), 2);
    // PE-DESIGN's table has no name field: the catalogue number identifies the colour.
    // Written as PE-DESIGN writes those chart colours: BROTHER EMBROIDERY 005 and 800.
    assert_eq!(objects.threads[0].catalog_number.as_deref(), Some("005"));
    assert_eq!(objects.threads[1].catalog_number.as_deref(), Some("800"));
    assert_eq!(objects.threads[0].brand.as_deref(), Some("EMBROIDERY"));
    assert!(pes::read(&bytes).expect("stitches").stitches.len() > 100);

    // A text in Lato Bold becomes a CEmbNText with one letter per character after the shape.
    run(
        &mut s,
        "text.add",
        json!({"text": "AB", "x": 40, "y": 50, "size": 10}),
    );
    run(
        &mut s,
        "design.export",
        json!({"path": "b.pes", "pesVersion": "11"}),
    );
    let objects =
        pes::read_objects(&std::fs::read(dir.join("b.pes")).expect("b.pes")).expect("objects");
    assert_eq!(objects.unparsed, None);
    assert_eq!((objects.shapes.len(), objects.texts.len()), (1, 1));
    let text = &objects.texts[0];
    assert_eq!(text.position, 1);
    let chars: Vec<u16> = text.letters.iter().map(|l| l.ch).collect();
    assert_eq!(chars, [65, 66]);
    assert!(
        text.letters
            .iter()
            .all(|l| l.font == "Lato" && l.size == 10.0)
    );
    // text.add centres the text on its point: the click PE-DESIGN stores is the line's start.
    assert!(
        text.origin.0 < 900.0 && text.origin.1 == 1000.0,
        "{:?}",
        text.origin
    );

    // A space is a letter, as in PE-DESIGN's r27.
    run(
        &mut s,
        "text.add",
        json!({"text": "A B", "x": 40, "y": 80, "size": 10}),
    );
    run(
        &mut s,
        "design.export",
        json!({"path": "c.pes", "pesVersion": "11"}),
    );
    let objects =
        pes::read_objects(&std::fs::read(dir.join("c.pes")).expect("c.pes")).expect("objects");
    let chars: Vec<u16> = objects.texts[1].letters.iter().map(|l| l.ch).collect();
    assert_eq!(chars, [65, 32, 66]);

    // Vertical text is refused with the object named, and nothing is written.
    run(
        &mut s,
        "text.add",
        json!({"text": "AB", "x": 40, "y": 30, "size": 10, "direction": "vertical"}),
    );
    let err = s
        .execute(
            "design.export",
            Some(&json!({"path": "d.pes", "pesVersion": "11"})),
        )
        .expect_err("vertical text is not written yet");
    assert!(err.to_string().contains("vertical"), "{err}");
    assert!(!dir.join("d.pes").exists());
}
