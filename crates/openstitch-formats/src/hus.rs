//! Husqvarna Viking HUS: a 0x2A-byte little-endian header (magic, record count, colour count,
//! extents, offsets of three compressed streams), one u16 HUS chart index per colour, then the
//! command, x and y streams, each compressed with [`crate::lzh`]. Record `i` is
//! `(command[i], x[i], y[i])`: a signed byte delta each, y up. Commands: `80` stitch, `81` jump,
//! `84` colour change, `88` trim (after an optional move), `90` end.
//!
//! The reader follows pyembroidery 1.5.1's `HusReader` (MIT), the oracle; the writer is its
//! inverse, with the header magic and offset layout libembroidery's `writeHus` (zlib) uses.
//! [`crate::vip`] shares the record streams.

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::{FormatError, Result, lzh, repeating_stops_palette};
use openstitch_core::{
    Command, HUS_CHART, Stitch, StitchPattern, Thread, nearest_index, round_coordinate,
};

pub const MAGIC: u32 = 0x00C8_AF5B;
pub const MAX_DELTA: f64 = 127.0;
/// Upper bound on the record count a reader accepts (and so on decompressed stream size).
pub const MAX_RECORDS: usize = 4_000_000;
/// Upper bound on the colour count a reader accepts.
pub const MAX_COLORS: usize = 1_000;
const HEADER: usize = 0x2A;

pub(crate) struct Header {
    pub records: usize,
    pub colors: usize,
    pub offsets: [usize; 3],
}

/// Reads the fields shared by HUS and VIP; the reader is left at offset 0x2A.
pub(crate) fn read_header(reader: &mut ByteReader, magic: u32, name: &str) -> Result<Header> {
    let found = reader.u32le()?;
    if found != magic {
        return Err(FormatError::BadSignature(format!(
            "{name} magic {found:#010x}"
        )));
    }
    let records = reader.u32le()? as usize;
    let colors = reader.u32le()? as usize;
    if records > MAX_RECORDS || colors > MAX_COLORS {
        return Err(FormatError::ValueOutOfRange(format!(
            "{name}: {records} records, {colors} colours"
        )));
    }
    reader.skip(8)?; // extents
    let offsets = [
        reader.u32le()? as usize,
        reader.u32le()? as usize,
        reader.u32le()? as usize,
    ];
    let len = reader.bytes.len();
    if !(offsets[0] <= offsets[1] && offsets[1] <= offsets[2] && offsets[2] <= len) {
        return Err(FormatError::Truncated(format!(
            "{name} stream offsets {offsets:?} outside a {len}-byte file"
        )));
    }
    reader.skip(10)?; // 8-byte string, u16
    Ok(Header {
        records,
        colors,
        offsets,
    })
}

/// Decompresses the three streams and appends their records to `pattern`, as pyembroidery does.
pub(crate) fn read_records(
    data: &[u8],
    header: &Header,
    pattern: &mut StitchPattern,
) -> Result<()> {
    let [c, xo, yo] = header.offsets;
    let commands = lzh::expand(&data[c..xo], header.records)?;
    let xs = lzh::expand(&data[xo..yo], header.records)?;
    let ys = lzh::expand(&data[yo..], header.records)?;
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    for ((&cmd, &bx), &by) in commands.iter().zip(&xs).zip(&ys) {
        let (dx, dy) = (signed8(bx), -signed8(by));
        let command = match cmd {
            0x80 => Command::Stitch,
            0x81 => Command::Jump,
            0x84 => Command::ColorChange,
            0x88 => {
                if dx != 0.0 || dy != 0.0 {
                    x += dx;
                    y += dy;
                    pattern.stitches.push(Stitch::new(x, y, Command::Jump));
                }
                pattern.stitches.push(Stitch::new(x, y, Command::Trim));
                continue;
            }
            _ => break, // 0x90 end, or an unmapped command
        };
        x += dx;
        y += dy;
        pattern.stitches.push(Stitch::new(x, y, command));
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    Ok(())
}

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let header = read_header(&mut reader, MAGIC, "HUS")?;
    let mut pattern = StitchPattern::new();
    for _ in 0..header.colors {
        let i = reader.u16le()? as usize % HUS_CHART.len();
        pattern
            .threads
            .push(HUS_CHART[i].clone().unwrap_or_else(|| Thread::rgb(0)));
    }
    read_records(data, &header, &mut pattern)?;
    Ok(pattern)
}

/// The record streams, extents and colour palette shared by the HUS and VIP writers.
pub(crate) struct Encoded {
    pub records: usize,
    pub streams: [Vec<u8>; 3],
    /// +x, +y, −x, −y extents (y up), as signed 16-bit values.
    pub extents: [i64; 4],
    pub palette: Vec<Thread>,
}

pub(crate) fn encode(input: &StitchPattern) -> Encoded {
    let mut pattern = input.interpolating_long_moves(MAX_DELTA, MAX_DELTA);
    pattern.fix_thread_count(None);
    let palette = repeating_stops_palette(&pattern);
    let (mut commands, mut xs, mut ys) = (Vec::new(), Vec::new(), Vec::new());
    let (mut xx, mut yy) = (0_i64, 0_i64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0_i64, 0_i64, 0_i64, 0_i64);
    let mut seen = false;
    for s in &pattern.stitches {
        let (dx, dy) = (round_coordinate(s.x) - xx, round_coordinate(s.y) - yy);
        let (code, moves) = match s.command {
            Command::Stitch => (0x80, true),
            Command::Jump => (0x81, true),
            Command::ColorChange | Command::Stop => (0x84, true),
            Command::Trim => (0x88, false),
            Command::End => break,
        };
        let (mut dx, mut dy) = if moves { (dx, dy) } else { (0, 0) };
        // Only stitches and jumps were interpolated: reach a distant colour change by jumps.
        while dx.abs() > 127 || dy.abs() > 127 {
            let (sx, sy) = (dx.clamp(-127, 127), dy.clamp(-127, 127));
            commands.push(0x81);
            xs.push(sx as u8);
            ys.push((-sy) as u8);
            (xx, yy, dx, dy) = (xx + sx, yy + sy, dx - sx, dy - sy);
        }
        xx += dx;
        yy += dy;
        commands.push(code);
        xs.push(dx as u8);
        ys.push((-dy) as u8);
        if matches!(s.command, Command::Stitch | Command::Jump) {
            if !seen {
                (min_x, min_y, max_x, max_y) = (xx, yy, xx, yy);
                seen = true;
            }
            min_x = min_x.min(xx);
            max_x = max_x.max(xx);
            min_y = min_y.min(yy);
            max_y = max_y.max(yy);
        }
    }
    commands.push(0x90);
    xs.push(0);
    ys.push(0);
    Encoded {
        records: commands.len(),
        streams: [
            lzh::compress(&commands),
            lzh::compress(&xs),
            lzh::compress(&ys),
        ],
        extents: [max_x, -min_y, min_x, -max_y],
        palette,
    }
}

/// The fields at 0x04–0x2A shared by HUS and VIP; `first_stream` is the command stream's offset.
pub(crate) fn write_header(w: &mut ByteWriter, e: &Encoded, colors: usize, first_stream: usize) {
    w.u32le(e.records as i64);
    w.u32le(colors as i64);
    for v in e.extents {
        w.u16le(v.clamp(i64::from(i16::MIN), i64::from(i16::MAX)));
    }
    let x_offset = first_stream + e.streams[0].len();
    w.u32le(first_stream as i64);
    w.u32le(x_offset as i64);
    w.u32le((x_offset + e.streams[1].len()) as i64);
    w.fill(0, 10);
}

pub fn write(pattern: &StitchPattern) -> Result<Vec<u8>> {
    let e = encode(pattern);
    let mut w = ByteWriter::default();
    w.u32le(i64::from(MAGIC));
    write_header(&mut w, &e, e.palette.len(), HEADER + 2 * e.palette.len());
    for t in &e.palette {
        w.u16le(nearest_index(t, &HUS_CHART).unwrap_or(0) as i64);
    }
    for s in &e.streams {
        w.append(s);
    }
    Ok(w.bytes)
}
