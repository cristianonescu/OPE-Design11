use std::io::{ErrorKind, Read};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    /// A line longer than the limit; its bytes were dropped as they streamed in.
    Oversized,
}

/// Splits a byte stream into newline-delimited messages without ever buffering much more than
/// `max_bytes` of one line: an over-long line is dropped as it streams in and reported once.
pub struct LineReader<R> {
    reader: R,
    max_bytes: usize,
    buffer: Vec<u8>,
    discarding: bool,
    at_eof: bool,
}

const CHUNK: usize = 64 << 10;

impl<R: Read> LineReader<R> {
    pub fn new(reader: R, max_bytes: usize) -> Self {
        LineReader {
            reader,
            max_bytes,
            buffer: Vec::new(),
            discarding: false,
            at_eof: false,
        }
    }

    fn text(bytes: &[u8]) -> Line {
        let mut text = String::from_utf8_lossy(bytes).into_owned();
        if text.ends_with('\r') {
            text.pop();
        }
        Line::Text(text)
    }
}

impl<R: Read> Iterator for LineReader<R> {
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
        loop {
            if let Some(newline) = self.buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=newline).collect();
                if std::mem::take(&mut self.discarding) || newline > self.max_bytes {
                    return Some(Line::Oversized);
                }
                return Some(Self::text(&line[..newline]));
            }
            if self.buffer.len() > self.max_bytes {
                self.buffer.clear();
                self.discarding = true;
            }
            if self.at_eof {
                if std::mem::take(&mut self.discarding) {
                    return Some(Line::Oversized);
                }
                if self.buffer.is_empty() {
                    return None;
                }
                let rest = std::mem::take(&mut self.buffer);
                return Some(Self::text(&rest));
            }
            let mut chunk = [0u8; CHUNK];
            match self.reader.read(&mut chunk) {
                Ok(0) => self.at_eof = true,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => self.at_eof = true,
            }
        }
    }
}
