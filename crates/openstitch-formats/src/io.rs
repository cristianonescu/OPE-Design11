use crate::{FormatError, Result};

/// Bounds-checked little-endian cursor. Every read returns [`FormatError::Truncated`] instead of
/// panicking; the offset may be moved past the end, after which reads fail.
pub(crate) struct ByteReader<'a> {
    pub bytes: &'a [u8],
    pub offset: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    pub fn u8(&mut self) -> Result<u8> {
        let b = self
            .peek()
            .ok_or_else(|| FormatError::Truncated(format!("at byte {}", self.offset)))?;
        self.offset += 1;
        Ok(b)
    }

    pub fn read(&mut self, count: usize) -> Result<&'a [u8]> {
        let slice = self
            .offset
            .checked_add(count)
            .and_then(|end| self.bytes.get(self.offset..end))
            .ok_or_else(|| {
                FormatError::Truncated(format!("need {count} bytes at {}", self.offset))
            })?;
        self.offset += count;
        Ok(slice)
    }

    pub fn skip(&mut self, count: usize) -> Result<()> {
        self.read(count).map(|_| ())
    }

    pub fn u16le(&mut self) -> Result<u32> {
        Ok(u32::from(self.u8()?) | u32::from(self.u8()?) << 8)
    }

    pub fn u24le(&mut self) -> Result<u32> {
        Ok(self.u16le()? | u32::from(self.u8()?) << 16)
    }

    pub fn u32le(&mut self) -> Result<u32> {
        Ok(self.u24le()? | u32::from(self.u8()?) << 24)
    }

    pub fn f32le(&mut self) -> Result<f32> {
        let b = self.read(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Little-endian output buffer; integer writes keep the low bytes (two's complement).
#[derive(Default)]
pub(crate) struct ByteWriter {
    pub bytes: Vec<u8>,
}

impl ByteWriter {
    pub fn u8(&mut self, v: i64) {
        self.bytes.push(v as u8);
    }
    pub fn u16le(&mut self, v: i64) {
        self.u8(v);
        self.u8(v >> 8);
    }
    pub fn u24le(&mut self, v: i64) {
        self.u16le(v);
        self.u8(v >> 16);
    }
    pub fn u32le(&mut self, v: i64) {
        self.u24le(v);
        self.u8(v >> 24);
    }
    pub fn f32le(&mut self, v: f64) {
        self.bytes.extend_from_slice(&(v as f32).to_le_bytes());
    }
    pub fn patch16le(&mut self, at: usize, v: i64) {
        self.bytes[at..at + 2].copy_from_slice(&[v as u8, (v >> 8) as u8]);
    }
    pub fn patch32le(&mut self, at: usize, v: i64) {
        self.bytes[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
    }
    pub fn append(&mut self, b: &[u8]) {
        self.bytes.extend_from_slice(b);
    }
    pub fn fill(&mut self, byte: u8, count: usize) {
        self.bytes.resize(self.bytes.len() + count, byte);
    }
    pub fn ascii(&mut self, s: &str) {
        self.bytes.extend_from_slice(s.as_bytes());
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn patch24le(&mut self, at: usize, v: i64) {
        self.bytes[at..at + 3].copy_from_slice(&[v as u8, (v >> 8) as u8, (v >> 16) as u8]);
    }
}

/// `s` cut or space-padded to `width` characters.
pub(crate) fn padded(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let n = out.chars().count();
    out.extend(std::iter::repeat_n(' ', width - n));
    out
}

pub(crate) fn signed8(b: u8) -> f64 {
    f64::from(b as i8)
}

/// Trims horizontal whitespace (spaces, tabs), not line breaks.
pub(crate) fn trim_horizontal(s: &str) -> &str {
    s.trim_matches(|c: char| {
        c.is_whitespace()
            && !matches!(
                c,
                '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
            )
    })
}
