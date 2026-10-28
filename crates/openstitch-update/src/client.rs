//! HTTPS client: the release list and verified downloads.

use crate::checksums::Sha256;
use crate::github::{Release, parse_releases};
use sha2::Digest;
use std::fmt;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use ureq::ResponseExt;

/// Largest release asset accepted (the biggest today is 23 MB).
pub const MAX_ASSET_BYTES: u64 = 256 << 20;
/// Largest text answer (release list, checksum file) accepted.
pub const MAX_TEXT_BYTES: u64 = 8 << 20;

/// What `GET /releases` returned.
#[derive(Debug)]
pub enum Fetched {
    /// The list and the `ETag` to send next time.
    Releases {
        list: Vec<Release>,
        etag: Option<String>,
    },
    /// `304`: nothing changed since the `ETag` the caller sent.
    NotModified,
}

#[derive(Debug)]
pub enum DownloadError {
    /// Transport, TLS, timeout, or a non-2xx status.
    Http(String),
    Status(u16),
    Io(std::io::Error),
    /// The URL is not one this client may fetch (not HTTPS, or not a release asset).
    Url(String),
    /// The body is larger than the limit.
    TooLarge,
    /// The downloaded bytes do not hash to the published digest.
    Checksum {
        expected: Sha256,
        actual: Sha256,
    },
    Cancelled,
    /// A reply that could not be parsed.
    Format(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Http(e) => write!(f, "network error: {e}"),
            DownloadError::Status(s) => write!(f, "server answered {s}"),
            DownloadError::Io(e) => write!(f, "file error: {e}"),
            DownloadError::Url(u) => write!(f, "refused URL {u}"),
            DownloadError::TooLarge => write!(f, "the download is larger than allowed"),
            DownloadError::Checksum { expected, actual } => {
                write!(f, "checksum mismatch: expected {expected}, got {actual}")
            }
            DownloadError::Cancelled => write!(f, "cancelled"),
            DownloadError::Format(e) => write!(f, "unexpected reply: {e}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<ureq::Error> for DownloadError {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::StatusCode(s) => DownloadError::Status(s),
            ureq::Error::Io(e) => DownloadError::Io(e),
            ureq::Error::BodyExceedsLimit(_) => DownloadError::TooLarge,
            other => DownloadError::Http(other.to_string()),
        }
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e)
    }
}

/// A blocking HTTPS client bound to one API base and one download prefix.
pub struct Client {
    agent: ureq::Agent,
    api_base: String,
    download_prefix: String,
}

impl Client {
    /// The GitHub client: HTTPS only, 10 s per request phase, a `User-Agent` that names the
    /// running version as GitHub asks.
    pub fn github(version: &str) -> Client {
        Client::new(crate::API_BASE, crate::DOWNLOAD_PREFIX, true, version)
    }

    /// A client for any base (tests point it at a local HTTP server).
    pub fn new(api_base: &str, download_prefix: &str, https_only: bool, version: &str) -> Client {
        let config = ureq::Agent::config_builder()
            .https_only(https_only)
            .http_status_as_error(false)
            // Redirects are followed by hand so every hop is checked against the allow-list.
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs(60)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(10)))
            .user_agent(format!(
                "OpenStitch/{version} (+https://github.com/{})",
                crate::REPO
            ))
            .build();
        Client {
            agent: ureq::Agent::new_with_config(config),
            api_base: api_base.trim_end_matches('/').to_string(),
            download_prefix: download_prefix.to_string(),
        }
    }

    /// `GET {api_base}/repos/{repo}/releases?per_page=20`, conditional on `etag`.
    pub fn releases(&self, repo: &str, etag: Option<&str>) -> Result<Fetched, DownloadError> {
        let url = format!("{}/repos/{repo}/releases?per_page=20", self.api_base);
        let mut headers = vec![
            ("Accept", "application/vnd.github+json".to_string()),
            ("X-GitHub-Api-Version", "2022-11-28".to_string()),
        ];
        if let Some(etag) = etag {
            headers.push(("If-None-Match", etag.to_string()));
        }
        let api = self.api_base.clone();
        let mut res = self.fetch(&url, &headers, &|u| u.starts_with(&api))?;
        let status = res.status().as_u16();
        if status == 304 {
            return Ok(Fetched::NotModified);
        }
        if !(200..300).contains(&status) {
            return Err(DownloadError::Status(status));
        }
        let etag = res
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = res
            .body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()?;
        let list = parse_releases(&text).map_err(DownloadError::Format)?;
        Ok(Fetched::Releases { list, etag })
    }

    /// Downloads a small text asset (the checksum file).
    pub fn text(&self, url: &str) -> Result<String, DownloadError> {
        let mut res = self.fetch(url, &[], &|u| self.asset_url_ok(u))?;
        Ok(res
            .body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()?)
    }

    /// Downloads `url` to `dest`, hashing as it goes; `progress` gets (bytes so far, total if
    /// known). The file is written as `<dest>.part` and renamed only when complete and, when
    /// `expected` is given, matching; otherwise nothing is left behind.
    pub fn download(
        &self,
        url: &str,
        dest: &Path,
        expected: Option<Sha256>,
        max_bytes: u64,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Sha256, DownloadError> {
        let mut res = self.fetch(url, &[], &|u| self.asset_url_ok(u))?;
        let total = res.body().content_length();
        if total.is_some_and(|t| t > max_bytes) {
            return Err(DownloadError::TooLarge);
        }
        let part = part_path(dest);
        let result = (|| {
            let mut file = std::fs::File::create(&part)?;
            let mut reader = res.body_mut().with_config().limit(max_bytes).reader();
            let mut hasher = sha2::Sha256::new();
            let mut buf = vec![0u8; 64 << 10];
            let mut done = 0u64;
            progress(0, total);
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(DownloadError::Cancelled);
                }
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                file.write_all(&buf[..n])?;
                done += n as u64;
                progress(done, total);
            }
            file.sync_all()?;
            drop(file);
            if total.is_some_and(|t| t != done) {
                return Err(DownloadError::Http(format!(
                    "incomplete download: {done} of {} bytes",
                    total.unwrap_or(0)
                )));
            }
            let actual = Sha256(hasher.finalize().into());
            if let Some(expected) = expected
                && expected != actual
            {
                return Err(DownloadError::Checksum { expected, actual });
            }
            std::fs::rename(&part, dest)?;
            Ok(actual)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        result
    }

    /// `GET url` with `headers`, following up to five redirects, each hop checked with
    /// `allowed` before it is requested. A 304 is returned as is; any other non-2xx is an error.
    fn fetch(
        &self,
        url: &str,
        headers: &[(&str, String)],
        allowed: &dyn Fn(&str) -> bool,
    ) -> Result<ureq::http::Response<ureq::Body>, DownloadError> {
        let mut url = url.to_string();
        for _ in 0..=5 {
            if !allowed(&url) {
                return Err(DownloadError::Url(url));
            }
            let mut req = self.agent.get(&url);
            for (k, v) in headers {
                req = req.header(*k, v.as_str());
            }
            let res = req.call()?;
            let status = res.status().as_u16();
            match status {
                200..=299 | 304 => return Ok(res),
                301 | 302 | 303 | 307 | 308 => {
                    let location = res
                        .headers()
                        .get("location")
                        .and_then(|v| v.to_str().ok())
                        .ok_or(DownloadError::Status(status))?;
                    url = resolve(res.get_uri(), location);
                }
                _ => return Err(DownloadError::Status(status)),
            }
        }
        Err(DownloadError::Http("too many redirects".into()))
    }

    /// Release assets live under the download prefix; GitHub serves their bytes from its
    /// object store.
    fn asset_url_ok(&self, url: &str) -> bool {
        url.starts_with(&self.download_prefix)
            || url.starts_with("https://objects.githubusercontent.com/")
            || url.starts_with("https://release-assets.githubusercontent.com/")
    }
}

/// `location` against the request `base`: absolute URLs as they are, absolute paths on the
/// same scheme and authority; anything else (a relative path) is left for the allow-list to
/// refuse.
fn resolve(base: &ureq::http::Uri, location: &str) -> String {
    if location.contains("://") {
        return location.to_string();
    }
    match (base.scheme_str(), base.authority()) {
        (Some(scheme), Some(auth)) if location.starts_with('/') => {
            format!("{scheme}://{auth}{location}")
        }
        _ => location.to_string(),
    }
}

fn part_path(dest: &Path) -> std::path::PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}

#[cfg(test)]
pub(crate) mod mock {
    //! A one-thread HTTP/1.1 server on 127.0.0.1 for the tests.
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;

    #[derive(Clone)]
    pub struct Route {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
        /// Answer 304 when the request carries this `If-None-Match`.
        pub etag: Option<String>,
    }

    pub struct Server {
        pub base: String,
        routes: Arc<Mutex<HashMap<String, Route>>>,
        pub hits: Arc<Mutex<Vec<String>>>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Server {
        pub fn start() -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let routes: Arc<Mutex<HashMap<String, Route>>> = Default::default();
            let hits: Arc<Mutex<Vec<String>>> = Default::default();
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (r, h, s) = (routes.clone(), hits.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                for conn in listener.incoming() {
                    if s.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(conn) = conn {
                        serve(conn, &r, &h);
                    }
                }
            });
            Server {
                base: format!("http://{addr}"),
                routes,
                hits,
                stop,
                thread: Some(thread),
            }
        }

        pub fn route(&self, path: &str, status: u16, body: impl Into<Vec<u8>>) -> &Server {
            self.routes.lock().expect("lock").insert(
                path.to_string(),
                Route {
                    status,
                    headers: Vec::new(),
                    body: body.into(),
                    etag: None,
                },
            );
            self
        }

        pub fn route_with(&self, path: &str, route: Route) -> &Server {
            self.routes
                .lock()
                .expect("lock")
                .insert(path.to_string(), route);
            self
        }

        pub fn redirect(&self, path: &str, to: &str) -> &Server {
            self.route_with(
                path,
                Route {
                    status: 302,
                    headers: vec![("Location".into(), to.into())],
                    body: Vec::new(),
                    etag: None,
                },
            )
        }

        pub fn url(&self, path: &str) -> String {
            format!("{}{path}", self.base)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            // Unblock accept().
            let _ = TcpStream::connect(self.base.trim_start_matches("http://"));
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    fn serve(conn: TcpStream, routes: &Mutex<HashMap<String, Route>>, hits: &Mutex<Vec<String>>) {
        let mut conn = conn;
        let mut reader = BufReader::new(conn.try_clone().expect("clone"));
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            return;
        }
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        let mut none_match = None;
        let mut content_length = 0usize;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                match k.trim().to_ascii_lowercase().as_str() {
                    "if-none-match" => none_match = Some(v.trim().to_string()),
                    "content-length" => content_length = v.trim().parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
        if content_length > 0 {
            let mut sink = vec![0; content_length];
            let _ = reader.read_exact(&mut sink);
        }
        hits.lock().expect("lock").push(path.clone());
        let route = routes.lock().expect("lock").get(&path).cloned();
        let (status, headers, body) = match route {
            None => (404, Vec::new(), b"not found".to_vec()),
            Some(r) if r.etag.is_some() && r.etag == none_match => (304, Vec::new(), Vec::new()),
            Some(r) => {
                let mut headers = r.headers;
                if let Some(e) = r.etag {
                    headers.push(("ETag".into(), e));
                }
                (r.status, headers, r.body)
            }
        };
        let mut out = format!("HTTP/1.1 {status} X\r\nConnection: close\r\n");
        for (k, v) in headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        let _ = conn.write_all(out.as_bytes());
        let _ = conn.write_all(&body);
        let _ = conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{Route, Server};
    use super::*;

    fn client(s: &Server) -> Client {
        Client::new(&s.base, &format!("{}/download/", s.base), false, "test")
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("openstitch-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir.join(name)
    }

    #[test]
    fn lists_releases_and_honours_etags() {
        let s = Server::start();
        s.route_with(
            "/repos/o/r/releases?per_page=20",
            Route {
                status: 200,
                headers: Vec::new(),
                body: crate::github::SAMPLE.into(),
                etag: Some("\"abc\"".into()),
            },
        );
        let c = client(&s);
        let Fetched::Releases { list, etag } = c.releases("o/r", None).expect("list") else {
            panic!("expected a list");
        };
        assert_eq!(list.len(), 3);
        assert_eq!(etag.as_deref(), Some("\"abc\""));
        assert!(matches!(
            c.releases("o/r", Some("\"abc\"")).expect("304"),
            Fetched::NotModified
        ));
        assert!(matches!(
            c.releases("o/r", Some("\"old\"")).expect("200"),
            Fetched::Releases { .. }
        ));
        assert!(matches!(
            c.releases("o/missing", None),
            Err(DownloadError::Status(404))
        ));
        s.route("/repos/o/bad/releases?per_page=20", 200, "{}");
        assert!(matches!(
            c.releases("o/bad", None),
            Err(DownloadError::Format(_))
        ));
        s.route("/repos/o/down/releases?per_page=20", 503, "");
        assert!(matches!(
            c.releases("o/down", None),
            Err(DownloadError::Status(503))
        ));
    }

    #[test]
    fn downloads_verify_and_follow_redirects() {
        let s = Server::start();
        let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let sum = Sha256::of(&payload);
        s.route("/download/v1/asset.zip", 200, payload.clone());
        s.redirect("/download/v1/moved.zip", "/download/v1/asset.zip");
        let c = client(&s);
        let dest = temp("asset.zip");
        let _ = std::fs::remove_file(&dest);
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let got = c
            .download(
                &s.url("/download/v1/asset.zip"),
                &dest,
                Some(sum),
                MAX_ASSET_BYTES,
                &cancel,
                &mut |done, total| seen.push((done, total)),
            )
            .expect("download");
        assert_eq!(got, sum);
        assert_eq!(std::fs::read(&dest).expect("file"), payload);
        assert_eq!(seen.first(), Some(&(0, Some(payload.len() as u64))));
        assert_eq!(
            seen.last(),
            Some(&(payload.len() as u64, Some(payload.len() as u64)))
        );
        assert!(!part_path(&dest).exists());
        let via = temp("moved.zip");
        c.download(
            &s.url("/download/v1/moved.zip"),
            &via,
            Some(sum),
            MAX_ASSET_BYTES,
            &cancel,
            &mut |_, _| {},
        )
        .expect("redirect followed");
        assert_eq!(std::fs::read(&via).expect("file"), payload);
        let _ = std::fs::remove_file(&dest);
        let _ = std::fs::remove_file(&via);
    }

    #[test]
    fn download_failures_leave_no_file() {
        let s = Server::start();
        let payload = b"hello".to_vec();
        s.route("/download/v1/a.zip", 200, payload.clone());
        s.route("/other/a.zip", 200, payload.clone());
        s.redirect("/download/v1/escape.zip", &s.url("/other/a.zip"));
        let c = client(&s);
        let cancel = AtomicBool::new(false);
        let dest = temp("fail.zip");
        let _ = std::fs::remove_file(&dest);
        let mut noop = |_: u64, _: Option<u64>| {};
        let wrong = Sha256::of(b"something else");
        let e = c
            .download(
                &s.url("/download/v1/a.zip"),
                &dest,
                Some(wrong),
                1 << 20,
                &cancel,
                &mut noop,
            )
            .expect_err("mismatch");
        assert!(matches!(e, DownloadError::Checksum { .. }), "{e}");
        assert!(!dest.exists() && !part_path(&dest).exists());
        let e = c
            .download(
                &s.url("/download/v1/a.zip"),
                &dest,
                None,
                3,
                &cancel,
                &mut noop,
            )
            .expect_err("too large");
        assert!(matches!(e, DownloadError::TooLarge), "{e}");
        assert!(!dest.exists());
        let e = c
            .download(
                &s.url("/download/v1/none.zip"),
                &dest,
                None,
                1 << 20,
                &cancel,
                &mut noop,
            )
            .expect_err("404");
        assert!(matches!(e, DownloadError::Status(404)), "{e}");
        let e = c
            .download(
                &s.url("/other/a.zip"),
                &dest,
                None,
                1 << 20,
                &cancel,
                &mut noop,
            )
            .expect_err("outside the prefix");
        assert!(matches!(e, DownloadError::Url(_)), "{e}");
        let e = c
            .download(
                &s.url("/download/v1/escape.zip"),
                &dest,
                None,
                1 << 20,
                &cancel,
                &mut noop,
            )
            .expect_err("redirect outside the prefix");
        assert!(matches!(e, DownloadError::Url(_)), "{e}");
        assert!(!dest.exists());
        cancel.store(true, Ordering::Relaxed);
        let e = c
            .download(
                &s.url("/download/v1/a.zip"),
                &dest,
                None,
                1 << 20,
                &cancel,
                &mut noop,
            )
            .expect_err("cancelled");
        assert!(matches!(e, DownloadError::Cancelled), "{e}");
        assert!(!dest.exists() && !part_path(&dest).exists());
        assert!(
            s.hits
                .lock()
                .expect("lock")
                .iter()
                .all(|p| p != "/other/a.zip"),
            "the server never saw a request outside the prefix"
        );
    }

    #[test]
    fn text_assets_and_https_only() {
        let s = Server::start();
        s.route("/download/v1/SHA256SUMS", 200, "abc  x\n");
        let c = client(&s);
        assert_eq!(
            c.text(&s.url("/download/v1/SHA256SUMS")).expect("text"),
            "abc  x\n"
        );
        assert!(matches!(c.text(&s.url("/x")), Err(DownloadError::Url(_))));
        let strict = Client::new(&s.base, &format!("{}/download/", s.base), true, "test");
        let e = strict
            .text(&s.url("/download/v1/SHA256SUMS"))
            .expect_err("plain http refused");
        assert!(matches!(e, DownloadError::Http(_)), "{e}");
        let real = Client::github("v0.1.0-alpha.3");
        assert!(matches!(
            real.text("http://github.com/cristianonescu/OpenStitch/releases/download/v1/x"),
            Err(DownloadError::Url(_))
        ));
        assert!(matches!(
            real.text("https://example.com/x"),
            Err(DownloadError::Url(_))
        ));
    }
}
