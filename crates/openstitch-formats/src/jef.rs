//! Janome JEF: little-endian header (stitch offset, date, colour count, point count, hoop code,
//! extents, hoop-edge distances), one Janome chart index and one `0x0D` per colour, then
//! 2-byte signed (dx, dy) records with y up. Controls: `80 01 dx dy` colour change,
//! `80 02 dx dy` jump, `80 10` end. A colour index of 0 marks a stop.

use crate::io::{ByteReader, ByteWriter, signed8};
use crate::{FormatError, Result};
use openstitch_core::{
    Command, JEF_CHART, Stitch, StitchPattern, Thread, nearest_index, round_coordinate,
};

pub const MAX_DELTA: f64 = 127.0;
/// Reader heuristic: a jump run longer than this (0.1 mm) implies a trim (3 mm).
pub const DEFAULT_TRIM_DISTANCE: f64 = 30.0;

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
    /// Write each trim as `trim_jumps` zero-length jumps (off: trims are dropped and readers
    /// infer them from jump length).
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

pub fn write_with(input: &StitchPattern, options: &WriteOptions) -> Result<Vec<u8>> {
    let mut pattern = input.interpolating_long_moves(MAX_DELTA, MAX_DELTA);
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

    let mut points = 1_i64;
    for s in &pattern.stitches {
        points += match s.command {
            Command::Stitch => 1,
            Command::Jump | Command::ColorChange | Command::Stop => 2,
            Command::Trim if options.command_trims => 2 * options.trim_jumps as i64,
            Command::Trim => 0,
            Command::End => break,
        };
    }

    let mut w = ByteWriter::default();
    w.u32le(0x74 + palette.len() as i64 * 8);
    w.u32le(0x14);
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
            Command::ColorChange | Command::Stop => {
                xx += dx;
                yy += dy;
                w.append(&[0x80, 0x01]);
                w.u8(dx);
                w.u8(-dy);
            }
            Command::Jump => {
                xx += dx;
                yy += dy;
                w.append(&[0x80, 0x02]);
                w.u8(dx);
                w.u8(-dy);
            }
            Command::Trim => {
                if options.command_trims {
                    for _ in 0..options.trim_jumps {
                        w.append(&[0x80, 0x02, 0x00, 0x00]);
                    }
                }
            }
            Command::End => break,
        }
    }
    w.append(&[0x80, 0x10]);
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
