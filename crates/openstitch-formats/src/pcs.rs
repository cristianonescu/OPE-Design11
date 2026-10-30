//! Pfaff PCS: version byte, hoop byte (2 = 80×80 mm, 3 = large), little-endian colour count,
//! one `R G B 00` entry per colour, a 16-bit record count, then 9-byte absolute records
//! `xf x(24le) yf y(24le) ctrl` with y up, in 1/6 mm (0.1 mm × 5/3): `xf` and `yf` are the
//! fractional parts in 1/256 (PE-DESIGN writes them; pyembroidery skips them). `ctrl` 0x00 is
//! a stitch, bit 0x01 a colour change (PE-DESIGN puts the colour's palette index in the
//! record's first byte and zero coordinates), bit 0x04 a jump; anything else ends the design.
//! No trim or end record exists.
//! The writer lays the file out as PE-DESIGN 11.50's export does (see [`write`]).

use crate::io::{ByteReader, ByteWriter};
use crate::layout::{self, Event};
use crate::{FormatError, Result, repeating_stops_palette};
use openstitch_core::{Command, Stitch, StitchPattern, Thread, nearest_index};

/// 0.1 mm per file unit is 5/3; written as one constant so reads match the reference exactly.
pub const UNIT_SCALE: f64 = 5.0 / 3.0;
const VERSION: u8 = 0x32;

/// Header hoop byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hoop {
    Small = 2,
    Large = 3,
}

impl Hoop {
    /// 80×80 mm when the design fits, else the large hoop.
    pub fn fitting(width: f64, height: f64) -> Self {
        if width <= 800.0 && height <= 800.0 {
            Self::Small
        } else {
            Self::Large
        }
    }
}

/// The 16-entry colour table PE-DESIGN writes in every PCS file (the VGA palette), which its
/// colour-change records index.
pub const PALETTE: [u32; 16] = [
    0x000000, 0x000080, 0x0000FF, 0x008080, 0x00FFFF, 0x800080, 0xFF00FF, 0x800000, 0xFF0000,
    0x008000, 0x00FF00, 0x808000, 0xFFFF00, 0x808080, 0xC0C0C0, 0xFFFFFF,
];

/// Where PE-DESIGN puts the centre of a design, in file units: the middle of the 120 × 115 mm
/// large hoop.
pub const CENTER: (i64, i64) = (360, 345);

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    reader.skip(2)?; // version, hoop
    let color_count = reader.u16le()?;
    let mut table = Vec::with_capacity(color_count as usize);
    for _ in 0..color_count {
        let c = reader.read(4)?;
        table.push(Thread::rgb(
            u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]),
        ));
    }
    reader.skip(2)?; // record count; the reference reads to end of file instead

    let signed24 = |v: u32| {
        if v > 0x7F_FFFF {
            i64::from(v) - 0x100_0000
        } else {
            i64::from(v)
        }
    };
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    // Palette index of each colour-change record (PE-DESIGN); all zero in other files.
    let mut indexes = Vec::new();
    while reader.remaining() >= 9 {
        let index = reader.u8()?;
        let fx = signed24(reader.u24le()?);
        let yf = reader.u8()?;
        let fy = signed24(reader.u24le()?);
        let ctrl = reader.u8()?;
        let (px, py) = (
            (fx as f64 + f64::from(index) / 256.0) * UNIT_SCALE,
            -(fy as f64 + f64::from(yf) / 256.0) * UNIT_SCALE,
        );
        if ctrl == 0x00 {
            (x, y) = (px, py);
            pattern.stitches.push(Stitch::new(x, y, Command::Stitch));
        } else if ctrl & 0x01 != 0 {
            indexes.push(usize::from(index));
            // A colour record before any stitch selects the first colour.
            if pattern
                .stitches
                .iter()
                .any(|s| s.command == Command::Stitch)
            {
                pattern
                    .stitches
                    .push(Stitch::new(x, y, Command::ColorChange));
            }
        } else if ctrl & 0x04 != 0 {
            (x, y) = (px, py);
            pattern.stitches.push(Stitch::new(x, y, Command::Jump));
        } else {
            break;
        }
    }
    pattern.stitches.push(Stitch::new(x, y, Command::End));
    pattern.threads = if indexes.iter().any(|&i| i != 0) {
        indexes
            .iter()
            .map(|&i| table.get(i).cloned().unwrap_or_else(|| Thread::rgb(0)))
            .collect()
    } else {
        table
    };
    Ok(pattern)
}

// MARK: Writing

/// Writes the file as PE-DESIGN 11.50 exports it (`compat/reference/machine/machine-formats.pcs`):
/// version `'2'`, the large hoop whatever the design's size, the 16-colour [`PALETTE`], a
/// record count of stitches and jumps, then a colour record (palette index of the nearest
/// entry in its first byte, `ctrl` 3) before each block; every frame move as a jump record at
/// the current position, a jump to the move's end and a stitch there (the first needle point;
/// the first move of the design is the stitch alone), followed by the stream's own stitch at
/// that point if it has one; coordinates in 1/6 mm with a 1/256
/// fraction byte (truncated) and the design's centre at [`CENTER`]. Trims are dropped (PCS has no trim record); a stop
/// is written as a colour change to the same thread.
pub fn write(input: &StitchPattern) -> Result<Vec<u8>> {
    let mut pattern = input.clone();
    pattern.fix_thread_count(None);
    let palette: Vec<Option<Thread>> = PALETTE.iter().map(|&c| Some(Thread::rgb(c))).collect();
    let indexes: Vec<usize> = repeating_stops_palette(&pattern)
        .iter()
        .map(|t| nearest_index(t, &palette).unwrap_or(0))
        .collect();
    let events = layout::events(&pattern);
    let made_up_first_move = layout::starts_with_stitch(&pattern);

    let mut w = ByteWriter::default();
    w.u8(i64::from(VERSION));
    w.u8(Hoop::Large as i64);
    w.u16le(PALETTE.len() as i64);
    for c in PALETTE {
        w.append(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 0]);
    }
    let count_index = w.len();
    w.u16le(0);

    // Fixed point: the integer part in 1/6 mm and a fraction byte in 1/256, truncated.
    let fixed =
        |v: i64, center: i64| ((v as f64 * 3.0 / 5.0 + center as f64) * 256.0).floor() as i64;
    let record = |w: &mut ByteWriter, x: i64, y: i64, ctrl: u8| {
        let (fx, fy) = (fixed(x, CENTER.0), fixed(-y, CENTER.1));
        w.u8(fx & 0xFF);
        w.u24le(fx >> 8);
        w.u8(fy & 0xFF);
        w.u24le(fy >> 8);
        w.u8(i64::from(ctrl));
    };
    let color = |w: &mut ByteWriter, index: usize| {
        w.u8(index as i64);
        w.fill(0, 7);
        w.u8(0x03);
    };
    let mut records = 0_usize;
    let mut block = 0_usize;
    let (mut x, mut y) = (0_i64, 0_i64);
    let mut started = false;
    color(&mut w, indexes.first().copied().unwrap_or(0));
    let mut i = 0;
    while i < events.len() {
        match events[i] {
            Event::Move { x: tx, y: ty, .. } => {
                if started {
                    record(&mut w, x, y, 0x04);
                    record(&mut w, tx, ty, 0x04);
                    records += 2;
                }
                // The stitch at the move's end is the first needle point. A stitch the stream
                // has at that very point (a PE-DESIGN design's landing stitch) is written as
                // well, a second zero-length record, as PE-DESIGN writes it
                // (compat machine-sample-basic, #135).
                record(&mut w, tx, ty, 0x00);
                records += 1;
                if !started
                    && made_up_first_move
                    && layout::next_stitch(&events, i) == Some((tx, ty))
                {
                    i += 1; // the move made up to the first stitch ends on that stitch
                }
                started = true;
                (x, y) = (tx, ty);
            }
            Event::Stitch { x: tx, y: ty } => {
                record(&mut w, tx, ty, 0x00);
                records += 1;
                (x, y) = (tx, ty);
            }
            Event::ColorChange | Event::Stop => {
                block += 1;
                color(&mut w, indexes.get(block).copied().unwrap_or(0));
            }
            Event::End => break,
        }
        i += 1;
    }
    if records > 0xFFFF {
        return Err(FormatError::ValueOutOfRange(format!(
            "PCS holds at most 65535 records, got {records}"
        )));
    }
    w.patch16le(count_index, records as i64);
    Ok(w.bytes)
}
