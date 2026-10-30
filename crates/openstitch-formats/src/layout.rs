//! How PE-DESIGN 11.50 lays a stitch stream out in the machine formats it exports, measured on
//! its exports of one OpenStitch design (`compat/reference/machine/machine-formats.*`: two
//! running lines with a trimmed frame move between them, a colour change, and 15 mm satin
//! stitches that every relative format has to split). The writers in this crate reproduce
//! those exports record for record; what each format does with the pieces below is in the
//! writer's own module and in `docs/FILE_FORMATS.md`.
//!
//! Shared observations:
//!
//! * The design is centred: the stream starts at the centre of the stitch bounds, which the
//!   relative formats take as (0, 0).
//! * A stitch longer than a record holds is never written as two stitches (except in HUS): the
//!   full-size pieces are jumps and only the remainder is a stitch, so the needle goes down once.
//! * A frame move (a run of jumps in the stream) is one move to the next needle point; several
//!   formats fold its remainder into the first stitch after it.
//! * The first move of the design is treated as a trimmed move, like every interior move.

use openstitch_core::{Command, StitchPattern, round_coordinate};

/// One step of a stream, with absolute 0.1 mm coordinates (+y down) after centring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// A frame move to `(x, y)`: every run of jumps collapsed to its last position, a trim
    /// without a following jump as a zero-length move. `trimmed` is set for the first move of
    /// the design and for moves preceded by a trim.
    Move {
        x: i64,
        y: i64,
        trimmed: bool,
    },
    Stitch {
        x: i64,
        y: i64,
    },
    ColorChange,
    Stop,
    End,
}

/// The pattern as events, centred so that the middle of its needle points' bounds is (0, 0).
/// An odd extent puts the origin half a unit towards the minimum, as PE-DESIGN 11.50 does
/// (`floor((min + max) / 2)`): its DST of `machine/sample-basic` spans x −134..135 and
/// y −194..195 for 269 × 389 units; the even-sized references are unaffected (#135).
pub(crate) fn events(pattern: &StitchPattern) -> Vec<Event> {
    let mut needle = pattern
        .stitches
        .iter()
        .filter(|s| s.command == Command::Stitch);
    let (cx, cy) = match needle.next() {
        Some(first) => {
            let b = needle.fold((first.x, first.y, first.x, first.y), |b, s| {
                (b.0.min(s.x), b.1.min(s.y), b.2.max(s.x), b.3.max(s.y))
            });
            (
                (round_coordinate(b.0) + round_coordinate(b.2)).div_euclid(2),
                (round_coordinate(b.1) + round_coordinate(b.3)).div_euclid(2),
            )
        }
        None => (0, 0),
    };
    let mut out: Vec<Event> = Vec::with_capacity(pattern.stitches.len());
    let mut first_move = true;
    let mut trim_pending = false;
    for s in &pattern.stitches {
        let (x, y) = (round_coordinate(s.x) - cx, round_coordinate(s.y) - cy);
        match s.command {
            Command::Jump => {
                let trimmed = trim_pending || first_move;
                first_move = false;
                trim_pending = false;
                if let Some(Event::Move {
                    x: mx,
                    y: my,
                    trimmed: t,
                }) = out.last_mut()
                {
                    *mx = x;
                    *my = y;
                    *t |= trimmed;
                } else {
                    out.push(Event::Move { x, y, trimmed });
                }
            }
            Command::Trim => trim_pending = true,
            Command::Stitch => {
                if trim_pending {
                    out.push(Event::Move {
                        x: position(&out),
                        y: position_y(&out),
                        trimmed: true,
                    });
                    trim_pending = false;
                }
                if first_move {
                    // A stream that starts with a stitch: the move to it is the first move.
                    out.push(Event::Move {
                        x,
                        y,
                        trimmed: true,
                    });
                    first_move = false;
                }
                out.push(Event::Stitch { x, y });
            }
            Command::ColorChange => {
                trim_pending = false;
                out.push(Event::ColorChange);
            }
            Command::Stop => {
                trim_pending = false;
                out.push(Event::Stop);
            }
            Command::End => {
                out.push(Event::End);
                break;
            }
        }
    }
    if out.last() != Some(&Event::End) {
        out.push(Event::End);
    }
    out
}

/// Whether the stream's first stitch or jump is a stitch: then [`events`] starts with a move it
/// made up to that stitch, and the stitch at the move's end is that same needle point, not a
/// landing stitch of its own.
pub(crate) fn starts_with_stitch(pattern: &StitchPattern) -> bool {
    pattern
        .stitches
        .iter()
        .find(|s| matches!(s.command, Command::Stitch | Command::Jump))
        .is_some_and(|s| s.command == Command::Stitch)
}

fn position(events: &[Event]) -> i64 {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Move { x, .. } | Event::Stitch { x, .. } => Some(*x),
            _ => None,
        })
        .unwrap_or(0)
}

fn position_y(events: &[Event]) -> i64 {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Move { y, .. } | Event::Stitch { y, .. } => Some(*y),
            _ => None,
        })
        .unwrap_or(0)
}

/// The next stitch after `i` when it comes before any other event, as `(x, y)`.
pub(crate) fn next_stitch(events: &[Event], i: usize) -> Option<(i64, i64)> {
    match events.get(i + 1) {
        Some(Event::Stitch { x, y }) => Some((*x, *y)),
        _ => None,
    }
}

/// Splits a move so that no piece exceeds `max` on either axis: every piece but the last takes
/// `max` along the longer axis and the proportional share (truncated) of the other; the last
/// piece is the remainder. A move within `max` is one piece; a zero move is one zero piece.
/// PE-DESIGN's DST, EXP and HUS splitting (e.g. (−200, 150) at 121 is (−121, 90), (−79, 60)).
pub(crate) fn split_proportional(dx: i64, dy: i64, max: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let (mut rx, mut ry) = (dx, dy);
    while rx.abs() > max || ry.abs() > max {
        let (px, py) = if rx.abs() >= ry.abs() {
            let px = max * rx.signum();
            (px, ry * px / rx)
        } else {
            let py = max * ry.signum();
            (rx * py / ry, py)
        };
        out.push((px, py));
        rx -= px;
        ry -= py;
    }
    out.push((rx, ry));
    out
}

/// Splits a move with each axis clamped to ±`max` independently: (−203, 150) at 127 is
/// (−127, 127), (−76, 23). PE-DESIGN's JEF, SEW and XXX splitting.
pub(crate) fn split_clamped(dx: i64, dy: i64, max: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let (mut rx, mut ry) = (dx, dy);
    while rx.abs() > max || ry.abs() > max {
        let (px, py) = (rx.clamp(-max, max), ry.clamp(-max, max));
        out.push((px, py));
        rx -= px;
        ry -= py;
    }
    out.push((rx, ry));
    out
}

/// Splits a move so that only the axes over `max` move in the leading pieces, by `max` each,
/// and the last piece takes everything else: (−5, −150) at 127 is (0, −127), (−5, −23) and
/// (−203, 150) is (−127, 127), (−76, 23). PE-DESIGN's JEF and SEW splitting.
pub(crate) fn split_overflow(dx: i64, dy: i64, max: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let (mut rx, mut ry) = (dx, dy);
    while rx.abs() > max || ry.abs() > max {
        let px = if rx.abs() > max { max * rx.signum() } else { 0 };
        let py = if ry.abs() > max { max * ry.signum() } else { 0 };
        out.push((px, py));
        rx -= px;
        ry -= py;
    }
    out.push((rx, ry));
    out
}

/// Splits a move into `n = ceil(longer axis / max)` pieces: `n − 1` equal pieces of
/// `(dx / n, dy / n)` (truncated) and a last piece with the remainder. PE-DESIGN's VIP
/// splitting ((−197, 200) at 75 is (−65, 66), (−65, 66), (−67, 68)).
pub(crate) fn split_even(dx: i64, dy: i64, max: i64) -> Vec<(i64, i64)> {
    let n = (dx.abs().max(dy.abs()) + max - 1) / max;
    if n <= 1 {
        return vec![(dx, dy)];
    }
    let (sx, sy) = (dx / n, dy / n);
    let mut out = vec![(sx, sy); (n - 1) as usize];
    out.push((dx - sx * (n - 1), dy - sy * (n - 1)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_split_takes_the_maximum_along_the_longer_axis() {
        assert_eq!(split_proportional(-200, 150, 121), [(-121, 90), (-79, 60)]);
        assert_eq!(split_proportional(-5, -150, 120), [(-4, -120), (-1, -30)]);
        assert_eq!(
            split_proportional(-400, -400, 80),
            [(-80, -80), (-80, -80), (-80, -80), (-80, -80), (-80, -80)]
        );
        assert_eq!(split_proportional(30, -40, 127), [(30, -40)]);
        assert_eq!(split_proportional(0, 0, 127), [(0, 0)]);
    }

    #[test]
    fn clamped_split_clamps_each_axis_on_its_own() {
        assert_eq!(split_clamped(-203, 150, 127), [(-127, 127), (-76, 23)]);
        assert_eq!(split_clamped(-203, 150, 120), [(-120, 120), (-83, 30)]);
        assert_eq!(split_clamped(0, 150, 127), [(0, 127), (0, 23)]);
    }

    #[test]
    fn overflow_split_moves_only_the_axes_over_the_limit_first() {
        assert_eq!(split_overflow(-5, -150, 127), [(0, -127), (-5, -23)]);
        assert_eq!(split_overflow(-203, 150, 127), [(-127, 127), (-76, 23)]);
        assert_eq!(split_overflow(-197, 200, 127), [(-127, 127), (-70, 73)]);
        assert_eq!(split_overflow(300, 0, 127), [(127, 0), (127, 0), (46, 0)]);
        assert_eq!(split_overflow(30, -40, 127), [(30, -40)]);
    }

    #[test]
    fn even_split_truncates_equal_pieces_and_keeps_the_remainder_last() {
        assert_eq!(split_even(-197, 200, 75), [(-65, 66), (-65, 66), (-67, 68)]);
        assert_eq!(
            split_even(-397, -400, 75),
            [
                (-66, -66),
                (-66, -66),
                (-66, -66),
                (-66, -66),
                (-66, -66),
                (-67, -70)
            ]
        );
        assert_eq!(split_even(-203, 150, 75), [(-67, 50), (-67, 50), (-69, 50)]);
        assert_eq!(split_even(2, -150, 75), [(1, -75), (1, -75)]);
        assert_eq!(split_even(40, 10, 75), [(40, 10)]);
    }

    #[test]
    fn events_centre_the_design_collapse_jump_runs_and_mark_trimmed_moves() {
        let mut p = StitchPattern::new();
        p.jump_to(100.0, 100.0); // jumps do not move the centre: needle points do
        p.jump_to(200.0, 200.0);
        p.stitch_to(200.0, 200.0);
        p.stitch_to(203.0, 200.0);
        p.push(Command::Trim);
        p.jump_to(200.0, 600.0);
        p.stitch_to(200.0, 600.0);
        p.push(Command::ColorChange);
        p.jump_to(600.0, 600.0);
        p.stitch_to(600.0, 590.0);
        p.push(Command::End);
        assert_eq!(
            events(&p),
            [
                Event::Move {
                    x: -200,
                    y: -200,
                    trimmed: true
                },
                Event::Stitch { x: -200, y: -200 },
                Event::Stitch { x: -197, y: -200 },
                Event::Move {
                    x: -200,
                    y: 200,
                    trimmed: true
                },
                Event::Stitch { x: -200, y: 200 },
                Event::ColorChange,
                Event::Move {
                    x: 200,
                    y: 200,
                    trimmed: false
                },
                Event::Stitch { x: 200, y: 190 },
                Event::End,
            ]
        );
    }

    #[test]
    fn a_stream_that_starts_with_a_stitch_gets_a_first_move_to_it() {
        let mut p = StitchPattern::new();
        p.stitch_to(10.0, 10.0);
        p.stitch_to(30.0, 10.0);
        assert_eq!(
            events(&p),
            [
                Event::Move {
                    x: -10,
                    y: 0,
                    trimmed: true
                },
                Event::Stitch { x: -10, y: 0 },
                Event::Stitch { x: 10, y: 0 },
                Event::End,
            ]
        );
    }
}
