use serde_json::{Value, json};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// Where a running editor listens and the token its clients must present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub address: SocketAddr,
    pub token: String,
}

/// `automation.json` next to the user settings in the OS config directory.
pub fn default_endpoint_file() -> Option<PathBuf> {
    openstitch_engine::user::default_path()
        .and_then(|p| p.parent().map(|d| d.join("automation.json")))
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Endpoint {
    /// Reads an endpoint file written by [`Endpoint::write`].
    pub fn read(path: &Path) -> io::Result<Endpoint> {
        let text = std::fs::read_to_string(path)?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        let address = value["address"]
            .as_str()
            .and_then(|a| a.parse().ok())
            .ok_or_else(|| invalid(format!("{}: no valid 'address'", path.display())))?;
        let token = value["token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| invalid(format!("{}: no 'token'", path.display())))?
            .to_string();
        Ok(Endpoint { address, token })
    }

    /// Writes the endpoint file readable by the user only. An existing file (or symlink) is
    /// removed first and the new one is created exclusively, so nothing else is overwritten.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            create_private_dir(dir)?;
        }
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let body = json!({
            "address": self.address.to_string(),
            "token": self.token,
            "pid": std::process::id(),
        });
        writeln!(file, "{body}")?;
        file.sync_all()
    }
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// 32 bytes from the OS random source, as lowercase hex.
pub(crate) fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares without an early exit, so timing does not reveal how much of a guess matched.
pub(crate) fn tokens_match(expected: &str, presented: &str) -> bool {
    let (a, b) = (expected.as_bytes(), presented.as_bytes());
    let mut diff = a.len() ^ b.len();
    for (i, x) in a.iter().enumerate() {
        diff |= usize::from(x ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_hex_and_compare_exactly() {
        let (a, b) = (new_token().expect("token"), new_token().expect("token"));
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(tokens_match(&a, &a));
        assert!(!tokens_match(&a, &b));
        assert!(!tokens_match(&a, &a[..63]));
        assert!(!tokens_match(&a, &format!("{a}0")));
        assert!(!tokens_match(&a, ""));
    }

    #[test]
    fn endpoint_file_round_trips_and_is_private() {
        let dir = std::env::temp_dir().join(format!("openstitch-endpoint-{}", std::process::id()));
        let path = dir.join("sub").join("automation.json");
        let e = Endpoint {
            address: "127.0.0.1:4567".parse().expect("addr"),
            token: "abc".into(),
        };
        e.write(&path).expect("write");
        e.write(&path).expect("rewrite replaces");
        assert_eq!(Endpoint::read(&path).expect("read"), e);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::write(&path, "{}").expect("overwrite");
        assert!(Endpoint::read(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
