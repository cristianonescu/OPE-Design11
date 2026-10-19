//! Requests from the loopback automation socket (`--listen`), run on the UI thread between
//! frames so they share the editor's undo history with the user's own edits.

use crate::app::EditorApp;
use openstitch_engine::Effect;
use openstitch_remote::Policy;
use serde_json::{Map, Value, json};

impl EditorApp {
    /// Runs every request socket clients have queued, with their capabilities and file roots.
    pub(crate) fn serve_remote(&mut self) {
        let Some(server) = self.remote.take() else {
            return;
        };
        while let Some(job) = server.try_next() {
            let saved = server.policy().apply(&mut self.session);
            let reply = self.remote_request(job.request());
            Policy::restore(&mut self.session, saved);
            job.respond(reply);
        }
        self.remote = Some(server);
    }

    /// Edits are one Undo step each; undo/redo act like the Edit menu, so a client's undo also
    /// undoes a grouped user gesture as a whole.
    fn remote_request(&mut self, request: &Value) -> Value {
        let command = request
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match self.session.permits(command).map(|c| c.effect) {
            Ok(Effect::History) => {
                let undo = command == "history.undo";
                let possible = if undo {
                    self.session.can_undo()
                } else {
                    self.session.can_redo()
                };
                if !possible {
                    // The engine's "nothing to undo/redo" error.
                    return self.session.handle(request);
                }
                if undo {
                    self.undo();
                } else {
                    self.redo();
                }
                let mut reply = Map::new();
                if let Some(id) = request.get("id") {
                    reply.insert("id".into(), id.clone());
                }
                reply.insert("ok".into(), json!(true));
                reply.insert(
                    "result".into(),
                    json!({ "canUndo": self.session.can_undo(), "canRedo": self.session.can_redo() }),
                );
                Value::Object(reply)
            }
            Ok(Effect::Edit) => {
                let before = self.session.design().clone();
                let reply = self.session.handle(request);
                if *self.session.design() != before {
                    self.undo_groups.push(1);
                    self.redo_groups.clear();
                    self.refresh();
                }
                reply
            }
            _ => self.session.handle(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{EditorApp, Launch};
    use openstitch_engine::Capabilities;
    use openstitch_remote::{Client, ServerConfig};
    use serde_json::{Value, json};

    /// Sends `request` from a client thread and drains it on this ("UI") thread.
    fn send(app: &mut EditorApp, client: &mut Option<Client>, request: Value) -> Value {
        let mut c = client.take().expect("client");
        let t = std::thread::spawn(move || (c.request(&request), c));
        while !t.is_finished() {
            app.serve_remote();
            std::thread::yield_now();
        }
        let (reply, c) = t.join().expect("join");
        *client = Some(c);
        reply.expect("reply")
    }

    #[test]
    fn socket_requests_share_the_editor_history() {
        let mut app = EditorApp::new(Launch::default());
        let server = openstitch_remote::Server::start(ServerConfig::default()).expect("listen");
        let mut client = Some(Client::new(server.endpoint().clone()).expect("client"));
        app.remote = Some(server);
        let rect = json!({"command": "shape.rectangle",
            "params": {"x": 0, "y": 0, "width": 20, "height": 10}});
        let added = send(&mut app, &mut client, rect.clone());
        assert_eq!(added["ok"], json!(true), "{added}");
        assert_eq!(app.session.design().objects.len(), 1);
        assert!(app.is_dirty(), "the editor recompiled");
        // The user's Undo undoes the client's edit, and the client's undo the user's.
        app.undo();
        assert!(app.session.design().objects.is_empty());
        app.redo();
        let undone = send(
            &mut app,
            &mut client,
            json!({"id": 3, "command": "history.undo"}),
        );
        assert_eq!(undone["id"], json!(3));
        assert_eq!(undone["result"]["canRedo"], json!(true));
        assert!(app.session.design().objects.is_empty());
        let nothing = send(&mut app, &mut client, json!({"command": "history.undo"}));
        assert_eq!(nothing["ok"], json!(false));
        // The editor's own session keeps its capabilities and no sandbox.
        assert_eq!(app.session.capabilities(), Capabilities::FULL);
        assert!(app.session.sandbox().is_none());
    }

    #[test]
    fn read_only_socket_clients_cannot_edit_or_undo() {
        let mut app = EditorApp::new(Launch::default());
        let server = openstitch_remote::Server::start(ServerConfig {
            capabilities: Capabilities::FULL.read_only(),
            ..Default::default()
        })
        .expect("listen");
        let mut client = Some(Client::new(server.endpoint().clone()).expect("client"));
        app.remote = Some(server);
        let refused = send(
            &mut app,
            &mut client,
            json!({"command": "shape.rectangle",
                "params": {"x": 0, "y": 0, "width": 5, "height": 5}}),
        );
        assert_eq!(refused["ok"], json!(false));
        let undo = send(&mut app, &mut client, json!({"command": "history.undo"}));
        assert_eq!(undo["ok"], json!(false));
        assert!(app.session.design().objects.is_empty());
    }
}
