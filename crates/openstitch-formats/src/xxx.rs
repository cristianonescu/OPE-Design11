//! Singer XXX: 0xFC-byte little-endian header (record count, colour count, extents, end
//! position), the offset of the end of the stitch data at 0xFC, then records from 0x100:
//! 2-byte signed (dx, dy) stitches with y up, `7D dx(16le) dy(16le)` long move,
//! `7F 01 dx dy` jump, `7F 03 dx dy` trim (plus move), `7F 08 dx dy` colour change,
//! `7F 7F 02 14` end. After the end marker: `00 00`, then `00 R G B` per colour, zero entries
//! up to 21 colours, `00 FF FF FF`, `00 01`.

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::{FormatError, Result};
use openstitch_core::{Command, Stitch, StitchPattern, Thread, round_coordinate};

/// Deltas of ±124 or more are written in the long form; moves are split at 124 first.
pub const MAX_DELTA: f64 = 124.0;
const STITCH_OFFSET: usize = 0x100;

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    reader.skip(0x27)?;
    let color_count = reader.u16le()?;
    if data.len() < STITCH_OFFSET {
        return Err(FormatError::Truncated(
            "XXX header shorter than 0x100 bytes".into(),
        ));
    }
    reader.offset = STITCH_OFFSET;

    let signed16 = |v: u32| f64::from(v as u16 as i16);
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let mut ended = false;
    while reader.remaining() > 0 {
        let b1 = reader.u8()?;
        if b1 == 0x7D || b1 == 0x7E {
            if reader.remaining() < 4 {
                break;
            }
            x += signed16(reader.u16le()?);
            y -= signed16(reader.u16le()?);
            pattern.stitches.push(Stitch::new(x, y, Command::Jump));
            continue;
        }
        if reader.remaining() < 1 {
            break;
        }
        let b2 = reader.u8()?;
        if b1 != 0x7F {
            x += signed8(b1);
            y -= signed8(b2);
            pattern.stitches.push(Stitch::new(x, y, Command::Stitch));
            continue;
        }
        if reader.remaining() < 2 {
            break;
        }
        let (dx, dy) = (signed8(reader.u8()?), -signed8(reader.u8()?));
        match b2 {
            0x01 => {
                x += dx;
                y += dy;
                pattern.stitches.push(Stitch::new(x, y, Command::Jump));
            }
            0x03 => {
                pattern.stitches.push(Stitch::new(x, y, Command::Trim));
                if dx != 0.0 || dy != 0.0 {
                    x += dx;
                    y += dy;
                    pattern.stitches.push(Stitch::new(x, y, Command::Jump));
                }
            }
            0x08 | 0x0A..=0x17 => pattern
                .stitches
                .push(Stitch::new(x, y, Command::ColorChange)),
            0x7F | 0x18 => {
                ended = true;
                break;
            }
            _ => {}
        }
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));

    if ended && reader.remaining() >= 2 {
        reader.skip(2)?;
        for _ in 0..color_count {
            if reader.remaining() < 4 {
                break;
            }
            let c = reader.read(4)?;
            pattern.threads.push(Thread::rgb(
                u32::from(c[1]) << 16 | u32::from(c[2]) << 8 | u32::from(c[3]),
            ));
        }
    }
    Ok(pattern)
}

// MARK: Writing

pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let mut pattern = input.interpolating_long_moves(MAX_DELTA, MAX_DELTA);
    pattern.fix_thread_count(None);

    // Records first: the header needs their count, extents and end position.
    let mut body = ByteWriter::default();
    let mut records = 0_i64;
    let (mut xx, mut yy) = (0_i64, 0_i64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    let mut visit = |xx: i64, yy: i64| {
        min_x = min_x.min(xx);
        max_x = max_x.max(xx);
        min_y = min_y.min(yy);
        max_y = max_y.max(yy);
    };
    for s in &pattern.stitches {
        let dx = round_coordinate(s.x) - xx;
        let dy = round_coordinate(s.y) - yy;
        match s.command {
            Command::Stitch => {
                xx += dx;
                yy += dy;
                if dx.abs() < 124 && dy.abs() < 124 {
                    body.u8(dx);
                    body.u8(-dy);
                } else {
                    body.u8(0x7D);
                    body.u16le(dx);
                    body.u16le(-dy);
                }
            }
            Command::Jump => {
                xx += dx;
                yy += dy;
                body.append(&[0x7F, 0x01]);
                body.u8(dx);
                body.u8(-dy);
            }
            Command::Trim => body.append(&[0x7F, 0x03, 0x00, 0x00]), // cut in place
            Command::ColorChange | Command::Stop => {
                xx += dx;
                yy += dy;
                body.append(&[0x7F, 0x08]);
                body.u8(dx);
                body.u8(-dy);
            }
            Command::End => break,
        }
        records += 1;
        visit(xx, yy);
    }
    visit(xx, yy); // end position
    if records == 0 {
        (min_x, min_y, max_x, max_y) = (0, 0, 0, 0);
    }

    let mut w = ByteWriter::default();
    w.fill(0, 0x17);
    w.u32le(records);
    w.fill(0, 0x0C);
    w.u32le(pattern.threads.len() as i64);
    w.u16le(0);
    w.u16le(max_x - min_x);
    w.u16le(max_y - min_y);
    w.u16le(xx);
    w.u16le(-yy);
    w.u16le(-min_x);
    w.u16le(max_y);
    w.fill(0, 0x42 + 4 + 0x73);
    w.u16le(0x20);
    w.fill(0, 8);
    w.u32le((STITCH_OFFSET + body.len()) as i64);
    w.append(&body.bytes);
    w.append(&[0x7F, 0x7F, 0x02, 0x14, 0x00, 0x00]);
    for t in &pattern.threads {
        w.append(&[0, t.red, t.green, t.blue]);
    }
    for _ in pattern.threads.len()..21 {
        w.u32le(0);
    }
    w.append(&[0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x01]);
    Ok(w.bytes)
}
