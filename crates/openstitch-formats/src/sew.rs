//! Janome SEW: little-endian colour count and one Janome SEW chart index per colour, zero
//! padding up to 0x1D78, then JEF-style 2-byte signed (dx, dy) records with y up. Controls:
//! `80 01 dx dy` colour change, `80 02 dx dy` jump (`80 04` is read as a jump too), `80 10` end
//! (PE-DESIGN pads 16 zero bytes after it). pyembroidery 1.5.1 only reads SEW; the records
//! are laid out as PE-DESIGN 11.50's export (see [`write`]).

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::jef;
use crate::layout;
use crate::{FormatError, Result, repeating_stops_palette};
use openstitch_core::{Command, SEW_CHART, Stitch, StitchPattern, Thread, nearest_index};

pub const MAX_DELTA: f64 = 127.0;
const STITCH_OFFSET: usize = 0x1D78;

/// The SEW chart as the file indexes it: entry 0 is "Unknown".
fn chart() -> &'static [Option<Thread>] {
    &SEW_CHART
}

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    let color_count = reader.u16le()?;
    let chart = chart();
    for _ in 0..color_count {
        let i = reader.u16le()? as usize % chart.len();
        pattern.threads.push(
            chart[i]
                .clone()
                .unwrap_or_else(|| Thread::rgb(0).with_name("Unknown")),
        );
    }
    if data.len() < STITCH_OFFSET {
        return Err(FormatError::Truncated(
            "SEW header shorter than 0x1D78 bytes".into(),
        ));
    }
    reader.offset = STITCH_OFFSET;

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
        let command = match b1 {
            _ if b1 & 0x01 != 0 => {
                pattern
                    .stitches
                    .push(Stitch::new(x, y, Command::ColorChange));
                continue;
            }
            0x02 | 0x04 => Command::Jump,
            // `80 10` ends the design; PE-DESIGN pads 16 zero bytes after it.
            _ => break,
        };
        x += signed8(p0);
        y -= signed8(p1);
        pattern.stitches.push(Stitch::new(x, y, command));
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    // The frame move from the machine's start point to the first needle point is not kept:
    // where PE-DESIGN starts the stream is not yet known (see `write`), and the design starts
    // at its first needle point in every other respect.
    if let Some(first) = pattern
        .stitches
        .iter()
        .position(|s| s.command == Command::Stitch)
    {
        pattern.stitches.drain(..first);
    }
    Ok(pattern)
}

// MARK: Writing

/// Writes the records as PE-DESIGN 11.50's SEW export lays them out (the JEF stream of
/// [`jef::janome_records`] with a zero-length stitch before each colour change), starting at
/// the centre of the design. PE-DESIGN starts its SEW stream 15.9 mm left of and 9.7 mm below
/// the centre in the one reference file, a rule not yet known, and fills 340 header bytes
/// after the colour list whose meaning is unknown; OpenStitch leaves them zero. Trims are the
/// jump runs (SEW has no trim record); a stop is written as a colour change to the same
/// thread. Colours are stored as the nearest SEW chart entry.
pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let palette = sew_palette(&pattern);
    if 2 + palette.len() * 2 > STITCH_OFFSET {
        return Err(FormatError::ValueOutOfRange(format!(
            "too many SEW colours: {}",
            palette.len()
        )));
    }

    let mut w = ByteWriter::default();
    w.u16le(palette.len() as i64);
    for &i in &palette {
        w.u16le(i as i64);
    }
    w.fill(0, STITCH_OFFSET - w.len());
    let (records, _) = jef::janome_records(
        &layout::events(&pattern),
        &jef::WriteOptions::default(),
        true,
    );
    w.append(&records);
    w.fill(0, 16);
    Ok(w.bytes)
}

/// Nearest SEW chart index (never 0, "Unknown") for each colour block.
pub fn sew_palette(pattern: &StitchPattern) -> Vec<usize> {
    let mut chart = chart().to_vec();
    chart[0] = None;
    repeating_stops_palette(pattern)
        .iter()
        .map(|t| nearest_index(t, &chart).unwrap_or(1))
        .collect()
}
