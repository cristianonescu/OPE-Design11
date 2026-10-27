//! Inspector model for the selection and the commands its controls issue.

use super::request::CommandRequest;
use openstitch_core::{PEC_CHART, Thread, nearest_index};
use openstitch_design::patterns::{self, Pattern, PatternKind};
use openstitch_design::{
    Arrange, ChainShape, Design, Feather, FeatherSide, LineSewing, LineType, RegionSewing,
    RegionType, RunningStitchPath, UnderlayStyle,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// A property across several objects.
#[derive(Clone, Debug, PartialEq)]
pub enum Common<T> {
    Empty,
    Same(T),
    Mixed,
}

impl<T: PartialEq> Common<T> {
    pub fn of(values: impl IntoIterator<Item = T>) -> Self {
        let mut out = Common::Empty;
        for v in values {
            out = match out {
                Common::Empty => Common::Same(v),
                Common::Same(s) if s == v => Common::Same(s),
                _ => return Common::Mixed,
            };
        }
        out
    }

    pub fn same(&self) -> Option<&T> {
        match self {
            Common::Same(v) => Some(v),
            _ => None,
        }
    }
}

/// Every region sew type, preceded by "None" (not sewn).
pub fn region_types() -> Vec<(Option<RegionType>, &'static str)> {
    std::iter::once((None, "None"))
        .chain(RegionType::ALL.into_iter().map(|k| (Some(k), k.label())))
        .collect()
}

/// Every line sew type, preceded by "None".
pub fn line_types() -> Vec<(Option<LineType>, &'static str)> {
    std::iter::once((None, "None"))
        .chain(LineType::ALL.into_iter().map(|k| (Some(k), k.label())))
        .collect()
}

pub fn region_name(k: Option<RegionType>) -> &'static str {
    k.map_or("none", RegionType::id)
}

pub fn line_name(k: Option<LineType>) -> &'static str {
    k.map_or("none", LineType::id)
}

/// One inspector control for a sewing attribute; changing it sends `{param: value}`.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Number {
        label: &'static str,
        /// Draft key (unique per control).
        key: &'static str,
        param: &'static str,
        value: f64,
        range: (f64, f64),
        speed: f64,
        suffix: &'static str,
        integer: bool,
    },
    Choice {
        label: &'static str,
        param: &'static str,
        value: String,
        /// (parameter value, label).
        options: Vec<(String, String)>,
    },
    Toggle {
        label: &'static str,
        param: &'static str,
        value: bool,
    },
}

impl Control {
    /// The command parameter this control sets.
    pub fn param(&self) -> &'static str {
        match self {
            Control::Number { param, .. }
            | Control::Choice { param, .. }
            | Control::Toggle { param, .. } => param,
        }
    }

    /// The command parameters for a new value of this control.
    pub fn params(&self, value: Value) -> Value {
        let param = self.param();
        let value = match (self, value.as_f64()) {
            (Control::Number { integer: true, .. }, Some(v)) => json!(v.round() as i64),
            _ => value,
        };
        json!({ param: value })
    }
}

#[allow(clippy::too_many_arguments)]
fn num(
    label: &'static str,
    key: &'static str,
    param: &'static str,
    value: f64,
    range: (f64, f64),
    speed: f64,
    suffix: &'static str,
) -> Control {
    Control::Number {
        label,
        key,
        param,
        value,
        range,
        speed,
        suffix,
        integer: false,
    }
}

fn times(
    label: &'static str,
    key: &'static str,
    param: &'static str,
    value: u32,
    max: u32,
) -> Control {
    Control::Number {
        label,
        key,
        param,
        value: f64::from(value),
        range: (1.0, f64::from(max)),
        speed: 0.05,
        suffix: "×",
        integer: true,
    }
}

fn arrange(value: Arrange) -> Control {
    let id = |a: Arrange| match a {
        Arrange::Normal => "normal",
        Arrange::Mirror => "mirror",
        Arrange::Alternate => "alternate",
    };
    Control::Choice {
        label: "Arrange",
        param: "arrange",
        value: id(value).into(),
        options: [
            (Arrange::Normal, "Normal"),
            (Arrange::Mirror, "Mirror"),
            (Arrange::Alternate, "Alternate"),
        ]
        .iter()
        .map(|(a, l)| (id(*a).to_string(), l.to_string()))
        .collect(),
    }
}

/// Library patterns of `kind` as choice options.
pub fn pattern_options(kind: PatternKind) -> Vec<(String, String)> {
    pattern_options_with(kind, &[])
}

/// Bundled patterns of `kind`, then the design's own (marked "(user)").
pub fn pattern_options_with(kind: PatternKind, user: &[Pattern]) -> Vec<(String, String)> {
    patterns::library()
        .iter()
        .filter(|p| p.kind == kind)
        .map(|p| (p.id.clone(), p.name.clone()))
        .chain(
            user.iter()
                .filter(|p| p.kind == kind)
                .map(|p| (p.id.clone(), format!("{} (user)", p.name))),
        )
        .collect()
}

fn toggle(label: &'static str, param: &'static str, value: bool) -> Control {
    Control::Toggle {
        label,
        param,
        value,
    }
}

fn choice(
    label: &'static str,
    param: &'static str,
    value: &str,
    options: &[(&str, &str)],
) -> Control {
    Control::Choice {
        label,
        param,
        value: value.into(),
        options: options
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect(),
    }
}

/// The controls the inspector shows for region sewing `r`, by sew type.
pub fn region_controls(r: &RegionSewing) -> Vec<Control> {
    region_controls_with(r, &[])
}

/// [`region_controls`] offering the design's own patterns too.
pub fn region_controls_with(r: &RegionSewing, user: &[Pattern]) -> Vec<Control> {
    let mut v = Vec::new();
    let row_based = matches!(r.kind, RegionType::Fill | RegionType::Satin);
    if row_based {
        v.push(toggle("Auto direction", "autoDirection", r.auto_direction));
    }
    let centred = matches!(
        r.kind,
        RegionType::ConcentricCircle
            | RegionType::Radial
            | RegionType::Spiral
            | RegionType::FlexibleSpiral
    );
    if !r.auto_direction && !centred {
        v.push(num(
            "Direction",
            "region.angle",
            "angle",
            r.angle,
            (-360.0, 360.0),
            1.0,
            "°",
        ));
    }
    let uses_density = !matches!(
        r.kind,
        RegionType::CrossStitch
            | RegionType::Stippling
            | RegionType::MotifFill
            | RegionType::DecorativeFill
            | RegionType::NetFill
            | RegionType::FlexibleSpiral
    );
    if uses_density {
        let unit = if r.kind == RegionType::ZigzagNetFill {
            " /mm"
        } else {
            " rows/mm"
        };
        v.push(num(
            "Density",
            "region.density",
            "density",
            r.density,
            (0.5, 10.0),
            0.05,
            unit,
        ));
    }
    if r.kind == RegionType::Fill {
        v.push(num(
            "Stitch length",
            "region.stitchLength",
            "stitchLength",
            r.stitch_length,
            (1.0, 10.0),
            0.05,
            " mm",
        ));
    }
    if row_based {
        v.push(num(
            "Pull compensation",
            "region.pullCompensation",
            "pullCompensation",
            r.pull_compensation,
            (0.0, 2.0),
            0.01,
            " mm",
        ));
        v.push(toggle("Underlay", "underlay", r.underlay));
        if r.underlay {
            let style = match r.underlay_style {
                UnderlayStyle::Edge => "edge",
                UnderlayStyle::Zigzag => "zigzag",
                UnderlayStyle::EdgeZigzag => "edgeZigzag",
                UnderlayStyle::Double => "double",
                UnderlayStyle::EdgeFill => "edgeFill",
            };
            v.push(choice(
                "Underlay style",
                "underlayStyle",
                style,
                &[
                    ("edge", "Edge"),
                    ("zigzag", "Zigzag"),
                    ("edgeZigzag", "Edge and zigzag"),
                    ("double", "Double"),
                    ("edgeFill", "Edge and fill"),
                ],
            ));
            if r.underlay_style != UnderlayStyle::Edge {
                v.push(num(
                    "Underlay spacing",
                    "region.underlaySpacing",
                    "underlaySpacing",
                    r.underlay_spacing_mm(),
                    (1.0, 10.0),
                    0.05,
                    " mm",
                ));
            }
        }
        v.push(num(
            "Gradation (far side)",
            "region.gradation",
            "gradation",
            r.gradation.unwrap_or(100.0),
            (5.0, 100.0),
            0.5,
            " %",
        ));
        let feather = r.feather.unwrap_or(Feather {
            side: FeatherSide::Start,
            width: 0.0,
        });
        v.push(num(
            "Feathered edge",
            "region.featherWidth",
            "featherWidth",
            feather.width,
            (0.0, 10.0),
            0.05,
            " mm",
        ));
        if r.feather.is_some() {
            let side = match feather.side {
                FeatherSide::Start => "start",
                FeatherSide::End => "end",
                FeatherSide::Both => "both",
            };
            v.push(choice(
                "Feather side",
                "featherSide",
                side,
                &[("start", "Start"), ("end", "End"), ("both", "Both")],
            ));
        }
        if r.kind == RegionType::Satin {
            v.push(num(
                "Split stitches over",
                "region.splitLength",
                "splitLength",
                r.split_length.unwrap_or(0.0),
                (0.0, 12.0),
                0.05,
                " mm",
            ));
        }
    }
    v.extend(region_type_controls(r, user));
    v.extend(travel_control(r));
    v
}

/// Running Stitch Path for fill and satin where the guide offers it (Inside not with
/// gradation; nothing with Auto direction or a feathered edge); a travel on/off toggle for
/// the other types and for fill and satin that jump (designs saved before the attribute).
fn travel_control(r: &RegionSewing) -> Option<Control> {
    match r.running_stitch_path {
        Some(_) if r.running_stitch_path_available() => Some(Control::Choice {
            label: "Running stitch path",
            param: "runningStitchPath",
            value: r.travel_path().unwrap_or_default().id().into(),
            options: [
                (RunningStitchPath::Inside, "Inside of region"),
                (RunningStitchPath::AlongOutline, "Along outline"),
                (RunningStitchPath::OnOutline, "On outline"),
            ]
            .into_iter()
            .filter(|(p, _)| r.gradation.is_none() || *p != RunningStitchPath::Inside)
            .map(|(p, l)| (p.id().to_string(), l.to_string()))
            .collect(),
        }),
        Some(_) if matches!(r.kind, RegionType::Fill | RegionType::Satin) => None,
        travel => Some(toggle("Travel inside", "travel", travel.is_some())),
    }
}

/// Net patterns as choice options (zigzag net offers the first four).
fn net_options(zigzag: bool) -> Vec<(String, String)> {
    let n = if zigzag {
        patterns::ZIGZAG_NET_PATTERNS
    } else {
        patterns::NET_PATTERNS.len()
    };
    patterns::NET_PATTERNS[..n]
        .iter()
        .map(|p| (p.id.to_string(), p.name.to_string()))
        .collect()
}

/// Controls specific to the region types beyond fill and satin.
fn region_type_controls(r: &RegionSewing, user: &[Pattern]) -> Vec<Control> {
    let (pw, ph) = r.pattern_size_mm();
    let pattern = |kind: PatternKind| Control::Choice {
        label: "Pattern",
        param: "pattern",
        value: r
            .pattern
            .clone()
            .unwrap_or_else(|| patterns::default_for(kind).id.clone()),
        options: pattern_options_with(kind, user),
    };
    let size = || {
        vec![
            num(
                "Pattern width",
                "region.patternWidth",
                "patternWidth",
                pw,
                (0.5, 50.0),
                0.05,
                " mm",
            ),
            num(
                "Pattern height",
                "region.patternHeight",
                "patternHeight",
                ph,
                (0.5, 50.0),
                0.05,
                " mm",
            ),
        ]
    };
    let spacing = |label, lo| {
        num(
            label,
            "region.spacing",
            "spacing",
            r.spacing_mm(),
            (lo, 20.0),
            0.05,
            " mm",
        )
    };
    let pitch = num(
        "Run pitch",
        "region.runPitch",
        "runPitch",
        r.run_pitch_mm(),
        (1.0, 10.0),
        0.05,
        " mm",
    );
    let percent = |label, key, param, value: Option<f64>, default: f64, max: f64| {
        num(
            label,
            key,
            param,
            value.unwrap_or(default),
            (0.0, max),
            0.5,
            " %",
        )
    };
    let mut v = Vec::new();
    match r.kind {
        RegionType::Fill => v.push(percent(
            "Frequency",
            "region.frequency",
            "frequency",
            r.frequency,
            33.0,
            50.0,
        )),
        RegionType::Satin => {}
        RegionType::ProgrammableFill => {
            v.push(pattern(PatternKind::Fill));
            v.extend(size());
            v.push(percent(
                "Row offset",
                "region.rowOffset",
                "rowOffset",
                r.row_offset,
                0.0,
                100.0,
            ));
            v.push(percent(
                "Column offset",
                "region.columnOffset",
                "columnOffset",
                r.column_offset,
                0.0,
                100.0,
            ));
            v.push(percent(
                "Frequency",
                "region.frequency",
                "frequency",
                r.frequency,
                33.0,
                50.0,
            ));
            v.push(toggle("Base sewing", "baseSewing", r.base_sewing));
        }
        RegionType::MotifFill => {
            v.push(pattern(PatternKind::Motif));
            v.extend(size());
            v.push(spacing("Spacing", 0.0));
            v.push(percent(
                "Row offset",
                "region.rowOffset",
                "rowOffset",
                r.row_offset,
                50.0,
                100.0,
            ));
            v.push(pitch);
        }
        RegionType::Piping => {
            v.push(pitch);
            v.push(percent(
                "Frequency",
                "region.frequency",
                "frequency",
                r.frequency,
                33.0,
                50.0,
            ));
        }
        RegionType::CrossStitch => {
            v.push(spacing("Size", 0.5));
            v.push(toggle("Triple", "triple", r.triple));
        }
        RegionType::ConcentricCircle | RegionType::Radial | RegionType::Spiral => {
            v.push(pitch);
            if r.kind != RegionType::Radial {
                v.push(toggle("Start inside", "startInside", r.start_inside));
            }
            if r.kind == RegionType::Spiral {
                v.push(toggle(
                    "Counter-clockwise",
                    "counterclockwise",
                    r.counterclockwise,
                ));
            }
        }
        RegionType::FlexibleSpiral => {
            v.push(spacing("Spacing", 0.5));
            v.push(pitch);
            v.push(toggle("Start inside", "startInside", r.start_inside));
            v.push(toggle(
                "Counter-clockwise",
                "counterclockwise",
                r.counterclockwise,
            ));
        }
        RegionType::Stippling => {
            v.push(spacing("Spacing", 0.5));
            v.push(pitch);
        }
        RegionType::NetFill | RegionType::ZigzagNetFill => {
            let zigzag = r.kind == RegionType::ZigzagNetFill;
            v.push(Control::Choice {
                label: "Pattern",
                param: "pattern",
                value: r
                    .pattern
                    .clone()
                    .unwrap_or_else(|| patterns::NET_PATTERNS[0].id.to_string()),
                options: net_options(zigzag),
            });
            v.push(spacing("Spacing", 0.5));
            v.push(pitch);
            if zigzag {
                v.push(num(
                    "Zigzag width",
                    "region.patternWidth",
                    "patternWidth",
                    pw,
                    (0.5, 50.0),
                    0.05,
                    " mm",
                ));
                v.push(toggle("Under sewing", "underlay", r.underlay));
            }
        }
        RegionType::DecorativeFill => {
            v.push(pattern(PatternKind::Decorative));
            v.extend(size());
            v.push(pitch);
            v.push(percent(
                "Random shift",
                "region.randomShift",
                "randomShift",
                r.random_shift,
                0.0,
                50.0,
            ));
        }
    }
    v
}

/// The controls the inspector shows for line sewing `l`, by sew type.
pub fn line_controls(l: &LineSewing) -> Vec<Control> {
    line_controls_with(l, &[])
}

/// [`line_controls`] offering the design's own motifs too.
pub fn line_controls_with(l: &LineSewing, user: &[Pattern]) -> Vec<Control> {
    let pitch = num(
        "Run pitch",
        "line.pitch",
        "pitch",
        l.pitch,
        (1.0, 10.0),
        0.05,
        " mm",
    );
    let width = |label| {
        num(
            label,
            "line.width",
            "width",
            l.width,
            (0.5, 10.0),
            0.05,
            " mm",
        )
    };
    let spacing = |label, lo| {
        num(
            label,
            "line.spacing",
            "spacing",
            l.spacing_mm(),
            (lo, 20.0),
            0.05,
            " mm",
        )
    };
    match l.kind {
        LineType::Running => vec![
            num(
                "Stitch length",
                "line.pitch",
                "pitch",
                l.pitch,
                (1.0, 10.0),
                0.05,
                " mm",
            ),
            times("Run times", "line.runTimes", "runTimes", l.run_times, 5),
        ],
        LineType::Triple => vec![num(
            "Stitch length",
            "line.pitch",
            "pitch",
            l.pitch,
            (1.0, 10.0),
            0.05,
            " mm",
        )],
        LineType::Zigzag => vec![
            width("Width"),
            num(
                "Density",
                "line.density",
                "density",
                l.density,
                (0.5, 10.0),
                0.05,
                " /mm",
            ),
            Control::Toggle {
                label: "Half stitch",
                param: "halfStitch",
                value: l.half_stitch,
            },
        ],
        LineType::Motif => vec![
            Control::Choice {
                label: "Motif",
                param: "motif",
                value: l
                    .motif
                    .clone()
                    .unwrap_or_else(|| patterns::default_for(PatternKind::Motif).id.clone()),
                options: pattern_options_with(PatternKind::Motif, user),
            },
            width("Width"),
            num(
                "Height",
                "line.height",
                "height",
                l.height.unwrap_or(3.0),
                (0.5, 20.0),
                0.05,
                " mm",
            ),
            spacing("Spacing", 0.0),
            pitch,
            arrange(l.arrange),
        ],
        LineType::Stem => vec![
            width("Width"),
            spacing("Spacing", 0.5),
            num(
                "Angle",
                "line.angle",
                "angle",
                l.angle.unwrap_or(45.0),
                (45.0, 135.0),
                1.0,
                "°",
            ),
            Control::Toggle {
                label: "Single stitch",
                param: "single",
                value: l.single,
            },
        ],
        LineType::Candlewick => vec![
            width("Size"),
            num(
                "Density",
                "line.density",
                "density",
                l.density,
                (0.5, 10.0),
                0.05,
                " /mm",
            ),
            spacing("Spacing", 0.5),
            pitch,
        ],
        LineType::EStitch | LineType::VStitch => {
            let mut v = vec![
                spacing("Interval", 0.5),
                width("Stroke width"),
                pitch,
                times("Run times", "line.runTimes", "runTimes", l.run_times, 6),
            ];
            if l.kind == LineType::EStitch {
                v.push(times(
                    "Stroke times",
                    "line.strokeTimes",
                    "strokeTimes",
                    l.stroke_times.unwrap_or(1),
                    3,
                ));
            }
            v.push(arrange(l.arrange));
            v
        }
        LineType::Chain => vec![
            Control::Choice {
                label: "Stitch type",
                param: "chainShape",
                value: match l.chain_shape {
                    ChainShape::Triangle => "triangle".into(),
                    ChainShape::Diamond => "diamond".into(),
                },
                options: vec![
                    ("triangle".into(), "Triangle".into()),
                    ("diamond".into(), "Diamond".into()),
                ],
            },
            width("Size"),
            times("Times", "line.runTimes", "runTimes", l.run_times, 6),
            arrange(l.arrange),
        ],
        LineType::Cutting => vec![Control::Toggle {
            label: "Leave uncut sections",
            param: "leaveUncut",
            value: l.leave_uncut,
        }],
    }
}

/// What the inspector shows for the selection.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorModel {
    /// Selected shape objects (attributes apply to these).
    pub shapes: BTreeSet<u64>,
    /// Selected imported stitch patterns (transforms only).
    pub stitch_patterns: usize,
    /// Every selected shape has a closed outline, so region sewing is possible.
    pub closed: bool,
    pub region_kind: Common<Option<RegionType>>,
    /// Attribute values of the first selected region (shown when values differ).
    pub region: Option<RegionSewing>,
    pub line_kind: Common<Option<LineType>>,
    pub line: Option<LineSewing>,
    pub hole_sewing: Common<bool>,
}

impl InspectorModel {
    pub fn new(design: &Design, selection: &BTreeSet<u64>) -> Self {
        let selected: Vec<_> = design
            .objects
            .iter()
            .filter(|o| selection.contains(&o.id))
            .collect();
        let shapes: Vec<_> = selected.iter().filter(|o| o.is_outline()).collect();
        InspectorModel {
            shapes: shapes.iter().map(|o| o.id).collect(),
            stitch_patterns: selected.len() - shapes.len(),
            closed: !shapes.is_empty() && shapes.iter().all(|o| o.has_closed_contour()),
            region_kind: Common::of(shapes.iter().map(|o| o.region.as_ref().map(|r| r.kind))),
            region: shapes.iter().find_map(|o| o.region.clone()),
            line_kind: Common::of(shapes.iter().map(|o| o.line.as_ref().map(|l| l.kind))),
            line: shapes.iter().find_map(|o| o.line.clone()),
            hole_sewing: Common::of(
                shapes
                    .iter()
                    .filter(|o| o.region.is_some())
                    .map(|o| o.hole_sewing),
            ),
        }
    }

    fn with_ids(&self, mut params: Value) -> Value {
        params["ids"] = CommandRequest::ids(&self.shapes);
        params
    }

    /// `object.setRegion` with `params` (e.g. `{"angle": 45}`) for every selected shape.
    pub fn set_region(&self, params: Value) -> CommandRequest {
        CommandRequest::new("object.setRegion", self.with_ids(params))
    }

    pub fn set_region_type(&self, k: Option<RegionType>) -> CommandRequest {
        self.set_region(json!({ "type": region_name(k) }))
    }

    pub fn set_line(&self, params: Value) -> CommandRequest {
        CommandRequest::new("object.setLine", self.with_ids(params))
    }

    pub fn set_line_type(&self, k: Option<LineType>) -> CommandRequest {
        self.set_line(json!({ "type": line_name(k) }))
    }

    /// Hole sewing applies only to selected shapes that have region sewing.
    pub fn set_hole_sewing(&self, design: &Design, on: bool) -> Option<CommandRequest> {
        let ids: BTreeSet<u64> = design
            .objects
            .iter()
            .filter(|o| self.shapes.contains(&o.id) && o.region.is_some())
            .map(|o| o.id)
            .collect();
        (!ids.is_empty()).then(|| {
            CommandRequest::new(
                "object.setHoleSewing",
                json!({ "ids": CommandRequest::ids(&ids), "enabled": on }),
            )
        })
    }
}

/// Brother PEC chart entries 1–64 for the colour picker.
pub fn pec_palette() -> Vec<(usize, &'static Thread)> {
    PEC_CHART
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.as_ref().map(|t| (i, t)))
        .collect()
}

/// The PEC chart number closest to `t`.
pub fn pec_index(t: &Thread) -> Option<usize> {
    nearest_index(t, &PEC_CHART)
}
