use openstitch_engine::{Capabilities, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-print-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn banner(dir: &PathBuf) -> Session {
    let mut s = Session::new(dir);
    run(
        &mut s,
        "design.new",
        json!({"name": "Banner", "width": 230, "height": 160}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 20, "y": 60, "width": 190, "height": 40}),
    );
    s
}

#[test]
fn every_mode_writes_a_pdf() {
    let dir = temp_dir();
    let mut s = banner(&dir);
    let r = run(&mut s, "design.print", json!({"path": "actual.pdf"}));
    // 230 × 160 mm fits landscape A4 at 1:1, then the information page.
    assert_eq!(r["pages"], 2);
    let r = run(
        &mut s,
        "design.print",
        json!({"path": "a3.pdf", "paper": "a3"}),
    );
    assert_eq!(r["pages"], 2);
    let r = run(
        &mut s,
        "design.print",
        json!({"path": "reduced.pdf", "mode": "reduced", "paper": "letter"}),
    );
    assert_eq!(r["pages"], 1);
    run(&mut s, "sections.set", json!({"hoop": "130x180"}));
    let r = run(
        &mut s,
        "design.print",
        json!({"path": "sheets.pdf", "mode": "positioning", "templateGrid": true}),
    );
    assert_eq!(r["pages"], 3);
    for f in ["actual.pdf", "a3.pdf", "reduced.pdf", "sheets.pdf"] {
        let bytes = std::fs::read(dir.join(f)).expect("pdf");
        assert!(
            bytes.starts_with(b"%PDF-1.4") && bytes.ends_with(b"%%EOF\n"),
            "{f}"
        );
    }
}

#[test]
fn print_and_stitch_needs_a_background_image() {
    let dir = temp_dir();
    let mut s = banner(&dir);
    let e = s
        .execute(
            "design.print",
            Some(&json!({"path": "img.pdf", "mode": "image"})),
        )
        .expect_err("no background")
        .to_string();
    assert!(e.contains("background"), "{e}");
}

#[test]
fn print_bad_parameters_and_capabilities() {
    let dir = temp_dir();
    let mut s = banner(&dir);
    for bad in [
        json!({}),
        json!({"path": "x.png"}),
        json!({"path": "x.pdf", "mode": "poster"}),
        json!({"path": "x.pdf", "paper": "a0"}),
        json!({"path": "x.pdf", "templateGrid": "yes"}),
    ] {
        assert!(s.execute("design.print", Some(&bad)).is_err(), "{bad}");
    }
    assert!(!dir.join("x.pdf").exists());
    s.set_capabilities(Capabilities::FULL.read_only());
    assert!(
        s.execute("design.print", Some(&json!({"path": "y.pdf"})))
            .is_err()
    );
    assert!(!dir.join("y.pdf").exists());
}

#[test]
fn print_and_stitch_prints_the_background_and_an_alignment_sheet() {
    let dir = temp_dir();
    let mut s = banner(&dir);
    let png = openstitch_digitize::Raster::from_fn(40, 20, |x, _| [x as u8 * 6, 0, 200, 255])
        .and_then(|r| r.encode_png())
        .expect("png");
    std::fs::write(dir.join("photo.png"), png).expect("write");
    run(
        &mut s,
        "background.set",
        json!({"path": "photo.png", "width": 120}),
    );
    let r = run(
        &mut s,
        "design.print",
        json!({"path": "pas.pdf", "mode": "image", "mirror": true}),
    );
    assert_eq!(r["pages"], 2);
    let pdf = std::fs::read(dir.join("pas.pdf")).expect("pdf");
    let has = |needle: &[u8]| pdf.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"/Subtype /Image /Width 40 /Height 20"));
}
