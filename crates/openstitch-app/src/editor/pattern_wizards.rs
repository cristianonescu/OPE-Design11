//! Pattern wizard forms (Design → Background Fill…, …): the settings each dialog shows, turned
//! into one `wizard.*` command, so applying a wizard is one Undo. Defaults match the engine's.

use crate::editor::CommandRequest;
use openstitch_design::{Design, ObjectKind};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// Which wizard dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WizardKind {
    Background,
    Applique,
    Patch,
    Cutwork,
}

impl WizardKind {
    pub const ALL: [WizardKind; 4] = [
        WizardKind::Background,
        WizardKind::Applique,
        WizardKind::Patch,
        WizardKind::Cutwork,
    ];

    /// Short name for command lines (`--wizard`).
    pub fn key(self) -> &'static str {
        match self {
            WizardKind::Background => "background",
            WizardKind::Applique => "applique",
            WizardKind::Patch => "patch",
            WizardKind::Cutwork => "cutwork",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            WizardKind::Background => "Background Fill",
            WizardKind::Applique => "Appliqué Wizard",
            WizardKind::Patch => "Embroidered Patch",
            WizardKind::Cutwork => "Cutwork Wizard",
        }
    }
}

/// Background fill types, as the engine names them.
pub const FILL_TYPES: [(&str, &str); 5] = [
    ("echo", "Echo fill"),
    ("stippling", "Stippling fill"),
    ("decorative", "Decorative fill"),
    ("crosshatch", "Crosshatch"),
    ("stripes", "Stripes"),
];

/// Line sew types for echo, crosshatch and stripe lines.
pub const LINE_SEWS: [(&str, &str); 3] = [
    ("running", "Running stitch"),
    ("triple", "Triple stitch"),
    ("chain", "Chain stitch"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundForm {
    /// Closed shape to fill; `None` fills the hoop.
    pub area: Option<u64>,
    pub fill_type: &'static str,
    /// Gap around the patterns (echo: also between lines), mm.
    pub spacing: f64,
    pub echo_clipped: bool,
    pub link_ends: bool,
    pub sew: &'static str,
    pub pitch: f64,
    pub run_times: u32,
    pub chain_diamond: bool,
    pub chain_size: f64,
    pub stipple_spacing: f64,
    pub pattern_width: f64,
    pub pattern_height: f64,
    pub connect_edges: bool,
    pub line_spacing: f64,
    pub angle: f64,
    pub skip_overlap: bool,
}

impl Default for BackgroundForm {
    fn default() -> Self {
        BackgroundForm {
            area: None,
            fill_type: "echo",
            spacing: 5.0,
            echo_clipped: true,
            link_ends: false,
            sew: "running",
            pitch: 2.5,
            run_times: 1,
            chain_diamond: false,
            chain_size: 2.0,
            stipple_spacing: 2.0,
            pattern_width: 10.0,
            pattern_height: 8.0,
            connect_edges: true,
            line_spacing: 10.0,
            angle: 45.0,
            skip_overlap: true,
        }
    }
}

impl BackgroundForm {
    /// Range of the gap for the current fill type, mm.
    pub fn spacing_range(&self) -> (f64, f64) {
        match self.fill_type {
            "echo" => (1.0, 30.0),
            "stippling" => (1.0, 7.0),
            _ => (0.0, 20.0),
        }
    }

    /// Switches the fill type, keeping the gap inside the new type's range.
    pub fn set_fill_type(&mut self, t: &'static str) {
        self.fill_type = t;
        let (lo, hi) = self.spacing_range();
        self.spacing = self.spacing.clamp(lo, hi);
    }

    /// Whether the line sew settings apply.
    pub fn uses_lines(&self) -> bool {
        matches!(self.fill_type, "echo" | "crosshatch" | "stripes")
    }

    /// Closed shapes that can be the area: not among the patterns.
    pub fn area_candidates(design: &Design, selection: &BTreeSet<u64>) -> Vec<u64> {
        design
            .objects
            .iter()
            .filter(|o| o.kind != ObjectKind::Stitches && o.has_closed_contour())
            .map(|o| o.id)
            .filter(|id| !selection.contains(id))
            .collect()
    }

    /// The command, or `None` while no pattern (other than the area) is selected.
    pub fn request(&self, selection: &BTreeSet<u64>) -> Option<CommandRequest> {
        let ids: BTreeSet<u64> = selection
            .iter()
            .copied()
            .filter(|&id| Some(id) != self.area)
            .collect();
        if ids.is_empty() {
            return None;
        }
        let mut p = Map::new();
        p.insert("ids".into(), CommandRequest::ids(&ids));
        if let Some(a) = self.area {
            p.insert("area".into(), json!(a));
        }
        p.insert("fillType".into(), json!(self.fill_type));
        let (lo, hi) = self.spacing_range();
        p.insert("spacing".into(), json!(self.spacing.clamp(lo, hi)));
        if self.uses_lines() {
            p.insert("sew".into(), json!(self.sew));
            match self.sew {
                "chain" => {
                    p.insert(
                        "chainShape".into(),
                        json!(if self.chain_diamond {
                            "diamond"
                        } else {
                            "triangle"
                        }),
                    );
                    p.insert("chainSize".into(), json!(self.chain_size));
                    p.insert("runTimes".into(), json!(self.run_times.clamp(1, 5)));
                }
                "running" => {
                    p.insert("pitch".into(), json!(self.pitch));
                    p.insert("runTimes".into(), json!(self.run_times.clamp(1, 5)));
                }
                _ => {
                    p.insert("pitch".into(), json!(self.pitch));
                }
            }
        }
        match self.fill_type {
            "echo" => {
                p.insert(
                    "echoType".into(),
                    json!(if self.echo_clipped { "clipped" } else { "echo" }),
                );
                if self.echo_clipped {
                    p.insert("linkEchoEnds".into(), json!(self.link_ends));
                }
            }
            "stippling" => {
                p.insert("stippleSpacing".into(), json!(self.stipple_spacing));
            }
            "decorative" => {
                p.insert("patternWidth".into(), json!(self.pattern_width));
                p.insert("patternHeight".into(), json!(self.pattern_height));
                p.insert("connectEdges".into(), json!(self.connect_edges));
            }
            _ => {
                p.insert("lineSpacing".into(), json!(self.line_spacing));
                p.insert("angle".into(), json!(self.angle));
                p.insert("linkEchoEnds".into(), json!(self.link_ends));
            }
        }
        if self.fill_type != "echo" {
            p.insert("skipOverlap".into(), json!(self.skip_overlap));
        }
        Some(CommandRequest::new(
            "wizard.backgroundFill",
            Value::Object(p),
        ))
    }
}

/// Appliqué material choices.
pub const MATERIALS: [(&str, &str); 3] = [
    ("running", "Running stitch (cut by hand)"),
    ("cutting", "Cutting (cutwork needles)"),
    ("none", "None"),
];

/// Tack-down choices.
pub const TACK_DOWNS: [(&str, &str); 4] = [
    ("zigzag", "Zigzag stitch"),
    ("eStitch", "E stitch"),
    ("vStitch", "V stitch"),
    ("none", "None"),
];

/// Covering stitch choices.
pub const COVERS: [(&str, &str); 4] = [
    ("zigzag", "Zigzag stitch"),
    ("eStitch", "E stitch"),
    ("vStitch", "V stitch"),
    ("chain", "Chain stitch"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct AppliqueForm {
    pub material: &'static str,
    pub tack_down: &'static str,
    pub tack_width: f64,
    pub tack_density: f64,
    pub tack_interval: f64,
    pub cover: &'static str,
    /// Zigzag width, E/V stroke or chain size, mm.
    pub cover_width: f64,
    pub cover_density: f64,
    pub cover_interval: f64,
    pub cover_pitch: f64,
    pub chain_diamond: bool,
    pub cover_times: u32,
    pub replace: bool,
    pub distance: f64,
    pub holes: bool,
}

impl Default for AppliqueForm {
    fn default() -> Self {
        AppliqueForm {
            material: "running",
            tack_down: "zigzag",
            tack_width: 2.0,
            tack_density: 1.0,
            tack_interval: 3.0,
            cover: "zigzag",
            cover_width: 3.0,
            cover_density: 5.0,
            cover_interval: 3.0,
            cover_pitch: 2.5,
            chain_diamond: false,
            cover_times: 1,
            replace: false,
            distance: 0.0,
            holes: true,
        }
    }
}

impl AppliqueForm {
    pub fn request(&self, selection: &BTreeSet<u64>) -> Option<CommandRequest> {
        if selection.is_empty() {
            return None;
        }
        let mut p = Map::new();
        p.insert("ids".into(), CommandRequest::ids(selection));
        p.insert("material".into(), json!(self.material));
        p.insert("tackDown".into(), json!(self.tack_down));
        match self.tack_down {
            "zigzag" => {
                p.insert("tackDownWidth".into(), json!(self.tack_width));
                p.insert("tackDownDensity".into(), json!(self.tack_density));
            }
            "none" => {}
            _ => {
                p.insert("tackDownWidth".into(), json!(self.tack_width));
                p.insert("tackDownInterval".into(), json!(self.tack_interval));
            }
        }
        p.insert("cover".into(), json!(self.cover));
        p.insert("coverWidth".into(), json!(self.cover_width));
        match self.cover {
            "zigzag" => {
                p.insert("coverDensity".into(), json!(self.cover_density));
            }
            "chain" => {
                p.insert(
                    "chainShape".into(),
                    json!(if self.chain_diamond {
                        "diamond"
                    } else {
                        "triangle"
                    }),
                );
                p.insert("coverTimes".into(), json!(self.cover_times.clamp(1, 5)));
            }
            _ => {
                p.insert("coverInterval".into(), json!(self.cover_interval));
                p.insert("coverPitch".into(), json!(self.cover_pitch));
            }
        }
        if self.replace {
            p.insert("output".into(), json!("replace"));
            p.insert("holes".into(), json!(self.holes));
        } else {
            p.insert("output".into(), json!("add"));
            p.insert("distance".into(), json!(self.distance));
        }
        Some(CommandRequest::new("wizard.applique", Value::Object(p)))
    }
}

/// Patch cutting line choices.
pub const PATCH_CUTS: [(&str, &str); 3] = [
    ("running", "Running stitch (cut by hand)"),
    ("cutting", "Cutting (cutwork needles)"),
    ("none", "None"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct PatchForm {
    pub border: bool,
    pub width: f64,
    pub density: f64,
    pub distance: f64,
    pub cutting: &'static str,
    pub cut_distance: f64,
}

impl Default for PatchForm {
    fn default() -> Self {
        PatchForm {
            border: true,
            width: 3.0,
            density: 4.5,
            distance: 5.0,
            cutting: "running",
            cut_distance: 3.0,
        }
    }
}

impl PatchForm {
    /// The command, or `None` with nothing selected or nothing to make.
    pub fn request(&self, selection: &BTreeSet<u64>) -> Option<CommandRequest> {
        if selection.is_empty() || (!self.border && self.cutting == "none") {
            return None;
        }
        let mut p = Map::new();
        p.insert("ids".into(), CommandRequest::ids(selection));
        p.insert("border".into(), json!(self.border));
        if self.border {
            p.insert("width".into(), json!(self.width));
            p.insert("density".into(), json!(self.density));
            p.insert("distance".into(), json!(self.distance));
        }
        p.insert("cutting".into(), json!(self.cutting));
        if self.cutting != "none" {
            p.insert("cutDistance".into(), json!(self.cut_distance));
        }
        Some(CommandRequest::new("wizard.patch", Value::Object(p)))
    }
}

/// The cutwork wizard's net patterns, in the engine's order (`netPattern` 1–5).
pub const NET_PATTERNS: [&str; 5] = ["Square", "Diamond", "Triangle", "Honeycomb", "Star"];

#[derive(Clone, Debug, PartialEq)]
pub struct CutworkForm {
    pub stabilize: bool,
    /// Cut with the cutwork needles (else sew a line to cut by hand).
    pub needles: bool,
    pub secure: bool,
    pub net: bool,
    /// 1–5.
    pub net_pattern: u32,
    pub net_spacing: f64,
    pub net_direction: f64,
    pub cover: bool,
    pub cover_width: f64,
    pub cover_density: f64,
}

impl Default for CutworkForm {
    fn default() -> Self {
        CutworkForm {
            stabilize: true,
            needles: true,
            secure: true,
            net: true,
            net_pattern: 1,
            net_spacing: 3.0,
            net_direction: 0.0,
            cover: true,
            cover_width: 4.0,
            cover_density: 6.5,
        }
    }
}

impl CutworkForm {
    pub fn request(&self, selection: &BTreeSet<u64>) -> Option<CommandRequest> {
        if selection.is_empty() {
            return None;
        }
        let mut p = Map::new();
        p.insert("ids".into(), CommandRequest::ids(selection));
        p.insert("stabilize".into(), json!(self.stabilize));
        p.insert(
            "cutting".into(),
            json!(if self.needles { "cutting" } else { "running" }),
        );
        p.insert("secureStabilizer".into(), json!(self.secure));
        p.insert("netFill".into(), json!(self.net));
        if self.net {
            p.insert("netPattern".into(), json!(self.net_pattern.clamp(1, 5)));
            p.insert("netSpacing".into(), json!(self.net_spacing));
            p.insert("netDirection".into(), json!(self.net_direction));
        }
        p.insert("cover".into(), json!(self.cover));
        if self.cover {
            p.insert("coverWidth".into(), json!(self.cover_width));
            p.insert("coverDensity".into(), json!(self.cover_density));
        }
        Some(CommandRequest::new("wizard.cutwork", Value::Object(p)))
    }
}

/// Every wizard form, kept between openings of the dialogs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatternWizards {
    pub open: Option<WizardKind>,
    pub background: BackgroundForm,
    pub applique: AppliqueForm,
    pub patch: PatchForm,
    pub cutwork: CutworkForm,
}

impl PatternWizards {
    /// The open wizard's command for `selection`.
    pub fn request(&self, kind: WizardKind, selection: &BTreeSet<u64>) -> Option<CommandRequest> {
        match kind {
            WizardKind::Background => self.background.request(selection),
            WizardKind::Applique => self.applique.request(selection),
            WizardKind::Patch => self.patch.request(selection),
            WizardKind::Cutwork => self.cutwork.request(selection),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;

    fn session() -> Session {
        let mut s = Session::new(std::env::temp_dir());
        for (c, p) in [
            ("design.new", json!({"width": 100, "height": 100})),
            (
                "shape.ellipse",
                json!({"cx": 50, "cy": 50, "rx": 12, "fill": "Red"}),
            ),
            (
                "shape.rectangle",
                json!({"x": 10, "y": 10, "width": 80, "height": 80, "outline": "Black"}),
            ),
        ] {
            s.execute(c, Some(&p)).expect(c);
        }
        s
    }

    #[test]
    fn every_background_fill_type_applies_in_one_command() {
        let mut w = PatternWizards::default();
        let selection = BTreeSet::from([1, 2]);
        assert_eq!(
            BackgroundForm::area_candidates(&session().design().clone(), &BTreeSet::from([1])),
            [2]
        );
        for area in [None, Some(2)] {
            for (t, _) in FILL_TYPES {
                for sew in LINE_SEWS.map(|(s, _)| s) {
                    w.background.area = area;
                    w.background.set_fill_type(t);
                    w.background.sew = sew;
                    let r = w
                        .request(WizardKind::Background, &selection)
                        .expect("request");
                    let mut s = session();
                    let before = s.design().clone();
                    let result = s
                        .execute(r.command, Some(&r.params))
                        .unwrap_or_else(|e| panic!("{t} {sew} {area:?}: {e}"));
                    assert_eq!(CommandRequest::created_ids(&result).len(), 1);
                    s.execute("history.undo", None).expect("undo");
                    assert_eq!(s.design(), &before, "{t}: one undo step");
                }
            }
        }
    }

    #[test]
    fn every_applique_choice_applies_in_one_command() {
        let mut w = PatternWizards::default();
        let selection = BTreeSet::from([1]);
        for replace in [false, true] {
            for material in MATERIALS.map(|(k, _)| k) {
                for tack in TACK_DOWNS.map(|(k, _)| k) {
                    for cover in COVERS.map(|(k, _)| k) {
                        w.applique = AppliqueForm {
                            material,
                            tack_down: tack,
                            cover,
                            replace,
                            distance: 2.0,
                            ..Default::default()
                        };
                        let r = w.request(WizardKind::Applique, &selection).expect("r");
                        let mut s = session();
                        let before = s.design().clone();
                        let result = s
                            .execute(r.command, Some(&r.params))
                            .unwrap_or_else(|e| panic!("{material} {tack} {cover}: {e}"));
                        let stages =
                            2 + usize::from(material != "none") + usize::from(tack != "none");
                        assert_eq!(CommandRequest::created_ids(&result).len(), stages);
                        s.execute("history.undo", None).expect("undo");
                        assert_eq!(s.design(), &before);
                    }
                }
            }
        }
    }

    #[test]
    fn applique_request_carries_the_settings() {
        let f = AppliqueForm {
            cover: "chain",
            chain_diamond: true,
            cover_times: 2,
            replace: true,
            holes: false,
            tack_down: "vStitch",
            ..Default::default()
        };
        assert!(f.request(&BTreeSet::new()).is_none());
        let r = f.request(&BTreeSet::from([3])).expect("request");
        assert_eq!(
            r.params,
            json!({"ids": [3], "material": "running", "tackDown": "vStitch", "tackDownWidth": 2.0,
                   "tackDownInterval": 3.0, "cover": "chain", "coverWidth": 3.0,
                   "chainShape": "diamond", "coverTimes": 2, "output": "replace", "holes": false})
        );
    }

    #[test]
    fn every_patch_choice_applies_in_one_command() {
        let selection = BTreeSet::from([1, 2]);
        for border in [true, false] {
            for (cutting, _) in PATCH_CUTS {
                let f = PatchForm {
                    border,
                    cutting,
                    ..Default::default()
                };
                let Some(r) = f.request(&selection) else {
                    assert!(!border && cutting == "none");
                    continue;
                };
                let mut s = session();
                let before = s.design().clone();
                let result = s
                    .execute(r.command, Some(&r.params))
                    .unwrap_or_else(|e| panic!("{border} {cutting}: {e}"));
                let made = usize::from(border) + usize::from(cutting != "none");
                assert_eq!(CommandRequest::created_ids(&result).len(), made);
                s.execute("history.undo", None).expect("undo");
                assert_eq!(s.design(), &before);
            }
        }
        let r = PatchForm::default()
            .request(&BTreeSet::from([1]))
            .expect("request");
        assert_eq!(
            r.params,
            json!({"ids": [1], "border": true, "width": 3.0, "density": 4.5, "distance": 5.0,
                   "cutting": "running", "cutDistance": 3.0})
        );
    }

    #[test]
    fn cutwork_settings_apply_in_one_command() {
        let selection = BTreeSet::from([1]);
        for k in 0..32u32 {
            let bit = |b: u32| k & (1 << b) != 0;
            let f = CutworkForm {
                stabilize: bit(0),
                needles: bit(1),
                secure: bit(2),
                net: bit(3),
                cover: bit(4),
                net_pattern: 1 + k % 5,
                ..Default::default()
            };
            let r = f.request(&selection).expect("request");
            let mut s = session();
            let before = s.design().clone();
            let result = s
                .execute(r.command, Some(&r.params))
                .unwrap_or_else(|e| panic!("{f:?}: {e}"));
            let made = 1
                + [bit(0), bit(2), bit(3)].iter().filter(|&&b| b).count()
                + 2 * usize::from(bit(4));
            assert_eq!(CommandRequest::created_ids(&result).len(), made, "{f:?}");
            s.execute("history.undo", None).expect("undo");
            assert_eq!(s.design(), &before);
        }
        let r = CutworkForm::default().request(&selection).expect("request");
        assert_eq!(
            r.params,
            json!({"ids": [1], "stabilize": true, "cutting": "cutting", "secureStabilizer": true,
                   "netFill": true, "netPattern": 1, "netSpacing": 3.0, "netDirection": 0.0,
                   "cover": true, "coverWidth": 4.0, "coverDensity": 6.5})
        );
    }

    #[test]
    fn background_request_carries_the_settings() {
        let mut f = BackgroundForm {
            area: Some(2),
            spacing: 25.0,
            ..Default::default()
        };
        assert!(f.request(&BTreeSet::from([2])).is_none(), "only the area");
        let r = f.request(&BTreeSet::from([1, 2])).expect("request");
        assert_eq!(
            r.params,
            json!({"ids": [1], "area": 2, "fillType": "echo", "spacing": 25.0, "sew": "running",
                   "pitch": 2.5, "runTimes": 1, "echoType": "clipped", "linkEchoEnds": false})
        );
        f.set_fill_type("stippling");
        assert_eq!(f.spacing, 7.0, "clamped to the stippling range");
        let r = f.request(&BTreeSet::from([1])).expect("request");
        assert_eq!(r.params["stippleSpacing"], 2.0);
        assert_eq!(r.params["skipOverlap"], true);
        assert!(r.params.get("sew").is_none());
    }
}
