//! A stitch stream in millimetres, the unit every measurement works in, built from a machine
//! file's [`StitchPattern`] (0.1 mm) or from a `stitches.points` reply (mm).

use openstitch_core::{Command, StitchPattern};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One entry of a stitch stream: position in mm (+y down), command, colour block index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub x: f64,
    pub y: f64,
    pub command: Command,
    pub block: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    pub entries: Vec<Entry>,
}

/// Which point of a trace an alignment moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    /// Top-left of the bounds of every stitch and jump.
    BoundsMin,
    BoundsCenter,
    FirstStitch,
}

pub fn command_name(c: Command) -> &'static str {
    match c {
        Command::Stitch => "stitch",
        Command::Jump => "jump",
        Command::Trim => "trim",
        Command::ColorChange => "colorChange",
        Command::Stop => "stop",
        Command::End => "end",
    }
}

pub fn parse_command(s: &str) -> Option<Command> {
    Some(match s {
        "stitch" => Command::Stitch,
        "jump" => Command::Jump,
        "trim" => Command::Trim,
        "colorChange" => Command::ColorChange,
        "stop" => Command::Stop,
        "end" => Command::End,
        _ => return None,
    })
}

impl Trace {
    /// Entries of a machine-level pattern; colour changes start the next block and are kept.
    pub fn from_pattern(p: &StitchPattern) -> Self {
        let mut block = 0;
        let entries = p
            .stitches
            .iter()
            .map(|s| {
                let e = Entry {
                    x: s.x / 10.0,
                    y: s.y / 10.0,
                    command: s.command,
                    block,
                };
                if s.command == Command::ColorChange {
                    block += 1;
                }
                e
            })
            .collect();
        Trace { entries }
    }

    /// Entries of a `stitches.points` reply (`{"points": [{"x", "y", "command", "color"}]}`).
    pub fn from_points_reply(reply: &Value) -> Result<Self, String> {
        let points = reply
            .get("points")
            .and_then(Value::as_array)
            .ok_or("the step's reply has no `points` array (use stitches.points)")?;
        let mut entries = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            let num = |k: &str| {
                p.get(k)
                    .and_then(Value::as_f64)
                    .ok_or_else(|| format!("point {i}: no number `{k}`"))
            };
            let command = p
                .get("command")
                .and_then(Value::as_str)
                .and_then(parse_command)
                .ok_or_else(|| format!("point {i}: unknown command"))?;
            let block = p.get("color").and_then(Value::as_u64).unwrap_or(0) as usize;
            entries.push(Entry {
                x: num("x")?,
                y: num("y")?,
                command,
                block,
            });
        }
        Ok(Trace { entries })
    }

    /// Only the entries of colour block `block` (colour-change markers dropped).
    pub fn block(&self, block: usize) -> Self {
        Trace {
            entries: self
                .entries
                .iter()
                .filter(|e| e.block == block && e.command != Command::ColorChange)
                .copied()
                .collect(),
        }
    }

    pub fn block_count(&self) -> usize {
        if self.stitches().next().is_none() {
            return 0;
        }
        let mut blocks: Vec<usize> = self
            .entries
            .iter()
            .filter(|e| e.command == Command::Stitch)
            .map(|e| e.block)
            .collect();
        blocks.dedup();
        blocks.len()
    }

    pub fn stitches(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.command == Command::Stitch)
    }

    /// Bounds `(min_x, min_y, max_x, max_y)` of stitches and jumps, mm.
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut it = self
            .entries
            .iter()
            .filter(|e| matches!(e.command, Command::Stitch | Command::Jump));
        let f = it.next()?;
        Some(it.fold((f.x, f.y, f.x, f.y), |b, e| {
            (b.0.min(e.x), b.1.min(e.y), b.2.max(e.x), b.3.max(e.y))
        }))
    }

    /// The anchor's position, mm.
    pub fn anchor(&self, anchor: Anchor) -> Option<(f64, f64)> {
        match anchor {
            Anchor::BoundsMin => self.bounds().map(|b| (b.0, b.1)),
            Anchor::BoundsCenter => self
                .bounds()
                .map(|b| ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0)),
            Anchor::FirstStitch => self.stitches().next().map(|e| (e.x, e.y)),
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        for e in &mut self.entries {
            e.x += dx;
            e.y += dy;
        }
    }

    /// Moves the trace so that `anchor` lands on `to` (mm).
    pub fn align(&mut self, anchor: Anchor, to: (f64, f64)) {
        if let Some((x, y)) = self.anchor(anchor) {
            self.translate(to.0 - x, to.1 - y);
        }
    }

    /// Consecutive needle penetrations: pairs of entries where the second is a stitch and the
    /// first is a stitch or jump (the segment the thread lies along).
    pub fn segments(&self) -> impl Iterator<Item = (&Entry, &Entry)> {
        self.entries.windows(2).filter_map(|w| {
            (w[1].command == Command::Stitch
                && matches!(w[0].command, Command::Stitch | Command::Jump))
            .then_some((&w[0], &w[1]))
        })
    }

    /// The trace without its first `first` and last `last` segments (see [`Trace::segments`]):
    /// it starts at the first kept segment's first entry and ends at the last kept segment's
    /// last entry. Empty when nothing is kept.
    pub fn excluding(&self, first: usize, last: usize) -> Self {
        let ends: Vec<usize> = (1..self.entries.len())
            .filter(|&i| {
                self.entries[i].command == Command::Stitch
                    && matches!(self.entries[i - 1].command, Command::Stitch | Command::Jump)
            })
            .collect();
        if first + last >= ends.len() {
            return Trace::default();
        }
        let (from, to) = (ends[first] - 1, ends[ends.len() - 1 - last]);
        Trace {
            entries: self.entries[from..=to].to_vec(),
        }
    }

    /// Consecutive stitch points (stitch entries only, other entries skipped) per block.
    pub fn stitch_pairs(&self) -> Vec<(Entry, Entry)> {
        let pts: Vec<Entry> = self.stitches().copied().collect();
        pts.windows(2)
            .filter(|w| w[0].block == w[1].block)
            .map(|w| (w[0], w[1]))
            .collect()
    }
}

pub fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// Distance from `p` to the segment `a`–`b`.
pub fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let d = (b.0 - a.0, b.1 - a.1);
    let l2 = d.0 * d.0 + d.1 * d.1;
    if l2 == 0.0 {
        return distance(p, a);
    }
    let t = (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / l2).clamp(0.0, 1.0);
    distance(p, (a.0 + t * d.0, a.1 + t * d.1))
}

pub fn distance_to_polyline(p: (f64, f64), line: &[[f64; 2]], closed: bool) -> f64 {
    let n = line.len();
    if n == 1 {
        return distance(p, (line[0][0], line[0][1]));
    }
    let edges = if closed { n } else { n.saturating_sub(1) };
    (0..edges)
        .map(|i| {
            let (a, b) = (line[i], line[(i + 1) % n]);
            distance_to_segment(p, (a[0], a[1]), (b[0], b[1]))
        })
        .fold(f64::INFINITY, f64::min)
}

/// Even-odd point-in-polygon test.
pub fn inside(p: (f64, f64), poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > p.1) != (b[1] > p.1) && p.0 < a[0] + (p.1 - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        {
            c = !c;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn excluding_drops_whole_segments_at_both_ends() {
        // A jump, a tie-in (out 0.3 mm and back), three crossings, a lock (out and back).
        let mut p = StitchPattern::new();
        p.jump_to(100.0, 100.0);
        for (x, y) in [
            (100.0, 103.0),
            (100.0, 100.0),
            (100.0, 120.0),
            (107.0, 100.0),
            (107.0, 120.0),
            (107.0, 117.0),
            (107.0, 120.0),
        ] {
            p.stitch_to(x, y);
        }
        let t = Trace::from_pattern(&p);
        assert_eq!(t.segments().count(), 7);
        let body = t.excluding(2, 2);
        assert_eq!(body.segments().count(), 3);
        let pts: Vec<(f64, f64)> = body.entries.iter().map(|e| (e.x, e.y)).collect();
        assert_eq!(
            pts,
            [(10.0, 10.0), (10.0, 12.0), (10.7, 10.0), (10.7, 12.0)]
        );
        assert_eq!(t.excluding(0, 0), t);
        assert_eq!(t.excluding(4, 3), Trace::default());
    }

    #[test]
    fn traces_come_from_patterns_and_point_replies() {
        let mut p = StitchPattern::new();
        p.stitch_to(100.0, 100.0);
        p.push(Command::ColorChange);
        p.stitch_to(150.0, 120.0);
        let t = Trace::from_pattern(&p);
        assert_eq!(t.entries.len(), 3);
        assert_eq!(t.entries[2].block, 1);
        assert_eq!((t.entries[2].x, t.entries[2].y), (15.0, 12.0));
        assert_eq!(t.block_count(), 2);
        assert_eq!(t.block(1).entries.len(), 1);

        let r = json!({"points": [
            {"index": 0, "x": 10.0, "y": 10.0, "command": "jump", "color": 0},
            {"index": 1, "x": 10.0, "y": 12.0, "command": "stitch", "color": 0},
        ]});
        let t = Trace::from_points_reply(&r).expect("trace");
        assert_eq!(t.entries[0].command, Command::Jump);
        assert_eq!(t.segments().count(), 1);
        assert!(Trace::from_points_reply(&json!({"x": 1})).is_err());
        assert!(
            Trace::from_points_reply(&json!({"points": [{"x": 1, "y": 1, "command": "sew"}]}))
                .is_err()
        );
    }

    #[test]
    fn alignment_moves_the_anchor() {
        let mut p = StitchPattern::new();
        p.jump_to(-30.0, -10.0);
        p.stitch_to(20.0, 10.0);
        let mut t = Trace::from_pattern(&p);
        t.align(Anchor::BoundsMin, (10.0, 10.0));
        assert_eq!(t.bounds(), Some((10.0, 10.0, 15.0, 12.0)));
        t.align(Anchor::BoundsCenter, (0.0, 0.0));
        assert_eq!(t.bounds(), Some((-2.5, -1.0, 2.5, 1.0)));
        t.align(Anchor::FirstStitch, (0.0, 0.0));
        assert_eq!(t.anchor(Anchor::FirstStitch), Some((0.0, 0.0)));
    }

    #[test]
    fn geometry_helpers() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(inside((5.0, 5.0), &sq));
        assert!(!inside((15.0, 5.0), &sq));
        assert!((distance_to_polyline((5.0, -1.0), &sq, true) - 1.0).abs() < 1e-12);
        assert!((distance_to_polyline((-1.0, 5.0), &sq, false) - 5.0990195135927845).abs() < 1e-9);
    }
}
