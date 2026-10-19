//! Splitting a design that exceeds a format's limits into several files, as PE-DESIGN does when
//! a design is too large for one file. The limits are those the writers enforce:
//!
//! | Format | Limit |
//! |---|---|
//! | PES, PEC | 255 colour blocks; 16 MB stitch block |
//! | DST | 999 colour changes; 9,999,999 records |
//! | PCS | 65,535 records |
//! | SEW | 3,771 colours (the header is 0x1D78 bytes) |
//! | VIP | 100 colours |
//!
//! A design is cut at colour changes when it can be, so each file holds whole colour blocks.
//! A single block too large for one file is cut between stitches. Every part after the first
//! starts with a jump to its first needle position, and coordinates stay absolute, so the parts
//! line up when sewn one after another in the same hoop position.

use crate::{FileType, FormatError, Result};
use openstitch_core::{Command, Stitch, StitchPattern, Thread};

/// One colour block: its thread and its records (no colour change, no end).
struct Block {
    thread: Option<Thread>,
    stitches: Vec<Stitch>,
}

fn blocks(pattern: &StitchPattern) -> Vec<Block> {
    let mut out = vec![Block {
        thread: pattern.threads.first().cloned(),
        stitches: Vec::new(),
    }];
    for s in &pattern.stitches {
        match s.command {
            Command::ColorChange => out.push(Block {
                thread: pattern.threads.get(out.len()).cloned(),
                stitches: Vec::new(),
            }),
            Command::End => {}
            _ => out
                .last_mut()
                .expect("at least one block")
                .stitches
                .push(*s),
        }
    }
    out.retain(|b| b.stitches.iter().any(|s| s.command == Command::Stitch));
    out
}

/// A part made of `blocks`; `lead_in` prepends a jump to the first position.
fn assemble(
    source: &StitchPattern,
    blocks: &[&[Stitch]],
    threads: &[Option<Thread>],
    lead_in: bool,
) -> StitchPattern {
    let mut p = StitchPattern {
        metadata: source.metadata.clone(),
        ..StitchPattern::new()
    };
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            let at = p.stitches.last().copied().unwrap_or(block[0]);
            p.stitches
                .push(Stitch::new(at.x, at.y, Command::ColorChange));
        }
        if i == 0
            && lead_in
            && let Some(first) = block.first()
        {
            p.stitches
                .push(Stitch::new(first.x, first.y, Command::Jump));
        }
        p.stitches.extend_from_slice(block);
    }
    let (x, y) = p.stitches.last().map_or((0.0, 0.0), |s| (s.x, s.y));
    p.stitches.push(Stitch::new(x, y, Command::End));
    p.threads = threads.iter().flatten().cloned().collect();
    p
}

fn fits(ty: FileType, p: &StitchPattern) -> Result<bool> {
    match ty.write(p) {
        Ok(_) => Ok(true),
        Err(FormatError::ValueOutOfRange(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Largest `n` in `1..=max` with `ok(n)`, assuming `ok` holds up to some point and fails after;
/// `None` if `ok(1)` fails. Doubling then bisection keeps the number of trial writes small.
fn largest(max: usize, mut ok: impl FnMut(usize) -> Result<bool>) -> Result<Option<usize>> {
    if max == 0 || !ok(1)? {
        return Ok(None);
    }
    let (mut good, mut step) = (1, 1);
    while good < max {
        let next = (good + step).min(max);
        if ok(next)? {
            good = next;
            step *= 2;
        } else {
            let (mut lo, mut hi) = (good, next); // ok(lo), !ok(hi)
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if ok(mid)? {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Ok(Some(lo));
        }
    }
    Ok(Some(good))
}

/// `pattern` as one or more patterns that `ty` can each write. Returns the pattern itself when
/// it fits; fails when the format cannot write it for a reason splitting does not fix.
pub fn split_for(ty: FileType, pattern: &StitchPattern) -> Result<Vec<StitchPattern>> {
    if fits(ty, pattern)? {
        return Ok(vec![pattern.clone()]);
    }
    let blocks = blocks(pattern);
    let mut parts = Vec::new();
    let mut i = 0;
    // A block cut between stitches continues from `offset` in the next part.
    let mut offset = 0;
    while i < blocks.len() {
        let lead_in = !parts.is_empty();
        let make = |n: usize| {
            let slices: Vec<&[Stitch]> = (i..i + n)
                .map(|k| {
                    if k == i {
                        &blocks[k].stitches[offset..]
                    } else {
                        &blocks[k].stitches[..]
                    }
                })
                .collect();
            let threads: Vec<Option<Thread>> =
                (i..i + n).map(|k| blocks[k].thread.clone()).collect();
            assemble(pattern, &slices, &threads, lead_in)
        };
        if let Some(n) = largest(blocks.len() - i, |n| fits(ty, &make(n)))? {
            parts.push(make(n));
            i += n;
            offset = 0;
            continue;
        }
        // Block `i` alone is too large: take as many of its records as fit.
        let rest = &blocks[i].stitches[offset..];
        let thread = [blocks[i].thread.clone()];
        let part = |n: usize| assemble(pattern, &[&rest[..n]], &thread, lead_in);
        let Some(n) = largest(rest.len(), |n| fits(ty, &part(n)))? else {
            return Err(FormatError::ValueOutOfRange(format!(
                "{ty}: even a single record does not fit"
            )));
        };
        parts.push(part(n));
        offset += n;
        if offset >= blocks[i].stitches.len() {
            i += 1;
            offset = 0;
        }
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dst, pcs, pec, vip};

    fn many_colours(blocks: usize, stitches: usize) -> StitchPattern {
        let mut p = StitchPattern::new();
        for b in 0..blocks {
            if b > 0 {
                p.push(Command::ColorChange);
            }
            p.threads
                .push(Thread::rgb((b as u32 * 0x01_0203) & 0xFF_FFFF));
            for k in 0..stitches {
                p.stitch_to(
                    (b % 50) as f64 * 10.0 + (k % 2) as f64 * 5.0,
                    (b / 50) as f64 * 10.0 + k as f64,
                );
            }
        }
        p.push(Command::End);
        p
    }

    fn sewn(parts: &[StitchPattern]) -> Vec<(f64, f64)> {
        parts
            .iter()
            .flat_map(|p| p.stitches.iter())
            .filter(|s| s.command == Command::Stitch)
            .map(|s| (s.x, s.y))
            .collect()
    }

    #[test]
    fn fitting_designs_are_not_split() {
        let p = many_colours(3, 10);
        assert_eq!(split_for(FileType::Pes, &p).expect("split"), vec![p]);
    }

    #[test]
    fn colour_limits_split_at_colour_changes() {
        let p = many_colours(300, 4);
        assert!(matches!(
            pec::write(&p),
            Err(FormatError::ValueOutOfRange(_))
        ));
        for (ty, max_blocks) in [(FileType::Pes, 255), (FileType::Vip, vip::MAX_COLORS)] {
            let parts = split_for(ty, &p).expect("split");
            assert_eq!(parts.len(), 300_usize.div_ceil(max_blocks), "{ty}");
            assert_eq!(parts[0].color_block_count(), max_blocks, "{ty}");
            assert!(parts.iter().all(|q| ty.write(q).is_ok()));
            assert_eq!(
                sewn(&parts),
                sewn(std::slice::from_ref(&p)),
                "{ty}: every stitch, in order"
            );
            let threads: Vec<Thread> = parts.iter().flat_map(|q| q.threads.clone()).collect();
            assert_eq!(threads, p.threads);
            // Later parts open with a jump to their first stitch.
            assert_eq!(parts[1].stitches[0].command, Command::Jump);
        }
        let changes = split_for(FileType::Dst, &many_colours(1500, 2)).expect("dst");
        assert_eq!(changes.len(), 2);
        assert!(changes[0].count(Command::ColorChange) <= dst::MAX_COLOR_CHANGES);
    }

    #[test]
    fn a_block_too_large_for_one_file_is_cut_between_stitches() {
        let p = many_colours(2, 70_000); // each block alone exceeds 65,535 PCS records
        assert!(pcs::write(&p).is_err());
        let parts = split_for(FileType::Pcs, &p).expect("split");
        assert!(parts.len() >= 2);
        assert!(parts.iter().all(|q| pcs::write(q).is_ok()));
        assert_eq!(sewn(&parts), sewn(&[p]));
    }
}
