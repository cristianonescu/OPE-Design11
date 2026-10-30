//! Husqvarna Viking HUS: a 0x2A-byte little-endian header (magic, record count, colour count,
//! extents, offsets of three compressed streams), one u16 HUS chart index per colour, then the
//! command, x and y streams, each compressed with [`crate::lzh`]. Record `i` is
//! `(command[i], x[i], y[i])`: a signed byte delta each, y up. Commands: `80` stitch, `81` jump,
//! `84` colour change, `88` trim (after an optional move), `90` end.
//!
//! The reader follows pyembroidery 1.5.1's `HusReader` (MIT), the oracle, and accepts both
//! magics; the writer lays the records out as PE-DESIGN 11.50's export does (see [`encode`]).
//! [`crate::vip`] shares the record streams.

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::layout::{self, Event};
use crate::{FormatError, Result, lzh, repeating_stops_palette};
use openstitch_core::{Command, HUS_CHART, Stitch, StitchPattern, Thread, nearest_index};

/// The magic PE-DESIGN 11.50 writes (`compat/reference/machine/machine-formats.hus`).
pub const MAGIC: u32 = 0x00C8_FC5D;
/// The magic libembroidery writes and pyembroidery's fixtures carry; read as well.
pub const MAGIC_LIBEMBROIDERY: u32 = 0x00C8_AF5B;
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
pub(crate) fn read_header(reader: &mut ByteReader, magics: &[u32], name: &str) -> Result<Header> {
    let found = reader.u32le()?;
    if !magics.contains(&found) {
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
    // Index of the first jump of the current jump run, for PE-DESIGN's trim marker.
    let mut run_start: Option<usize> = None;
    for ((&cmd, &bx), &by) in commands.iter().zip(&xs).zip(&ys) {
        let (dx, dy) = (signed8(bx), -signed8(by));
        if cmd != 0x81 {
            run_start = None;
        }
        let command = match cmd {
            0x80 => Command::Stitch,
            0x81 => {
                // PE-DESIGN marks a trimmed frame move with a zero-length jump after it.
                if dx == 0.0
                    && dy == 0.0
                    && let Some(start) = run_start
                {
                    let before = if start > 0 {
                        pattern.stitches[start - 1]
                    } else {
                        Stitch::new(0.0, 0.0, Command::Stitch)
                    };
                    if pattern
                        .stitches
                        .get(start.wrapping_sub(1))
                        .is_none_or(|s| s.command != Command::Trim)
                    {
                        pattern
                            .stitches
                            .insert(start, Stitch::new(before.x, before.y, Command::Trim));
                    }
                    continue;
                }
                if run_start.is_none() {
                    run_start = Some(pattern.stitches.len());
                }
                Command::Jump
            }
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
    let header = read_header(&mut reader, &[MAGIC, MAGIC_LIBEMBROIDERY], "HUS")?;
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

/// Which of PE-DESIGN's two record layouts to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialect {
    Hus,
    Vip,
}

/// Longest HUS jump record PE-DESIGN writes (8 mm).
pub const HUS_MAX_JUMP: i64 = 80;
/// Longest HUS stitch record PE-DESIGN writes (12 mm).
pub const HUS_MAX_STITCH: i64 = 120;
/// VIP piece length: PE-DESIGN cuts a move into `ceil(longer axis / this)` equal pieces. One
/// reference file bounds it to 7.5–7.9 mm; 7.5 mm reproduces it.
pub const VIP_PIECE: i64 = 75;

/// Encodes the stream as PE-DESIGN 11.50 exports it (`compat/reference/machine/machine-formats.hus`
/// and `.vip`), the design centred on (0, 0).
///
/// HUS: every frame move as `81` jumps of at most 8 mm (the longer axis full, the other
/// proportional) to the move's end, a zero-length `81` after it when the move is trimmed
/// (the first move excepted), then a zero-length `80` stitch at the move's end (the first
/// needle point; the stream's own stitch at that point, if any, follows as a second one); a stitch over 12 mm as `80` stitches of at most 12 mm, split the same way;
/// `84` for a colour change; `90` at the end, with no return to the origin.
///
/// VIP: `88` (zero-length) before a trimmed move (the first move excepted); every frame move
/// folded into the stitch after it and cut into `ceil(longer axis / 7.5 mm)` equal pieces, the
/// last one an `80` stitch and the rest `81` jumps; a stitch over 7.5 mm likewise; `84` and
/// `90` as HUS.
pub(crate) fn encode(input: &StitchPattern, dialect: Dialect) -> Encoded {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let palette = repeating_stops_palette(&pattern);
    let events = layout::events(&pattern);
    let made_up_first_move = layout::starts_with_stitch(&pattern);
    let (mut commands, mut xs, mut ys) = (Vec::new(), Vec::new(), Vec::new());
    let (mut x, mut y) = (0_i64, 0_i64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0_i64, 0_i64, 0_i64, 0_i64);
    fn push(
        commands: &mut Vec<u8>,
        xs: &mut Vec<u8>,
        ys: &mut Vec<u8>,
        code: u8,
        dx: i64,
        dy: i64,
    ) {
        commands.push(code);
        xs.push(dx as u8);
        ys.push((-dy) as u8);
    }
    let mut visit = |x: i64, y: i64| {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    };
    let mut first_move = true;
    let mut i = 0;
    while i < events.len() {
        match events[i] {
            Event::Move {
                x: tx,
                y: ty,
                trimmed,
            } => {
                let trim = trimmed && !first_move;
                let made_up = first_move && made_up_first_move;
                first_move = false;
                match dialect {
                    Dialect::Hus => {
                        for (dx, dy) in layout::split_proportional(tx - x, ty - y, HUS_MAX_JUMP) {
                            push(&mut commands, &mut xs, &mut ys, 0x81, dx, dy);
                        }
                        if trim {
                            push(&mut commands, &mut xs, &mut ys, 0x81, 0, 0);
                        }
                        // The zero-length stitch at the move's end is the first needle point. A
                        // stitch the stream has at that very point (a PE-DESIGN design's landing
                        // stitch) follows as a second zero-length `80`, as PE-DESIGN writes it
                        // (compat machine-sample-basic, #135).
                        push(&mut commands, &mut xs, &mut ys, 0x80, 0, 0);
                        if made_up && layout::next_stitch(&events, i) == Some((tx, ty)) {
                            i += 1; // the move made up to the first stitch ends on that stitch
                        }
                        (x, y) = (tx, ty);
                    }
                    Dialect::Vip => {
                        if trim {
                            push(&mut commands, &mut xs, &mut ys, 0x88, 0, 0);
                        }
                        let (target, folded) = match layout::next_stitch(&events, i) {
                            Some(p) => (p, true),
                            None => ((tx, ty), false),
                        };
                        let pieces = layout::split_even(target.0 - x, target.1 - y, VIP_PIECE);
                        let (last, head) = pieces.split_last().expect("at least one piece");
                        for &(dx, dy) in head {
                            push(&mut commands, &mut xs, &mut ys, 0x81, dx, dy);
                        }
                        let code = if folded { 0x80 } else { 0x81 };
                        push(&mut commands, &mut xs, &mut ys, code, last.0, last.1);
                        if folded {
                            i += 1;
                        }
                        (x, y) = target;
                    }
                }
                visit(x, y);
            }
            Event::Stitch { x: tx, y: ty } => {
                match dialect {
                    Dialect::Hus => {
                        for (dx, dy) in layout::split_proportional(tx - x, ty - y, HUS_MAX_STITCH) {
                            push(&mut commands, &mut xs, &mut ys, 0x80, dx, dy);
                        }
                    }
                    Dialect::Vip => {
                        let pieces = layout::split_even(tx - x, ty - y, VIP_PIECE);
                        let (last, head) = pieces.split_last().expect("at least one piece");
                        for &(dx, dy) in head {
                            push(&mut commands, &mut xs, &mut ys, 0x81, dx, dy);
                        }
                        push(&mut commands, &mut xs, &mut ys, 0x80, last.0, last.1);
                    }
                }
                (x, y) = (tx, ty);
                visit(x, y);
            }
            Event::ColorChange | Event::Stop => {
                push(&mut commands, &mut xs, &mut ys, 0x84, 0, 0);
            }
            Event::End => break,
        }
        i += 1;
    }
    push(&mut commands, &mut xs, &mut ys, 0x90, 0, 0);
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
    let e = encode(pattern, Dialect::Hus);
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
