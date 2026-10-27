//! The measurements library. Each [`Measure`] turns a stitch trace (mm) or a machine file into
//! one JSON value that a spec compares with an expected value or with the same measurement of
//! a reference file.
//!
//! Geometry (rails, polygons, stations) is given in Design Page millimetres, +y down. A file
//! whose origin differs (every PES/PEC file, a DST) is lined up first with the source's
//! `align` (see [`crate::spec::Source`]).

use crate::compare::{CompareOptions, pes_classes, pes_summary, thread_value};
use crate::trace::{Entry, Trace, distance, distance_to_polyline, distance_to_segment, inside};
use openstitch_core::{Command, StitchPattern};
use openstitch_formats::FileType;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandName {
    Stitch,
    Jump,
    Trim,
    ColorChange,
    Stop,
    End,
}

impl CommandName {
    pub fn command(self) -> Command {
        match self {
            Self::Stitch => Command::Stitch,
            Self::Jump => Command::Jump,
            Self::Trim => Command::Trim,
            Self::ColorChange => Command::ColorChange,
            Self::Stop => Command::Stop,
            Self::End => Command::End,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoundsProperty {
    Width,
    Height,
    MinX,
    MinY,
    MaxX,
    MaxY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stat {
    Min,
    Max,
    Mean,
    Median,
}

/// A point on a band's middle line and the band's direction there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Station {
    pub at: [f64; 2],
    pub axis: [f64; 2],
}

fn default_within() -> f64 {
    0.05
}

fn default_ring_within() -> f64 {
    0.3
}

/// What [`Measure::Polar`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolarProperty {
    MinRadius,
    MaxRadius,
    /// Middle of the angular span, degrees.
    MidAngle,
    /// Width of the angular span, degrees.
    AngleSpan,
}

fn default_bin() -> f64 {
    1.0
}

/// What to measure. `kind` selects the measurement; the other keys are its parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Measure {
    /// Needle penetrations (stitch entries).
    StitchCount,
    /// Stitches between consecutive needle penetrations (stitch entry after stitch entry).
    StitchSegments,
    /// Entries with this command.
    CommandCount { command: CommandName },
    /// Entries with this command between the first and the last stitch of a colour block:
    /// trims and jumps inside an object, not the lead-in jump or the closing trim.
    InteriorMoves { command: CommandName },
    /// Colour blocks that sew at least one stitch.
    ColorBlocks,
    /// Stitches per colour block, in sewing order.
    BlockStitchCounts,
    /// One bound of the stitches and jumps, mm.
    Bounds { property: BoundsProperty },
    /// Needle points (stitch entries) seen from `center` (mm), with angles clockwise on screen
    /// from 12 o'clock (−y), in (−180, 180], and radii in mm. Points within `ringWithin` mm of
    /// the circle of radius `ring` about the centre (an outline sewn with text fitted to it)
    /// are left out. The angular span is the circle minus its largest gap between points.
    Polar {
        center: [f64; 2],
        #[serde(default)]
        ring: Option<f64>,
        #[serde(default = "default_ring_within", rename = "ringWithin")]
        ring_within: f64,
        property: PolarProperty,
    },
    /// Statistic of stitch lengths (consecutive stitch entries), mm; stitches shorter than
    /// `minLength` are left out.
    StitchLength {
        stat: Stat,
        #[serde(default, rename = "minLength")]
        min_length: f64,
    },
    /// Stitch lengths counted in bins of `bin` mm: `{"0.00": n, "0.50": n, …}` by bin start.
    StitchLengthHistogram {
        #[serde(default = "default_bin")]
        bin: f64,
    },
    /// Stitches with both ends on the same rail (within `within` mm) and no longer than
    /// `maxLength`: stitches lying along an edge.
    EdgeStitches {
        rails: Vec<Vec<[f64; 2]>>,
        #[serde(default = "default_within")]
        within: f64,
        #[serde(default, rename = "maxLength")]
        max_length: Option<f64>,
    },
    /// Needle points on no rail.
    OffRailPoints {
        rails: Vec<Vec<[f64; 2]>>,
        #[serde(default = "default_within")]
        within: f64,
    },
    /// Share (0–1) of stitches whose two ends are on different rails.
    SideAlternation {
        rails: Vec<Vec<[f64; 2]>>,
        #[serde(default = "default_within")]
        within: f64,
    },
    /// The stitch direction (degrees, 0–180, +y down) carrying the most stitch length, in
    /// `bin`° bins.
    DominantAngle {
        #[serde(default = "default_bin")]
        bin: f64,
    },
    /// Largest angle (degrees) between a stitch passing within `radius` mm of a station and
    /// the perpendicular to the band there; stitches shorter than `minLength` are ignored.
    /// Fails when no stitch passes a station.
    AngleOffPerpendicular {
        stations: Vec<Station>,
        radius: f64,
        #[serde(default, rename = "minLength")]
        min_length: f64,
    },
    /// Stitches whose midpoint is outside the polygon by more than `within` mm.
    StitchesOutside {
        polygon: Vec<[f64; 2]>,
        #[serde(default = "default_within")]
        within: f64,
    },
    /// Travel between sewn parts: for each run of jumps / trims, the distance from the last
    /// stitch before it to the first stitch after it, mm.
    TravelGaps,
    /// Length of every jump move, mm.
    TotalJumpLength,
    /// Where the needle starts and ends: `[x0, y0, x1, y1]`, mm from the top-left of the needle
    /// points' bounds. The start is the landing of the jump right before the first stitch (a
    /// needle point in PE-DESIGN's files), or else the first stitch; the end is the last stitch.
    NeedleEnds,
    /// Area covered by straight (horizontal, 0°) fill rows: each row's needle extent (with the
    /// stitches into and out of it from the neighbouring rows), summed, times the mean row
    /// spacing, mm². Single points (under sewing across, travel) are not rows.
    StitchedArea,
    /// Fill at 0°: the needle-grid shift from each fill row to the next modulo `stitchLength`,
    /// mm, using needle points strictly inside `interior` (x range, mm). Fill rows have two
    /// consecutive interior needle points one stitch length apart; under sewing, edge runs and
    /// travel are skipped.
    RowShift {
        #[serde(rename = "stitchLength")]
        stitch_length: f64,
        interior: [f64; 2],
    },
    /// Fill at 0° without under sewing: the mean distance between the distinct lines of the
    /// fill rows (runs of two or more needle points on one horizontal line), mm: (last line −
    /// first line) / (lines − 1), so the file's 0.1 mm rounding of each line cancels out.
    /// Under sewing edge runs would count as lines, so the spec turns under sewing off.
    RowSpacing,
    /// Fill at 0°: the width of the fill rows' needle extent (runs of two or more needle
    /// points on one horizontal line), mm: the outline's width plus twice the pull
    /// compensation. Under sewing and travel lie inside the rows' extent.
    RowWidth,
    /// PES: the four characters after `#PES`.
    PesVersion,
    /// PES: object section summary (version, classes, objects, segments, threads, metadata).
    PesSummary,
    /// PES: the class list.
    PesClasses,
    /// File metadata value (`name`, `category`, `author`, `keywords`, `comments`, …).
    Metadata { key: String },
    /// The thread table: `[{rgb, name, catalog, brand, chart, kind}]`.
    Threads,
    /// Value at a JSON pointer in a labelled script step's reply; `pluck` takes one field of
    /// every element of an array.
    ReplyValue {
        pointer: String,
        #[serde(default)]
        pluck: Option<String>,
    },
    /// Whether the command is refused after the script ran (the design is unchanged either
    /// way). `errorContains` also requires the error text to contain it.
    Refused {
        command: String,
        #[serde(default)]
        params: Value,
        #[serde(default, rename = "errorContains")]
        error_contains: Option<String>,
    },
    /// Semantic comparison of the source file with `against` (a repository file) or with the
    /// reference file: the number of differences; the report lists each.
    Compare {
        #[serde(default)]
        against: Option<String>,
        #[serde(default)]
        options: CompareOptions,
    },
    /// Whether the source file is byte-identical to `against` or the reference file.
    BytesIdentical {
        #[serde(default)]
        against: Option<String>,
    },
}

/// What a measurement is taken on.
pub struct Subject<'a> {
    pub trace: &'a Trace,
    /// The machine file the trace was read from, if any.
    pub file: Option<(&'a [u8], FileType, &'a StitchPattern)>,
}

impl Measure {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::StitchCount => "stitchCount",
            Self::StitchSegments => "stitchSegments",
            Self::CommandCount { .. } => "commandCount",
            Self::InteriorMoves { .. } => "interiorMoves",
            Self::ColorBlocks => "colorBlocks",
            Self::BlockStitchCounts => "blockStitchCounts",
            Self::Bounds { .. } => "bounds",
            Self::Polar { .. } => "polar",
            Self::StitchLength { .. } => "stitchLength",
            Self::StitchLengthHistogram { .. } => "stitchLengthHistogram",
            Self::EdgeStitches { .. } => "edgeStitches",
            Self::OffRailPoints { .. } => "offRailPoints",
            Self::SideAlternation { .. } => "sideAlternation",
            Self::DominantAngle { .. } => "dominantAngle",
            Self::AngleOffPerpendicular { .. } => "angleOffPerpendicular",
            Self::StitchesOutside { .. } => "stitchesOutside",
            Self::TravelGaps => "travelGaps",
            Self::TotalJumpLength => "totalJumpLength",
            Self::NeedleEnds => "needleEnds",
            Self::StitchedArea => "stitchedArea",
            Self::RowShift { .. } => "rowShift",
            Self::RowSpacing => "rowSpacing",
            Self::RowWidth => "rowWidth",
            Self::PesVersion => "pesVersion",
            Self::PesSummary => "pesSummary",
            Self::PesClasses => "pesClasses",
            Self::Metadata { .. } => "metadata",
            Self::Threads => "threads",
            Self::ReplyValue { .. } => "replyValue",
            Self::Refused { .. } => "refused",
            Self::Compare { .. } => "compare",
            Self::BytesIdentical { .. } => "bytesIdentical",
        }
    }

    /// Needs the source to be a machine file.
    pub fn needs_file(&self) -> bool {
        matches!(
            self,
            Self::PesVersion
                | Self::PesSummary
                | Self::PesClasses
                | Self::Metadata { .. }
                | Self::Threads
                | Self::Compare { .. }
                | Self::BytesIdentical { .. }
        )
    }

    /// Evaluated by the runner against the session or a second file, not from a trace.
    pub fn is_runner_kind(&self) -> bool {
        matches!(
            self,
            Self::ReplyValue { .. }
                | Self::Refused { .. }
                | Self::Compare { .. }
                | Self::BytesIdentical { .. }
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = |what: &str, v: f64| {
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(format!("{what} must be a finite number > 0"))
            }
        };
        let non_negative = |what: &str, v: f64| {
            if v.is_finite() && v >= 0.0 {
                Ok(())
            } else {
                Err(format!("{what} must be a finite number ≥ 0"))
            }
        };
        let rails_ok = |rails: &[Vec<[f64; 2]>], within: f64| {
            if rails.len() < 2 || rails.iter().any(|r| r.len() < 2) {
                return Err("rails: at least two polylines of two points or more".to_string());
            }
            non_negative("within", within)
        };
        match self {
            Self::StitchLength { min_length, .. } => non_negative("minLength", *min_length),
            Self::StitchLengthHistogram { bin } | Self::DominantAngle { bin } => {
                positive("bin", *bin)
            }
            Self::EdgeStitches {
                rails,
                within,
                max_length,
            } => {
                rails_ok(rails, *within)?;
                max_length.map_or(Ok(()), |m| non_negative("maxLength", m))
            }
            Self::OffRailPoints { rails, within } | Self::SideAlternation { rails, within } => {
                rails_ok(rails, *within)
            }
            Self::AngleOffPerpendicular {
                stations,
                radius,
                min_length,
            } => {
                if stations.is_empty() {
                    return Err("stations: at least one".into());
                }
                if stations.iter().any(|s| s.axis[0].hypot(s.axis[1]) == 0.0) {
                    return Err("stations: axis must not be zero".into());
                }
                positive("radius", *radius)?;
                non_negative("minLength", *min_length)
            }
            Self::StitchesOutside { polygon, within } => {
                if polygon.len() < 3 {
                    return Err("polygon: at least three points".into());
                }
                non_negative("within", *within)
            }
            Self::RowShift {
                stitch_length,
                interior,
            } => {
                positive("stitchLength", *stitch_length)?;
                if interior[0] < interior[1] {
                    Ok(())
                } else {
                    Err("interior: [x0, x1] with x0 < x1".into())
                }
            }
            Self::ReplyValue { pointer, .. } => {
                if pointer.is_empty() || pointer.starts_with('/') {
                    Ok(())
                } else {
                    Err("pointer must be a JSON pointer (\"\" or starting with /)".into())
                }
            }
            Self::Compare { options, .. } => {
                options.counts.validate()?;
                non_negative("positionMm", options.position_mm)?;
                non_negative("boundsMm", options.bounds_mm)
            }
            _ => Ok(()),
        }
    }

    /// Evaluates a trace or file measurement.
    pub fn evaluate(&self, s: &Subject) -> Result<Value, String> {
        let t = s.trace;
        Ok(match self {
            Self::StitchCount => json!(t.stitches().count()),
            Self::StitchSegments => json!(stitch_pairs(t).count()),
            Self::CommandCount { command } => json!(
                t.entries
                    .iter()
                    .filter(|e| e.command == command.command())
                    .count()
            ),
            Self::InteriorMoves { command } => json!(interior_moves(t, command.command())),
            Self::ColorBlocks => json!(t.block_count()),
            Self::BlockStitchCounts => json!(block_stitch_counts(t)),
            Self::Bounds { property } => {
                let b = t.bounds().ok_or("nothing is sewn")?;
                json!(match property {
                    BoundsProperty::Width => b.2 - b.0,
                    BoundsProperty::Height => b.3 - b.1,
                    BoundsProperty::MinX => b.0,
                    BoundsProperty::MinY => b.1,
                    BoundsProperty::MaxX => b.2,
                    BoundsProperty::MaxY => b.3,
                })
            }
            Self::Polar {
                center,
                ring,
                ring_within,
                property,
            } => json!(polar(t, *center, *ring, *ring_within, *property)?),
            Self::StitchLength { stat, min_length } => {
                let mut v: Vec<f64> = stitch_lengths(t)
                    .into_iter()
                    .filter(|l| *l >= *min_length)
                    .collect();
                if v.is_empty() {
                    return Err("no stitches".into());
                }
                v.sort_by(f64::total_cmp);
                json!(match stat {
                    Stat::Min => v[0],
                    Stat::Max => v[v.len() - 1],
                    Stat::Mean => v.iter().sum::<f64>() / v.len() as f64,
                    Stat::Median => v[v.len() / 2],
                })
            }
            Self::StitchLengthHistogram { bin } => {
                let mut bins = std::collections::BTreeMap::<i64, u64>::new();
                for l in stitch_lengths(t) {
                    *bins.entry((l / bin).floor() as i64).or_default() += 1;
                }
                let map: serde_json::Map<String, Value> = bins
                    .into_iter()
                    .map(|(k, n)| (format!("{:.2}", k as f64 * bin), json!(n)))
                    .collect();
                Value::Object(map)
            }
            Self::EdgeStitches {
                rails,
                within,
                max_length,
            } => {
                let n = stitch_pairs(t)
                    .filter(|(a, b)| {
                        let (ra, rb) = (rail_of(a, rails, *within), rail_of(b, rails, *within));
                        ra.is_some()
                            && ra == rb
                            && max_length.is_none_or(|m| distance((a.x, a.y), (b.x, b.y)) <= m)
                    })
                    .count();
                json!(n)
            }
            Self::OffRailPoints { rails, within } => json!(
                t.stitches()
                    .filter(|e| rail_of(e, rails, *within).is_none())
                    .count()
            ),
            Self::SideAlternation { rails, within } => {
                let pairs: Vec<_> = stitch_pairs(t).collect();
                if pairs.is_empty() {
                    return Err("no stitches".into());
                }
                let alternating = pairs
                    .iter()
                    .filter(|(a, b)| {
                        let (ra, rb) = (rail_of(a, rails, *within), rail_of(b, rails, *within));
                        ra.is_some() && rb.is_some() && ra != rb
                    })
                    .count();
                json!(alternating as f64 / pairs.len() as f64)
            }
            Self::DominantAngle { bin } => {
                let n = (180.0 / bin).ceil() as usize;
                let mut bins = vec![0.0; n];
                for (a, b) in stitch_pairs(t) {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let len = dx.hypot(dy);
                    if len > 0.0 {
                        let deg = dy.atan2(dx).to_degrees().rem_euclid(180.0);
                        bins[((deg / bin).round() as usize) % n] += len;
                    }
                }
                let best = (0..n)
                    .max_by(|&a, &b| bins[a].total_cmp(&bins[b]).then(b.cmp(&a)))
                    .ok_or("no bins")?;
                if bins[best] == 0.0 {
                    return Err("no stitches".into());
                }
                json!(best as f64 * bin)
            }
            Self::AngleOffPerpendicular {
                stations,
                radius,
                min_length,
            } => {
                let mut worst: f64 = 0.0;
                for st in stations {
                    let (sx, sy) = (st.at[0], st.at[1]);
                    let al = st.axis[0].hypot(st.axis[1]);
                    let (ax, ay) = (st.axis[0] / al, st.axis[1] / al);
                    let mut seen = 0;
                    for (a, b) in stitch_pairs(t) {
                        let (dx, dy) = (b.x - a.x, b.y - a.y);
                        let len = dx.hypot(dy);
                        if len < min_length.max(1e-9)
                            || distance_to_segment((sx, sy), (a.x, a.y), (b.x, b.y)) > *radius
                        {
                            continue;
                        }
                        seen += 1;
                        let cos = ((dx * ax + dy * ay) / len).abs().min(1.0);
                        worst = worst.max(90.0 - cos.acos().to_degrees());
                    }
                    if seen == 0 {
                        return Err(format!("no stitch passes the station at ({sx}, {sy})"));
                    }
                }
                json!(worst)
            }
            Self::StitchesOutside { polygon, within } => json!(
                t.segments()
                    .filter(|(a, b)| {
                        let m = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                        !inside(m, polygon) && distance_to_polyline(m, polygon, true) > *within
                    })
                    .count()
            ),
            Self::TravelGaps => json!(travel_gaps(t)),
            Self::NeedleEnds => json!(needle_ends(t).ok_or("no stitches")?),
            Self::TotalJumpLength => {
                let mut total = 0.0;
                for w in t.entries.windows(2) {
                    if w[1].command == Command::Jump {
                        total += distance((w[0].x, w[0].y), (w[1].x, w[1].y));
                    }
                }
                json!(total)
            }
            Self::StitchedArea => json!(stitched_area(t).ok_or("no fill rows")?),
            Self::RowSpacing => json!(row_spacing(t).ok_or("fewer than two fill rows")?),
            Self::RowWidth => json!(row_width(t).ok_or("no fill rows")?),
            Self::RowShift {
                stitch_length,
                interior,
            } => json!(row_shifts(t, *stitch_length, *interior)),
            Self::PesVersion | Self::PesSummary | Self::PesClasses => {
                let (bytes, ty, _) = s.file.ok_or("needs a machine file")?;
                if ty != FileType::Pes {
                    return Err(format!("needs a PES file, not {}", ty.extension()));
                }
                let objects = openstitch_formats::pes::read_objects(bytes)
                    .map_err(|e| format!("PES objects: {e}"))?;
                match self {
                    Self::PesVersion => json!(objects.version),
                    Self::PesClasses => json!(pes_classes(&objects)),
                    _ => pes_summary(&objects),
                }
            }
            Self::Metadata { key } => {
                let (_, _, p) = s.file.ok_or("needs a machine file")?;
                json!(p.metadata.get(key))
            }
            Self::Threads => {
                let (_, _, p) = s.file.ok_or("needs a machine file")?;
                Value::Array(p.threads.iter().map(thread_value).collect())
            }
            Self::ReplyValue { .. }
            | Self::Refused { .. }
            | Self::Compare { .. }
            | Self::BytesIdentical { .. } => {
                return Err(format!("{} is evaluated by the runner", self.kind()));
            }
        })
    }
}

/// Consecutive stitch entries (both stitches, same block).
fn stitch_pairs(t: &Trace) -> impl Iterator<Item = (&Entry, &Entry)> {
    t.entries.windows(2).filter_map(|w| {
        (w[0].command == Command::Stitch
            && w[1].command == Command::Stitch
            && w[0].block == w[1].block)
            .then_some((&w[0], &w[1]))
    })
}

fn stitch_lengths(t: &Trace) -> Vec<f64> {
    stitch_pairs(t)
        .map(|(a, b)| distance((a.x, a.y), (b.x, b.y)))
        .collect()
}

fn rail_of(e: &Entry, rails: &[Vec<[f64; 2]>], within: f64) -> Option<usize> {
    rails
        .iter()
        .enumerate()
        .map(|(i, r)| (i, distance_to_polyline((e.x, e.y), r, false)))
        .filter(|(_, d)| *d <= within + 1e-9)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn block_stitch_counts(t: &Trace) -> Vec<usize> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for e in t.stitches() {
        match out.last_mut() {
            Some((b, n)) if *b == e.block => *n += 1,
            _ => out.push((e.block, 1)),
        }
    }
    out.into_iter().map(|(_, n)| n).collect()
}

fn interior_moves(t: &Trace, command: Command) -> usize {
    let mut blocks: Vec<usize> = t.entries.iter().map(|e| e.block).collect();
    blocks.dedup();
    blocks
        .into_iter()
        .map(|b| {
            let idx: Vec<usize> = (0..t.entries.len())
                .filter(|&i| t.entries[i].block == b)
                .collect();
            let stitch = |i: &&usize| t.entries[**i].command == Command::Stitch;
            match (idx.iter().find(stitch), idx.iter().rev().find(stitch)) {
                (Some(&first), Some(&last)) => (first..last)
                    .filter(|&i| t.entries[i].command == command)
                    .count(),
                _ => 0,
            }
        })
        .sum()
}

fn needle_ends(t: &Trace) -> Option<[f64; 4]> {
    let e = &t.entries;
    let first = e.iter().position(|x| x.command == Command::Stitch)?;
    let last = e.iter().rposition(|x| x.command == Command::Stitch)?;
    let start = if first > 0 && e[first - 1].command == Command::Jump {
        first - 1
    } else {
        first
    };
    let needle = || {
        e[start..=last]
            .iter()
            .filter(|x| matches!(x.command, Command::Stitch | Command::Jump))
    };
    let min_x = needle().map(|x| x.x).fold(f64::INFINITY, f64::min);
    let min_y = needle().map(|x| x.y).fold(f64::INFINITY, f64::min);
    Some([
        e[start].x - min_x,
        e[start].y - min_y,
        e[last].x - min_x,
        e[last].y - min_y,
    ])
}

fn travel_gaps(t: &Trace) -> Vec<f64> {
    let mut out = Vec::new();
    let (mut last, mut moved): (Option<&Entry>, bool) = (None, false);
    for e in &t.entries {
        match e.command {
            Command::Stitch => {
                if let (Some(l), true) = (last, moved) {
                    out.push(distance((l.x, l.y), (e.x, e.y)));
                }
                (last, moved) = (Some(e), false);
            }
            Command::Jump | Command::Trim => moved = true,
            _ => {}
        }
    }
    out
}

/// Runs of consecutive stitch points on one horizontal line (same y), in sewing order: (y, xs),
/// plus the x of the stitch points just before and after the run (when there are any).
/// Under sewing across the fill and travel stitches give runs of one point.
#[allow(clippy::type_complexity)]
fn horizontal_runs(t: &Trace) -> Vec<(f64, Vec<f64>, [Option<(f64, f64)>; 2])> {
    let pts: Vec<&Entry> = t.stitches().collect();
    let mut rows: Vec<(f64, Vec<f64>, [Option<(f64, f64)>; 2])> = Vec::new();
    for (i, e) in pts.iter().enumerate() {
        match rows.last_mut() {
            Some((y, xs, _)) if (*y - e.y).abs() < 1e-6 => xs.push(e.x),
            _ => {
                if let Some(last) = rows.last_mut() {
                    last.2[1] = Some((e.x, e.y));
                }
                let before = i.checked_sub(1).map(|k| (pts[k].x, pts[k].y));
                rows.push((e.y, vec![e.x], [before, None]));
            }
        }
    }
    rows
}

/// Fill rows (runs of two or more needle points on one line) × the mean distance between the
/// rows' distinct lines. A row's length is the needle extent of the run together with the
/// stitches into and out of it from a point at most 0.5 mm away across the rows: a row whose
/// own needle points stop one grid step short of the edge is still covered to the edge by those
/// slanted stitches. The file's 0.1 mm rounding of the row lines does not change the mean gap.
/// Single points (under sewing across the fill, travel) are not rows.
fn stitched_area(t: &Trace) -> Option<f64> {
    let rows: Vec<(f64, f64)> = horizontal_runs(t)
        .into_iter()
        .filter(|(_, xs, _)| xs.len() >= 2)
        .map(|(y, mut xs, ends)| {
            xs.extend(
                ends.into_iter()
                    .flatten()
                    .filter(|&(_, ny)| (ny - y).abs() <= 0.5)
                    .map(|(x, _)| x),
            );
            let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (hi - lo, y)
        })
        .filter(|&(w, _)| w > 0.0)
        .collect();
    let spacing = mean_line_gap(rows.iter().map(|r| r.1).collect())?;
    Some(rows.iter().map(|r| r.0).sum::<f64>() * spacing)
}

/// Mean distance between the distinct values of `ys` (closer than 0.05 mm = one line).
fn mean_line_gap(mut ys: Vec<f64>) -> Option<f64> {
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    (ys.len() >= 2).then(|| (ys[ys.len() - 1] - ys[0]) / (ys.len() - 1) as f64)
}

/// Fill rows: horizontal runs of two or more needle points (y, xs).
fn fill_rows(t: &Trace) -> impl Iterator<Item = (f64, Vec<f64>)> {
    horizontal_runs(t)
        .into_iter()
        .filter(|(_, xs, _)| xs.len() >= 2)
        .map(|(y, xs, _)| (y, xs))
}

/// See [`Measure::RowSpacing`].
fn row_spacing(t: &Trace) -> Option<f64> {
    mean_line_gap(fill_rows(t).map(|(y, _)| y).collect())
}

/// See [`Measure::RowWidth`].
fn row_width(t: &Trace) -> Option<f64> {
    let xs: Vec<f64> = fill_rows(t).flat_map(|(_, xs)| xs).collect();
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (hi > lo).then_some(hi - lo)
}

/// Shift of the needle grid from each fill row to the next, modulo `l`. A fill row is a
/// horizontal run with two consecutive needle points inside `interior` one stitch length apart
/// (±0.15 mm); its grid is the first such point. Under sewing, edge runs and travel are not
/// fill rows and are skipped.
fn row_shifts(t: &Trace, l: f64, interior: [f64; 2]) -> Vec<f64> {
    let phases: Vec<f64> = horizontal_runs(t)
        .into_iter()
        .filter_map(|(_, xs, _)| {
            let inner: Vec<f64> = xs
                .into_iter()
                .filter(|&x| x > interior[0] + 0.05 && x < interior[1] - 0.05)
                .collect();
            inner
                .windows(2)
                .find(|w| ((w[1] - w[0]).abs() - l).abs() <= 0.15)
                .map(|w| w[0].rem_euclid(l))
        })
        .collect();
    phases
        .windows(2)
        .map(|w| (w[1] - w[0]).rem_euclid(l))
        .collect()
}

/// See [`Measure::Polar`].
fn polar(
    t: &Trace,
    center: [f64; 2],
    ring: Option<f64>,
    within: f64,
    property: PolarProperty,
) -> Result<f64, String> {
    let points: Vec<(f64, f64)> = t
        .stitches()
        .map(|e| {
            let (dx, dy) = (e.x - center[0], e.y - center[1]);
            (dx.atan2(-dy).to_degrees(), dx.hypot(dy))
        })
        .filter(|&(_, r)| ring.is_none_or(|ring| (r - ring).abs() > within))
        .collect();
    if points.is_empty() {
        return Err("no needle points off the ring".into());
    }
    let radii = || points.iter().map(|p| p.1);
    match property {
        PolarProperty::MinRadius => return Ok(radii().fold(f64::INFINITY, f64::min)),
        PolarProperty::MaxRadius => return Ok(radii().fold(0.0, f64::max)),
        _ => {}
    }
    let mut a: Vec<f64> = points.iter().map(|p| p.0.rem_euclid(360.0)).collect();
    a.sort_by(f64::total_cmp);
    // The largest gap, the wrap from the last angle round to the first included.
    let (mut gap, mut after) = (a[0] + 360.0 - a[a.len() - 1], a[0]);
    for w in a.windows(2) {
        if w[1] - w[0] > gap {
            (gap, after) = (w[1] - w[0], w[1]);
        }
    }
    let span = 360.0 - gap;
    Ok(match property {
        PolarProperty::AngleSpan => span,
        _ => {
            let mid = (after + span / 2.0).rem_euclid(360.0);
            if mid > 180.0 { mid - 360.0 } else { mid }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_measures_angles_and_radii_off_the_ring() {
        // A 30 mm ring about (50, 50), and points 32–38 mm out from 30° left of 12 o'clock to
        // 30° right of it, plus a stray point on the ring at 6 o'clock.
        let mut p: Vec<(f64, f64)> = (0..=60)
            .map(|k| {
                let a = f64::from(k - 30).to_radians();
                let r = 32.0 + 6.0 * f64::from(k % 2);
                (50.0 + r * a.sin(), 50.0 - r * a.cos())
            })
            .collect();
        p.extend((0..36).map(|k| {
            let a = f64::from(k * 10).to_radians();
            (50.0 + 30.0 * a.sin(), 50.0 - 30.0 * a.cos())
        }));
        let t = trace(&p);
        let m = |property| polar(&t, [50.0, 50.0], Some(30.0), 0.3, property).expect("polar");
        assert!((m(PolarProperty::MidAngle)).abs() < 1e-9);
        assert!((m(PolarProperty::AngleSpan) - 60.0).abs() < 1e-9);
        assert!((m(PolarProperty::MinRadius) - 32.0).abs() < 1e-9);
        assert!((m(PolarProperty::MaxRadius) - 38.0).abs() < 1e-9);
        // Round the bottom (across ±180°), and without a ring every point counts.
        let flipped = trace(&p.iter().map(|&(x, y)| (x, 100.0 - y)).collect::<Vec<_>>());
        let mid = polar(
            &flipped,
            [50.0, 50.0],
            Some(30.0),
            0.3,
            PolarProperty::MidAngle,
        )
        .expect("polar");
        assert!((mid.abs() - 180.0).abs() < 1e-9, "{mid}");
        let all = polar(&t, [50.0, 50.0], None, 0.3, PolarProperty::MinRadius).expect("polar");
        assert!((all - 30.0).abs() < 1e-9);
        assert!(polar(&trace(&[]), [0.0, 0.0], None, 0.3, PolarProperty::MinRadius).is_err());
    }

    fn trace(points: &[(f64, f64)]) -> Trace {
        Trace {
            entries: points
                .iter()
                .map(|&(x, y)| Entry {
                    x,
                    y,
                    command: Command::Stitch,
                    block: 0,
                })
                .collect(),
        }
    }

    /// A 40 × 40 mm square at (10, 10): an edge run 1 mm inside at 2.5 mm, under sewing rows
    /// across (vertical, 2 mm apart), a diagonal travel, then 0° rows `gap` apart with 4 mm
    /// stitches shifted `shift` per row and row ends on the outline.
    fn square(gap: f64, shift: f64) -> Vec<(f64, f64)> {
        square_with(gap, shift, false)
    }

    /// [`square`]; with `brick`, odd rows leave out their edge points, as PE-DESIGN sews them.
    fn square_with(gap: f64, shift: f64, brick: bool) -> Vec<(f64, f64)> {
        let mut p = Vec::new();
        for k in 0..=15 {
            p.push((11.0 + 2.5 * k as f64 * 38.0 / 37.5, 49.0));
        }
        for (i, x) in (0..20).map(|i| (i, 11.0 + 2.0 * i as f64)) {
            for k in 0..10 {
                let y = 11.0 + 38.0 * k as f64 / 9.0;
                p.push((x, if i % 2 == 0 { 60.0 - y } else { y }));
            }
        }
        for k in 0..20 {
            let f = k as f64 / 20.0;
            p.push((49.0 - 39.0 * f, 49.0 - 39.0 * f));
        }
        let rows = (40.0 / gap).round() as usize;
        for r in 0..=rows {
            let y = ((10.0 + r as f64 * gap) * 10.0).round() / 10.0;
            let phase = ((r as f64 + 1.0) * shift).rem_euclid(4.0);
            let mut xs = vec![10.0];
            let mut x = 10.0 + phase;
            while x < 50.0 - 1e-9 {
                if x > 10.0 + 1e-9 {
                    xs.push(x);
                }
                x += 4.0;
            }
            xs.push(50.0);
            if brick && r % 2 == 1 && r < rows {
                xs.pop();
                xs.remove(0);
            }
            if r % 2 == 1 {
                xs.reverse();
            }
            p.extend(xs.into_iter().map(|x| (x, y)));
        }
        p
    }

    #[test]
    fn row_shift_skips_under_sewing_and_travel() {
        for shift in [0.0, 1.0, 2.0] {
            let shifts = row_shifts(&trace(&square(1.0 / 4.5, shift)), 4.0, [10.0, 50.0]);
            assert_eq!(shifts.len(), 180, "{shift}");
            for s in &shifts {
                let d = (s - shift).rem_euclid(4.0);
                assert!(d.min(4.0 - d) < 0.08, "{shift}: {shifts:?}");
            }
        }
    }

    /// [`square_with`]'s top fill alone (no under sewing or travel), rows `gap` apart with each
    /// row end moved out by `pull`, odd rows without their edge points.
    fn top_fill(gap: f64, pull: f64) -> Vec<(f64, f64)> {
        let rows = (40.0 / gap).round() as usize;
        let mut p = Vec::new();
        for r in 0..=rows {
            let y = ((10.0 + r as f64 * gap) * 10.0).round() / 10.0;
            let mut xs: Vec<f64> = (0..=10).map(|k| 10.0 + 4.0 * k as f64).collect();
            xs[0] -= pull;
            xs[10] += pull;
            if r % 2 == 1 {
                xs = xs[1..10].to_vec();
                xs.reverse();
            }
            p.extend(xs.into_iter().map(|x| (x, y)));
        }
        p
    }

    #[test]
    fn row_spacing_is_the_mean_gap_despite_rounding() {
        for d in [0.1, 2.5, 4.5, 6.0] {
            let s = row_spacing(&trace(&top_fill(1.0 / d, 0.0))).expect("rows");
            assert!((s - 1.0 / d).abs() < 0.002, "{d}: {s}");
        }
        assert_eq!(row_spacing(&trace(&[(10.0, 10.0), (14.0, 10.0)])), None);
    }

    #[test]
    fn row_width_adds_the_pull_compensation_twice() {
        for pull in [0.0, 0.5, 2.0] {
            let w = row_width(&trace(&top_fill(1.0 / 4.5, pull))).expect("rows");
            assert!((w - (40.0 + 2.0 * pull)).abs() < 1e-9, "{pull}: {w}");
        }
        // Under sewing and travel lie inside the rows.
        let w = row_width(&trace(&square_with(1.0 / 4.5, 1.0, true))).expect("rows");
        assert!((w - 40.0).abs() < 1e-9, "{w}");
    }

    #[test]
    fn stitched_area_of_a_square_with_under_sewing() {
        for gap in [0.25, 1.0 / 4.5, 0.4] {
            let a = stitched_area(&trace(&square(gap, 4.0 / 3.0))).expect("rows");
            assert!((a - 1600.0).abs() < 0.02 * 1600.0, "{gap}: {a}");
        }
    }

    #[test]
    fn stitched_area_counts_rows_whose_edge_points_are_left_out() {
        let a = stitched_area(&trace(&square_with(1.0 / 4.5, 1.0, true))).expect("rows");
        assert!((a - 1600.0).abs() < 0.02 * 1600.0, "{a}");
        let shifts = row_shifts(
            &trace(&square_with(1.0 / 4.5, 1.0, true)),
            4.0,
            [10.0, 50.0],
        );
        assert_eq!(shifts.len(), 180);
    }

    #[test]
    fn stitched_area_leaves_out_a_hole() {
        // 40 × 40 mm with a 10 × 10 mm hole in the middle: the left and right parts of the
        // hole's rows are sewn as separate runs.
        let gap = 0.25;
        let mut p = Vec::new();
        for r in 0..=160 {
            let y = 10.0 + r as f64 * gap;
            if (25.0..=35.0).contains(&y) {
                continue;
            }
            p.extend([(10.0, y), (30.0, y), (50.0, y)]);
        }
        for (lo, hi) in [(10.0, 25.0), (35.0, 50.0)] {
            for r in 0..40 {
                let y = 25.0 + (r as f64 + 0.5) * gap;
                p.extend([(lo, y), (hi, y)]);
            }
        }
        let a = stitched_area(&trace(&p)).expect("rows");
        assert!((a - 1500.0).abs() < 0.02 * 1500.0, "{a}");
    }

    #[test]
    fn needle_ends_start_at_the_jump_landing() {
        // A 20 × 10 mm square outline from (5, 3), entered by a jump at the middle of its
        // bottom edge and ending there.
        let mut t = trace(&[
            (15.0, 13.0),
            (25.0, 13.0),
            (25.0, 3.0),
            (5.0, 3.0),
            (5.0, 13.0),
            (15.0, 13.0),
        ]);
        t.entries.insert(
            0,
            Entry {
                x: 15.0,
                y: 13.0,
                command: Command::Jump,
                block: 0,
            },
        );
        t.entries[1].x = 15.3;
        assert_eq!(needle_ends(&t), Some([10.0, 10.0, 10.0, 10.0]));
        // Without a jump the first stitch is the start.
        let t = trace(&[(0.0, 0.0), (4.0, 2.0)]);
        assert_eq!(needle_ends(&t), Some([0.0, 0.0, 4.0, 2.0]));
        assert_eq!(needle_ends(&Trace::default()), None);
    }
}
