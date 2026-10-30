//! Machine-level embroidery model shared by every OpenStitch crate.
//!
//! A [`StitchPattern`] is the generated stitch stream that file codecs read and write:
//! absolute coordinates in 0.1 mm, +x right, +y down (the PEC/PES convention), one
//! [`Thread`] per colour block.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

mod brother;
mod charts;
mod trims;

pub use brother::{
    BROTHER_EMBROIDERY, BROTHER_EMBROIDERY_BRAND, brother_embroidery_code,
    pec_index_of_brother_code,
};
pub use charts::{HUS_CHART, JEF_CHART, PEC_CHART, SEW_CHART, SHV_CHART};

/// Brother chart entries a colour is matched against: 1–61. PE-DESIGN 11.50 never picked 62–64
/// in the owner's swatch capture (`compat/reference/pes/thread-match-swatches.pes`:
/// rgb(255, 128, 0) went to 11 Deep Gold, 2210 away, not to 62 Orange, 625 away), so those
/// three are kept for files that name them but are not candidates (#119, #136).
pub const PEC_BASIC_COLORS: usize = 61;

/// The Brother chart entries a colour may be matched to: `PEC_CHART[..=PEC_BASIC_COLORS]`
/// (index 0 is unused). Use with [`nearest_index`] instead of the whole `PEC_CHART`.
pub fn pec_basic_chart() -> &'static [Option<Thread>] {
    &PEC_CHART[..=PEC_BASIC_COLORS]
}

/// The Brother chart thread PE-DESIGN 11.50 gives an imported colour: the nearest entry 1–61
/// ([`nearest_index`] over [`pec_basic_chart`]), as the [`PEC_CHART`] thread (its name, chart
/// number and brand `Brother`; the PES writers write its BROTHER EMBROIDERY code,
/// [`brother_embroidery_code`]). PE-DESIGN does this to every fill and stroke colour of a
/// vector import (`compat/reference/pes/thread-match-swatches.pes`: twelve RGB fills, twelve
/// chart threads; issue #119).
pub fn nearest_brother_thread(color: &Thread) -> Thread {
    nearest_index(color, pec_basic_chart())
        .and_then(|i| PEC_CHART[i].clone())
        .unwrap_or_else(|| color.clone())
}

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
    /// Thread chart name as stored in a PES v5+ thread table; only ever what a file supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<Cow<'static, str>>,
    /// PES v5+ colour type code; `None` = custom colour ([`Thread::CUSTOM_KIND`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
}

impl Thread {
    /// PES colour type written when a thread has no other: custom colour.
    pub const CUSTOM_KIND: u32 = 0x0A;

    pub const fn rgb(rgb: u32) -> Self {
        Self {
            red: (rgb >> 16) as u8,
            green: (rgb >> 8) as u8,
            blue: rgb as u8,
            name: None,
            catalog_number: None,
            brand: None,
            chart: None,
            kind: None,
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
            chart: None,
            kind: None,
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

/// Equal-weight squared Euclidean distance on 8-bit sRGB: ΔR² + ΔG² + ΔB² (0–195075).
///
/// Chart matching uses no channel weighting, red-mean term or CIE/Lab conversion (#119). An
/// earlier red-mean metric picked a different thread on image and foreign-file imports.
pub fn color_distance(a: &Thread, b: &Thread) -> i64 {
    let r = i64::from(a.red) - i64::from(b.red);
    let g = i64::from(a.green) - i64::from(b.green);
    let bl = i64::from(a.blue) - i64::from(b.blue);
    r * r + g * g + bl * bl
}

/// City-block (L1) distance on 8-bit sRGB: |ΔR| + |ΔG| + |ΔB| (0–765).
pub fn city_block_distance(a: &Thread, b: &Thread) -> i64 {
    (i64::from(a.red) - i64::from(b.red)).abs()
        + (i64::from(a.green) - i64::from(b.green)).abs()
        + (i64::from(a.blue) - i64::from(b.blue)).abs()
}

/// Index of the closest `Some` entry by [`color_distance`]; ties go to the later index.
///
/// Charts whose index is the entry's ID (`PEC_CHART`: index 1–64 is Brother's chart number)
/// return that ID; `None` slots are skipped and never matched. The tie rule is PE-DESIGN
/// 11.50's as far as the owner's swatch capture shows it (`thread-match-swatches.pes`:
/// rgb(240, 200, 220) went to chart entry 57, not to 42, which holds the same RGB in the
/// chart table); issue #119 asked for the earlier index, and the reference wins.
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

/// "Approximate" sew-palette match (#119): two running minima are kept at once, squared
/// Euclidean (initial 3·255² + 1 = 195076) and city block (initial 3·255 + 1 = 766), each with
/// the later index on ties, as [`nearest_index`]. The result is the Euclidean winner unless
/// the city-block minimum is numerically smaller than the Euclidean minimum, which it is in
/// essentially every real case (L1 ≤ 765 while Euclid² reaches 195075), so this is the L1
/// winner with Euclid² as the fallback when the two minima are equal (both 0 or both 1: an
/// exact or one-unit match).
pub fn approximate_index(color: &Thread, chart: &[Option<Thread>]) -> Option<usize> {
    let mut sq = (3 * 255 * 255 + 1, None);
    let mut city = (3 * 255 + 1, None);
    for (i, candidate) in chart.iter().enumerate() {
        if let Some(c) = candidate {
            let d = color_distance(color, c);
            if d <= sq.0 {
                sq = (d, Some(i));
            }
            let d = city_block_distance(color, c);
            if d <= city.0 {
                city = (d, Some(i));
            }
        }
    }
    if city.0 < sq.0 { city.1 } else { sq.1 }
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
