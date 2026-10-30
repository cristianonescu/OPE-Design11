//! Tajima DST: 512-byte text header followed by 3-byte balanced-ternary move records.
//! Units are 0.1 mm; the file's y axis points up, so y is negated on read and write.
//! The writer lays the records out as PE-DESIGN 11.50's DST export does (see [`write_with`]).

use crate::io::{ByteReader, ByteWriter, padded};
use crate::layout::{self, Event};
use crate::{FormatError, Result};
use openstitch_core::{Command, Stitch, StitchPattern, Thread};

pub const HEADER_SIZE: usize = 512;
pub const MAX_DELTA: f64 = 121.0;
/// Default number of jump records that encode a trim: PE-DESIGN 11.50's export opens with
/// three zero-length jumps and pads every frame move to at least three jump records
/// (`compat/reference/machine/machine-formats.dst`). Its "Number of jumps for trim" setting
/// is read with the same count.
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
    /// The fewest jump records a frame move is written with (the machine's "jumps for trim"
    /// count): shorter moves are padded with zero-length jumps, and the file opens with this
    /// many zero-length jumps.
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

/// Writes the stream as PE-DESIGN 11.50 exports it (`compat/reference/machine/machine-formats.dst`):
/// the design centred on (0, 0); `trim_jumps` zero-length jumps first; every frame move as
/// jumps of at most 12.1 mm (the longer axis full, the other proportional), padded with
/// zero-length jumps inserted before its last record so that a run of consecutive jump
/// records is never shorter than `trim_jumps` (the machine trims on such a run, and PE-DESIGN
/// trims every interior move); a stitch longer than 12.1 mm as jumps plus one stitch for the
/// remainder (one needle penetration); a colour change as `C3` with no move; and, after the
/// last stitch, a jump run back to (0, 0) before the end record, padded to `trim_jumps` only
/// when the way back is a single record (`machine-formats.dst`: two zero jumps, then 5 mm);
/// a longer way back is written as its pieces alone (`sample-basic.DST`: 2 records,
/// `hoop-oversize-100.dst`: 3, `long-jump-300.dst`: 12; #135). `ST` counts every record, the
/// end record included.
pub fn write_with(input: &StitchPattern, options: WriteOptions) -> Result<Vec<u8>> {
    let events = layout::events(input);
    let n = options.trim_jumps.max(1);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0_i64, 0_i64, 0_i64, 0_i64);
    for e in &events {
        if let Event::Move { x, y, .. } | Event::Stitch { x, y } = e {
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
    }

    let max = MAX_DELTA as i64;
    let mut body = ByteWriter::default();
    let mut at = (0_i64, 0_i64);
    // Consecutive jump records written so far, zero-length ones included.
    let mut run = n;
    let mut changes = 0_usize;
    for _ in 0..n {
        body.append(&encode(0, 0, true)?);
    }
    // A frame move: jumps for the pieces, zero-length jumps before the last one so that the
    // run of consecutive jump records reaches `n`.
    fn frame_move(
        body: &mut ByteWriter,
        at: &mut (i64, i64),
        run: &mut usize,
        n: usize,
        to: (i64, i64),
    ) -> Result<()> {
        let pieces = layout::split_proportional(to.0 - at.0, to.1 - at.1, MAX_DELTA as i64);
        let pad = n.saturating_sub(*run + pieces.len());
        let (last, head) = pieces.split_last().expect("at least one piece");
        for &(dx, dy) in head {
            body.append(&encode(dx, dy, true)?);
        }
        for _ in 0..pad {
            body.append(&encode(0, 0, true)?);
        }
        body.append(&encode(last.0, last.1, true)?);
        *run += pieces.len() + pad;
        *at = to;
        Ok(())
    }
    for e in &events {
        match *e {
            Event::Move { x: tx, y: ty, .. } => {
                frame_move(&mut body, &mut at, &mut run, n, (tx, ty))?;
            }
            Event::Stitch { x: tx, y: ty } => {
                let pieces = layout::split_proportional(tx - at.0, ty - at.1, max);
                let (last, head) = pieces.split_last().expect("at least one piece");
                for &(dx, dy) in head {
                    body.append(&encode(dx, dy, true)?);
                }
                body.append(&encode(last.0, last.1, false)?);
                run = 0;
                at = (tx, ty);
            }
            Event::ColorChange | Event::Stop => {
                body.append(&[0, 0, 0b1100_0011]);
                changes += 1;
                run = 0;
            }
            Event::End => {
                let pieces = layout::split_proportional(-at.0, -at.1, max).len();
                let pad_to = if pieces == 1 { n } else { 0 };
                frame_move(&mut body, &mut at, &mut run, pad_to, (0, 0))?;
                break;
            }
        }
    }
    body.append(&[0, 0, 0b1111_0011]);
    let records = body.len() / 3;
    if records > MAX_RECORDS || changes > MAX_COLOR_CHANGES {
        return Err(FormatError::ValueOutOfRange(format!(
            "DST's header holds at most {MAX_RECORDS} records and {MAX_COLOR_CHANGES} colour changes, got {records} and {changes}"
        )));
    }

    let mut w = ByteWriter::default();
    // PE-DESIGN labels a DST with the file name it saves to, extension included, cut to 16
    // characters; the exporter records the file stem as the PEC label.
    let label = match input.metadata.get(crate::pec::LABEL_KEY) {
        Some(stem) => format!("{stem}.DST"),
        None => input.name().unwrap_or("Untitled").to_owned(),
    };
    w.ascii(&format!("LA:{}\r", padded(&label, 16)));
    w.ascii(&format!("ST:{}\r", left_pad(records.to_string(), 7)));
    w.ascii(&format!("CO:{}\r", left_pad(changes.to_string(), 3)));
    // Extents with y up. AX/AY are filled as PE-DESIGN fills them: the start point measured
    // from the lower left corner of the extents (one reference file; Tajima's meaning of the
    // two fields is not documented).
    let (plus_x, minus_x, plus_y, minus_y) = (max_x, -min_x, -min_y, max_y);
    w.ascii(&format!("+X:{}\r", left_pad(plus_x.to_string(), 5)));
    w.ascii(&format!("-X:{}\r", left_pad(minus_x.to_string(), 5)));
    w.ascii(&format!("+Y:{}\r", left_pad(plus_y.to_string(), 5)));
    w.ascii(&format!("-Y:{}\r", left_pad(minus_y.to_string(), 5)));
    w.ascii(&format!("AX:+{}\r", left_pad(minus_x.to_string(), 5)));
    w.ascii(&format!("AY:+{}\r", left_pad(minus_y.to_string(), 5)));
    w.ascii("MX:+    0\r");
    w.ascii("MY:+    0\r");
    w.ascii("PD:******\r");
    if options.extended_header {
        if let Some(author) = input.metadata.get("author") {
            w.ascii(&format!("AU:{author}\r"));
        }
        if let Some(copyright) = input.metadata.get("copyright") {
            w.ascii(&format!("CP:{copyright}\r"));
        }
        for t in &input.threads {
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
    w.append(&body.bytes);
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
