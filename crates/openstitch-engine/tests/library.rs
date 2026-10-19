#![cfg(feature = "library")]

use openstitch_engine::capabilities::{LIBRARY_READING_COMMANDS, required};
use openstitch_engine::{Capabilities, Capability, Sandbox, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-lib-{tag}-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    // Canonical, so paths in results compare equal (macOS /var is a link to /private/var).
    dir.canonicalize().expect("canonical")
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn fails(s: &mut Session, id: &str, params: Value) -> String {
    s.execute(id, Some(&params))
        .err()
        .unwrap_or_else(|| panic!("{id} should fail"))
        .to_string()
}

fn p(path: &Path) -> String {
    path.display().to_string()
}

/// rose.pes (one square), tulip.dst (two squares, two colours), notes.txt and sub/daisy.pes.
fn fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("sub")).expect("sub");
    let mut s = Session::new(dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20}),
    );
    run(&mut s, "design.export", json!({"path": "rose.pes"}));
    run(&mut s, "design.export", json!({"path": "sub/daisy.pes"}));
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 40, "y": 10, "width": 30, "height": 20, "fill": "#ff0000"}),
    );
    run(&mut s, "design.export", json!({"path": "tulip.dst"}));
    std::fs::write(dir.join("notes.txt"), "not a design").expect("txt");
}

fn names(rows: &Value) -> Vec<String> {
    rows.as_array()
        .expect("array")
        .iter()
        .map(|r| r["name"].as_str().expect("name").to_string())
        .collect()
}

#[test]
fn library_reading_commands_exist_and_need_file_capabilities() {
    let s = Session::new(temp_dir("caps"));
    for id in LIBRARY_READING_COMMANDS {
        assert!(s.commands().iter().any(|c| c.id == *id), "{id}");
    }
    for c in s
        .commands()
        .into_iter()
        .filter(|c| c.id.starts_with("library."))
    {
        let needs = required(c);
        assert!(!needs.contains(&Capability::Edit), "{}", c.id);
        assert!(
            needs.contains(&Capability::ReadFiles) || needs.contains(&Capability::WriteFiles),
            "{}",
            c.id
        );
    }
    assert!(!s.commands().iter().any(|c| c.id == "library.delete"));
}

#[test]
fn list_and_info_report_property_information() {
    let dir = temp_dir("list");
    fixture(&dir);
    let mut s = Session::new(&dir);
    let l = run(
        &mut s,
        "library.list",
        json!({"path": p(&dir), "colors": true}),
    );
    assert_eq!(names(&l["folders"]), ["sub"]);
    assert_eq!(names(&l["designs"]), ["rose.pes", "tulip.dst"]);
    assert_eq!(l["designCount"], 2);
    assert_eq!(l["more"], false);
    let rose = &l["designs"][0];
    assert_eq!(rose["format"], "pes");
    assert_eq!(rose["readable"], true);
    assert!(rose["stitches"].as_u64().expect("value") > 10);
    assert!(
        (rose["widthMm"].as_f64().expect("value") - 20.0).abs() < 1.5,
        "{rose}"
    );
    assert_eq!(rose["colorChanges"], 1);
    assert_eq!(rose["colorSequence"].as_array().expect("value").len(), 1);
    assert!(rose["sewingMinutes"].as_f64().expect("value") > 0.0);

    let page = run(
        &mut s,
        "library.list",
        json!({"path": p(&dir), "offset": 1, "limit": 1, "info": false}),
    );
    assert_eq!(names(&page["designs"]), ["tulip.dst"]);
    assert!(page["designs"][0].get("stitches").is_none());

    let tulip = run(&mut s, "library.info", json!({"path": "tulip.dst"}));
    assert_eq!(tulip["format"], "dst");
    assert!(
        tulip["colorChanges"].as_u64().expect("value") >= 2,
        "{tulip}"
    );
    assert!(tulip["colors"].is_array());
    let e = fails(&mut s, "library.info", json!({"path": "notes.txt"}));
    assert!(e.contains("not an embroidery design"), "{e}");
    let e = fails(&mut s, "library.list", json!({"path": "rose.pes"}));
    assert!(e.contains("not a folder"), "{e}");
}

#[test]
fn search_matches_every_given_criterion() {
    let dir = temp_dir("search");
    fixture(&dir);
    let mut s = Session::new(&dir);
    let all = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "subfolders": true}),
    );
    assert_eq!(
        names(&all["designs"]),
        ["rose.pes", "tulip.dst", "daisy.pes"]
    );
    let top = run(&mut s, "library.search", json!({"path": p(&dir)}));
    assert_eq!(top["matches"], 2);
    let pes = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "subfolders": true, "formats": ["pes"], "name": "d*s?.pes"}),
    );
    assert_eq!(names(&pes["designs"]), ["daisy.pes"]);
    let colours = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "colorChanges": {"min": 2}}),
    );
    assert_eq!(names(&colours["designs"]), ["tulip.dst"]);
    let wide = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "widthMm": {"min": 40}, "stitches": {"max": 1_000_000}}),
    );
    assert_eq!(names(&wide["designs"]), ["tulip.dst"]);
    let none = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "properties": {"author": "nobody"}}),
    );
    assert_eq!(none["matches"], 0);
    let limited = run(
        &mut s,
        "library.search",
        json!({"path": p(&dir), "subfolders": true, "limit": 1}),
    );
    assert_eq!(limited["designs"].as_array().expect("value").len(), 1);
    assert_eq!(limited["more"], true);
    for bad in [
        json!({"path": p(&dir), "formats": ["doc"]}),
        json!({"path": p(&dir), "stitches": {"min": -1}}),
        json!({"path": p(&dir), "properties": {"colour": "red"}}),
    ] {
        fails(&mut s, "library.search", bad);
    }
}

#[test]
fn convert_writes_new_files_and_never_overwrites() {
    let dir = temp_dir("convert");
    fixture(&dir);
    let out = dir.join("out");
    std::fs::create_dir(&out).expect("out");
    let mut s = Session::new(&dir);
    let r = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&dir)], "format": "jef", "output": p(&out)}),
    );
    assert_eq!(r["written"], 2, "{r}");
    assert!(out.join("rose.jef").is_file() && out.join("tulip.jef").is_file());
    let again = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&dir)], "format": "jef", "output": p(&out)}),
    );
    assert_eq!(again["written"], 0);
    assert_eq!(again["results"][0]["status"], "skipped");
    let both = run(
        &mut s,
        "library.convert",
        json!({"paths": ["rose.pes"], "format": "jef", "output": "out", "conflict": "keepBoth"}),
    );
    assert_eq!(both["results"][0]["to"], p(&out.join("rose (2).jef")));
    // Next to each source; a file already in the format is skipped.
    let same = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&dir), p(&dir.join("sub"))], "format": "dst"}),
    );
    assert!(dir.join("rose.dst").is_file() && dir.join("sub/daisy.dst").is_file());
    assert_eq!(same["written"], 2, "{same}");
    assert_eq!(same["results"][1]["status"], "skipped");
    let e = fails(
        &mut s,
        "library.convert",
        json!({"paths": ["rose.pes"], "format": "phc"}),
    );
    assert!(e.contains("cannot write"), "{e}");
}

#[test]
fn catalogues_in_every_format() {
    let dir = temp_dir("catalog");
    fixture(&dir);
    let mut s = Session::new(&dir);
    let pdf = run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "output": "cat.pdf", "style": 1, "title": "Flowers"}),
    );
    assert_eq!(pdf["designs"], 2);
    assert_eq!(pdf["pages"], 2);
    assert!(
        std::fs::read(dir.join("cat.pdf"))
            .expect("value")
            .starts_with(b"%PDF")
    );
    let e = fails(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "output": "cat.pdf"}),
    );
    assert!(e.contains("exists"), "{e}");
    run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "output": "cat.pdf", "overwrite": true, "paper": "letter"}),
    );

    let csv = run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "subfolders": true, "output": "cat.csv"}),
    );
    assert_eq!(csv["designs"], 3);
    let text = std::fs::read_to_string(dir.join("cat.csv")).expect("value");
    assert!(
        text.contains("daisy.pes") && text.contains("tulip.dst"),
        "{text}"
    );

    let png = run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "output": "pages.png", "style": "instructions", "dpi": 36}),
    );
    assert_eq!(png["written"].as_array().expect("value").len(), 2);
    assert!(dir.join("pages-1.png").is_file() && dir.join("pages-2.png").is_file());
    run(
        &mut s,
        "library.catalog",
        json!({"paths": ["rose.pes"], "output": "one.png", "dpi": 36}),
    );
    assert!(dir.join("one.png").is_file());

    let html = run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&dir)], "output": "web.html", "style": 12}),
    );
    assert!(
        html["written"].as_array().expect("value").len() >= 3,
        "{html}"
    );
    assert!(dir.join("web.html").is_file() && dir.join("web").is_dir());
    for bad in [
        json!({"paths": [p(&dir)], "output": "cat.doc"}),
        json!({"paths": [p(&dir)], "output": "x.pdf", "style": 3}),
        json!({"paths": [p(&dir.join("sub"))], "output": "x.pdf", "format": "pdf", "units": "cm"}),
        json!({"paths": ["notes.txt"], "output": "x.pdf"}),
    ] {
        fails(&mut s, "library.catalog", bad);
    }
}

#[test]
fn organise_folders_and_files() {
    let dir = temp_dir("organise");
    fixture(&dir);
    let mut s = Session::new(&dir);
    let made = run(
        &mut s,
        "library.createFolder",
        json!({"path": p(&dir), "name": "Spring"}),
    );
    let spring = dir.join("Spring");
    assert_eq!(made["path"], p(&spring));
    fails(
        &mut s,
        "library.createFolder",
        json!({"path": p(&dir), "name": "Spring"}),
    );
    fails(
        &mut s,
        "library.createFolder",
        json!({"path": p(&dir), "name": "a/b"}),
    );

    let r = run(
        &mut s,
        "library.rename",
        json!({"path": "rose.pes", "name": "Red rose"}),
    );
    assert_eq!(r["path"], p(&dir.join("Red rose.pes")));

    let c = run(
        &mut s,
        "library.copy",
        json!({"paths": ["Red rose.pes", "sub"], "destination": "Spring"}),
    );
    assert_eq!(c["done"], 2, "{c}");
    assert!(spring.join("Red rose.pes").is_file() && spring.join("sub/daisy.pes").is_file());
    let skip = run(
        &mut s,
        "library.copy",
        json!({"paths": ["Red rose.pes"], "destination": "Spring"}),
    );
    assert_eq!(skip["results"][0]["status"], "skipped");
    let both = run(
        &mut s,
        "library.copy",
        json!({"paths": ["Red rose.pes"], "destination": "Spring", "conflict": "keepBoth"}),
    );
    assert_eq!(
        both["results"][0]["to"],
        p(&spring.join("Red rose (2).pes"))
    );

    let m = run(
        &mut s,
        "library.move",
        json!({"paths": ["tulip.dst"], "destination": "sub"}),
    );
    assert_eq!(m["done"], 1);
    assert!(!dir.join("tulip.dst").exists() && dir.join("sub/tulip.dst").is_file());
    let into_itself = run(
        &mut s,
        "library.move",
        json!({"paths": ["sub"], "destination": p(&dir.join("sub"))}),
    );
    assert_eq!(into_itself["results"][0]["status"], "failed");
}

/// Read root `library` (designs), write root `work`, and `outside` with a design in it.
fn sandboxed() -> (Session, PathBuf, PathBuf, PathBuf) {
    let base = temp_dir("sandbox");
    let (library, work, outside) = (
        base.join("library"),
        base.join("work"),
        base.join("outside"),
    );
    for d in [&library, &work, &outside] {
        fixture(d);
    }
    let s = Session::with_sandbox(Sandbox::new(&[&library], &[&work]).expect("sandbox"));
    (s, library, work, outside)
}

#[test]
fn sandbox_confines_every_library_path() {
    let (mut s, library, work, outside) = sandboxed();
    // Reading inside the roots works.
    run(&mut s, "library.list", json!({"path": p(&library)}));
    run(
        &mut s,
        "library.search",
        json!({"path": p(&work), "subfolders": true}),
    );
    run(
        &mut s,
        "library.info",
        json!({"path": p(&library.join("rose.pes"))}),
    );
    // Relative paths resolve against the write root.
    assert_eq!(
        names(&run(&mut s, "library.list", json!({"path": "sub"}))["designs"]),
        ["daisy.pes"]
    );

    let refused = |e: String| assert!(e.contains("outside the allowed"), "{e}");
    refused(fails(&mut s, "library.list", json!({"path": p(&outside)})));
    refused(fails(
        &mut s,
        "library.info",
        json!({"path": p(&outside.join("rose.pes"))}),
    ));
    refused(fails(
        &mut s,
        "library.search",
        json!({"path": p(&outside)}),
    ));
    let e = fails(
        &mut s,
        "library.list",
        json!({"path": format!("{}/../outside", p(&library))}),
    );
    assert!(e.contains("'.' or '..'"), "{e}");

    // Read roots are read-only: no conversion output, rename, move or new folder there.
    refused(fails(
        &mut s,
        "library.convert",
        json!({"paths": [p(&library)], "format": "jef", "output": p(&library)}),
    ));
    let same = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&library.join("rose.pes"))], "format": "jef"}),
    );
    assert_eq!(same["failed"], 1);
    assert!(!library.join("rose.jef").exists());
    refused(fails(
        &mut s,
        "library.convert",
        json!({"paths": [p(&outside)], "format": "jef", "output": p(&work)}),
    ));
    refused(fails(
        &mut s,
        "library.rename",
        json!({"path": p(&library.join("rose.pes")), "name": "x"}),
    ));
    refused(fails(
        &mut s,
        "library.move",
        json!({"paths": [p(&library.join("rose.pes"))], "destination": p(&work)}),
    ));
    refused(fails(
        &mut s,
        "library.createFolder",
        json!({"path": p(&library), "name": "x"}),
    ));
    refused(fails(
        &mut s,
        "library.copy",
        json!({"paths": [p(&work.join("rose.pes"))], "destination": p(&outside)}),
    ));
    refused(fails(
        &mut s,
        "library.copy",
        json!({"paths": [p(&outside.join("rose.pes"))], "destination": p(&work)}),
    ));
    refused(fails(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&library)], "output": p(&outside.join("c.pdf"))}),
    ));
    refused(fails(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&outside)], "output": "c.pdf"}),
    ));
    // A root itself cannot be renamed or moved: its parent is not writable.
    refused(fails(
        &mut s,
        "library.rename",
        json!({"path": p(&work), "name": "w2"}),
    ));
    assert!(work.is_dir());

    // Copying from the read root into the write root, and converting into it, works.
    let c = run(
        &mut s,
        "library.copy",
        json!({"paths": [p(&library.join("sub"))], "destination": p(&work.join("sub"))}),
    );
    assert_eq!(c["done"], 1, "{c}");
    let v = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&library)], "format": "jef", "output": p(&work)}),
    );
    assert_eq!(v["written"], 2, "{v}");
    run(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&library)], "output": "c.html"}),
    );
    assert!(work.join("c.html").is_file());
    let outside_entries: Vec<_> = std::fs::read_dir(&outside).expect("value").collect();
    assert_eq!(outside_entries.len(), 4, "nothing written outside");
}

#[cfg(unix)]
#[test]
fn links_out_of_the_roots_are_left_out() {
    use std::os::unix::fs::symlink;
    let (mut s, library, work, outside) = sandboxed();
    symlink(outside.join("tulip.dst"), library.join("stolen.dst")).expect("link");
    symlink(&outside, library.join("elsewhere")).expect("link");
    symlink(&outside, work.join("web")).expect("link");
    symlink(library.join("rose.pes"), library.join("alias.pes")).expect("link");

    let l = run(&mut s, "library.list", json!({"path": p(&library)}));
    assert_eq!(names(&l["designs"]), ["alias.pes", "rose.pes", "tulip.dst"]);
    assert_eq!(names(&l["folders"]), ["sub"]);
    assert_eq!(l["outsideRoots"], 2);
    let found = run(
        &mut s,
        "library.search",
        json!({"path": p(&library), "subfolders": true, "name": "stolen"}),
    );
    assert_eq!(found["matches"], 0);
    assert_eq!(found["outsideRoots"], 1);
    let e = fails(
        &mut s,
        "library.info",
        json!({"path": p(&library.join("stolen.dst"))}),
    );
    assert!(e.contains("outside the allowed"), "{e}");
    let v = run(
        &mut s,
        "library.convert",
        json!({"paths": [p(&library)], "format": "jef", "output": p(&work)}),
    );
    assert_eq!(v["outsideRoots"], 1);
    assert!(!work.join("stolen.jef").exists());
    // The HTML image folder would land outside through the `web` link.
    let e = fails(
        &mut s,
        "library.catalog",
        json!({"paths": [p(&library)], "output": "web.html"}),
    );
    assert!(e.contains("outside the allowed"), "{e}");
    assert!(!outside.join("rose.png").exists());
}

#[test]
fn read_only_and_no_files_modes() {
    let dir = temp_dir("modes");
    fixture(&dir);
    let mut s = Session::new(&dir);
    s.set_capabilities(Capabilities::FULL.read_only());
    run(&mut s, "library.list", json!({"path": p(&dir)}));
    run(&mut s, "library.search", json!({"path": p(&dir)}));
    run(&mut s, "library.info", json!({"path": "rose.pes"}));
    for (id, params) in [
        (
            "library.convert",
            json!({"paths": ["rose.pes"], "format": "jef"}),
        ),
        (
            "library.catalog",
            json!({"paths": ["rose.pes"], "output": "c.pdf"}),
        ),
        (
            "library.createFolder",
            json!({"path": p(&dir), "name": "x"}),
        ),
        ("library.rename", json!({"path": "rose.pes", "name": "x"})),
        (
            "library.copy",
            json!({"paths": ["rose.pes"], "destination": "sub"}),
        ),
        (
            "library.move",
            json!({"paths": ["rose.pes"], "destination": "sub"}),
        ),
    ] {
        let e = fails(&mut s, id, params);
        assert!(e.contains("not permitted"), "{id}: {e}");
    }
    assert!(!dir.join("rose.jef").exists() && !dir.join("x").exists());

    s.set_capabilities(Capabilities::FULL.without_files());
    for (id, params) in [
        ("library.list", json!({"path": p(&dir)})),
        ("library.info", json!({"path": "rose.pes"})),
        ("library.search", json!({"path": p(&dir)})),
        (
            "library.createFolder",
            json!({"path": p(&dir), "name": "x"}),
        ),
    ] {
        let e = fails(&mut s, id, params);
        assert!(e.contains("not permitted"), "{id}: {e}");
    }
    let permitted: Vec<_> = s.permitted_commands().iter().map(|c| c.id).collect();
    assert!(!permitted.iter().any(|id| id.starts_with("library.")));
}
