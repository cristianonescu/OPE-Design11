//! Singer XXX: 0xFC-byte little-endian header (record count, colour count, extents, end
//! position), the offset of the end of the stitch data at 0xFC, then records from 0x100:
//! 2-byte signed (dx, dy) stitches with y up, `7D dx(16le) dy(16le)` long stitch,
//! `7E dx(16le) dy(16le)` trimmed frame move, `7F 01 dx dy` jump, `7F 03 dx dy` trim (plus
//! move), `7F 08 dx dy` colour change, `7F 7F 03 14` end. After the end marker: `00 00`, then
//! `00 R G B` per colour, zero entries up to 21 colours, `00 FF FF FF`, `00 01`.
//! The writer lays the records out as PE-DESIGN 11.50's export does (see [`write`]).

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::layout::{self, Event};
use crate::{FormatError, Result};
use openstitch_core::{Command, Stitch, StitchPattern, Thread};

/// Largest delta of a short record; jumps and split stitches are clamped to ±120 as
/// PE-DESIGN clamps them.
pub const MAX_DELTA: f64 = 120.0;
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
            // 7E is PE-DESIGN's trimmed frame move; 7D a long stitch, read as a move like
            // the reference reader does.
            if b1 == 0x7E {
                pattern.stitches.push(Stitch::new(x, y, Command::Trim));
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

/// Writes the stream as PE-DESIGN 11.50 exports it (`compat/reference/machine/machine-formats.xxx`):
/// the design centred on (0, 0); `7F 01 00 00` first; every frame move folded into the stitch
/// after it and followed by a zero-length stitch there (the first needle point): a trimmed
/// move (the first one and every move after a trim) as one `7E` record with 16-bit deltas, a
/// plain move as `7F 01` jumps with each axis clamped to ±120; a stitch over 12 mm as `7F 01`
/// jumps plus one stitch for the remainder; a colour change as `7F 08 00 00` written after the
/// move to the next block's first needle point, before its zero-length stitch; and after the
/// last stitch a `7F 01` jump run back to (0, 0), then `7F 7F 03 14`. The header's end
/// position is the last stitch; the two words after it and the word at 0xF6 are zero, as
/// PE-DESIGN writes them. PE-DESIGN fills the unused colour slots with a default palette and
/// appends a second table after the colour list; OpenStitch writes zeros there.
pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let events = layout::events(&pattern);
    let max = MAX_DELTA as i64;

    let mut body = ByteWriter::default();
    let mut records = 0_i64;
    let (mut x, mut y) = (0_i64, 0_i64);
    let (mut last_stitch_x, mut last_stitch_y) = (0_i64, 0_i64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0_i64, 0_i64, 0_i64, 0_i64);
    let mut visit = |x: i64, y: i64| {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    };
    fn jump(body: &mut ByteWriter, records: &mut i64, dx: i64, dy: i64) {
        body.append(&[0x7F, 0x01]);
        body.u8(dx);
        body.u8(-dy);
        *records += 1;
    }
    jump(&mut body, &mut records, 0, 0);
    let mut color_pending = false;
    let mut i = 0;
    while i < events.len() {
        match events[i] {
            Event::Move {
                x: tx,
                y: ty,
                trimmed,
            } => {
                let target = layout::next_stitch(&events, i).unwrap_or((tx, ty));
                if trimmed {
                    body.u8(0x7E);
                    body.u16le(target.0 - x);
                    body.u16le(-(target.1 - y));
                    records += 1;
                } else {
                    for (dx, dy) in layout::split_clamped(target.0 - x, target.1 - y, max) {
                        jump(&mut body, &mut records, dx, dy);
                    }
                }
                (x, y) = target;
                visit(x, y);
                if color_pending {
                    body.append(&[0x7F, 0x08, 0x00, 0x00]);
                    color_pending = false;
                }
                body.append(&[0, 0]);
                records += 1;
                (last_stitch_x, last_stitch_y) = (x, y);
                if layout::next_stitch(&events, i).is_some() {
                    i += 1;
                }
            }
            Event::Stitch { x: tx, y: ty } => {
                if color_pending {
                    body.append(&[0x7F, 0x08, 0x00, 0x00]);
                    color_pending = false;
                }
                let pieces = layout::split_clamped(tx - x, ty - y, max);
                let (last, head) = pieces.split_last().expect("at least one piece");
                for &(dx, dy) in head {
                    jump(&mut body, &mut records, dx, dy);
                }
                body.u8(last.0);
                body.u8(-last.1);
                records += 1;
                x = tx;
                y = ty;
                visit(x, y);
                (last_stitch_x, last_stitch_y) = (x, y);
            }
            Event::ColorChange | Event::Stop => color_pending = true,
            Event::End => {
                if color_pending {
                    body.append(&[0x7F, 0x08, 0x00, 0x00]);
                }
                for (dx, dy) in layout::split_clamped(-x, -y, max) {
                    jump(&mut body, &mut records, dx, dy);
                }
                break;
            }
        }
        i += 1;
    }

    let mut w = ByteWriter::default();
    w.fill(0, 0x17);
    w.u32le(records);
    w.fill(0, 0x0C);
    w.u32le(pattern.threads.len() as i64);
    w.u16le(0);
    w.u16le(max_x - min_x);
    w.u16le(max_y - min_y);
    w.u16le(last_stitch_x);
    w.u16le(-last_stitch_y);
    w.u16le(0);
    w.u16le(0);
    w.fill(0, 0x42 + 4 + 0x73);
    w.u16le(0);
    w.fill(0, 8);
    w.u32le((STITCH_OFFSET + body.len()) as i64);
    w.append(&body.bytes);
    w.append(&[0x7F, 0x7F, 0x03, 0x14, 0x00, 0x00]);
    for t in &pattern.threads {
        w.append(&[0, t.red, t.green, t.blue]);
    }
    for _ in pattern.threads.len()..21 {
        w.u32le(0);
    }
    w.append(&[0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x01]);
    Ok(w.bytes)
}
