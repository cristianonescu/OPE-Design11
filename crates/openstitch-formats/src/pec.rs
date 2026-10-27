//! Brother PEC: the machine-readable section embedded in every PES file (and stand-alone .pec).
//!
//! Layout (offsets relative to the start of the PEC section):
//! - 0x000 `LA:` + 16-char label + CR
//! - 0x022 icon byte stride (6) and icon height (38)
//! - 0x030 colour count − 1, then one PEC chart index per colour block, space padded to 0x200
//! - 0x200 stitch block: 2 zero bytes, u24 block length, `31 FF F0`, u16 width, u16 height,
//!   u16 0x1E0, u16 0x1B0, encoded stitches, `FF`
//! - then (colours + 1) 48×38 1-bit icons: whole design first, then one per colour block.

use crate::io::{ByteReader, ByteWriter, trim_horizontal};
use crate::{FormatError, Result};
use openstitch_core::{
    Bounds, Command, PEC_CHART, Stitch, StitchPattern, Thread, nearest_index, round_coordinate,
};

pub const ICON_STRIDE: usize = 6;
pub const ICON_HEIGHT: usize = 38;
pub const MAX_DELTA: f64 = 2047.0;

const FLAG_LONG: u8 = 0x80;
const FLAG_TRIM: u8 = 0x20;
const FLAG_JUMP: u8 = 0x10;

// MARK: Reading

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    let mut reader = ByteReader::new(data);
    let magic = reader.read(8)?;
    if magic != b"#PEC0001" {
        return Err(FormatError::BadSignature(
            String::from_utf8_lossy(magic).into_owned(),
        ));
    }
    read_section(&mut reader)
}

pub(crate) fn read_section(reader: &mut ByteReader<'_>) -> Result<StitchPattern> {
    let mut pattern = StitchPattern::new();
    // Every PEC section starts with its `LA:` label. PE-DESIGN 11 saves a design that lies off
    // the page without one (the PES header's PEC offset points at zeros), and its stitches are
    // then only in the object section; decoding those zeros would give an empty design.
    if reader.read(3)? != b"LA:" {
        return Err(FormatError::UnsupportedFormat(
            "no PEC section (no `LA:` label at the PEC offset): PE-DESIGN writes none for a \
             design that lies off the page; move it onto the page and save it again"
                .into(),
        ));
    }
    let label = String::from_utf8_lossy(reader.read(16)?).into_owned();
    let label = trim_horizontal(&label);
    if !label.is_empty() {
        pattern.set_name(label);
    }
    reader.skip(0xF)?;
    reader.u8()?; // icon stride
    reader.u8()?; // icon height
    reader.skip(0xC)?;
    let color_changes = usize::from(reader.u8()?);
    let color_bytes = reader.read(color_changes + 1)?;
    pattern.threads = color_bytes
        .iter()
        .map(|&b| {
            PEC_CHART[usize::from(b) % PEC_CHART.len()]
                .clone()
                .unwrap_or_else(|| Thread::rgb(0).with_name("Unknown"))
        })
        .collect();
    reader.skip(0x1D0 - color_changes)?;
    reader.u24le()?; // stitch block length
    reader.skip(0x0B)?;
    pattern.stitches = read_stitches(reader)?;
    Ok(pattern)
}

fn signed12(v: u32) -> i32 {
    let b = (v & 0xFFF) as i32;
    if b > 0x7FF { b - 0x1000 } else { b }
}

fn signed7(v: u8) -> i32 {
    if v > 63 {
        i32::from(v) - 128
    } else {
        i32::from(v)
    }
}

pub(crate) fn read_stitches(reader: &mut ByteReader<'_>) -> Result<Vec<Stitch>> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    while reader.remaining() >= 2 {
        let v1 = reader.u8()?;
        if v1 == 0xFF && reader.peek() == Some(0x00) {
            break;
        }
        let mut v2 = reader.u8()?;
        if v1 == 0xFE && v2 == 0xB0 {
            reader.skip(1)?;
            out.push(Stitch::new(x, y, Command::ColorChange));
            continue;
        }
        let (mut jump, mut trim) = (false, false);
        let dx = if v1 & FLAG_LONG != 0 {
            trim |= v1 & FLAG_TRIM != 0;
            jump |= v1 & FLAG_JUMP != 0;
            let dx = signed12(u32::from(v1) << 8 | u32::from(v2));
            v2 = reader.u8()?;
            dx
        } else {
            signed7(v1)
        };
        let dy = if v2 & FLAG_LONG != 0 {
            trim |= v2 & FLAG_TRIM != 0;
            jump |= v2 & FLAG_JUMP != 0;
            signed12(u32::from(v2) << 8 | u32::from(reader.u8()?))
        } else {
            signed7(v2)
        };
        let (dx, dy) = (f64::from(dx), f64::from(dy));
        x += dx;
        y += dy;
        if jump {
            out.push(Stitch::new(x, y, Command::Jump));
        } else if trim {
            out.push(Stitch::new(x - dx, y - dy, Command::Trim));
            out.push(Stitch::new(x, y, Command::Jump));
        } else {
            out.push(Stitch::new(x, y, Command::Stitch));
        }
    }
    out.push(Stitch::new(x, y, Command::End));
    Ok(out)
}

// MARK: Writing

pub fn write(pattern: &StitchPattern) -> Result<Vec<u8>> {
    let mut w = ByteWriter::default();
    w.ascii("#PEC0001");
    write_section(pattern, &mut w)?;
    Ok(w.bytes)
}

/// Converts stops to colour changes with a repeated thread, fixes the thread count and
/// splits over-long moves: the shape every PEC writer needs.
pub(crate) fn prepare(input: &StitchPattern) -> StitchPattern {
    let mut p = input.clone();
    let mut threads: Vec<Thread> = input.threads.iter().take(1).cloned().collect();
    let mut block = 0;
    for s in &input.stitches {
        if s.command == Command::ColorChange {
            block += 1;
            if let Some(t) = input.threads.get(block) {
                threads.push(t.clone());
            }
        } else if s.command == Command::Stop
            && let Some(t) = input.threads.get(block)
        {
            threads.push(t.clone());
        }
    }
    for s in &mut p.stitches {
        if s.command == Command::Stop {
            s.command = Command::ColorChange;
        }
    }
    p.threads = threads;
    p.fix_thread_count(None);
    p.interpolating_long_moves(MAX_DELTA, MAX_DELTA)
}

/// Metadata key for the PEC label when it differs from the design name. PE-DESIGN 11 labels the
/// PEC section with the first 8 characters of the file name it saves to, whatever the Design
/// Property name (compat/reference: `pes-design-properties.pes` is named "Rose" and labelled
/// "pes-desi"); `design.export` sets this key from the output path.
pub const LABEL_KEY: &str = "pec.label";

/// The PEC label: the first 8 characters of the name, space padded to 16 bytes.
///
/// Padding by characters would misalign the header for non-ASCII names, so this keeps whole
/// UTF-8 characters that fit in 16 bytes and the header stays 512 bytes.
fn label_bytes(name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    for c in name.chars().take(8) {
        let mut buf = [0; 4];
        let enc = c.encode_utf8(&mut buf).as_bytes();
        if out.len() + enc.len() > 16 {
            break;
        }
        out.extend_from_slice(enc);
    }
    out.resize(16, b' ');
    out
}

/// Writes the PEC section and returns the colour list as written at 0x30 (count − 1 first;
/// empty when nothing is sewn), which PES v6 repeats after the section.
pub(crate) fn write_section(input: &StitchPattern, w: &mut ByteWriter) -> Result<Vec<u8>> {
    let pattern = prepare(input);
    let bounds = pattern.bounds();
    let color_indexes = pec_color_indexes(if pattern.color_block_count() == 0 {
        &[]
    } else {
        &pattern.threads
    });

    // Header
    let section_start = w.len();
    w.ascii("LA:");
    let label = pattern
        .metadata
        .get(LABEL_KEY)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| pattern.name())
        .unwrap_or("Untitled");
    w.append(&label_bytes(label));
    w.ascii("\r");
    w.fill(0x20, 12);
    w.append(&[0xFF, 0x00]);
    w.u8(ICON_STRIDE as i64);
    w.u8(ICON_HEIGHT as i64);
    if color_indexes.is_empty() {
        w.append(&[
            0x20, 0x20, 0x20, 0x20, 0x64, 0x20, 0x00, 0x20, 0x00, 0x20, 0x20, 0x20, 0xFF,
        ]);
    } else {
        if color_indexes.len() >= 256 {
            return Err(FormatError::ValueOutOfRange(format!(
                "{} colour blocks; PEC allows 255",
                color_indexes.len()
            )));
        }
        w.fill(0x20, 12);
        w.u8(color_indexes.len() as i64 - 1);
        for &i in &color_indexes {
            w.u8(i as i64);
        }
    }
    w.fill(0x20, 463_usize.saturating_sub(color_indexes.len()));
    debug_assert_eq!(w.len() - section_start, 512);

    // Stitch block
    let block_start = w.len();
    w.append(&[0x00, 0x00]);
    w.u24le(0);
    w.append(&[0x31, 0xFF, 0xF0]);
    w.u16le(round_coordinate(bounds.width()));
    w.u16le(round_coordinate(bounds.height()));
    w.u16le(0x1E0);
    w.u16le(0x1B0);
    encode_stitches(&pattern.stitches, w);
    if w.len() - block_start > 0xFF_FFFF {
        return Err(FormatError::ValueOutOfRange(format!(
            "PEC stitch block of {} bytes exceeds its 24-bit length field",
            w.len() - block_start
        )));
    }
    w.patch24le(block_start + 2, (w.len() - block_start) as i64);

    // Icons
    let sewn = |s: &[Stitch]| {
        s.iter()
            .copied()
            .filter(|s| s.command == Command::Stitch)
            .collect::<Vec<_>>()
    };
    w.append(&render_icon(&sewn(&pattern.stitches), &bounds, 4.0));
    for (_, block) in pattern.color_blocks() {
        w.append(&render_icon(&sewn(&block), &bounds, 5.0));
    }
    let mut list = Vec::with_capacity(color_indexes.len() + 1);
    if !color_indexes.is_empty() {
        list.push((color_indexes.len() - 1) as u8);
        list.extend(color_indexes.iter().map(|&i| i as u8));
    }
    Ok(list)
}

/// Maps each colour block's thread to a PEC chart index. Each distinct colour first claims the
/// nearest still-free chart entry (first-appearance order), then every block takes the nearest
/// claimed entry, so distinct design colours stay distinct on the machine.
pub fn pec_color_indexes(threads: &[Thread]) -> Vec<usize> {
    let mut free: Vec<Option<Thread>> = PEC_CHART.to_vec();
    let mut claimed: Vec<Option<Thread>> = vec![None; free.len()];
    let mut seen: Vec<u32> = Vec::new();
    for t in threads {
        if seen.contains(&t.rgb_value()) {
            continue;
        }
        seen.push(t.rgb_value());
        let Some(i) = nearest_index(t, &free) else {
            break;
        };
        free[i] = None;
        claimed[i] = Some(t.clone());
    }
    threads
        .iter()
        .map(|t| nearest_index(t, &claimed).unwrap_or(20))
        .collect()
}

fn encode_stitches(stitches: &[Stitch], w: &mut ByteWriter) {
    let (mut xx, mut yy) = (0_i64, 0_i64);
    let mut jumping = true;
    let mut pending_trim = false;
    let mut color_two = true;
    let mut first = true;
    let value = |w: &mut ByteWriter, v: i64, long: bool, flag: u8| {
        if !long && -64 < v && v < 63 {
            w.u8(v & 0x7F);
        } else {
            let code = (v & 0x0FFF) | 0x8000 | i64::from(flag) << 8;
            w.u8(code >> 8);
            w.u8(code & 0xFF);
        }
    };
    for s in stitches {
        let dx = round_coordinate(s.x) - xx;
        let dy = round_coordinate(s.y) - yy;
        match s.command {
            Command::Stitch => {
                xx += dx;
                yy += dy;
                if jumping {
                    if pending_trim {
                        value(w, 0, true, FLAG_TRIM);
                        value(w, 0, true, FLAG_TRIM);
                        pending_trim = false;
                    }
                    // pyembroidery pads a diagonal first stitch after a jump with a `00 00`
                    // record. PE-DESIGN 11 never writes one (its files go straight from the jump
                    // to the stitch), so neither does OpenStitch. A file that has the padding
                    // still rewrites byte for byte: it reads as a zero-length stitch.
                    jumping = false;
                }
                value(w, dx, false, 0);
                value(w, dy, false, 0);
            }
            Command::Jump => {
                xx += dx;
                yy += dy;
                jumping = true;
                let flag = if pending_trim { FLAG_TRIM } else { FLAG_JUMP };
                pending_trim = false;
                value(w, dx, true, flag);
                value(w, dy, true, flag);
            }
            Command::Trim => {
                if !first {
                    pending_trim = true;
                }
            }
            Command::ColorChange | Command::Stop => {
                if jumping {
                    value(w, 0, false, 0);
                    value(w, 0, false, 0);
                    jumping = false;
                }
                pending_trim = false;
                w.append(&[0xFE, 0xB0, if color_two { 0x02 } else { 0x01 }]);
                color_two = !color_two;
            }
            Command::End => {
                w.u8(0xFF);
                return;
            }
        }
        first = false;
    }
    w.u8(0xFF);
}

// MARK: Icons

/// Empty 48×38 1-bit PEC icon with its rounded frame; 6 bytes per row.
/// Generated from pyembroidery 1.5.1 (MIT License, see THIRD_PARTY_NOTICES.md).
pub fn blank_icon() -> Vec<u8> {
    let mut icon = Vec::with_capacity(ICON_STRIDE * ICON_HEIGHT);
    let rows: [[u8; 6]; 5] = [
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        [0xF0, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F],
        [0x08, 0x00, 0x00, 0x00, 0x00, 0x10],
        [0x04, 0x00, 0x00, 0x00, 0x00, 0x20],
        [0x02, 0x00, 0x00, 0x00, 0x00, 0x40],
    ];
    for r in [0, 1, 2, 3] {
        icon.extend_from_slice(&rows[r]);
    }
    for _ in 0..30 {
        icon.extend_from_slice(&rows[4]);
    }
    for r in [3, 2, 1, 0] {
        icon.extend_from_slice(&rows[r]);
    }
    icon
}

/// Renders stitch points into a 48×38 monochrome preview icon.
pub fn render_icon(points: &[Stitch], bounds: &Bounds, margin: f64) -> Vec<u8> {
    let mut icon = blank_icon();
    let width = (ICON_STRIDE * 8) as f64;
    let height = (icon.len() / ICON_STRIDE) as f64;
    let dw = if bounds.width() == 0.0 {
        1.0
    } else {
        bounds.width()
    };
    let dh = if bounds.height() == 0.0 {
        1.0
    } else {
        bounds.height()
    };
    let scale = ((width - margin) / dw).min((height - margin) / dh);
    let tx = -(bounds.max_x + bounds.min_x) / 2.0 * scale + width / 2.0;
    let ty = -(bounds.max_y + bounds.min_y) / 2.0 * scale + height / 2.0;
    for p in points {
        let x = (p.x * scale + tx).floor() as i64;
        let y = (p.y * scale + ty).floor() as i64;
        if x < 0 || y < 0 {
            continue;
        }
        let index = (y as usize)
            .saturating_mul(ICON_STRIDE)
            .saturating_add(x as usize / 8);
        if let Some(byte) = icon.get_mut(index) {
            *byte |= 1 << (x % 8);
        }
    }
    icon
}

/// ASCII rendering for debugging and CLI previews.
pub fn icon_ascii(icon: &[u8]) -> String {
    icon.chunks(ICON_STRIDE)
        .map(|row| {
            (0..row.len() * 8)
                .map(|i| {
                    if row[i / 8] >> (i % 8) & 1 == 1 {
                        '#'
                    } else {
                        ' '
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
