//! Pfaff PCS: version byte, hoop byte (2 = 80×80 mm, 3 = large), little-endian colour count,
//! one `R G B 00` entry per colour, a 16-bit record count, then 9-byte absolute records
//! `00 x(24le) 00 y(24le) ctrl` with y up, in 1/6 mm (0.1 mm × 5/3). `ctrl` 0x00 is a stitch,
//! bit 0x01 a colour change, bit 0x04 a jump; anything else ends the design. No trim or end
//! record exists. pyembroidery 1.5.1 only reads PCS, so the writer is the exact inverse of
//! that reader, with libembroidery's version byte (`'2'`) for the field the reader skips.

use crate::io::{ByteReader, ByteWriter};
use crate::{FormatError, Result, repeating_stops_palette};
use openstitch_core::{Command, Stitch, StitchPattern, Thread, round_coordinate};

/// 0.1 mm per file unit is 5/3; written as one constant so reads match the reference exactly.
pub const UNIT_SCALE: f64 = 5.0 / 3.0;
const VERSION: u8 = 0x32;

/// Header hoop byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hoop {
    Small = 2,
    Large = 3,
}

impl Hoop {
    /// 80×80 mm when the design fits, else the large hoop.
    pub fn fitting(width: f64, height: f64) -> Self {
        if width <= 800.0 && height <= 800.0 {
            Self::Small
        } else {
            Self::Large
        }
    }
}

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    reader.skip(2)?; // version, hoop
    let color_count = reader.u16le()?;
    for _ in 0..color_count {
        let c = reader.read(4)?;
        pattern.threads.push(Thread::rgb(
            u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]),
        ));
    }
    reader.skip(2)?; // record count; the reference reads to end of file instead

    let signed24 = |v: u32| {
        if v > 0x7F_FFFF {
            i64::from(v) - 0x100_0000
        } else {
            i64::from(v)
        }
    };
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    while reader.remaining() >= 9 {
        reader.skip(1)?;
        let fx = signed24(reader.u24le()?);
        reader.skip(1)?;
        let fy = signed24(reader.u24le()?);
        let ctrl = reader.u8()?;
        let (px, py) = (fx as f64 * UNIT_SCALE, (-fy) as f64 * UNIT_SCALE);
        if ctrl == 0x00 {
            (x, y) = (px, py);
            pattern.stitches.push(Stitch::new(x, y, Command::Stitch));
        } else if ctrl & 0x01 != 0 {
            pattern
                .stitches
                .push(Stitch::new(x, y, Command::ColorChange));
        } else if ctrl & 0x04 != 0 {
            (x, y) = (px, py);
            pattern.stitches.push(Stitch::new(x, y, Command::Jump));
        } else {
            break;
        }
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    Ok(pattern)
}

// MARK: Writing

/// Trims are dropped (PCS has no trim record); a stop is written as a colour change to the
/// same thread.
pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let palette = repeating_stops_palette(&pattern);
    let bounds = pattern.bounds();

    let mut w = ByteWriter::default();
    w.u8(i64::from(VERSION));
    w.u8(Hoop::fitting(bounds.width(), bounds.height()) as i64);
    w.u16le(palette.len() as i64);
    for t in &palette {
        w.append(&[t.red, t.green, t.blue, 0]);
    }
    let count_index = w.len();
    w.u16le(0);

    let mut records = 0_usize;
    let (mut fx, mut fy) = (0_i64, 0_i64);
    for s in &pattern.stitches {
        let ctrl = match s.command {
            Command::Stitch | Command::Jump => {
                fx = round_coordinate(s.x * 3.0 / 5.0);
                fy = round_coordinate(-s.y * 3.0 / 5.0);
                if s.command == Command::Stitch {
                    0x00
                } else {
                    0x04
                }
            }
            Command::ColorChange | Command::Stop => 0x01,
            Command::Trim => continue,
            Command::End => break,
        };
        w.u8(0);
        w.u24le(fx);
        w.u8(0);
        w.u24le(fy);
        w.u8(ctrl);
        records += 1;
    }
    if records > 0xFFFF {
        return Err(FormatError::ValueOutOfRange(format!(
            "PCS holds at most 65535 records, got {records}"
        )));
    }
    w.bytes[count_index] = (records & 0xFF) as u8;
    w.bytes[count_index + 1] = (records >> 8) as u8;
    Ok(w.bytes)
}
