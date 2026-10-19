use openstitch_engine::capabilities::{FILE_READING_COMMANDS, required};
use openstitch_engine::{Access, Capabilities, Capability, Effect, EngineState, Session};
use serde_json::{Value, json};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-caps-{n}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.execute(id, Some(&params))
        .unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A session with one square and a saved project and machine file to open/import.
fn prepared(caps: Capabilities) -> (Session, PathBuf) {
    let dir = temp_dir();
    let mut s = Session::new(&dir);
    run(
        &mut s,
        "shape.rectangle",
        json!({"x": 10, "y": 10, "width": 20, "height": 20}),
    );
    run(&mut s, "project.save", json!({"path": "a.osd"}));
    run(&mut s, "design.export", json!({"path": "a.pes"}));
    s.set_capabilities(caps);
    (s, dir)
}

#[test]
fn every_command_declares_what_it_needs() {
    let s = Session::new(temp_dir());
    for c in s.commands() {
        let needs = required(c);
        match c.effect {
            Effect::Read => assert!(!needs.contains(&Capability::Edit), "{}", c.id),
            Effect::Edit | Effect::History | Effect::Settings => {
                assert!(needs.contains(&Capability::Edit), "{}", c.id)
            }
            Effect::File => assert!(needs.contains(&Capability::WriteFiles), "{}", c.id),
        }
        // A string `path` parameter is a file path; such commands must need file access.
        let takes_file = c
            .params
            .iter()
            .any(|p| p.name == "path" && p.kind == "string");
        if takes_file {
            assert!(
                needs.contains(&Capability::ReadFiles) || needs.contains(&Capability::WriteFiles),
                "{} takes a file path but needs no file capability",
                c.id
            );
        }
    }
    for id in FILE_READING_COMMANDS {
        assert!(
            s.commands().iter().any(|c| c.id == *id),
            "{id} is not a command"
        );
    }
}

#[test]
fn read_only_allows_reads_and_refuses_everything_else() {
    let (mut s, dir) = prepared(Capabilities::FULL.read_only());
    let before = s.design().clone();
    for (id, params) in [
        ("design.info", json!({})),
        ("design.generate", json!({})),
        ("object.list", json!({})),
        ("object.get", json!({"id": 1})),
        ("machine.hoops", json!({})),
        ("design.measure", json!({"from": [0, 0], "to": [1, 1]})),
        ("commands.list", json!({})),
    ] {
        run(&mut s, id, params);
    }
    for (id, params) in [
        (
            "shape.rectangle",
            json!({"x": 0, "y": 0, "width": 5, "height": 5}),
        ),
        ("object.move", json!({"id": 1, "dx": 5})),
        ("object.delete", json!({"id": 1})),
        ("machine.set", json!({"hoop": "130x180"})),
        ("design.new", json!({})),
        ("history.undo", json!({})),
        ("project.open", json!({"path": "a.osd"})),
        ("stitches.import", json!({"path": "a.pes"})),
        ("design.export", json!({"path": "b.pes"})),
        ("project.save", json!({"path": "b.osd"})),
    ] {
        let e = s.execute(id, Some(&params)).expect_err(id).to_string();
        assert!(e.contains("not permitted"), "{id}: {e}");
    }
    assert_eq!(s.design(), &before);
    assert!(!dir.join("b.pes").exists() && !dir.join("b.osd").exists());
    // Only read commands are offered.
    assert!(
        s.permitted_commands()
            .iter()
            .all(|c| c.effect == Effect::Read)
    );
}

#[test]
fn no_files_allows_edits_but_no_file_access() {
    let (mut s, dir) = prepared(Capabilities::FULL.without_files());
    run(&mut s, "object.move", json!({"id": 1, "dx": 5}));
    run(&mut s, "history.undo", json!({}));
    for (id, params) in [
        ("project.open", json!({"path": "a.osd"})),
        ("stitches.import", json!({"path": "a.pes"})),
        ("design.export", json!({"path": "c.pes"})),
        ("project.save", json!({"path": "c.osd"})),
    ] {
        let e = s.execute(id, Some(&params)).expect_err(id).to_string();
        assert!(e.contains("not permitted"), "{id}: {e}");
    }
    assert!(!dir.join("c.pes").exists());
    let ids: Vec<_> = s.permitted_commands().iter().map(|c| c.id).collect();
    assert!(ids.contains(&"shape.rectangle") && !ids.contains(&"design.export"));
    assert_eq!(s.capabilities().names(), ["edit"]);
}

#[test]
fn file_paths_are_refused_even_for_unlisted_file_commands() {
    // The safety net: any command that resolves a path is refused when file access is off,
    // whether or not it is in FILE_READING_COMMANDS.
    let state = EngineState {
        design: Default::default(),
        base_dir: temp_dir(),
        sandbox: None,
        capabilities: Capabilities::FULL.without_files(),
        user: Default::default(),
    };
    assert!(state.resolve("x.pes", Access::Read, "test").is_err());
    assert!(state.resolve("x.pes", Access::Write, "test").is_err());
    let read_only = EngineState {
        capabilities: Capabilities::FULL.read_only(),
        ..state
    };
    assert!(read_only.resolve("x.pes", Access::Read, "test").is_ok());
    assert!(read_only.resolve("x.pes", Access::Write, "test").is_err());
}

#[test]
fn commands_list_reports_requirements() {
    let mut s = Session::new(temp_dir());
    let list = run(&mut s, "commands.list", json!({}));
    let find = |id: &str| {
        list.as_array()
            .and_then(|a| a.iter().find(|c| c["id"] == id))
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(find("design.info")["requires"], json!([]));
    assert_eq!(
        find("stitches.import")["requires"],
        json!(["edit", "readFiles"])
    );
    assert_eq!(find("design.export")["requires"], json!(["writeFiles"]));
}
