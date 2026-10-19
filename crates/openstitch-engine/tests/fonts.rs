//! User TrueType/OpenType fonts: loading from a path, use in text objects, `.osd` round trip,
//! hostile files and the scripts layout does not shape.

use openstitch_design::fonts::MAX_FONT_BYTES;
use openstitch_engine::{Capabilities, Session, project};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-fonts-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn lato() -> &'static [u8] {
    include_bytes!("../../openstitch-design/fonts/Lato-Bold.ttf")
}

/// A session whose directory holds `user.ttf` (a copy of Lato Bold standing in for a font the
/// user picked).
fn session_with_font() -> (Session, PathBuf) {
    let dir = temp_dir();
    std::fs::write(dir.join("user.ttf"), lato()).expect("font");
    (Session::new(&dir), dir)
}

#[test]
fn a_user_font_lays_out_like_the_same_bundled_font() {
    let (mut s, _) = session_with_font();
    let r = run(&mut s, "fonts.loadTrueType", json!({"path": "user.ttf"}));
    assert_eq!(
        r["id"], "lato-bold-2",
        "default id from the family, made unique"
    );
    assert_eq!(r["family"], "Lato");
    assert!(r["glyphs"].as_u64() > Some(90));
    let r = run(
        &mut s,
        "fonts.loadTrueType",
        json!({"path": "user.ttf", "id": "my-lato"}),
    );
    assert_eq!(r["id"], "my-lato");
    let list = run(&mut s, "fonts.list", json!({}));
    let rows = list.as_array().expect("rows");
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3]["source"], "design");
    assert_eq!(rows[3]["file"], "user.ttf");

    let a = run(
        &mut s,
        "text.add",
        json!({"text": "Ag8", "font": "my-lato"}),
    );
    let b = run(
        &mut s,
        "text.add",
        json!({"text": "Ag8", "font": "lato-bold"}),
    );
    let d = s.design();
    let obj = |r: &Value| &d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")];
    assert_eq!(obj(&a).contours, obj(&b).contours);
    assert_eq!(obj(&a).text.as_ref().expect("text").font, "my-lato");

    // Fonts by family resolve to the bundled font first.
    let c = run(&mut s, "text.add", json!({"text": "x", "font": "Lato"}));
    assert_eq!(obj_font(&s, &c), "lato-bold");
}

fn obj_font(s: &Session, r: &Value) -> String {
    let d = s.design();
    d.objects[d.index_of(r["id"].as_u64().expect("id")).expect("o")]
        .text
        .as_ref()
        .expect("text")
        .font
        .clone()
}

#[test]
fn fonts_travel_with_the_design_and_undo() {
    let (mut s, dir) = session_with_font();
    run(
        &mut s,
        "fonts.loadTrueType",
        json!({"path": "user.ttf", "id": "mine"}),
    );
    run(&mut s, "text.add", json!({"text": "Hi", "font": "mine"}));
    assert!(
        s.execute("fonts.remove", Some(&json!({"id": "mine"})))
            .is_err(),
        "in use"
    );
    run(&mut s, "project.save", json!({"path": "f.osd"}));
    // The file no longer needs the font file.
    std::fs::remove_file(dir.join("user.ttf")).expect("rm");
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "f.osd"}));
    assert_eq!(t.design(), s.design());
    run(&mut t, "object.setText", json!({"id": 1, "text": "Hello"}));
    assert_eq!(t.design().objects[0].glyph_groups().len(), 5);

    run(&mut s, "history.undo", json!({}));
    run(&mut s, "fonts.remove", json!({"id": "mine"}));
    assert!(s.design().fonts.is_empty());
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().fonts.is_empty(), "loading is one undo step");

    // A design without fonts saves exactly as before (no "fonts" key).
    let plain = String::from_utf8(project::encode(s.design()).expect("enc")).expect("utf8");
    assert!(!plain.contains("\"fonts\""));
}

#[test]
fn hostile_font_files_fail_cleanly() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    let mut garbage = Vec::new();
    let mut x = 0x9E37_79B9_u32;
    for _ in 0..100_000 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        garbage.push(x as u8);
    }
    let mut lying_header = lato()[..12].to_vec();
    lying_header.extend([0xFFu8; 64]);
    let mut ttc_bomb = b"ttcf\x00\x02\x00\x00\x7f\xff\xff\xff".to_vec();
    ttc_bomb.extend([0u8; 128]);
    let mut wrong_offsets =
        b"ttcf\x00\x01\x00\x00\x00\x00\x00\x02\xff\xff\xff\x00\x00\x00\x00\x00".to_vec();
    wrong_offsets.extend(lato());
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("empty.ttf", Vec::new()),
        ("garbage.ttf", garbage),
        ("truncated.ttf", lato()[..lato().len() / 3].to_vec()),
        ("header.ttf", lying_header),
        ("bomb.ttc", ttc_bomb),
        ("offsets.ttc", wrong_offsets),
        ("text.ttf", b"this is not a font\n".repeat(50)),
    ];
    for (name, bytes) in &files {
        std::fs::write(dir.join(name), bytes).expect("write");
    }
    // Over the size limit: refused before it is read.
    let big = std::fs::File::create(dir.join("big.ttf")).expect("big");
    big.set_len(MAX_FONT_BYTES as u64 + 1).expect("len");
    let before = s.design().clone();
    for name in files.iter().map(|f| f.0).chain(["big.ttf", "missing.ttf"]) {
        let r = s.execute("fonts.loadTrueType", Some(&json!({ "path": name })));
        assert!(r.is_err(), "{name} accepted");
        assert_eq!(s.design(), &before, "{name} changed the design");
    }
}

#[test]
fn hostile_project_fonts_are_refused() {
    let (mut s, dir) = session_with_font();
    run(
        &mut s,
        "fonts.loadTrueType",
        json!({"path": "user.ttf", "id": "mine"}),
    );
    let good: Value =
        serde_json::from_slice(&project::encode(s.design()).expect("enc")).expect("json");
    let mutate = |f: &dyn Fn(&mut Value)| {
        let mut v = good.clone();
        f(&mut v);
        serde_json::to_vec(&v).expect("bytes")
    };
    let cases = [
        mutate(&|v| v["design"]["fonts"][0]["data"]["trueType"]["bytes"] = json!("!!!")),
        mutate(&|v| v["design"]["fonts"][0]["data"]["trueType"]["bytes"] = json!("AAAA")),
        mutate(&|v| v["design"]["fonts"][0]["data"]["trueType"]["face"] = json!(9)),
        mutate(&|v| v["design"]["fonts"][0]["id"] = json!("lato-bold")),
        mutate(&|v| v["design"]["fonts"][0]["id"] = json!("Bad id")),
        mutate(&|v| {
            let f = v["design"]["fonts"][0].clone();
            v["design"]["fonts"] = json!([f.clone(), f]);
        }),
        mutate(&|v| v["design"]["fonts"][0]["data"] = json!({"woff": "AAAA"})),
    ];
    for (i, bytes) in cases.iter().enumerate() {
        std::fs::write(dir.join("bad.osd"), bytes).expect("write");
        let mut t = Session::new(&dir);
        assert!(
            t.execute("project.open", Some(&json!({"path": "bad.osd"})))
                .is_err(),
            "case {i} opened"
        );
    }
}

#[test]
fn font_loading_obeys_the_sandbox_and_capabilities() {
    let root = temp_dir();
    let outside = temp_dir();
    std::fs::write(outside.join("user.ttf"), lato()).expect("font");
    std::fs::write(root.join("user.ttf"), lato()).expect("font");
    let mut s = Session::sandboxed(&root);
    let escape = outside.join("user.ttf").display().to_string();
    assert!(
        s.execute("fonts.loadTrueType", Some(&json!({ "path": escape })))
            .is_err()
    );
    run(&mut s, "fonts.loadTrueType", json!({"path": "user.ttf"}));
    let mut t = Session::sandboxed(&root);
    t.set_capabilities(Capabilities::FULL.without_files());
    assert!(
        t.execute("fonts.loadTrueType", Some(&json!({"path": "user.ttf"})))
            .is_err()
    );
}

#[test]
fn text_in_scripts_the_font_lacks_is_missing_not_unshaped() {
    let mut s = Session::new(temp_dir());
    // TrueType text is shaped; Lato simply has no Arabic letters.
    let r = run(&mut s, "text.add", json!({"text": "Ab \u{0628}\u{0627}"}));
    assert_eq!(r["glyphs"], 2);
    assert_eq!(r["shaping"], json!([]));
    assert_eq!(r["missing"], json!(["\u{0628}", "\u{0627}"]));
    let r = run(&mut s, "object.setText", json!({"id": 1, "text": "Plain"}));
    assert_eq!(r["shaping"], json!([]));
    assert_eq!(r["missing"], json!([]));
}

/// CJK through a system font, when the machine has one (fonts are never bundled for this).
#[test]
fn cjk_text_with_a_system_font() {
    let candidates = [
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
    ];
    let Some(path) = candidates.iter().map(Path::new).find(|p| p.exists()) else {
        eprintln!("no CJK system font; skipped");
        return;
    };
    let mut s = Session::new(temp_dir());
    let Ok(r) = s.execute(
        "fonts.loadTrueType",
        Some(&json!({ "path": path.display().to_string(), "id": "cjk" })),
    ) else {
        eprintln!("{} is not loadable here; skipped", path.display());
        return;
    };
    assert!(r["glyphs"].as_u64() > Some(5000), "{r}");
    let r = run(
        &mut s,
        "text.add",
        json!({"text": "\u{4E2D}\u{6587}\u{65E5}\u{672C}", "font": "cjk", "size": 20}),
    );
    assert_eq!(r["glyphs"], 4);
    assert_eq!(r["missing"], json!([]));
    let stats = run(&mut s, "design.generate", json!({}));
    assert!(stats["stitches"].as_u64() > Some(500), "{stats}");
}
