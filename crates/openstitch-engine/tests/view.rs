//! View aids saved with the design (guidelines) and the Fabric Selector.

use openstitch_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "openstitch-view-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn guidelines_are_set_saved_and_undone() {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    assert_eq!(
        run(&mut s, "guides.list", json!({})),
        json!({"vertical": [], "horizontal": []})
    );
    run(&mut s, "guides.set", json!({"vertical": [10, 25.5]}));
    let out = run(&mut s, "guides.set", json!({"horizontal": [-3]}));
    assert_eq!(out, json!({"vertical": [10.0, 25.5], "horizontal": [-3.0]}));
    assert_eq!(s.design().guides.vertical, [100.0, 255.0]);
    run(&mut s, "project.save", json!({"path": "g.osd"}));
    let mut t = Session::new(&dir);
    run(&mut t, "project.open", json!({"path": "g.osd"}));
    assert_eq!(t.design().guides, s.design().guides);
    run(&mut s, "history.undo", json!({}));
    assert!(s.design().guides.horizontal.is_empty());
    assert_eq!(s.design().guides.vertical.len(), 2);

    let many: Vec<f64> = (0..101).map(f64::from).collect();
    for params in [
        json!({"vertical": many}),
        json!({"vertical": [1e9]}),
        json!({"horizontal": ["a"]}),
        json!({"horizontal": 5}),
    ] {
        assert!(s.execute("guides.set", Some(&params)).is_err(), "{params}");
    }
    // A design without guidelines does not mention them in the file.
    let mut e = Session::new(&dir);
    run(&mut e, "project.save", json!({"path": "e.osd"}));
    let text = std::fs::read_to_string(dir.join("e.osd")).expect("read");
    assert!(!text.contains("guides"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fabric_selector_sets_recommended_attributes() {
    let mut s = Session::new(std::env::temp_dir());
    let list = run(&mut s, "fabric.list", json!({}));
    let fabrics = list.as_array().expect("list");
    assert!(fabrics.len() >= 6);
    for f in fabrics {
        assert!(f["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert!(f["advice"].as_str().is_some_and(|n| !n.is_empty()));
    }
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 30, "height": 20, "fill": "#ff0000"}),
    );
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 50, "y": 10, "width": 30, "height": 8, "fill": "#00ff00"}),
    );
    run(&mut s, "object.setRegion", json!({"id": 2, "type": "satin"}));
    run(&mut s, "text.add", json!({"text": "A", "x": 50, "y": 60}));
    let before = s.design().clone();

    let out = run(&mut s, "fabric.apply", json!({"fabric": "towel"}));
    assert_eq!(out["changed"].as_array().map(Vec::len), Some(3));
    let d = s.design();
    let towel = openstitch_engine::fabric::find("towel").expect("towel");
    let fill = d.objects[0].region.as_ref().expect("fill");
    assert_eq!(fill.density, towel.fill_density);
    assert!(fill.underlay);
    assert_eq!(fill.thread, before.objects[0].region.as_ref().expect("r").thread);
    let satin = d.objects[1].region.as_ref().expect("satin");
    assert_eq!(satin.density, towel.satin_density);
    assert_eq!(satin.pull_compensation, towel.pull_compensation);

    // Only the objects asked for; leather turns underlay off.
    run(&mut s, "fabric.apply", json!({"fabric": "leather", "ids": [1]}));
    let d = s.design();
    assert!(!d.objects[0].region.as_ref().expect("fill").underlay);
    assert_eq!(
        d.objects[1].region.as_ref().expect("satin").density,
        towel.satin_density
    );
    run(&mut s, "history.undo", json!({}));
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design(), &before);

    for params in [
        json!({"fabric": "chainmail"}),
        json!({}),
        json!({"fabric": "felt", "ids": [99]}),
        json!({"fabric": "felt", "ids": []}),
    ] {
        assert!(s.execute("fabric.apply", Some(&params)).is_err(), "{params}");
    }
}
