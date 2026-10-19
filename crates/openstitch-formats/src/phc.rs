//! Brother PHC (read only): a PE-DESIGN / Brother card format wrapping PEC stitch data.
//! Little-endian. 0x2B: u8 size of the block before the stitch header; 0x30: u16 offset of that
//! block; 0x4A: icon height, 0x4C: icon byte stride, 0x4D: u16 colour count and one PEC chart
//! index per colour, followed by the icons. At offset + size: a u16-sized section and a
//! u32-sized section (bounds), 10 bytes, a u8 colour count `n` and `n + 0x1D` bytes, then a
//! PEC-encoded stitch stream.
//!
//! Follows pyembroidery 1.5.1's `PhcReader` (MIT), the oracle; libembroidery's `readPhc`
//! (zlib) agrees. No public writer exists and PE-DESIGN cannot convert to PHC, so OpenStitch
//! does not write it. Consecutive blocks with the same thread become stops, as in the reference.

use crate::io::ByteReader;
use crate::{FormatError, Result, pec};
use openstitch_core::{Command, PEC_CHART, StitchPattern, Thread};

pub fn read(data: &[u8]) -> Result<StitchPattern> {
    if !data.starts_with(b"#PHC") {
        let n = data.len().min(8);
        return Err(FormatError::BadSignature(
            String::from_utf8_lossy(&data[..n]).into_owned(),
        ));
    }
    let mut r = ByteReader::new(data);
    let mut pattern = StitchPattern::new();
    r.offset = 0x4D;
    let colors = r.u16le()?;
    for &b in r.read(colors as usize)? {
        pattern.threads.push(
            PEC_CHART[usize::from(b) % PEC_CHART.len()]
                .clone()
                .unwrap_or_else(|| Thread::rgb(0)),
        );
    }
    r.offset = 0x2B;
    let pec_add = usize::from(r.u8()?);
    r.offset = 0x30;
    let pec_offset = r.u16le()? as usize;
    r.offset = pec_offset + pec_add;
    let first = r.u16le()? as usize;
    r.skip(first)?;
    let second = r.u32le()? as usize;
    r.skip(second)?;
    r.skip(10)?;
    let colors2 = usize::from(r.u8()?);
    r.skip(colors2 + 0x1D)?;
    pattern.stitches = pec::read_stitches(&mut r)?;
    duplicate_colors_as_stops(&mut pattern);
    Ok(pattern)
}

/// A colour change between two blocks with the same thread colour becomes a stop, and the
/// repeated thread is dropped (pyembroidery's `interpolate_duplicate_color_as_stop`).
pub(crate) fn duplicate_colors_as_stops(pattern: &mut StitchPattern) {
    let mut thread = 0;
    let mut new_block = true;
    let mut last_change: Option<usize> = None;
    for i in 0..pattern.stitches.len() {
        match pattern.stitches[i].command {
            Command::Stitch if new_block => {
                new_block = false;
                if let Some(at) = last_change
                    && thread != 0
                {
                    let (Some(prev), Some(cur)) =
                        (pattern.threads.get(thread - 1), pattern.threads.get(thread))
                    else {
                        return; // non-existent threads cannot repeat
                    };
                    if prev.rgb_value() == cur.rgb_value() {
                        pattern.threads.remove(thread);
                        pattern.stitches[at].command = Command::Stop;
                        continue;
                    }
                }
                thread += 1;
            }
            Command::ColorChange => {
                new_block = true;
                last_change = Some(i);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_core::Stitch;

    #[test]
    fn repeated_thread_becomes_a_stop() {
        let mut p = StitchPattern {
            threads: [0xFF0000, 0xFF0000, 0x0000FF]
                .into_iter()
                .map(Thread::rgb)
                .collect(),
            ..StitchPattern::new()
        };
        for c in [
            Command::Stitch,
            Command::ColorChange,
            Command::Stitch,
            Command::ColorChange,
            Command::Stitch,
            Command::End,
        ] {
            p.stitches.push(Stitch::new(0.0, 0.0, c));
        }
        duplicate_colors_as_stops(&mut p);
        assert_eq!(p.threads.len(), 2);
        assert_eq!(p.stitches[1].command, Command::Stop);
        assert_eq!(p.stitches[3].command, Command::ColorChange);
    }
}
