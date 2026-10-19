//! `stitchtool connect` forwards JSON lines to a running editor's session.

use openstitch_engine::Session;
use openstitch_remote::{Server, ServerConfig};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn connect_forwards_requests_to_the_editor() {
    let file = std::env::temp_dir().join(format!(
        "openstitch-connect-{}/automation.json",
        std::process::id()
    ));
    let server = Server::start(ServerConfig {
        endpoint_file: Some(file.clone()),
        ..Default::default()
    })
    .expect("listen");
    let mut child = Command::new(env!("CARGO_BIN_EXE_stitchtool"))
        .args(["connect", "--endpoint"])
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(
        stdin,
        r#"{{"id":1,"command":"shape.ellipse","params":{{"cx":0,"cy":0,"rx":5,"ry":5}}}}"#
    )
    .expect("write");
    writeln!(stdin, r#"{{"id":2,"command":"object.list"}}"#).expect("write");
    drop(stdin);

    let mut session = Session::new(".");
    let mut served = 0;
    while served < 2 {
        match server.wait_next(Duration::from_secs(20)) {
            Some(job) => {
                let reply = server.policy().handle(&mut session, job.request());
                job.respond(reply);
                served += 1;
            }
            None => panic!("no request from stitchtool"),
        }
    }
    let out = child.wait_with_output().expect("output");
    assert!(out.status.success());
    let replies: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["id"], json!(1));
    assert_eq!(replies[1]["result"].as_array().map(Vec::len), Some(1));

    let missing = Command::new(env!("CARGO_BIN_EXE_stitchtool"))
        .args(["connect", "--endpoint", "/nonexistent/automation.json"])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert!(!missing.status.success());
}
