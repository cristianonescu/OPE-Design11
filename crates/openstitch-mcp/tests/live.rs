//! `--live`: tool calls go to a running editor's session over the loopback transport.

use openstitch_engine::Session;
use openstitch_mcp::{Config, Server};
use openstitch_remote::{Client, ServerConfig};
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn tool_calls_run_in_the_live_editor_session() {
    let editor = openstitch_remote::Server::start(ServerConfig::default()).expect("listen");
    let client = Client::new(editor.endpoint().clone()).expect("client");
    // The editor's UI thread, owning its session.
    let (stop, stopped) = mpsc::channel::<()>();
    let host = std::thread::spawn(move || {
        let mut session = Session::new(".");
        while stopped.try_recv().is_err() {
            if let Some(job) = editor.wait_next(Duration::from_millis(20)) {
                let reply = editor.policy().handle(&mut session, job.request());
                job.respond(reply);
            }
        }
        session
    });

    let mut mcp = Server::new(Config::default())
        .expect("server")
        .with_live(client);
    assert!(mcp.is_live());
    let mut next = 0;
    let mut request = |method: &str, params: Value| {
        next += 1;
        let m = json!({"jsonrpc": "2.0", "id": next, "method": method, "params": params});
        let line = mcp.handle_line(&m.to_string()).expect("reply");
        serde_json::from_str::<Value>(&line).expect("json")
    };
    request("initialize", json!({"protocolVersion": "2025-11-25"}));
    let call = |name: &str, arguments: Value| json!({"name": name, "arguments": arguments});
    let added = request(
        "tools/call",
        call(
            "shape_rectangle",
            json!({"x": 0, "y": 0, "width": 10, "height": 10}),
        ),
    );
    assert_eq!(added["result"]["isError"], json!(false), "{added}");
    let failed = request("tools/call", call("object_get", json!({"id": 999})));
    assert_eq!(failed["result"]["isError"], json!(true), "{failed}");
    let preview = request("tools/call", call("preview", json!({})));
    assert_eq!(preview["result"]["isError"], json!(true));

    stop.send(()).expect("stop");
    let session = host.join().expect("join");
    assert_eq!(session.design().objects.len(), 1);
}
