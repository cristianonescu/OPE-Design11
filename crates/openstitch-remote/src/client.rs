use crate::endpoint::Endpoint;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

struct Connection {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

/// Talks to a running editor. Connects on first use and reconnects once when the editor has
/// closed an idle connection.
pub struct Client {
    endpoint: Endpoint,
    timeout: Duration,
    connection: Option<Connection>,
}

impl Client {
    /// Refuses addresses that are not loopback, so the token never leaves the machine.
    pub fn new(endpoint: Endpoint) -> io::Result<Client> {
        if !endpoint.address.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a loopback address", endpoint.address),
            ));
        }
        Ok(Client {
            endpoint,
            timeout: Duration::from_secs(60),
            connection: None,
        })
    }

    /// Reads the address and token from an endpoint file.
    pub fn from_file(path: &Path) -> io::Result<Client> {
        Client::new(Endpoint::read(path)?)
    }

    /// How long to wait for connecting and for each reply.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn connect(&self) -> io::Result<Connection> {
        let stream = TcpStream::connect_timeout(&self.endpoint.address, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        Ok(Connection {
            writer: stream.try_clone()?,
            reader: BufReader::new(stream),
        })
    }

    /// Sends one request object (the token is added) and returns the editor's reply.
    pub fn request(&mut self, request: &Value) -> io::Result<Value> {
        let Some(fields) = request.as_object() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a request must be a JSON object",
            ));
        };
        let mut fields = fields.clone();
        fields.insert("token".into(), json!(self.endpoint.token));
        let mut line = Value::Object(fields).to_string();
        line.push('\n');
        if let Some(mut connection) = self.connection.take() {
            match exchange(&mut connection, &line) {
                Ok(Some(reply)) => {
                    self.connection = Some(connection);
                    return Ok(reply);
                }
                // Closed while idle: the request was not read; send it on a new connection.
                Ok(None) => {}
                Err(e) => return Err(e),
            }
        }
        let mut connection = self.connect()?;
        let reply = exchange(&mut connection, &line)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the editor closed the connection",
            )
        })?;
        self.connection = Some(connection);
        Ok(reply)
    }

    /// Like [`Client::request`], with transport failures as `{"ok": false, "error"}` replies.
    pub fn handle(&mut self, request: &Value) -> Value {
        self.request(request).unwrap_or_else(|e| {
            let mut reply = serde_json::Map::new();
            if let Some(id) = request.get("id") {
                reply.insert("id".into(), id.clone());
            }
            reply.insert("ok".into(), json!(false));
            reply.insert(
                "error".into(),
                json!(format!("cannot reach the editor: {e}")),
            );
            Value::Object(reply)
        })
    }
}

/// Writes `line` and reads one reply; `None` if the server had already closed the connection.
fn exchange(connection: &mut Connection, line: &str) -> io::Result<Option<Value>> {
    if let Err(e) = connection
        .writer
        .write_all(line.as_bytes())
        .and_then(|()| connection.writer.flush())
    {
        return match e.kind() {
            io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted => Ok(None),
            _ => Err(e),
        };
    }
    let mut reply = Vec::new();
    let n = match (&mut connection.reader)
        .take(crate::MAX_REPLY_BYTES as u64 + 1)
        .read_until(b'\n', &mut reply)
    {
        Ok(n) => n,
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) =>
        {
            0
        }
        Err(e) => return Err(e),
    };
    if n == 0 {
        return Ok(None);
    }
    if reply.len() > crate::MAX_REPLY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply exceeds the size limit",
        ));
    }
    serde_json::from_slice(&reply)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
