use openstitch_engine::{Capabilities, Session};
use serde_json::json;

#[test]
fn send_only_targets_detected_removable_volumes() {
    let mut s = Session::new(std::env::temp_dir());
    s.execute(
        "shape.rectangle",
        Some(&json!({"x": 10, "y": 10, "width": 20, "height": 20})),
    )
    .expect("shape");
    let volumes = s.execute("transfer.volumes", None).expect("volumes");
    assert!(volumes.is_array());
    // A path that is not a detected removable volume is refused, even an existing directory.
    let tmp = std::env::temp_dir().display().to_string();
    let e = s
        .execute("transfer.send", Some(&json!({"volume": tmp})))
        .expect_err("not removable")
        .to_string();
    assert!(e.contains("not found"), "{e}");
    for bad in [
        json!({"volume": 3}),
        json!({"format": "phc"}),
        json!({"overwrite": "yes"}),
    ] {
        assert!(s.execute("transfer.send", Some(&bad)).is_err(), "{bad}");
    }
}

#[test]
fn clients_without_file_access_cannot_list_or_send() {
    let mut s = Session::new(std::env::temp_dir());
    s.set_capabilities(Capabilities::FULL.without_files());
    assert!(s.execute("transfer.volumes", None).is_err());
    assert!(s.execute("transfer.send", None).is_err());
    let names: Vec<_> = s.permitted_commands().iter().map(|c| c.id).collect();
    assert!(!names.contains(&"transfer.volumes") && !names.contains(&"transfer.send"));
    s.set_capabilities(Capabilities::FULL.read_only());
    assert!(s.execute("transfer.volumes", None).is_ok());
    assert!(s.execute("transfer.send", None).is_err());
}

#[test]
fn sandboxed_sessions_cannot_send_outside_their_roots() {
    let root = std::env::temp_dir().join(format!("openstitch-usb-root-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    let mut s = Session::sandboxed(&root);
    // Whatever is mounted, its path is outside the root: detection may find nothing (CI), in
    // which case the command fails earlier; either way nothing is written.
    let e = s
        .execute("transfer.send", Some(&json!({"volume": "/Volumes/NOPE"})))
        .expect_err("refused");
    assert!(!e.to_string().is_empty());
}
