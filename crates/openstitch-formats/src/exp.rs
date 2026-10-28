//! Melco EXP: headerless stream of 2-byte signed (dx, dy) stitch records, y up.
//! Control records start with 0x80: `80 02 dx dy` jump (PE-DESIGN 11.50; `80 04` is read as a
//! jump too), `80 80 07 00` trim, `80 01 00 00` colour change. No colour information is stored.
//! The writer lays the records out as PE-DESIGN's export does (see [`write`]).

use crate::Result;
use crate::io::{ByteReader, ByteWriter, signed8};
use crate::layout::{self, Event};
use openstitch_core::{Command, Stitch, StitchPattern};

pub const MAX_DELTA: f64 = 127.0;
/// Reader heuristic: a jump run moving further than one record holds (12.7 mm) on an axis is
/// a trimmed frame move; a single jump of up to a record is the long part of a split stitch
/// (PE-DESIGN writes no trim record and splits stitches over 12.7 mm into a jump and a stitch).
pub const DEFAULT_TRIM_DISTANCE: f64 = MAX_DELTA;

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    read_with(data, Some(DEFAULT_TRIM_DISTANCE))
}

/// Reads, inserting a trim before jump runs longer than `trim_distance` (`None`: no inference).
pub fn read_with(data: &[u8], trim_distance: Option<f64>) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    while reader.remaining() >= 2 {
        let (b0, b1) = (reader.u8()?, reader.u8()?);
        if b0 != 0x80 {
            x += signed8(b0);
            y -= signed8(b1);
            pattern.stitches.push(Stitch::new(x, y, Command::Stitch));
            continue;
        }
        if reader.remaining() < 2 {
            break;
        }
        let (p0, p1) = (reader.u8()?, reader.u8()?);
        let (dx, dy) = (signed8(p0), -signed8(p1));
        match b1 {
            0x80 => pattern.stitches.push(Stitch::new(x, y, Command::Trim)),
            0x02 | 0x04 => {
                x += dx;
                y += dy;
                pattern.stitches.push(Stitch::new(x, y, Command::Jump));
            }
            0x01 => {
                pattern
                    .stitches
                    .push(Stitch::new(x, y, Command::ColorChange));
                if dx != 0.0 || dy != 0.0 {
                    x += dx;
                    y += dy;
                    pattern.stitches.push(Stitch::new(x, y, Command::Jump));
                }
            }
            _ => break,
        }
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    pattern.infer_trims(None, trim_distance, true);
    Ok(pattern)
}

/// Writes the stream as PE-DESIGN 11.50 exports it (`compat/reference/machine/machine-formats.exp`):
/// the design centred on (0, 0); every frame move as `80 02` jumps of at most 12.7 mm (the
/// longer axis full, the other proportional); no trim record (the jump run is the trim); a
/// stitch over 12.7 mm as jumps plus one stitch for the remainder; `80 01 00 00` for a colour
/// change; and a jump run back to (0, 0) after the last stitch, with no end record.
pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let events = layout::events(input);
    let max = MAX_DELTA as i64;
    let mut w = ByteWriter::default();
    let (mut x, mut y) = (0_i64, 0_i64);
    let jump = |w: &mut ByteWriter, dx: i64, dy: i64| {
        w.append(&[0x80, 0x02]);
        w.u8(dx);
        w.u8(-dy);
    };
    for e in &events {
        match *e {
            Event::Move { x: tx, y: ty, .. } => {
                for (dx, dy) in layout::split_proportional(tx - x, ty - y, max) {
                    jump(&mut w, dx, dy);
                }
                x = tx;
                y = ty;
            }
            Event::Stitch { x: tx, y: ty } => {
                let pieces = layout::split_proportional(tx - x, ty - y, max);
                let (last, head) = pieces.split_last().expect("at least one piece");
                for &(dx, dy) in head {
                    jump(&mut w, dx, dy);
                }
                w.u8(last.0);
                w.u8(-last.1);
                x = tx;
                y = ty;
            }
            Event::ColorChange | Event::Stop => w.append(&[0x80, 0x01, 0x00, 0x00]),
            Event::End => {
                for (dx, dy) in layout::split_proportional(-x, -y, max) {
                    jump(&mut w, dx, dy);
                }
                break;
            }
        }
    }
    Ok(w.bytes)
}
