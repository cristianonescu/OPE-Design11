//! Husqvarna Viking SHV (read only). Big-endian. A 0x56-byte text header, a length-prefixed
//! design name, a 1-bit preview (`width` bytes × ⌈`height` / 2⌉ rows plus 4 bytes), a colour
//! count and 18 unknown bytes, then 14 bytes per colour: u32 BE number of 2-byte record words
//! in the block, a SHV chart index and 9 unknown bytes. The stitch records follow the table:
//! PE-DESIGN 11.50's own SHV export (`tests/fixtures/pe11/sample-basic.shv`) starts them at the
//! table's end with exactly as many words as the counts add up to. pyembroidery's reader starts
//! 2 bytes before the end; a file with one word fewer than the counts (as pyembroidery's layout
//! implies) is read that way. Records: signed byte `(dx, dy)` with y down; `80 01 dx(i16) dy(i16)`
//! moves and starts jump mode (later short records are jumps), `80 02` ends jump mode, `80 03`
//! is skipped. A colour block ends after its word count; there is no explicit colour change.
//!
//! Follows pyembroidery 1.5.1's `ShvReader` (MIT), the oracle. Neither pyembroidery nor
//! libembroidery has a working SHV writer, and the meaning of the unknown header bytes is not
//! published, so OpenStitch does not write SHV.

use crate::io::{ByteReader, signed8};
use crate::{FormatError, Result};
use openstitch_core::{Command, SHV_CHART, Stitch, StitchPattern, Thread};

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    reader.skip(0x56)?;
    let name_length = usize::from(reader.u8()?);
    if let Ok(name) = std::str::from_utf8(reader.read(name_length)?) {
        pattern.set_name(name);
    }
    let width = usize::from(reader.u8()?);
    let height = usize::from(reader.u8()?);
    reader.skip(4 + height.div_ceil(2) * width)?;
    let color_count = usize::from(reader.u8()?);
    reader.skip(18)?;
    let mut words_per_color = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        let words = reader.read(4)?;
        words_per_color.push(u32::from_be_bytes([words[0], words[1], words[2], words[3]]));
        let code = usize::from(reader.u8()?);
        pattern.threads.push(
            SHV_CHART[code % SHV_CHART.len()]
                .clone()
                .unwrap_or_else(|| Thread::rgb(0)),
        );
        reader.skip(9)?;
    }
    // PE-DESIGN's layout: the records start at the table's end and fill exactly the counted
    // words. pyembroidery's: they start 2 bytes earlier, so one word fewer follows the table.
    // Reading PE's file the pyembroidery way adds a stitch at the origin and, one word short at
    // the end, an extra colour change (issue #135).
    let counted: u64 = words_per_color.iter().map(|&w| u64::from(w)).sum();
    let following = (data.len() - reader.offset) as u64 / 2;
    if following < counted {
        reader.offset = reader
            .offset
            .checked_sub(2)
            .ok_or_else(|| FormatError::Truncated("SHV colour table".into()))?;
    }

    let signed16 = |b: &[u8]| f64::from(i16::from_be_bytes([b[0], b[1]]));
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let mut in_jump = false;
    let mut since_stop = 0_u64;
    let mut block = 0;
    let mut max = words_per_color.first().map_or(0, |&w| u64::from(w));
    while reader.remaining() >= 2 {
        let (b0, b1) = (reader.u8()?, reader.u8()?);
        if since_stop >= max {
            pattern
                .stitches
                .push(Stitch::new(x, y, Command::ColorChange));
            since_stop = 0;
            block += 1;
            max = words_per_color
                .get(block)
                .map_or(u64::from(u32::MAX), |&w| u64::from(w));
        }
        if b0 == 0x80 {
            since_stop += 1;
            match b1 {
                3 => continue,
                2 => {
                    in_jump = false;
                    continue;
                }
                1 => {
                    since_stop += 2;
                    let Ok(d) = reader.read(4) else {
                        break;
                    };
                    x += signed16(&d[0..2]);
                    y += signed16(&d[2..4]);
                    in_jump = true;
                    pattern.stitches.push(Stitch::new(x, y, Command::Jump));
                    continue;
                }
                _ => {} // read as an ordinary record, as the reference does
            }
        }
        since_stop += 1;
        x += signed8(b0);
        y += signed8(b1);
        let command = if in_jump {
            Command::Jump
        } else {
            Command::Stitch
        };
        pattern.stitches.push(Stitch::new(x, y, command));
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    Ok(pattern)
}
