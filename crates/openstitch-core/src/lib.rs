//! Machine-level embroidery model shared by every OpenStitch crate.
//!
//! A [`StitchPattern`] is the generated stitch stream that file codecs read and write:
//! absolute coordinates in 0.1 mm, +x right, +y down (the PEC/PES convention), one
//! [`Thread`] per colour block.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

mod charts;
mod trims;

pub use charts::{HUS_CHART, JEF_CHART, PEC_CHART, SEW_CHART, SHV_CHART};

/// Machine command attached to a needle position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    /// Needle penetration at the position.
    Stitch,
    /// Frame move without sewing (thread stays attached).
    Jump,
    /// Cut the thread at the current position.
    Trim,
    /// Switch to the next thread in the pattern's thread list.
    ColorChange,
    /// Machine pause (same thread continues).
    Stop,
    /// End of design.
    End,
}

/// One entry of a stitch stream.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stitch {
    pub x: f64,
    pub y: f64,
    pub command: Command,
}

impl Stitch {
    pub const fn new(x: f64, y: f64, command: Command) -> Self {
        Self { x, y, command }
    }
}

/// Thread colour with optional chart metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Cow<'static, str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_number: Option<Cow<'static, str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<Cow<'static, str>>,
}

impl Thread {
    pub const fn rgb(rgb: u32) -> Self {
        Self {
            red: (rgb >> 16) as u8,
            green: (rgb >> 8) as u8,
            blue: rgb as u8,
            name: None,
            catalog_number: None,
            brand: None,
        }
    }

    /// Constructor for the static thread charts.
    pub const fn chart(
        red: u8,
        green: u8,
        blue: u8,
        name: &'static str,
        catalog: &'static str,
        brand: &'static str,
    ) -> Self {
        Self {
            red,
            green,
            blue,
            name: Some(Cow::Borrowed(name)),
            catalog_number: Some(Cow::Borrowed(catalog)),
            brand: Some(Cow::Borrowed(brand)),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(Cow::Owned(name.into()));
        self
    }

    pub const fn rgb_value(&self) -> u32 {
        (self.red as u32) << 16 | (self.green as u32) << 8 | self.blue as u32
    }

    /// `#rrggbb`, lower case.
    pub fn hex(&self) -> String {
        format!("#{:06x}", self.rgb_value())
    }

    /// Parses `#rrggbb` (or `rrggbb`).
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if h.len() != 6 {
            return None;
        }
        u32::from_str_radix(h, 16).ok().map(Self::rgb)
    }
}

/// "Red-mean" weighted RGB distance (<https://www.compuphase.com/cmetric.htm>), integer form.
pub fn color_distance(a: &Thread, b: &Thread) -> i64 {
    let red_mean = ((i64::from(a.red) + i64::from(b.red)) as f64 / 2.0).round_ties_even() as i64;
    let r = i64::from(a.red) - i64::from(b.red);
    let g = i64::from(a.green) - i64::from(b.green);
    let bl = i64::from(a.blue) - i64::from(b.blue);
    (((512 + red_mean) * r * r) >> 8) + 4 * g * g + (((767 - red_mean) * bl * bl) >> 8)
}

/// Index of the closest `Some` entry; ties go to the later index.
pub fn nearest_index(color: &Thread, chart: &[Option<Thread>]) -> Option<usize> {
    let mut best = None;
    let mut best_distance = i64::MAX;
    for (i, candidate) in chart.iter().enumerate() {
        if let Some(c) = candidate {
            let d = color_distance(color, c);
            if d <= best_distance {
                best_distance = d;
                best = Some(i);
            }
        }
    }
    best
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Generated, machine-level design: an ordered stitch stream plus one thread per colour block.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StitchPattern {
    pub stitches: Vec<Stitch>,
    pub threads: Vec<Thread>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl StitchPattern {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(&self) -> Option<&str> {
        self.metadata.get("name").map(String::as_str)
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.metadata.insert("name".into(), name.into());
    }

    fn position(&self) -> (f64, f64) {
        self.stitches.last().map_or((0.0, 0.0), |s| (s.x, s.y))
    }

    pub fn stitch_to(&mut self, x: f64, y: f64) {
        self.stitches.push(Stitch::new(x, y, Command::Stitch));
    }

    pub fn jump_to(&mut self, x: f64, y: f64) {
        self.stitches.push(Stitch::new(x, y, Command::Jump));
    }

    /// Appends a command at the current position.
    pub fn push(&mut self, command: Command) {
        let (x, y) = self.position();
        self.stitches.push(Stitch::new(x, y, command));
    }

    /// Bounds of positions that sew or move (trim/colour/end markers excluded).
    pub fn bounds(&self) -> Bounds {
        let mut it = self
            .stitches
            .iter()
            .filter(|s| matches!(s.command, Command::Stitch | Command::Jump));
        let Some(first) = it.next() else {
            return Bounds::default();
        };
        it.fold(
            Bounds {
                min_x: first.x,
                min_y: first.y,
                max_x: first.x,
                max_y: first.y,
            },
            |b, s| Bounds {
                min_x: b.min_x.min(s.x),
                min_y: b.min_y.min(s.y),
                max_x: b.max_x.max(s.x),
                max_y: b.max_y.max(s.y),
            },
        )
    }

    pub fn count(&self, command: Command) -> usize {
        self.stitches
            .iter()
            .filter(|s| s.command == command)
            .count()
    }

    /// Colour changes + 1, or 0 when nothing is sewn.
    pub fn color_block_count(&self) -> usize {
        if self.count(Command::Stitch) == 0 {
            0
        } else {
            self.count(Command::ColorChange) + 1
        }
    }

    /// The stream split into non-empty colour blocks, each with its thread if known.
    pub fn color_blocks(&self) -> Vec<(Option<&Thread>, Vec<Stitch>)> {
        let mut blocks = Vec::new();
        let mut current = Vec::new();
        let mut index = 0;
        for s in &self.stitches {
            match s.command {
                Command::ColorChange => {
                    blocks.push((self.threads.get(index), std::mem::take(&mut current)));
                    index += 1;
                }
                Command::End => {}
                _ => current.push(*s),
            }
        }
        blocks.push((self.threads.get(index), current));
        blocks.retain(|b| !b.1.is_empty());
        blocks
    }

    /// Ensures exactly one thread per colour block, padding from `chart` cyclically
    /// (the Brother PEC chart when `None`).
    pub fn fix_thread_count(&mut self, chart: Option<&[Thread]>) {
        let needed = self.color_block_count().max(1);
        self.threads.truncate(needed);
        let pec: Vec<Thread>;
        let chart = match chart {
            Some(c) if !c.is_empty() => c,
            _ => {
                pec = PEC_CHART.iter().flatten().cloned().collect();
                &pec
            }
        };
        let mut i = 0;
        while self.threads.len() < needed {
            self.threads.push(chart[i % chart.len()].clone());
            i += 1;
        }
    }

    /// Copy whose consecutive stitch/jump deltas never exceed `max_stitch` / `max_jump` on either
    /// axis, inserting evenly spaced points of the same command (rounded half away from zero).
    pub fn interpolating_long_moves(&self, max_stitch: f64, max_jump: f64) -> StitchPattern {
        let mut out = StitchPattern {
            stitches: Vec::with_capacity(self.stitches.len()),
            ..self.clone()
        };
        let (mut px, mut py) = (0.0_f64, 0.0_f64);
        for s in &self.stitches {
            if matches!(s.command, Command::Stitch | Command::Jump) {
                let limit = if s.command == Command::Stitch {
                    max_stitch
                } else {
                    max_jump
                };
                let (dx, dy) = (s.x - px, s.y - py);
                let steps = (dx.abs().max(dy.abs()) / limit).ceil() as i64;
                for k in 1..steps.max(1) {
                    let t = k as f64 / steps as f64;
                    out.stitches.push(Stitch::new(
                        (px + dx * t).round(),
                        (py + dy * t).round(),
                        s.command,
                    ));
                }
                px = s.x;
                py = s.y;
            }
            out.stitches.push(*s);
        }
        out
    }

    /// Inserts trims implied by runs of jumps; see [`trims::infer_trims`].
    pub fn infer_trims(
        &mut self,
        jumps_required: Option<usize>,
        distance_required: Option<f64>,
        clipping: bool,
    ) {
        trims::infer_trims(
            &mut self.stitches,
            jumps_required,
            distance_required,
            clipping,
        );
    }
}

/// Round half to even, as the codecs do for absolute coordinates.
pub fn round_coordinate(v: f64) -> i64 {
    v.round_ties_even() as i64
}

#[cfg(test)]
mod tests;
