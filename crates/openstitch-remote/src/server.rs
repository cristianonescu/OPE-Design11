use crate::endpoint::{Endpoint, new_token, tokens_match};
use openstitch_engine::{Capabilities, Sandbox, Session};
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the listener is set up and what its clients may do.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// `0`: the OS picks a free port.
    pub port: u16,
    /// What every client may do (read-only mode, file access off).
    pub capabilities: Capabilities,
    /// Directories whose files commands may only read.
    pub read_roots: Vec<PathBuf>,
    /// Directories whose files commands may read and write. With no roots at all, every file
    /// command is refused.
    pub write_roots: Vec<PathBuf>,
    /// Largest accepted request line, in bytes; a bigger one closes the connection.
    pub max_request_bytes: usize,
    /// A connection that sends nothing for this long is closed.
    pub idle_timeout: Duration,
    /// How long a request waits for the UI thread before the client gets a timeout error.
    pub reply_timeout: Duration,
    /// Connections served at once; more are refused.
    pub max_connections: usize,
    /// Where the address and token are written (`None`: not written, e.g. in tests).
    pub endpoint_file: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            port: 0,
            capabilities: Capabilities::FULL,
            read_roots: Vec::new(),
            write_roots: Vec::new(),
            max_request_bytes: crate::DEFAULT_MAX_REQUEST_BYTES,
            idle_timeout: Duration::from_secs(300),
            reply_timeout: Duration::from_secs(30),
            max_connections: 4,
            endpoint_file: None,
        }
    }
}

/// The capabilities and file roots socket clients run with. The host's own session keeps its
/// settings: [`Policy::apply`] swaps the client's in for one request and [`Policy::restore`]
/// puts them back.
#[derive(Clone, Debug)]
pub struct Policy {
    pub capabilities: Capabilities,
    pub sandbox: Sandbox,
}

/// The host session's own settings while a client request runs.
#[must_use = "pass to Policy::restore"]
pub struct Saved(Capabilities, Option<Sandbox>);

impl Policy {
    pub fn apply(&self, session: &mut Session) -> Saved {
        let capabilities = session.capabilities();
        session.set_capabilities(self.capabilities);
        Saved(
            capabilities,
            session.set_sandbox(Some(self.sandbox.clone())),
        )
    }

    pub fn restore(session: &mut Session, saved: Saved) {
        session.set_capabilities(saved.0);
        session.set_sandbox(saved.1);
    }

    /// Runs one request with this policy (hosts without their own undo grouping).
    pub fn handle(&self, session: &mut Session, request: &Value) -> Value {
        let saved = self.apply(session);
        let reply = session.handle(request);
        Self::restore(session, saved);
        reply
    }
}

/// One request (token removed) waiting for the host's UI thread.
pub struct Job {
    request: Value,
    reply: Sender<Value>,
    abandoned: Arc<AtomicBool>,
}

impl Job {
    pub fn request(&self) -> &Value {
        &self.request
    }

    pub fn respond(self, reply: Value) {
        // The client may have timed out or gone away.
        let _ = self.reply.send(reply);
    }
}

type Waker = Box<dyn Fn() + Send + Sync>;

struct Shared {
    token: String,
    max_request_bytes: usize,
    idle_timeout: Duration,
    reply_timeout: Duration,
    max_connections: usize,
    active: AtomicUsize,
    /// Refused connections still being closed (see [`close_with`]).
    refusing: AtomicUsize,
    stop: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

/// A running listener. Requests arrive through [`Server::try_next`] on the host's thread;
/// dropping the server stops accepting and removes the endpoint file.
pub struct Server {
    endpoint: Endpoint,
    policy: Policy,
    jobs: Receiver<Job>,
    shared: Arc<Shared>,
    endpoint_file: Option<PathBuf>,
}

impl Server {
    /// Binds `127.0.0.1`, writes the endpoint file and starts accepting. Fails if a root is not
    /// an existing directory, the port is taken or the endpoint file cannot be written.
    pub fn start(config: ServerConfig) -> io::Result<Server> {
        let sandbox = Sandbox::new(&config.read_roots, &config.write_roots)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.0))?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))?;
        let endpoint = Endpoint {
            address: listener.local_addr()?,
            token: new_token()?,
        };
        if let Some(path) = &config.endpoint_file {
            endpoint.write(path)?;
        }
        let shared = Arc::new(Shared {
            token: endpoint.token.clone(),
            max_request_bytes: config.max_request_bytes.max(1),
            idle_timeout: config.idle_timeout,
            reply_timeout: config.reply_timeout,
            max_connections: config.max_connections.max(1),
            active: AtomicUsize::new(0),
            refusing: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            waker: Mutex::new(None),
        });
        let (tx, jobs) = mpsc::channel();
        let accept_shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("openstitch-remote".into())
            .spawn(move || accept(listener, accept_shared, tx))?;
        Ok(Server {
            endpoint,
            policy: Policy {
                capabilities: config.capabilities,
                sandbox,
            },
            jobs,
            shared,
            endpoint_file: config.endpoint_file,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.endpoint.address
    }

    pub fn token(&self) -> &str {
        &self.endpoint.token
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn endpoint_file(&self) -> Option<&std::path::Path> {
        self.endpoint_file.as_deref()
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Called from the connection threads whenever a request is queued, e.g. to repaint an
    /// idle UI so it drains the queue.
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut w) = self.shared.waker.lock() {
            *w = Some(Box::new(waker));
        }
    }

    /// The next queued request, skipping those whose client already gave up.
    pub fn try_next(&self) -> Option<Job> {
        loop {
            let job = self.jobs.try_recv().ok()?;
            if !job.abandoned.load(Ordering::SeqCst) {
                return Some(job);
            }
        }
    }

    /// Waits up to `timeout` for a request (headless hosts and tests).
    pub fn wait_next(&self, timeout: Duration) -> Option<Job> {
        match self.jobs.recv_timeout(timeout) {
            Ok(job) if !job.abandoned.load(Ordering::SeqCst) => Some(job),
            Ok(_) => self.try_next(),
            Err(_) => None,
        }
    }

    /// Runs every queued request on `session` with the client policy; returns how many ran.
    pub fn serve_pending(&self, session: &mut Session) -> usize {
        let mut n = 0;
        while let Some(job) = self.try_next() {
            let reply = self.policy.handle(session, job.request());
            job.respond(reply);
            n += 1;
        }
        n
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = TcpStream::connect_timeout(&self.endpoint.address, Duration::from_secs(1));
        // Leave the file alone if another instance has replaced it since.
        if let Some(path) = &self.endpoint_file
            && Endpoint::read(path).is_ok_and(|e| e == self.endpoint)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn accept(listener: TcpListener, shared: Arc<Shared>, jobs: Sender<Job>) {
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
            continue;
        }
        if shared.active.load(Ordering::SeqCst) >= shared.max_connections {
            refuse(stream, &shared);
            continue;
        }
        shared.active.fetch_add(1, Ordering::SeqCst);
        let (client_shared, client_jobs) = (Arc::clone(&shared), jobs.clone());
        let spawned = std::thread::Builder::new()
            .name("openstitch-remote-client".into())
            .spawn(move || {
                let _ = connection(stream, &client_shared, &client_jobs);
                client_shared.active.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            shared.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Connections refused at once; beyond this, extra connections are dropped without a reply.
const MAX_REFUSING: usize = 8;
/// How long a closing connection is drained for the client's unread request bytes.
const DRAIN_TIME: Duration = Duration::from_secs(1);
/// At most this much is drained before closing anyway.
const DRAIN_BYTES: usize = 4 << 20;

/// Answers "too many connections" on a short-lived thread, so the accept loop never waits for
/// the drain.
fn refuse(mut stream: TcpStream, shared: &Arc<Shared>) {
    if shared.refusing.fetch_add(1, Ordering::SeqCst) >= MAX_REFUSING {
        shared.refusing.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    let refusing = Arc::clone(shared);
    let spawned = std::thread::Builder::new()
        .name("openstitch-remote-refuse".into())
        .spawn(move || {
            close_with(&mut stream, &failure(None, "too many connections"));
            refusing.refusing.fetch_sub(1, Ordering::SeqCst);
        });
    if spawned.is_err() {
        shared.refusing.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Sends a final reply and closes gracefully: half-close the write side, then read and discard
/// what the client is still sending (bounded in time and bytes) before dropping the socket.
/// Closing a socket with unread received data makes the OS send a reset instead of a normal
/// close, and on Windows a reset discards the reply before the client has read it.
fn close_with(stream: &mut TcpStream, reply: &Value) {
    if write_line(stream, reply).is_err() {
        return;
    }
    let _ = stream.shutdown(Shutdown::Write);
    let deadline = Instant::now() + DRAIN_TIME;
    let mut buf = [0u8; 8192];
    let mut drained = 0;
    while drained < DRAIN_BYTES {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
}

fn failure(id: Option<&Value>, message: &str) -> Value {
    let mut reply = Map::new();
    if let Some(id) = id {
        reply.insert("id".into(), id.clone());
    }
    reply.insert("ok".into(), json!(false));
    reply.insert("error".into(), json!(message));
    Value::Object(reply)
}

fn write_line(stream: &mut TcpStream, value: &Value) -> io::Result<()> {
    let mut line = value.to_string();
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()
}

/// Serves one client: one request per line until it disconnects, idles out, sends an
/// oversized line or presents a wrong token.
fn connection(stream: TcpStream, shared: &Shared, jobs: &Sender<Job>) -> io::Result<()> {
    stream.set_read_timeout(Some(shared.idle_timeout))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let max = shared.max_request_bytes;
    let mut line = Vec::new();
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        line.clear();
        if (&mut reader)
            .take(max as u64 + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        let mut body = line.strip_suffix(b"\n").unwrap_or(&line);
        body = body.strip_suffix(b"\r").unwrap_or(body);
        if body.len() > max {
            let message = format!("request exceeds {max} bytes");
            close_with(&mut writer, &failure(None, &message));
            return Ok(());
        }
        if body.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(Value::Object(mut request)) = serde_json::from_slice::<Value>(body) else {
            write_line(&mut writer, &failure(None, "invalid JSON request"))?;
            continue;
        };
        let token = request.remove("token");
        if !matches!(&token, Some(Value::String(t)) if tokens_match(&shared.token, t)) {
            close_with(
                &mut writer,
                &failure(request.get("id"), "missing or invalid token"),
            );
            return Ok(());
        }
        let reply = dispatch(Value::Object(request), shared, jobs);
        write_line(&mut writer, &reply)?;
    }
}

/// Queues the request for the UI thread and waits for its reply.
fn dispatch(request: Value, shared: &Shared, jobs: &Sender<Job>) -> Value {
    let id = request.get("id").cloned();
    let abandoned = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let job = Job {
        request,
        reply: tx,
        abandoned: Arc::clone(&abandoned),
    };
    if jobs.send(job).is_err() {
        return failure(id.as_ref(), "the editor is shutting down");
    }
    if let Ok(waker) = shared.waker.lock()
        && let Some(wake) = waker.as_ref()
    {
        wake();
    }
    match rx.recv_timeout(shared.reply_timeout) {
        Ok(reply) => reply,
        Err(RecvTimeoutError::Timeout) => {
            abandoned.store(true, Ordering::SeqCst);
            failure(id.as_ref(), "timed out waiting for the editor")
        }
        Err(RecvTimeoutError::Disconnected) => {
            failure(id.as_ref(), "the editor dropped the request")
        }
    }
}
