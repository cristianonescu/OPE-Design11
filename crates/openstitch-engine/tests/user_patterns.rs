//! User stitch patterns (Programmable Stitch Creator): pattern.* commands, use in sewing,
//! persistence and hostile input.

use openstitch_design::Compiler;
use openstitch_design::patterns::PatternKind;
use openstitch_engine::{Session, project};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-user-patterns-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn zigzag_fill() -> Value {
    json!({
        "format": "openstitch.pattern", "version": 1, "kind": "fill",
        "id": "my-zigzag", "name": "My zigzag",
        "paths": [[[0, 0.2], [0.25, 0.8], [0.5, 0.2], [0.75, 0.8], [1, 0.2]]]
    })
}

fn star_motif() -> Value {
    json!({
        "format": "openstitch.pattern", "version": 1, "kind": "motif",
        "id": "my-star", "name": "My star",
        "paths": [[[0, 0], [0.3, -0.3], [0.5, -1], [0.7, -0.3], [1, 0]]]
    })
}

fn session_with_shape() -> Session {
    let mut s = Session::new(temp_dir());
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 40, "height": 30, "fill": "Red", "outline": "Black"}),
    );
    s
}

#[test]
fn user_patterns_are_defined_listed_used_and_undone() {
    let mut s = session_with_shape();
    let r = run(&mut s, "pattern.define", json!({"pattern": zigzag_fill()}));
    assert_eq!(
        r,
        json!({"kind": "fill", "id": "my-zigzag", "replaced": false})
    );
    run(
        &mut s,
        "pattern.define",
        json!({"pattern": star_motif().to_string()}),
    );
    let list = run(&mut s, "patterns.list", json!({}));
    let user: Vec<&Value> = list
        .as_array()
        .expect("list")
        .iter()
        .filter(|p| p["user"] == true)
        .collect();
    assert_eq!(user.len(), 2);

    // Programmable fill with the user tile differs from the default tile.
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "programmableFill"}),
    );
    let default_tile = Compiler::default().compile(s.design());
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "pattern": "my-zigzag"}),
    );
    let user_tile = Compiler::default().compile(s.design());
    assert_ne!(user_tile, default_tile);
    // The motif line uses the user motif.
    run(
        &mut s,
        "object.setLine",
        json!({"id": 1, "type": "motif", "motif": "my-star", "height": 3}),
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "motifFill", "pattern": "my-star"}),
    );
    assert_eq!(
        s.design().pattern_users(PatternKind::Motif, "my-star"),
        vec![1]
    );

    // Redefining replaces in place; the sewing follows.
    let before = Compiler::default().compile(s.design());
    let mut taller = star_motif();
    taller["paths"][0][2] = json!([0.5, -0.5]);
    let r = run(&mut s, "pattern.define", json!({"pattern": taller}));
    assert_eq!(r["replaced"], true);
    assert_eq!(s.design().patterns.len(), 2);
    assert_ne!(Compiler::default().compile(s.design()), before);

    // In use: cannot be deleted. Unused: can.
    assert!(
        s.execute(
            "pattern.delete",
            Some(&json!({"kind": "motif", "id": "my-star"}))
        )
        .is_err()
    );
    run(
        &mut s,
        "object.setRegion",
        json!({"id": 1, "type": "programmableFill", "pattern": "diamonds"}),
    );
    run(
        &mut s,
        "pattern.delete",
        json!({"kind": "fill", "id": "my-zigzag"}),
    );
    assert_eq!(s.design().patterns.len(), 1);
    run(&mut s, "history.undo", json!({}));
    assert_eq!(s.design().patterns.len(), 2);

    // pattern.get returns the document; bundled patterns too.
    let got = run(
        &mut s,
        "pattern.get",
        json!({"kind": "fill", "id": "my-zigzag"}),
    );
    assert_eq!(got["user"], true);
    let floats = |v: &Value| -> Vec<f64> {
        v.as_array()
            .into_iter()
            .flatten()
            .flat_map(|path| path.as_array().into_iter().flatten())
            .flat_map(|pt| pt.as_array().into_iter().flatten())
            .filter_map(Value::as_f64)
            .collect()
    };
    assert_eq!(
        floats(&got["pattern"]["paths"]),
        floats(&zigzag_fill()["paths"])
    );
    let bundled = run(
        &mut s,
        "pattern.get",
        json!({"kind": "motif", "id": "heart"}),
    );
    assert_eq!(bundled["user"], false);

    // .osd carries user patterns and reloads them identically.
    let bytes = project::encode(s.design()).expect("encode");
    assert_eq!(&project::decode(&bytes).expect("decode"), s.design());
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(text.contains("\"openstitch.pattern\""));
}

#[test]
fn designs_without_user_patterns_serialize_as_before() {
    let s = session_with_shape();
    let text = String::from_utf8(project::encode(s.design()).expect("encode")).expect("utf8");
    assert!(!text.contains("patterns"));
}

#[test]
fn patterns_round_trip_through_files_inside_the_sandbox() {
    let root = temp_dir();
    let mut s = Session::sandboxed(&root);
    run(&mut s, "pattern.define", json!({"pattern": zigzag_fill()}));
    run(
        &mut s,
        "pattern.export",
        json!({"kind": "fill", "id": "my-zigzag", "path": "zigzag.json"}),
    );
    run(
        &mut s,
        "pattern.export",
        json!({"kind": "motif", "id": "heart", "path": "heart.json"}),
    );
    let text = std::fs::read_to_string(root.join("zigzag.json")).expect("file");
    assert!(text.starts_with("{\"format\":\"openstitch.pattern\""));
    run(
        &mut s,
        "pattern.delete",
        json!({"kind": "fill", "id": "my-zigzag"}),
    );
    run(&mut s, "pattern.import", json!({"path": "zigzag.json"}));
    assert_eq!(s.design().patterns.len(), 1);
    // A bundled pattern exported and imported again is refused (reserved id).
    assert!(
        s.execute("pattern.import", Some(&json!({"path": "heart.json"})))
            .is_err()
    );
    // Outside the sandbox: refused.
    assert!(
        s.execute(
            "pattern.export",
            Some(&json!({"kind": "fill", "id": "my-zigzag", "path": "../out.json"}))
        )
        .is_err()
    );
}

#[test]
fn hostile_patterns_fail_cleanly() {
    let root = temp_dir();
    let mut s = Session::sandboxed(&root);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 0, "y": 0, "width": 10, "height": 10}),
    );
    let before = s.design().clone();
    let mut bad: Vec<Value> = vec![
        json!({}),
        json!({"pattern": 5}),
        json!({"pattern": "not json"}),
        json!({"pattern": [1, 2]}),
        json!({"pattern": {"format": "openstitch.pattern"}}),
    ];
    let mut doc = |f: &dyn Fn(&mut Value)| {
        let mut d = zigzag_fill();
        f(&mut d);
        bad.push(json!({ "pattern": d }));
    };
    doc(&|d| d["version"] = json!(2));
    doc(&|d| d["kind"] = json!("stamp"));
    doc(&|d| d["id"] = json!("Bad Id"));
    doc(&|d| d["id"] = json!("diamonds"));
    doc(&|d| d["id"] = json!("x".repeat(65)));
    doc(&|d| d["name"] = json!(""));
    doc(&|d| d["paths"] = json!([]));
    doc(&|d| d["paths"] = json!([[[0, 0]]]));
    doc(&|d| d["paths"] = json!([[[0, 0], [2, 0]]]));
    doc(&|d| d["paths"] = json!([[[0, 0], ["a", 0]]]));
    doc(&|d| d["paths"] = json!([[[0, 0], [1e308, 0]]]));
    doc(&|d| d["paths"] = json!(vec![json!([[0, 0], [1, 1]]); 65]));
    doc(&|d| d["paths"] = json!([vec![json!([0.5, 0.5]); 4001]]));
    doc(&|d| d["rowShift"] = json!(-1));
    doc(&|d| d["extra"] = json!(true));
    doc(&|d| {
        d["kind"] = json!("motif");
        d["paths"] = json!([[[0, 0], [0.5, 0.5]]]);
    });
    doc(&|d| {
        d["kind"] = json!("decorative");
        d["paths"] = json!([[[0, 0.2], [1, 0.8]]]);
    });
    for params in &bad {
        assert!(
            s.execute("pattern.define", Some(params)).is_err(),
            "{params} should fail"
        );
        assert_eq!(s.design(), &before, "{params} changed the design");
    }
    for (id, params) in [
        ("pattern.delete", json!({"kind": "fill", "id": "nope"})),
        ("pattern.delete", json!({"kind": "stamp", "id": "x"})),
        ("pattern.get", json!({"kind": "fill", "id": "nope"})),
        ("pattern.import", json!({"path": "missing.json"})),
        (
            "pattern.export",
            json!({"kind": "fill", "id": "nope", "path": "o.json"}),
        ),
        (
            "object.setRegion",
            json!({"id": 1, "type": "programmableFill", "pattern": "my-zigzag"}),
        ),
        (
            "object.setLine",
            json!({"id": 1, "type": "motif", "motif": "my-star"}),
        ),
    ] {
        assert!(s.execute(id, Some(&params)).is_err(), "{id} {params}");
        assert_eq!(s.design(), &before);
    }
    // Files: too large, not UTF-8, not a pattern.
    std::fs::write(root.join("big.json"), vec![b' '; (1 << 20) + 1]).expect("write");
    std::fs::write(root.join("binary.json"), [0xff, 0xfe, 0x00]).expect("write");
    std::fs::write(root.join("other.json"), "{\"format\":\"other\"}").expect("write");
    for f in ["big.json", "binary.json", "other.json"] {
        assert!(
            s.execute("pattern.import", Some(&json!({"path": f})))
                .is_err()
        );
        assert_eq!(s.design(), &before);
    }
    // At most 256 user patterns.
    for i in 0..256 {
        let mut d = zigzag_fill();
        d["id"] = json!(format!("p{i}"));
        run(&mut s, "pattern.define", json!({ "pattern": d }));
    }
    let mut d = zigzag_fill();
    d["id"] = json!("one-too-many");
    assert!(
        s.execute("pattern.define", Some(&json!({ "pattern": d })))
            .is_err()
    );
}

#[test]
fn damaged_projects_with_bad_patterns_are_refused() {
    let mut s = session_with_shape();
    run(&mut s, "pattern.define", json!({"pattern": zigzag_fill()}));
    let good: Value =
        serde_json::from_slice(&project::encode(s.design()).expect("encode")).expect("json");
    let mut out_of_box = good.clone();
    out_of_box["design"]["patterns"][0]["paths"][0][0] = json!([5, 5]);
    let mut duplicate = good.clone();
    let p = good["design"]["patterns"][0].clone();
    duplicate["design"]["patterns"] = json!([p.clone(), p]);
    let mut reserved = good.clone();
    reserved["design"]["patterns"][0]["id"] = json!("heart");
    for bad in [out_of_box, duplicate, reserved] {
        let bytes = serde_json::to_vec(&bad).expect("bytes");
        assert!(project::decode(&bytes).is_err(), "{bad}");
    }
    assert!(project::decode(&serde_json::to_vec(&good).expect("bytes")).is_ok());
}
