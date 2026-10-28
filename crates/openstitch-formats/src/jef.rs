//! Janome JEF: little-endian header (stitch offset, `0x0A`, date, colour count, point count, hoop code,
//! extents, hoop-edge distances), one Janome chart index and one `0x0D` per colour, then
//! 2-byte signed (dx, dy) records with y up. Controls: `80 01 dx dy` colour change,
//! `80 02 dx dy` jump, `80 10` end (PE-DESIGN pads two zero bytes after it). A colour index
//! of 0 marks a stop.
//! The writer lays the records out as PE-DESIGN 11.50's export does (see [`write_with`]).

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::layout::{self, Event};
use crate::{FormatError, Result};
use openstitch_core::{
    Command, JEF_CHART, Stitch, StitchPattern, Thread, nearest_index, round_coordinate,
};

pub const MAX_DELTA: f64 = 127.0;
/// Reader heuristic: a jump run moving further than one record holds (12.7 mm) on an axis is
/// a trimmed frame move. A single jump of up to a record is the long part of a split stitch:
/// PE-DESIGN 11.50 writes a stitch over 12.7 mm as a jump and a stitch, and its trimmed moves
/// as jump runs with no other marker.
pub const DEFAULT_TRIM_DISTANCE: f64 = MAX_DELTA;

/// Header hoop code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hoop {
    H110x110 = 0,
    H50x50 = 1,
    H140x200 = 2,
    H126x110 = 3,
    H200x200 = 4,
}

impl Hoop {
    /// Smallest hoop the design (0.1 mm) fits, falling back to 110×110.
    pub fn fitting(width: i64, height: i64) -> Self {
        if width < 500 && height < 500 {
            Self::H50x50
        } else if width < 1260 && height < 1100 {
            Self::H126x110
        } else if width < 1400 && height < 2000 {
            Self::H140x200
        } else if width < 2000 && height < 2000 {
            Self::H200x200
        } else {
            Self::H110x110
        }
    }
}

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    read_with(data, Some(DEFAULT_TRIM_DISTANCE))
}

/// Reads, inserting a trim before jump runs longer than `trim_distance` (`None`: no inference).
pub fn read_with(data: &[u8], trim_distance: Option<f64>) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    let stitch_offset = reader.u32le()? as usize;
    reader.skip(20)?;
    let color_count = reader.u32le()? as usize;
    reader.skip(88)?;
    if color_count > reader.remaining() / 4 {
        return Err(FormatError::Truncated(format!(
            "{color_count} JEF colours at {}",
            reader.offset
        )));
    }
    let mut threads: Vec<Option<Thread>> = Vec::with_capacity(color_count);
    for _ in 0..color_count {
        let index = (reader.u32le()? as i32).unsigned_abs() as usize;
        threads.push(if index == 0 {
            None
        } else {
            JEF_CHART[index % JEF_CHART.len()].clone()
        });
    }
    reader.offset = stitch_offset;

    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let mut color_index = 1;
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
        match b1 {
            0x02 => {
                x += signed8(p0);
                y -= signed8(p1);
                pattern.stitches.push(Stitch::new(x, y, Command::Jump));
            }
            0x01 => {
                if color_index < threads.len() && threads[color_index].is_none() {
                    pattern.stitches.push(Stitch::new(x, y, Command::Stop));
                    threads.remove(color_index);
                } else {
                    pattern
                        .stitches
                        .push(Stitch::new(x, y, Command::ColorChange));
                    color_index += 1;
                }
            }
            _ => break,
        }
    }
    pattern.threads = threads.into_iter().flatten().collect();
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    pattern.infer_trims(None, trim_distance, true);
    Ok(pattern)
}

// MARK: Writing

/// JEF writer options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    /// 14-digit `yyyyMMddHHmmss` stamp for the header; `None` stamps the current UTC time.
    pub date: Option<String>,
    /// Write `trim_jumps` zero-length jumps before every trimmed frame move (off, as
    /// PE-DESIGN: a trimmed move is its jump run alone and readers infer the trim).
    pub command_trims: bool,
    pub trim_jumps: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            date: None,
            command_trims: false,
            trim_jumps: 3,
        }
    }
}

pub fn write(pattern: &StitchPattern) -> Result<Vec<u8>> {
    write_with(pattern, &WriteOptions::default())
}

/// The current UTC time as `yyyyMMddHHmmss`.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}{:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// The record stream shared by JEF and SEW, as PE-DESIGN 11.50 writes it
/// (`compat/reference/machine/machine-formats.jef` and `.sew`): a zero-length jump first;
/// every frame move folded into the stitch that follows it and written as `80 02` jumps that
/// move only the axes over 12.7 mm (by 12.7 mm each) plus one stitch record for the remainder (so the needle first goes
/// down at that stitch, as it does after a DST jump); a stitch over 12.7 mm likewise as jumps
/// plus one stitch; `80 01 00 00` then `80 02 00 00` for a colour change (SEW puts a
/// zero-length stitch before the `80 01`); `80 10` at the end, with no return to the origin.
/// Returns the bytes and the header's point count: 1 + stitches + 2 × (jumps + colour changes).
pub(crate) fn janome_records(
    events: &[Event],
    options: &WriteOptions,
    stitch_before_color_change: bool,
) -> (Vec<u8>, i64) {
    let max = MAX_DELTA as i64;
    let mut w = ByteWriter::default();
    let (mut x, mut y) = (0_i64, 0_i64);
    let mut points = 1_i64;
    fn jump(w: &mut ByteWriter, points: &mut i64, dx: i64, dy: i64) {
        w.append(&[0x80, 0x02]);
        w.u8(dx);
        w.u8(-dy);
        *points += 2;
    }
    jump(&mut w, &mut points, 0, 0);
    let mut i = 0;
    let mut first_move = true;
    while i < events.len() {
        match events[i] {
            Event::Move {
                x: tx,
                y: ty,
                trimmed,
            } => {
                if trimmed && !first_move && options.command_trims {
                    for _ in 0..options.trim_jumps {
                        jump(&mut w, &mut points, 0, 0);
                    }
                }
                first_move = false;
                // Fold the move into the stitch after it: that stitch is the first needle point.
                let (target, folded) = match layout::next_stitch(events, i) {
                    Some(p) => (p, true),
                    None => ((tx, ty), false),
                };
                let pieces = layout::split_overflow(target.0 - x, target.1 - y, max);
                let (last, head) = pieces.split_last().expect("at least one piece");
                for &(dx, dy) in head {
                    jump(&mut w, &mut points, dx, dy);
                }
                if folded {
                    w.u8(last.0);
                    w.u8(-last.1);
                    points += 1;
                    i += 1;
                } else {
                    jump(&mut w, &mut points, last.0, last.1);
                }
                (x, y) = target;
            }
            Event::Stitch { x: tx, y: ty } => {
                let pieces = layout::split_overflow(tx - x, ty - y, max);
                let (last, head) = pieces.split_last().expect("at least one piece");
                for &(dx, dy) in head {
                    jump(&mut w, &mut points, dx, dy);
                }
                w.u8(last.0);
                w.u8(-last.1);
                points += 1;
                x = tx;
                y = ty;
            }
            Event::ColorChange | Event::Stop => {
                if stitch_before_color_change {
                    w.append(&[0, 0]);
                    points += 1;
                }
                w.append(&[0x80, 0x01, 0x00, 0x00]);
                points += 2;
                jump(&mut w, &mut points, 0, 0);
            }
            Event::End => break,
        }
        i += 1;
    }
    w.append(&[0x80, 0x10]);
    (w.bytes, points)
}

pub fn write_with(input: &StitchPattern, options: &WriteOptions) -> Result<Vec<u8>> {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let stamp = match &options.date {
        Some(d) if d.len() == 14 && d.bytes().all(|b| b.is_ascii_digit()) => d.clone(),
        Some(d) => {
            return Err(FormatError::ValueOutOfRange(format!(
                "JEF date {d:?} is not yyyyMMddHHmmss"
            )));
        }
        None => utc_stamp(),
    };

    let palette = jef_palette(&pattern);
    let bounds = pattern.bounds();
    let (width, height) = (
        round_coordinate(bounds.width()),
        round_coordinate(bounds.height()),
    );
    let (half_w, half_h) = (
        round_coordinate(width as f64 / 2.0),
        round_coordinate(height as f64 / 2.0),
    );
    let (records, points) = janome_records(&layout::events(&pattern), options, false);

    let mut w = ByteWriter::default();
    w.u32le(0x74 + palette.len() as i64 * 8);
    w.u32le(0x0A); // pyembroidery writes 0x14 here; PE-DESIGN 11.50 writes 0x0A

    w.ascii(&stamp);
    w.u8(0);
    w.u8(0);
    w.u32le(palette.len() as i64);
    w.u32le(points);
    w.u32le(Hoop::fitting(width, height) as i64);
    for _ in 0..2 {
        w.u32le(half_w);
        w.u32le(half_h);
    }
    for (hx, hy) in [(550, 550), (250, 250), (700, 1000), (700, 1000)] {
        let (ex, ey) = (hx - half_w, hy - half_h);
        if ex.min(ey) >= 0 {
            for v in [ex, ey, ex, ey] {
                w.u32le(v);
            }
        } else {
            for _ in 0..4 {
                w.u32le(-1);
            }
        }
    }
    for &i in &palette {
        w.u32le(i as i64);
    }
    for _ in &palette {
        w.u32le(0x0D);
    }
    w.append(&records);
    w.append(&[0, 0]); // PE-DESIGN pads the end record to four bytes
    Ok(w.bytes)
}

/// One Janome chart index per colour block (stops write 0, then repeat the colour).
/// Consecutive blocks whose different threads map to the same chart entry take the
/// next-closest entry instead, so the change stays visible on the machine.
pub fn jef_palette(pattern: &StitchPattern) -> Vec<usize> {
    let mut chart: Vec<Option<Thread>> = JEF_CHART.to_vec();
    let mut palette = Vec::new();
    let mut last_index: Option<usize> = None;
    let mut last_thread: Option<&Thread> = None;
    let mut toggled = false;
    let mut block = 0;
    for (i, s) in pattern.stitches.iter().enumerate() {
        if !(s.command == Command::ColorChange || s.command == Command::Stop || i == 0) {
            continue;
        }
        if s.command == Command::ColorChange || i == 0 {
            let Some(thread) = pattern
                .threads
                .get(block.min(pattern.threads.len().saturating_sub(1)))
            else {
                break;
            };
            block += 1;
            let mut index = nearest_index(thread, &chart).unwrap_or(1);
            if let Some(last) = last_index
                && index == last
                && last_thread != Some(thread)
            {
                let saved = chart[index].take();
                index = nearest_index(thread, &chart).unwrap_or(index);
                chart[last] = saved;
            }
            palette.push(index);
            last_index = Some(index);
            last_thread = Some(thread);
            toggled = false;
        }
        if s.command == Command::Stop {
            toggled = !toggled;
            palette.push(if toggled { 0 } else { last_index.unwrap_or(0) });
        }
    }
    palette
}
