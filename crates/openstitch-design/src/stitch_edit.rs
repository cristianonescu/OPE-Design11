//! Editing stitch patterns (PE-DESIGN guide, "Editing a Stitch Pattern"): needle points are
//! the `Stitch` and `Jump` entries of a [`StitchBlock`], addressed by their index in
//! `stitches`. Moving, inserting and deleting points, splitting at a point, connecting two
//! patterns, dividing by colour and splitting off an area. Trims and colour changes are not
//! points; colour is tracked through them.

use crate::geometry::{Vec2, v2};
use crate::model::StitchBlock;
use openstitch_core::{Command, Stitch, Thread};

/// Why an edit was refused (user-facing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError(pub String);

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(m: impl Into<String>) -> Result<T, EditError> {
    Err(EditError(m.into()))
}

/// Whether entry `c` is a needle point (has a position the user edits).
pub fn is_point(c: Command) -> bool {
    matches!(c, Command::Stitch | Command::Jump)
}

fn pos(s: &Stitch) -> Vec2 {
    v2(s.x, s.y)
}

fn check(b: &StitchBlock, i: usize) -> Result<(), EditError> {
    match b.stitches.get(i) {
        Some(s) if is_point(s.command) => Ok(()),
        Some(_) => err(format!(
            "entry {i} is a trim or colour change, not a needle point"
        )),
        None => err(format!(
            "no needle point {i} (the pattern has {} entries)",
            b.stitches.len()
        )),
    }
}

fn sews(b: &StitchBlock) -> bool {
    b.stitches.iter().any(|s| s.command == Command::Stitch)
}

/// Colour block index of every entry (colour changes count from the entry after them).
pub fn colors(b: &StitchBlock) -> Vec<usize> {
    let mut k = 0;
    b.stitches
        .iter()
        .map(|s| {
            let here = k;
            if s.command == Command::ColorChange {
                k += 1;
            }
            here
        })
        .collect()
}

fn thread_for(b: &StitchBlock, k: usize) -> Thread {
    b.threads
        .get(k)
        .or(b.threads.last())
        .cloned()
        .unwrap_or(Thread::rgb(0))
}

pub fn move_points(b: &mut StitchBlock, indexes: &[usize], d: Vec2) -> Result<(), EditError> {
    for &i in indexes {
        check(b, i)?;
    }
    let mut seen = indexes.to_vec();
    seen.sort_unstable();
    seen.dedup();
    for i in seen {
        let s = &mut b.stitches[i];
        s.x += d.x;
        s.y += d.y;
    }
    Ok(())
}

pub fn set_point(b: &mut StitchBlock, i: usize, p: Vec2) -> Result<(), EditError> {
    check(b, i)?;
    b.stitches[i].x = p.x;
    b.stitches[i].y = p.y;
    Ok(())
}

/// Inserts a needle point on the line into point `i` (from the previous needle point), at `at`
/// or the line's midpoint. With `jump`, the line up to the new point becomes a jump. Returns
/// the new point's index.
pub fn insert_point(
    b: &mut StitchBlock,
    i: usize,
    at: Option<Vec2>,
    jump: bool,
) -> Result<usize, EditError> {
    check(b, i)?;
    let Some(prev) = (0..i).rev().find(|&j| is_point(b.stitches[j].command)) else {
        return err("the first needle point has no line before it");
    };
    let p = at.unwrap_or_else(|| pos(&b.stitches[prev]).lerp(pos(&b.stitches[i]), 0.5));
    if jump {
        b.stitches.insert(i, Stitch::new(p.x, p.y, Command::Stitch));
        b.stitches.insert(i, Stitch::new(p.x, p.y, Command::Jump));
        Ok(i + 1)
    } else {
        b.stitches.insert(i, Stitch::new(p.x, p.y, Command::Stitch));
        Ok(i)
    }
}

/// Deletes needle points. Deleting the end of a jump removes the jump (and the trim before
/// it), so the stitches on either side are joined. At least one stitch must remain.
pub fn delete_points(b: &mut StitchBlock, indexes: &[usize]) -> Result<(), EditError> {
    for &i in indexes {
        check(b, i)?;
    }
    let mut remove = vec![false; b.stitches.len()];
    for &i in indexes {
        remove[i] = true;
        if b.stitches[i].command == Command::Jump {
            let mut j = i;
            while j > 0 && b.stitches[j - 1].command == Command::Trim {
                j -= 1;
                remove[j] = true;
            }
        }
    }
    let kept: Vec<Stitch> = b
        .stitches
        .iter()
        .zip(&remove)
        .filter(|(_, r)| !**r)
        .map(|(s, _)| *s)
        .collect();
    if !kept.iter().any(|s| s.command == Command::Stitch) {
        return err("that would delete every stitch; delete the object instead");
    }
    b.stitches = kept;
    Ok(())
}

/// The entries `keep` selects, re-linked: a stitch whose previous needle point was left out
/// starts with a jump, colour changes are re-inserted between colours, trims stay where their
/// point stays.
pub fn filter(b: &StitchBlock, keep: impl Fn(usize) -> bool) -> StitchBlock {
    let color = colors(b);
    let mut out = StitchBlock {
        stitches: Vec::new(),
        threads: Vec::new(),
    };
    let mut current: Option<usize> = None;
    let mut last_kept_point: Option<usize> = None;
    let mut prev_point: Option<usize> = None;
    for (i, s) in b.stitches.iter().enumerate() {
        match s.command {
            Command::Stitch | Command::Jump if keep(i) => {
                if current != Some(color[i]) {
                    if let Some(last) = out.stitches.last().copied() {
                        out.stitches
                            .push(Stitch::new(last.x, last.y, Command::ColorChange));
                    }
                    out.threads.push(thread_for(b, color[i]));
                    current = Some(color[i]);
                }
                let connected = last_kept_point.is_some() && last_kept_point == prev_point;
                if s.command == Command::Stitch && !connected {
                    out.stitches.push(Stitch::new(s.x, s.y, Command::Jump));
                }
                out.stitches.push(*s);
                last_kept_point = Some(i);
            }
            Command::Trim if last_kept_point.is_some() && last_kept_point == prev_point => {
                out.stitches.push(*s);
            }
            _ => {}
        }
        if is_point(s.command) {
            prev_point = Some(i);
        }
    }
    out
}

/// Splits at needle point `i`: the first part ends there, the second starts there. Not at the
/// pattern's first or last needle point.
pub fn split_at(b: &StitchBlock, i: usize) -> Result<(StitchBlock, StitchBlock), EditError> {
    check(b, i)?;
    let first = b.stitches.iter().position(|s| is_point(s.command));
    let last = b.stitches.iter().rposition(|s| is_point(s.command));
    if Some(i) == first || Some(i) == last {
        return err("cannot split at the entry or exit point");
    }
    let (a, c) = (filter(b, |j| j <= i), filter(b, |j| j >= i));
    if !sews(&a) || !sews(&c) {
        return err("both parts must keep at least one stitch");
    }
    Ok((a, c))
}

/// `b` sewn after `a` as one pattern: a jump from `a`'s exit to `b`'s entry; `b`'s first
/// colour block takes `a`'s last thread (the combined pattern keeps the first one's colour).
pub fn connect(a: &StitchBlock, b: &StitchBlock) -> StitchBlock {
    let mut out = a.clone();
    if out.threads.is_empty() {
        out.threads.push(thread_for(b, 0));
    }
    if let Some(first) = b.stitches.iter().find(|s| is_point(s.command)) {
        out.stitches
            .push(Stitch::new(first.x, first.y, Command::Jump));
    }
    out.stitches.extend(b.stitches.iter().copied());
    out.threads.extend(b.threads.iter().skip(1).cloned());
    out
}

/// One pattern per colour block that sews something, in sewing order.
pub fn divide_by_color(b: &StitchBlock) -> Vec<StitchBlock> {
    let color = colors(b);
    let n = color.last().map_or(0, |k| k + 1);
    (0..n)
        .map(|k| filter(b, |i| color[i] == k))
        .filter(sews)
        .collect()
}

/// Splits off the needle points `inside` selects: `(inside part, rest)`. Both must sew.
pub fn split_area(
    b: &StitchBlock,
    inside: impl Fn(Vec2) -> bool,
) -> Result<(StitchBlock, StitchBlock), EditError> {
    let flags: Vec<bool> = b.stitches.iter().map(|s| inside(pos(s))).collect();
    let (a, c) = (filter(b, |i| flags[i]), filter(b, |i| !flags[i]));
    if !sews(&a) {
        return err("no stitches inside the outline");
    }
    if !sews(&c) {
        return err("every stitch is inside the outline; nothing would be left");
    }
    Ok((a, c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(x: f64, c: Command) -> Stitch {
        Stitch::new(x, 0.0, c)
    }

    /// Red stitches 0-30, a jump, red 50-60, colour change, blue 70-80.
    fn sample() -> StitchBlock {
        use Command::*;
        StitchBlock {
            stitches: vec![
                st(0.0, Jump),
                st(0.0, Stitch),
                st(10.0, Stitch),
                st(20.0, Stitch),
                st(30.0, Stitch),
                st(30.0, Trim),
                st(50.0, Jump),
                st(50.0, Stitch),
                st(60.0, Stitch),
                st(60.0, ColorChange),
                st(70.0, Jump),
                st(70.0, Stitch),
                st(80.0, Stitch),
            ],
            threads: vec![Thread::rgb(0xff0000), Thread::rgb(0x0000ff)],
        }
    }

    fn xs(b: &StitchBlock) -> Vec<(f64, Command)> {
        b.stitches.iter().map(|s| (s.x, s.command)).collect()
    }

    #[test]
    fn move_insert_delete() {
        let mut b = sample();
        move_points(&mut b, &[2, 3, 3], v2(1.0, 2.0)).expect("moves");
        assert_eq!((b.stitches[2].x, b.stitches[2].y), (11.0, 2.0));
        assert_eq!(b.stitches[3].x, 21.0, "duplicates move once");
        assert!(
            move_points(&mut b, &[5], v2(1.0, 0.0)).is_err(),
            "trim is not a point"
        );
        assert!(move_points(&mut b, &[99], v2(1.0, 0.0)).is_err());

        let mut b = sample();
        assert_eq!(insert_point(&mut b, 2, None, false), Ok(2));
        assert_eq!(b.stitches[2].x, 5.0, "midpoint");
        let i = insert_point(&mut b, 4, Some(v2(12.0, 5.0)), true).expect("jump insert");
        assert_eq!(b.stitches[i - 1].command, Command::Jump);
        assert_eq!(b.stitches[i].command, Command::Stitch);
        assert!(
            insert_point(&mut b, 0, None, false).is_err(),
            "no line before the first point"
        );

        let mut b = sample();
        delete_points(&mut b, &[6]).expect("deletes the jump end");
        // The trim before the jump goes too: 30 is now sewn straight to 50.
        assert_eq!(
            xs(&b)[4..7],
            [
                (30.0, Command::Stitch),
                (50.0, Command::Stitch),
                (60.0, Command::Stitch)
            ]
        );
        let mut b = sample();
        let all: Vec<usize> = (0..b.stitches.len())
            .filter(|&i| is_point(b.stitches[i].command))
            .collect();
        assert!(delete_points(&mut b, &all).is_err());
        assert_eq!(b, sample(), "refused edits change nothing");
    }

    #[test]
    fn split_connect_divide() {
        let b = sample();
        assert!(split_at(&b, 0).is_err());
        assert!(split_at(&b, 12).is_err());
        let (a, c) = split_at(&b, 3).expect("splits");
        assert_eq!(a.stitches.last().map(|s| s.x), Some(20.0));
        assert_eq!(a.threads.len(), 1);
        assert_eq!(
            xs(&c)[..2],
            [(20.0, Command::Jump), (20.0, Command::Stitch)]
        );
        assert_eq!(c.threads.len(), 2);

        let joined = connect(&a, &c);
        assert_eq!(joined.threads.len(), 2, "c's first colour merges into a's");
        assert_eq!(
            joined
                .stitches
                .iter()
                .filter(|s| s.command == Command::Stitch)
                .count(),
            a.stitches
                .iter()
                .chain(&c.stitches)
                .filter(|s| s.command == Command::Stitch)
                .count()
        );

        let parts = divide_by_color(&b);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].threads, vec![Thread::rgb(0xff0000)]);
        assert_eq!(parts[1].threads, vec![Thread::rgb(0x0000ff)]);
        assert!(parts[1].stitches.iter().all(|s| s.x >= 70.0));
        assert!(
            !parts[0]
                .stitches
                .iter()
                .any(|s| s.command == Command::ColorChange)
        );
    }

    #[test]
    fn split_area_relinks_with_jumps() {
        let b = sample();
        let (inside, rest) = split_area(&b, |p| (15.0..=55.0).contains(&p.x)).expect("splits");
        assert_eq!(
            xs(&inside),
            [
                (20.0, Command::Jump),
                (20.0, Command::Stitch),
                (30.0, Command::Stitch),
                (30.0, Command::Trim),
                (50.0, Command::Jump),
                (50.0, Command::Stitch),
            ]
        );
        // The rest: 0-10, then 60 (its line from 50 is gone, so it starts with a jump), blue.
        assert_eq!(rest.threads.len(), 2);
        let starts: Vec<f64> = rest
            .stitches
            .windows(2)
            .filter(|w| w[0].command == Command::Jump && w[1].command == Command::Stitch)
            .map(|w| w[1].x)
            .collect();
        assert_eq!(starts, vec![0.0, 60.0, 70.0]);
        assert!(split_area(&b, |_| false).is_err());
        assert!(split_area(&b, |_| true).is_err());
    }
}
