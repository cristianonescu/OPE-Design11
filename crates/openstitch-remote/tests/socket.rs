//! The loopback transport end to end: a test host drains requests into a `Session` the way the
//! editor's UI thread does, and clients talk to it over TCP.

use openstitch_engine::{Capabilities, Session};
use openstitch_remote::{Client, Endpoint, Server, ServerConfig};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

fn temp_dir(name: &str) -> PathBuf {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openstitch-remote-{name}-{}-{t}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.canonicalize().expect("canonical")
}

/// A server whose requests run on a host thread that owns the session.
struct Host {
    endpoint: Endpoint,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Session>>,
}

impl Host {
    fn start(config: ServerConfig) -> Host {
        let server = Server::start(config).expect("server");
        let endpoint = server.endpoint().clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut session = Session::new(".");
            while !flag.load(Ordering::SeqCst) {
                if let Some(job) = server.wait_next(Duration::from_millis(20)) {
                    let reply = server.policy().handle(&mut session, job.request());
                    job.respond(reply);
                }
            }
            session
        });
        Host {
            endpoint,
            stop,
            thread: Some(thread),
        }
    }

    fn client(&self) -> Client {
        Client::new(self.endpoint.clone())
            .expect("client")
            .with_timeout(Duration::from_secs(10))
    }

    /// Stops the host and returns its session.
    fn finish(mut self) -> Session {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().expect("thread").join().expect("join")
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Sends raw lines and reads one reply per line; `None` once the server closed the connection.
fn raw(endpoint: &Endpoint, lines: &[String]) -> Vec<Option<Value>> {
    let mut stream = TcpStream::connect(endpoint.address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut out = Vec::new();
    for line in lines {
        // A server that already answered and closed may refuse the write; its reply can still
        // be waiting, so read regardless.
        let _ = stream
            .write_all(line.as_bytes())
            .and_then(|()| stream.write_all(b"\n"));
        let mut reply = String::new();
        out.push(match reader.read_line(&mut reply) {
            Ok(n) if n > 0 => Some(serde_json::from_str(&reply).expect("reply json")),
            _ => None,
        });
    }
    out
}

#[test]
fn round_trips_commands_through_the_host_session() {
    let host = Host::start(ServerConfig::default());
    let mut client = host.client();
    let added = client
        .request(&json!({"id": 7, "command": "shape.rectangle",
            "params": {"x": 0, "y": 0, "width": 20, "height": 10}}))
        .expect("reply");
    assert_eq!(added["id"], json!(7));
    assert_eq!(added["ok"], json!(true), "{added}");
    let listed = client
        .request(&json!({"command": "object.list"}))
        .expect("reply");
    assert_eq!(
        listed["result"].as_array().map(Vec::len),
        Some(1),
        "{listed}"
    );
    // A second client sees the same session, and undo/redo is that session's history.
    let mut other = host.client();
    let undone = other
        .request(&json!({"command": "history.undo"}))
        .expect("reply");
    assert_eq!(undone["result"], json!({"canUndo": false, "canRedo": true}));
    let unknown = other
        .request(&json!({"command": "no.such"}))
        .expect("reply");
    assert_eq!(unknown["ok"], json!(false));
    other
        .request(&json!({"command": "history.redo"}))
        .expect("reply");
    let session = host.finish();
    assert_eq!(session.design().objects.len(), 1);
    assert!(session.can_undo());
}

#[test]
fn rejects_missing_and_wrong_tokens_and_closes_the_connection() {
    let host = Host::start(ServerConfig::default());
    let good = format!(
        r#"{{"token":"{}","command":"object.list"}}"#,
        host.endpoint.token
    );
    let wrong = r#"{"id":"a","token":"00","command":"object.list"}"#.to_string();
    let replies = raw(&host.endpoint, &[wrong, good.clone()]);
    let first = replies[0].as_ref().expect("reply");
    assert_eq!(first["ok"], json!(false));
    assert_eq!(first["id"], json!("a"));
    assert!(first["error"].as_str().is_some_and(|e| e.contains("token")));
    assert!(replies[1].is_none(), "connection closed after a bad token");

    let missing = r#"{"command":"shape.rectangle","params":{"x":0,"y":0,"width":5,"height":5}}"#;
    let replies = raw(&host.endpoint, &[missing.to_string()]);
    assert_eq!(replies[0].as_ref().expect("reply")["ok"], json!(false));

    let replies = raw(&host.endpoint, &[good]);
    assert_eq!(replies[0].as_ref().expect("reply")["ok"], json!(true));
    assert!(host.finish().design().objects.is_empty(), "nothing ran");
}

#[test]
fn refuses_oversized_requests() {
    let host = Host::start(ServerConfig {
        max_request_bytes: 1024,
        ..Default::default()
    });
    let big = format!(
        r#"{{"token":"{}","command":"object.list","pad":"{}"}}"#,
        host.endpoint.token,
        "x".repeat(4096)
    );
    let small = format!(
        r#"{{"token":"{}","command":"object.list"}}"#,
        host.endpoint.token
    );
    let replies = raw(&host.endpoint, &[big, small.clone()]);
    let first = replies[0].as_ref().expect("reply");
    assert!(
        first["error"]
            .as_str()
            .is_some_and(|e| e.contains("exceeds 1024 bytes")),
        "{first}"
    );
    assert!(
        replies[1].is_none(),
        "connection closed after an oversized line"
    );
    let replies = raw(&host.endpoint, &[small, "not json".into()]);
    assert_eq!(replies[0].as_ref().expect("reply")["ok"], json!(true));
    assert_eq!(replies[1].as_ref().expect("reply")["ok"], json!(false));
}

#[test]
fn read_only_clients_cannot_edit() {
    let host = Host::start(ServerConfig {
        capabilities: Capabilities::FULL.read_only(),
        ..Default::default()
    });
    let mut client = host.client();
    let edit = client
        .request(&json!({"command": "shape.rectangle",
            "params": {"x": 0, "y": 0, "width": 5, "height": 5}}))
        .expect("reply");
    assert!(
        edit["error"]
            .as_str()
            .is_some_and(|e| e.contains("'edit' capability")),
        "{edit}"
    );
    let undo = client
        .request(&json!({"command": "history.undo"}))
        .expect("reply");
    assert_eq!(undo["ok"], json!(false));
    let read = client
        .request(&json!({"command": "design.info"}))
        .expect("reply");
    assert_eq!(read["ok"], json!(true), "{read}");
    // The host's own session keeps full capabilities between requests.
    let session = host.finish();
    assert_eq!(session.capabilities(), Capabilities::FULL);
    assert!(session.sandbox().is_none());
}

#[test]
fn file_access_is_confined_to_the_configured_roots() {
    // No roots: every file command is refused.
    let host = Host::start(ServerConfig::default());
    let dir = temp_dir("roots");
    let target = dir.join("a.osd");
    let mut client = host.client();
    let save = client
        .request(&json!({"command": "project.save",
            "params": {"path": target.display().to_string()}}))
        .expect("reply");
    assert_eq!(save["ok"], json!(false), "{save}");
    assert!(!target.exists());
    drop(host);

    // A write root: inside is allowed, outside and `..` are not.
    let outside = temp_dir("outside");
    let host = Host::start(ServerConfig {
        write_roots: vec![dir.clone()],
        ..Default::default()
    });
    let mut client = host.client();
    let save = |client: &mut Client, path: String| {
        client
            .request(&json!({"command": "project.save", "params": {"path": path}}))
            .expect("reply")
    };
    assert_eq!(save(&mut client, "a.osd".into())["ok"], json!(true));
    assert!(target.exists());
    let escaped = save(&mut client, outside.join("b.osd").display().to_string());
    assert!(
        escaped["error"]
            .as_str()
            .is_some_and(|e| e.contains("outside")),
        "{escaped}"
    );
    assert_eq!(save(&mut client, "../b.osd".into())["ok"], json!(false));
    drop(host);

    // File access off wins over roots.
    let host = Host::start(ServerConfig {
        write_roots: vec![dir.clone()],
        capabilities: Capabilities::FULL.without_files(),
        ..Default::default()
    });
    let mut client = host.client();
    let refused = save(&mut client, "c.osd".into());
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|e| e.contains("writeFiles")),
        "{refused}"
    );
    assert!(!dir.join("c.osd").exists());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn a_busy_host_times_out_and_skips_the_abandoned_request() {
    let server = Server::start(ServerConfig {
        reply_timeout: Duration::from_millis(100),
        ..Default::default()
    })
    .expect("server");
    let mut client = Client::new(server.endpoint().clone()).expect("client");
    let reply = client
        .request(&json!({"id": 1, "command": "shape.rectangle",
            "params": {"x": 0, "y": 0, "width": 5, "height": 5}}))
        .expect("reply");
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|e| e.contains("timed out")),
        "{reply}"
    );
    let mut session = Session::new(".");
    assert_eq!(server.serve_pending(&mut session), 0);
    assert!(session.design().objects.is_empty());
}

#[test]
fn limits_concurrent_connections() {
    let host = Host::start(ServerConfig {
        max_connections: 1,
        ..Default::default()
    });
    let mut first = host.client();
    assert_eq!(
        first
            .request(&json!({"command": "design.info"}))
            .expect("reply")["ok"],
        json!(true)
    );
    let line = format!(
        r#"{{"token":"{}","command":"design.info"}}"#,
        host.endpoint.token
    );
    let replies = raw(&host.endpoint, &[line]);
    let refused = replies[0].as_ref().expect("reply");
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|e| e.contains("too many")),
        "{refused}"
    );
}

#[test]
fn writes_and_removes_the_endpoint_file_and_clients_stay_on_loopback() {
    let dir = temp_dir("endpoint");
    let file = dir.join("automation.json");
    let server = Server::start(ServerConfig {
        endpoint_file: Some(file.clone()),
        ..Default::default()
    })
    .expect("server");
    assert!(server.address().ip().is_loopback());
    let endpoint = Endpoint::read(&file).expect("endpoint");
    assert_eq!(&endpoint, server.endpoint());
    let mut client = Client::from_file(&file).expect("client");
    let mut session = Session::new(".");
    let t = std::thread::spawn(move || client.request(&json!({"command": "design.info"})));
    while server.serve_pending(&mut session) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(t.join().expect("join").expect("reply")["ok"], json!(true));
    drop(server);
    assert!(!file.exists(), "removed on shutdown");

    let remote = Endpoint {
        address: "10.1.2.3:4000".parse().expect("addr"),
        token: "t".into(),
    };
    assert!(Client::new(remote).is_err());
    assert!(
        Server::start(ServerConfig {
            read_roots: vec![dir.join("missing")],
            ..Default::default()
        })
        .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Early closes (refused connection, bad token, oversized line) must not reset the connection
/// while the client's request bytes are still unread: a reset can destroy the reply before the
/// client reads it (always on Windows). The client here keeps sending and reads late.
#[test]
fn early_closes_deliver_the_reply_before_closing() {
    let host = Host::start(ServerConfig {
        max_connections: 1,
        max_request_bytes: 1024,
        ..Default::default()
    });
    let late_reply = |payload: &[u8]| {
        let mut stream = TcpStream::connect(host.endpoint.address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("timeout");
        let _ = stream.write_all(payload);
        std::thread::sleep(Duration::from_millis(300));
        let mut reply = String::new();
        BufReader::new(stream)
            .read_line(&mut reply)
            .expect("reply arrives intact");
        serde_json::from_str::<Value>(&reply).expect("reply json")
    };
    let junk = "x".repeat(64 << 10);
    let wrong = format!("{{\"token\":\"00\",\"command\":\"design.info\"}}\n{junk}\n");
    assert!(
        late_reply(wrong.as_bytes())["error"]
            .as_str()
            .is_some_and(|e| e.contains("token"))
    );
    let oversized = format!("{junk}\n{junk}\n");
    assert!(
        late_reply(oversized.as_bytes())["error"]
            .as_str()
            .is_some_and(|e| e.contains("exceeds"))
    );
    let mut first = host.client();
    assert_eq!(
        first
            .request(&json!({"command": "design.info"}))
            .expect("reply")["ok"],
        json!(true)
    );
    let line = format!(
        "{{\"token\":\"{}\",\"command\":\"design.info\"}}\n{junk}\n",
        host.endpoint.token
    );
    assert!(
        late_reply(line.as_bytes())["error"]
            .as_str()
            .is_some_and(|e| e.contains("too many"))
    );
}
