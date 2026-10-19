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
    /// Area covered by straight (horizontal, 0°) fill rows: their total length times the
    /// median row spacing, mm².
    StitchedArea,
    /// Fill at 0°: the needle-grid shift from each row to the next modulo `stitchLength`, mm,
    /// using needle points strictly inside `interior` (x range, mm).
    RowShift {
        #[serde(rename = "stitchLength")]
        stitch_length: f64,
        interior: [f64; 2],
    },
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
            Self::StitchedArea => "stitchedArea",
            Self::RowShift { .. } => "rowShift",
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

fn stitched_area(t: &Trace) -> Option<f64> {
    let rows: Vec<(f64, f64)> = stitch_pairs(t)
        .map(|(a, b)| ((b.x - a.x).abs(), (b.y - a.y).abs(), a.y))
        .filter(|&(dx, dy, _)| dx > 0.0 && dy < 0.05 * dx)
        .map(|(dx, _, y)| (dx, y))
        .collect();
    let mut ys: Vec<f64> = rows.iter().map(|r| r.1).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    let mut gaps: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_by(f64::total_cmp);
    let spacing = gaps[gaps.len() / 2];
    Some(rows.iter().map(|r| r.0).sum::<f64>() * spacing)
}

fn row_shifts(t: &Trace, l: f64, interior: [f64; 2]) -> Vec<f64> {
    let mut rows: Vec<(f64, Vec<f64>)> = Vec::new();
    for e in t.stitches() {
        let keep = e.x > interior[0] + 0.05 && e.x < interior[1] - 0.05;
        match rows.last_mut() {
            Some((y, xs)) if (*y - e.y).abs() < 1e-6 => {
                if keep {
                    xs.push(e.x);
                }
            }
            _ => rows.push((e.y, if keep { vec![e.x] } else { Vec::new() })),
        }
    }
    let phase = |xs: &[f64]| xs.first().map(|x| x.rem_euclid(l));
    rows.windows(2)
        .filter_map(|w| Some((phase(&w[1].1)? - phase(&w[0].1)?).rem_euclid(l)))
        .collect()
}
