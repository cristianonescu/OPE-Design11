//! Melco EXP: headerless stream of 2-byte signed (dx, dy) stitch records, y up.
//! Control records start with 0x80: `80 04 dx dy` jump, `80 80 07 00` trim,
//! `80 01 00 00` colour change. No colour information is stored.

use crate::Result;
use crate::io::{ByteReader, ByteWriter, signed8};
use openstitch_core::{Command, Stitch, StitchPattern, round_coordinate};

pub const MAX_DELTA: f64 = 127.0;

pub fn read(data: &[u8]) -> Result<StitchPattern> {
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
            0x02 => {
                x += dx;
                y += dy;
                pattern.stitches.push(Stitch::new(x, y, Command::Stitch));
            }
            0x04 => {
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
    Ok(pattern)
}

pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let pattern = input.interpolating_long_moves(MAX_DELTA, MAX_DELTA);
    let mut w = ByteWriter::default();
    let (mut xx, mut yy) = (0_i64, 0_i64);
    for s in &pattern.stitches {
        let dx = round_coordinate(s.x) - xx;
        let dy = round_coordinate(s.y) - yy;
        match s.command {
            Command::Stitch => {
                xx += dx;
                yy += dy;
                w.u8(dx);
                w.u8(-dy);
            }
            Command::Jump => {
                xx += dx;
                yy += dy;
                w.append(&[0x80, 0x04]);
                w.u8(dx);
                w.u8(-dy);
            }
            Command::Trim => w.append(&[0x80, 0x80, 0x07, 0x00]),
            Command::ColorChange | Command::Stop => w.append(&[0x80, 0x01, 0x00, 0x00]),
            Command::End => break,
        }
    }
    Ok(w.bytes)
}
