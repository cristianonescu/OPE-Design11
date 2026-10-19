//! Tajima DST: 512-byte text header followed by 3-byte balanced-ternary move records.
//! Units are 0.1 mm; the file's y axis points up, so y is negated on read and write.

use crate::io::{ByteReader, ByteWriter, padded};
use crate::{FormatError, Result};
use openstitch_core::{Command, Stitch, StitchPattern, Thread, round_coordinate};

pub const HEADER_SIZE: usize = 512;
pub const MAX_DELTA: f64 = 121.0;
/// Default number of jump records that encode a trim. PE-DESIGN exposes this as the
/// "Number of jumps for trim" DST setting; the default it ships with is not yet observed.
pub const DEFAULT_TRIM_JUMPS: usize = 3;

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    read_with(data, DEFAULT_TRIM_JUMPS)
}

/// Reads, turning every run of `trim_jumps` jumps into a trim.
pub fn read_with(data: &[u8], trim_jumps: usize) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    let header = reader.read(HEADER_SIZE.min(data.len()))?;
    parse_header(header, &mut pattern);

    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    while reader.remaining() >= 3 {
        let r = reader.read(3)?;
        let (dx, dy) = decode(r[0], r[1], r[2]);
        x += f64::from(dx);
        y += f64::from(dy);
        let command = if r[2] & 0b1111_0011 == 0b1111_0011 {
            break;
        } else if r[2] & 0b1100_0011 == 0b1100_0011 {
            Command::ColorChange
        } else if r[2] & 0b1000_0011 == 0b1000_0011 {
            Command::Jump
        } else {
            Command::Stitch
        };
        pattern.stitches.push(Stitch::new(x, y, command));
    }
    pattern.push(Command::End);
    pattern.infer_trims(Some(trim_jumps), None, true);
    Ok(pattern)
}

/// Decodes one record to (dx, dy) with +y down.
pub fn decode(b0: u8, b1: u8, b2: u8) -> (i32, i32) {
    let bit = |b: u8, n: u32| i32::from((b >> n) & 1);
    let x = 81 * (bit(b2, 2) - bit(b2, 3))
        + 27 * (bit(b1, 2) - bit(b1, 3))
        + 9 * (bit(b0, 2) - bit(b0, 3))
        + 3 * (bit(b1, 0) - bit(b1, 1))
        + (bit(b0, 0) - bit(b0, 1));
    let y = 81 * (bit(b2, 5) - bit(b2, 4))
        + 27 * (bit(b1, 5) - bit(b1, 4))
        + 9 * (bit(b0, 5) - bit(b0, 4))
        + 3 * (bit(b1, 7) - bit(b1, 6))
        + (bit(b0, 7) - bit(b0, 6));
    (x, -y)
}

/// `count` zero-sum diagonal jumps: (+2), (−4, +4, …), (±2). A single jump is zero-length.
pub fn trim_jump_sequence(count: usize) -> Vec<(i64, i64)> {
    if count < 2 {
        return vec![(0, 0)];
    }
    let mut out = vec![(2, 2)];
    let mut delta = -4_i64;
    for _ in 1..count - 1 {
        out.push((delta, delta));
        delta = -delta;
    }
    out.push((delta / 2, delta / 2));
    out
}

fn parse_header(header: &[u8], pattern: &mut StitchPattern) {
    let mut start = 0;
    for (i, &b) in header.iter().enumerate() {
        if b != 0x0D && b != 0x0A {
            continue;
        }
        if let Ok(line) = std::str::from_utf8(&header[start..i]) {
            let line = line.trim();
            if line.chars().count() > 3 {
                let key: String = line.chars().take(2).collect();
                let key = key.trim();
                let value: String = line.chars().skip(3).collect();
                let value = value.trim();
                match key {
                    "LA" => pattern.set_name(value),
                    "AU" => {
                        pattern.metadata.insert("author".into(), value.into());
                    }
                    "CP" => {
                        pattern.metadata.insert("copyright".into(), value.into());
                    }
                    "TC" => {
                        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
                        let hex = parts[0];
                        if let Ok(rgb) =
                            u32::from_str_radix(hex.strip_prefix('#').unwrap_or(hex), 16)
                        {
                            let mut t = Thread::rgb(rgb);
                            t.name = parts.get(1).map(|s| (*s).to_owned().into());
                            t.catalog_number = parts.get(2).map(|s| (*s).to_owned().into());
                            pattern.threads.push(t);
                        }
                    }
                    _ => {
                        pattern.metadata.insert(format!("dst.{key}"), value.into());
                    }
                }
            }
        }
        start = i + 1;
    }
}

// MARK: Writing

/// DST writer options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    /// Adds `AU`, `CP` and one `TC` line per thread to the header.
    pub extended_header: bool,
    /// Number of jump records written per trim.
    pub trim_jumps: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            extended_header: false,
            trim_jumps: DEFAULT_TRIM_JUMPS,
        }
    }
}

pub fn write(pattern: &StitchPattern) -> Result<Vec<u8>> {
    write_with(pattern, WriteOptions::default())
}

/// Largest record count the 7-digit `ST:` header field holds.
pub const MAX_RECORDS: usize = 9_999_999;
/// Largest colour-change count the 3-digit `CO:` header field holds.
pub const MAX_COLOR_CHANGES: usize = 999;

fn left_pad(s: String, width: usize) -> String {
    format!("{s:>width$}")
}

pub fn write_with(input: &StitchPattern, options: WriteOptions) -> Result<Vec<u8>> {
    let pattern = input.interpolating_long_moves(MAX_DELTA, MAX_DELTA);
    let bounds = pattern.bounds();
    let mut w = ByteWriter::default();
    w.ascii(&format!(
        "LA:{}\r",
        padded(pattern.name().unwrap_or("Untitled"), 16)
    ));
    // ST counts every move record written: stitches, jumps, and the jumps that encode a trim.
    let records = pattern.count(Command::Stitch)
        + pattern.count(Command::Jump)
        + options.trim_jumps.max(1) * pattern.count(Command::Trim);
    let changes = pattern.count(Command::ColorChange);
    if records > MAX_RECORDS || changes > MAX_COLOR_CHANGES {
        return Err(FormatError::ValueOutOfRange(format!(
            "DST's header holds at most {MAX_RECORDS} records and {MAX_COLOR_CHANGES} colour changes, got {records} and {changes}"
        )));
    }
    w.ascii(&format!("ST:{}\r", left_pad(records.to_string(), 7)));
    w.ascii(&format!(
        "CO:{}\r",
        left_pad(pattern.count(Command::ColorChange).to_string(), 3)
    ));
    w.ascii(&format!(
        "+X:{}\r",
        left_pad(round_coordinate(bounds.max_x).unsigned_abs().to_string(), 5)
    ));
    w.ascii(&format!(
        "-X:{}\r",
        left_pad(round_coordinate(bounds.min_x).unsigned_abs().to_string(), 5)
    ));
    w.ascii(&format!(
        "+Y:{}\r",
        left_pad(round_coordinate(bounds.max_y).unsigned_abs().to_string(), 5)
    ));
    w.ascii(&format!(
        "-Y:{}\r",
        left_pad(round_coordinate(bounds.min_y).unsigned_abs().to_string(), 5)
    ));
    let last = pattern.stitches.last();
    let ax = last.map_or(0, |s| s.x as i64);
    let ay = -last.map_or(0, |s| s.y as i64);
    let sign = |v: i64| if v >= 0 { '+' } else { '-' };
    w.ascii(&format!(
        "AX:{}{}\r",
        sign(ax),
        left_pad(ax.unsigned_abs().to_string(), 5)
    ));
    w.ascii(&format!(
        "AY:{}{}\r",
        sign(ay),
        left_pad(ay.unsigned_abs().to_string(), 5)
    ));
    w.ascii("MX:+    0\r");
    w.ascii("MY:+    0\r");
    w.ascii("PD:******\r");
    if options.extended_header {
        if let Some(author) = pattern.metadata.get("author") {
            w.ascii(&format!("AU:{author}\r"));
        }
        if let Some(copyright) = pattern.metadata.get("copyright") {
            w.ascii(&format!("CP:{copyright}\r"));
        }
        for t in &pattern.threads {
            w.ascii(&format!(
                "TC:{},{},{}\r",
                t.hex(),
                t.name.as_deref().unwrap_or(""),
                t.catalog_number.as_deref().unwrap_or("")
            ));
        }
    }
    w.u8(0x1A);
    if w.len() > HEADER_SIZE {
        return Err(FormatError::ValueOutOfRange(
            "DST header exceeds 512 bytes".into(),
        ));
    }
    w.fill(0x20, HEADER_SIZE - w.len());

    let (mut xx, mut yy) = (0_i64, 0_i64);
    for s in &pattern.stitches {
        let dx = round_coordinate(s.x) - xx;
        let dy = round_coordinate(s.y) - yy;
        match s.command {
            Command::Stitch | Command::Jump => {
                xx += dx;
                yy += dy;
                w.append(&encode(dx, dy, s.command == Command::Jump)?);
            }
            Command::Trim => {
                for (d, _) in trim_jump_sequence(options.trim_jumps) {
                    w.append(&encode(d, d, true)?);
                }
            }
            Command::ColorChange | Command::Stop => w.append(&[0, 0, 0b1100_0011]),
            Command::End => {}
        }
    }
    w.append(&[0, 0, 0b1111_0011]);
    Ok(w.bytes)
}

/// Encodes one move (+y down) as a 3-byte record.
pub fn encode(dx: i64, dy_down: i64, jump: bool) -> Result<[u8; 3]> {
    let mut b = [0_u8, 0, 0b0000_0011 | if jump { 0b1000_0000 } else { 0 }];
    // (power of three, record byte, bit for +, bit for -)
    const X_SLOTS: [(i64, usize, u32, u32); 5] = [
        (81, 2, 2, 3),
        (27, 1, 2, 3),
        (9, 0, 2, 3),
        (3, 1, 0, 1),
        (1, 0, 0, 1),
    ];
    const Y_SLOTS: [(i64, usize, u32, u32); 5] = [
        (81, 2, 5, 4),
        (27, 1, 5, 4),
        (9, 0, 5, 4),
        (3, 1, 7, 6),
        (1, 0, 7, 6),
    ];
    let mut fill = |value: i64, slots: &[(i64, usize, u32, u32)], axis: &str| -> Result<()> {
        let mut v = value;
        for &(power, byte, plus, minus) in slots {
            if v > power / 2 {
                b[byte] |= 1 << plus;
                v -= power;
            } else if v < -(power / 2) {
                b[byte] |= 1 << minus;
                v += power;
            }
        }
        if v != 0 {
            return Err(FormatError::ValueOutOfRange(format!(
                "DST {axis} delta {value} exceeds ±121"
            )));
        }
        Ok(())
    };
    fill(dx, &X_SLOTS, "x")?;
    fill(-dy_down, &Y_SLOTS, "y")?;
    Ok(b)
}
