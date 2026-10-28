use crate::fonts::DesignFont;
use crate::geometry::{Contour, Rect, Transform, Vec2, v2};
use crate::machine::MachineSettings;
use crate::patterns::{self, Pattern, PatternKind};
use crate::sewing::SewingStitch;
use crate::text::{self, Layout, TextError, TextSpec};
use openstitch_core::{Command, Stitch, StitchPattern, Thread};
use serde::{Deserialize, Serialize};

/// Editable embroidery document, distinct from the generated [`StitchPattern`]: it keeps
/// outlines and sewing attributes so stitches can be regenerated after edits.
/// Coordinates are 0.1 mm, +y down, origin at the Design Page's top-left corner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    pub name: String,
    pub page: DesignPage,
    pub objects: Vec<DesignObject>,
    /// Serialized as `nextObjectID` (the `.osd` field name).
    #[serde(rename = "nextObjectID")]
    pub next_object_id: u64,
    /// Design Settings: machine type, hoop, sewing area, trimming. Omitted while default.
    #[serde(default, skip_serializing_if = "MachineSettings::is_default")]
    pub machine: MachineSettings,
    /// Reference image shown behind the design (never sewn). Omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<BackgroundImage>,
    /// Fonts the design carries (user TrueType fonts), referred to by text objects. Omitted
    /// while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<DesignFont>,
    /// User stitch patterns (Programmable Stitch Creator) the design uses or carries, so the
    /// project sews the same anywhere. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<Pattern>,
    /// Sewing-machine stitch patterns made in the Stitch Design Factory, kept with the
    /// project (never sewn by the embroidery compiler). Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sewing: Vec<SewingStitch>,
    /// Guidelines (view aid, never sewn). Omitted while empty.
    #[serde(default, skip_serializing_if = "Guides::is_empty")]
    pub guides: Guides,
    /// Design Property: category, author, keywords, comments. Omitted while empty.
    #[serde(default, skip_serializing_if = "DesignProperties::is_empty")]
    pub properties: DesignProperties,
}

/// A raster image placed on the Design Page as a tracing reference (PE-DESIGN's background
/// image). The file is referenced, not embedded; units are 0.1 mm like the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundImage {
    /// Absolute path of the image file.
    pub path: String,
    /// Pixel size of the image when it was placed.
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f64,
    pub visible: bool,
    /// Rotation, crop and colour changes applied to the file's pixels (Image ▸ Rotate Right /
    /// Left, Modify Image); the file itself is never changed. Omitted while unchanged.
    #[serde(default, skip_serializing_if = "is_default")]
    pub adjust: ImageAdjust,
}

/// How the background image is shown, applied in this order: turned, cropped, then colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageAdjust {
    /// Quarter turns clockwise, 0–3.
    pub rotation: u8,
    /// Cut off the turned image: left, top, right, bottom, each a fraction of its side.
    pub crop: [f64; 4],
    /// −100 to 100; 0 = as in the file.
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
}

impl ImageAdjust {
    /// Largest crop on one side, and the smallest part of the image a crop keeps.
    pub const MAX_CROP: f64 = 0.9;
    pub const MIN_KEPT: f64 = 0.05;
}

/// Guidelines dragged out of the rulers (View ▸ Guideline), saved with the design; 0.1 mm
/// from the Design Page origin. Omitted while there are none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Guides {
    /// x of the vertical guidelines (dragged from the top ruler).
    pub vertical: Vec<f64>,
    /// y of the horizontal guidelines (dragged from the left ruler).
    pub horizontal: Vec<f64>,
}

impl Guides {
    /// At most this many guidelines in each direction.
    pub const MAX: usize = 100;

    pub fn is_empty(&self) -> bool {
        self.vertical.is_empty() && self.horizontal.is_empty()
    }
}

/// Design Property text fields, written to the PES v4+ header next to the design name and
/// read back from it. Each field is omitted while empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesignProperties {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub category: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub keywords: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comments: String,
}

impl DesignProperties {
    /// Field names, as `StitchPattern` metadata keys and command parameters.
    pub const KEYS: [&str; 4] = ["category", "author", "keywords", "comments"];
    /// A PES header string holds at most this many bytes.
    pub const MAX_BYTES: usize = 255;

    pub fn is_empty(&self) -> bool {
        self.fields().iter().all(|(_, v)| v.is_empty())
    }

    /// `(key, value)` in [`DesignProperties::KEYS`] order.
    pub fn fields(&self) -> [(&'static str, &str); 4] {
        [
            ("category", &self.category),
            ("author", &self.author),
            ("keywords", &self.keywords),
            ("comments", &self.comments),
        ]
    }

    pub fn field_mut(&mut self, key: &str) -> Option<&mut String> {
        match key {
            "category" => Some(&mut self.category),
            "author" => Some(&mut self.author),
            "keywords" => Some(&mut self.keywords),
            "comments" => Some(&mut self.comments),
            _ => None,
        }
    }

    /// The fields found in a stitch pattern's metadata (as a PES file supplies them).
    pub fn from_metadata(metadata: &std::collections::BTreeMap<String, String>) -> Self {
        let mut p = Self::default();
        for key in Self::KEYS {
            if let (Some(v), Some(f)) = (metadata.get(key), p.field_mut(key)) {
                f.clone_from(v);
            }
        }
        p
    }
}

impl Default for Design {
    fn default() -> Self {
        Self::new("Untitled", DesignPage::default())
    }
}

impl Design {
    pub fn new(name: impl Into<String>, page: DesignPage) -> Self {
        Design {
            name: name.into(),
            page,
            objects: Vec::new(),
            next_object_id: 1,
            machine: MachineSettings::default(),
            background: None,
            fonts: Vec::new(),
            patterns: Vec::new(),
            sewing: Vec::new(),
            guides: Guides::default(),
            properties: DesignProperties::default(),
        }
    }

    /// Adds `object` with a fresh id and returns that id.
    pub fn add(&mut self, mut object: DesignObject) -> u64 {
        object.id = self.next_object_id;
        self.next_object_id += 1;
        let id = object.id;
        self.objects.push(object);
        id
    }

    /// User pattern `id` of `kind` carried by this design.
    pub fn user_pattern(&self, kind: PatternKind, id: &str) -> Option<&Pattern> {
        self.patterns.iter().find(|p| p.kind == kind && p.id == id)
    }

    /// Pattern `id` of `kind` that this design can sew: its own, or a bundled one.
    pub fn find_pattern(&self, kind: PatternKind, id: &str) -> Option<&Pattern> {
        self.user_pattern(kind, id)
            .or_else(|| patterns::find(kind, id))
    }

    /// Ids of objects whose sewing uses pattern `id` of `kind`.
    pub fn pattern_users(&self, kind: PatternKind, id: &str) -> Vec<u64> {
        self.objects
            .iter()
            .filter(|o| {
                let region = o.region.as_ref().is_some_and(|r| {
                    r.pattern.as_deref() == Some(id)
                        && match kind {
                            PatternKind::Fill => r.kind == RegionType::ProgrammableFill,
                            PatternKind::Motif => r.kind == RegionType::MotifFill,
                            PatternKind::Decorative => r.kind == RegionType::DecorativeFill,
                        }
                });
                let line = o.line.as_ref().is_some_and(|l| {
                    kind == PatternKind::Motif
                        && l.kind == LineType::Motif
                        && l.motif.as_deref() == Some(id)
                });
                region || line
            })
            .map(|o| o.id)
            .collect()
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    /// A fresh group id (shares the object id counter, so it never collides with either).
    pub fn new_group_id(&mut self) -> u64 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    /// `ids` plus every other member of their groups, in sewing order, without duplicates.
    /// Unknown ids are dropped.
    pub fn with_groups(&self, ids: &[u64]) -> Vec<u64> {
        let groups: Vec<u64> = ids
            .iter()
            .filter_map(|&id| self.objects[self.index_of(id)?].group)
            .collect();
        self.objects
            .iter()
            .filter(|o| ids.contains(&o.id) || o.group.is_some_and(|g| groups.contains(&g)))
            .map(|o| o.id)
            .collect()
    }

    /// Clears groups left with fewer than two members (after deletes or shape operations).
    pub fn dissolve_single_groups(&mut self) {
        let mut counts = std::collections::HashMap::new();
        for g in self.objects.iter().filter_map(|o| o.group) {
            *counts.entry(g).or_insert(0usize) += 1;
        }
        for o in &mut self.objects {
            if o.group.is_some_and(|g| counts[&g] < 2) {
                o.group = None;
            }
        }
    }

    /// The contour text on a path follows: the first contour of shape object `id`, from the
    /// shape's start point (its line's entry, else its region's), as in PE-DESIGN, where the
    /// outline's start is where left-aligned fitted text begins.
    pub fn path_contour(&self, id: u64) -> Option<Contour> {
        let o = &self.objects[self.index_of(id)?];
        let c = (o.kind == ObjectKind::Shape).then(|| o.contours.first())??;
        let start = o
            .line
            .as_ref()
            .and_then(|l| l.entry)
            .or_else(|| o.region.as_ref().and_then(|r| r.entry));
        Some(match start {
            Some(p) if c.closed => c.starting_at(p),
            _ => c.clone(),
        })
    }

    /// Lays the text of object `index` out again from its [`TextSpec`], following its path
    /// object if it has one. A text whose path object no longer exists is detached first.
    pub fn layout_text(&mut self, index: usize) -> Result<Layout, TextError> {
        let Some(mut spec) = self.objects[index].text.clone() else {
            return Ok(Layout::default());
        };
        if spec.path.is_some_and(|id| self.path_contour(id).is_none()) {
            self.detach_text(index);
            return self.layout_text(index);
        }
        let path = spec.path.and_then(|id| self.path_contour(id));
        let layout = text::layout_in(&spec, path.as_ref(), &self.fonts)?;
        spec.glyphs = layout.glyph_counts();
        let o = &mut self.objects[index];
        o.contours = layout.contours();
        o.stitch_block = layout.stitch_block();
        o.text = Some(spec);
        Ok(layout)
    }

    /// Takes the text of object `index` off its path; it continues as straight text whose
    /// baseline starts (or, centred / right-aligned, centres or ends) where it stood.
    pub fn detach_text(&mut self, index: usize) {
        let o = &mut self.objects[index];
        let bounds = Rect::enclosing(o.contours.iter().flat_map(Contour::outline_points));
        let Some(spec) = &mut o.text else { return };
        spec.path = None;
        if let Some(b) = bounds {
            let x = match spec.align {
                text::TextAlign::Left => b.min_x,
                text::TextAlign::Center => b.center().x,
                text::TextAlign::Right => b.max_x,
            };
            spec.transform = Transform::translation(x, b.max_y);
        }
    }

    /// Re-lays out every text attached to a path, so it follows edits to the path. Texts whose
    /// font is unknown keep their contours.
    pub fn relayout_text(&mut self) {
        for i in 0..self.objects.len() {
            let o = &self.objects[i];
            if o.kind == ObjectKind::Text && o.text.as_ref().is_some_and(|t| t.path.is_some()) {
                let _ = self.layout_text(i);
            }
        }
    }
}

/// Design Page (PE-DESIGN term): the hoop-sized working area, 0.1 mm.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignPage {
    pub width: f64,
    pub height: f64,
}

impl Default for DesignPage {
    fn default() -> Self {
        DesignPage {
            width: 1000.0,
            height: 1000.0,
        }
    }
}

impl DesignPage {
    pub fn center(&self) -> Vec2 {
        v2(self.width / 2.0, self.height / 2.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    /// Outline-based: stitches are generated from contours and sewing attributes.
    Shape,
    /// Fixed needle points (imported embroidery data); only transforms apply.
    Stitches,
    /// Lettering: contours are laid out from [`DesignObject::text`] and sewn like a shape,
    /// glyph by glyph.
    Text,
}

/// One editable pattern (PE-DESIGN's shape, text and stitch patterns).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignObject {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub kind: ObjectKind,
    #[serde(default)]
    pub contours: Vec<Contour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stitch_block: Option<StitchBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<RegionSewing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSewing>,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Hole sewing: the region is sewn around its paired inner shape ([`Self::hole_inner`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hole_sewing: bool,
    /// Hole sewing: the inner object of the pair, whose shape this region is sewn around
    /// (`None` in designs saved before pairs were stored; see [`crate::holes::paired_inner`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_inner: Option<u64>,
    /// Text objects only: what the contours were laid out from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextSpec>,
    /// Group this object belongs to (PE-DESIGN grouping): selected, moved and transformed with
    /// the other members. Group ids come from the same counter as object ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Jump-stitch trimming for this object on multi-needle machines (`None` = on, PE-DESIGN's
    /// default). Off: travel into and within the object is never trimmed; a colour change
    /// still cuts the thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_jumps: Option<bool>,
    /// The first letter of a monogram keeps what the monogram was made from, so
    /// `monogram.edit` can rebuild its group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monogram: Option<Box<crate::monogram::MonogramSpec>>,
}

fn yes() -> bool {
    true
}

impl DesignObject {
    pub fn shape(
        contours: Vec<Contour>,
        region: Option<RegionSewing>,
        line: Option<LineSewing>,
    ) -> Self {
        DesignObject {
            id: 0,
            name: None,
            kind: ObjectKind::Shape,
            contours,
            stitch_block: None,
            region,
            line,
            visible: true,
            hole_sewing: false,
            hole_inner: None,
            text: None,
            group: None,
            trim_jumps: None,
            monogram: None,
        }
    }

    pub fn stitches(block: StitchBlock) -> Self {
        DesignObject {
            id: 0,
            name: None,
            kind: ObjectKind::Stitches,
            contours: Vec::new(),
            stitch_block: Some(block),
            region: None,
            line: None,
            visible: true,
            hole_sewing: false,
            hole_inner: None,
            text: None,
            group: None,
            trim_jumps: None,
            monogram: None,
        }
    }

    /// A text object laid out from `spec` (straight, `spec.path` ignored: use
    /// [`Design::relayout_text`] for text on a path).
    pub fn text(
        spec: TextSpec,
        region: Option<RegionSewing>,
        line: Option<LineSewing>,
    ) -> Result<(Self, Layout), TextError> {
        let layout = text::layout(&spec, None)?;
        let mut object = DesignObject::shape(layout.contours(), region, line);
        object.kind = ObjectKind::Text;
        object.stitch_block = layout.stitch_block();
        object.text = Some(TextSpec {
            glyphs: layout.glyph_counts(),
            ..spec
        });
        Ok((object, layout))
    }

    /// Outline-based (shape or text): sewn from contours and sewing attributes.
    pub fn is_outline(&self) -> bool {
        matches!(self.kind, ObjectKind::Shape | ObjectKind::Text)
    }

    /// The contours of each glyph of a text object (one group holding every contour for a
    /// shape, or when the stored glyph counts do not match the contours).
    pub fn glyph_groups(&self) -> Vec<&[Contour]> {
        let counts = self
            .text
            .as_ref()
            .map(|t| t.glyphs.as_slice())
            .unwrap_or(&[]);
        let total: u64 = counts.iter().map(|&n| u64::from(n)).sum();
        if counts.is_empty() || total != self.contours.len() as u64 {
            return vec![&self.contours];
        }
        let mut rest = self.contours.as_slice();
        counts
            .iter()
            .map(|&n| {
                let (head, tail) = rest.split_at(n as usize);
                rest = tail;
                head
            })
            .collect()
    }

    pub fn has_closed_contour(&self) -> bool {
        self.contours.iter().any(|c| c.closed)
    }

    /// Every geometric point, for bounds and transforms.
    pub fn points(&self) -> Vec<Vec2> {
        match self.kind {
            ObjectKind::Shape => self
                .contours
                .iter()
                .flat_map(|c| c.outline_points())
                .collect(),
            // Text: outlines plus the needle points of stitch glyphs (user-mapped text).
            ObjectKind::Text => self
                .contours
                .iter()
                .flat_map(|c| c.outline_points())
                .chain(
                    self.stitch_block
                        .iter()
                        .flat_map(|b| b.stitches.iter().map(|s| v2(s.x, s.y))),
                )
                .collect(),
            ObjectKind::Stitches => self
                .stitch_block
                .as_ref()
                .map(|b| b.stitches.iter().map(|s| v2(s.x, s.y)).collect())
                .unwrap_or_default(),
        }
    }

    pub fn bounds(&self) -> Option<Rect> {
        Rect::enclosing(self.points())
    }

    pub fn apply(&mut self, t: &Transform) {
        for c in &mut self.contours {
            *c = c.transformed(t);
        }
        if let Some(spec) = &mut self.text {
            spec.transform = spec.transform.then(t);
        }
        if let Some(m) = &mut self.monogram {
            m.center = t.apply(m.center);
        }
        if let Some(r) = &mut self.region {
            r.entry = r.entry.map(|p| t.apply(p));
            r.exit = r.exit.map(|p| t.apply(p));
            r.center = r.center.map(|p| t.apply(p));
            r.reference = r.reference.map(|p| t.apply(p));
            r.direction_curve = r.direction_curve.as_ref().map(|c| c.transformed(t));
            for st in &mut r.stamps {
                *st = st.transformed(t);
            }
            for l in &mut r.direction_lines {
                *l = [t.apply(l[0]), t.apply(l[1])];
            }
            r.skeleton = r.skeleton.as_ref().map(|s| s.transformed(t));
        }
        if let Some(l) = &mut self.line {
            l.entry = l.entry.map(|p| t.apply(p));
        }
        if let Some(block) = &mut self.stitch_block {
            for s in &mut block.stitches {
                let p = t.apply(v2(s.x, s.y));
                s.x = p.x;
                s.y = p.y;
            }
        }
    }
}

/// Imported, fixed stitches with one thread per colour block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StitchBlock {
    pub stitches: Vec<Stitch>,
    pub threads: Vec<Thread>,
}

impl From<&StitchPattern> for StitchBlock {
    fn from(p: &StitchPattern) -> Self {
        StitchBlock {
            stitches: p
                .stitches
                .iter()
                .filter(|s| s.command != Command::End)
                .copied()
                .collect(),
            threads: p.threads.clone(),
        }
    }
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegionType {
    Fill,
    Satin,
    ProgrammableFill,
    MotifFill,
    Piping,
    CrossStitch,
    ConcentricCircle,
    Radial,
    Spiral,
    FlexibleSpiral,
    Stippling,
    NetFill,
    ZigzagNetFill,
    DecorativeFill,
}

impl RegionType {
    /// Accepted density, rows (lines) per mm. Fill and satin take PE-DESIGN 11's range,
    /// 0.1–14.0 (its Density field clamps there); the other types keep 0.5–10.
    pub fn density_range(self) -> (f64, f64) {
        match self {
            RegionType::Fill | RegionType::Satin => (0.1, 14.0),
            _ => (0.5, 10.0),
        }
    }

    pub const ALL: [RegionType; 14] = [
        RegionType::Fill,
        RegionType::Satin,
        RegionType::ProgrammableFill,
        RegionType::MotifFill,
        RegionType::Piping,
        RegionType::CrossStitch,
        RegionType::ConcentricCircle,
        RegionType::Radial,
        RegionType::Spiral,
        RegionType::FlexibleSpiral,
        RegionType::Stippling,
        RegionType::NetFill,
        RegionType::ZigzagNetFill,
        RegionType::DecorativeFill,
    ];

    /// The JSON / command name (`"programmableFill"`).
    pub fn id(self) -> &'static str {
        match self {
            RegionType::Fill => "fill",
            RegionType::Satin => "satin",
            RegionType::ProgrammableFill => "programmableFill",
            RegionType::MotifFill => "motifFill",
            RegionType::Piping => "piping",
            RegionType::CrossStitch => "crossStitch",
            RegionType::ConcentricCircle => "concentricCircle",
            RegionType::Radial => "radial",
            RegionType::Spiral => "spiral",
            RegionType::FlexibleSpiral => "flexibleSpiral",
            RegionType::Stippling => "stippling",
            RegionType::NetFill => "netFill",
            RegionType::ZigzagNetFill => "zigzagNetFill",
            RegionType::DecorativeFill => "decorativeFill",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Display name (PE-DESIGN's term).
    pub fn label(self) -> &'static str {
        match self {
            RegionType::Fill => "Fill",
            RegionType::Satin => "Satin",
            RegionType::ProgrammableFill => "Programmable fill",
            RegionType::MotifFill => "Motif fill",
            RegionType::Piping => "Piping",
            RegionType::CrossStitch => "Cross stitch",
            RegionType::ConcentricCircle => "Concentric circle",
            RegionType::Radial => "Radial",
            RegionType::Spiral => "Spiral",
            RegionType::FlexibleSpiral => "Flexible spiral",
            RegionType::Stippling => "Stippling",
            RegionType::NetFill => "Net fill",
            RegionType::ZigzagNetFill => "Zigzag net fill",
            RegionType::DecorativeFill => "Decorative fill",
        }
    }
}

/// Underlay (under sewing) of fill and satin regions, when `underlay` is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnderlayStyle {
    /// Running stitch along the outline.
    #[default]
    Edge,
    /// Sparse rows across the region at the sewing direction, inset from the edge.
    Zigzag,
    /// Edge run, then zigzag rows.
    EdgeZigzag,
    /// Two crossing layers of sparse rows (±45° to the sewing direction).
    Double,
    /// PE-DESIGN's fill under sewing: a running stitch 1 mm inside the outline, then tatami
    /// rows across the sewing direction, 1 mm inside it (see [`crate::region`]).
    EdgeFill,
}

/// Which row ends a feathered edge roughens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatherSide {
    #[default]
    Start,
    End,
    Both,
}

/// How fill and satin move between separate parts of one region (PE-DESIGN Running Stitch
/// Path). Every value travels with running stitches; none jumps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunningStitchPath {
    /// The shortest path inside the region.
    #[default]
    Inside,
    /// Along the outline, just inside it.
    AlongOutline,
    /// On the outline.
    OnOutline,
}

impl RunningStitchPath {
    pub fn id(self) -> &'static str {
        match self {
            RunningStitchPath::Inside => "inside",
            RunningStitchPath::AlongOutline => "alongOutline",
            RunningStitchPath::OnOutline => "onOutline",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [
            RunningStitchPath::Inside,
            RunningStitchPath::AlongOutline,
            RunningStitchPath::OnOutline,
        ]
        .into_iter()
        .find(|p| p.id() == id)
    }
}

/// Reads `runningStitchPath`, or the old `travel` flag (true: inside, false: jumps).
fn running_stitch_path<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<RunningStitchPath>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Travel(bool),
        Path(RunningStitchPath),
    }
    Ok(match Option::<Stored>::deserialize(d)? {
        None | Some(Stored::Travel(false)) => None,
        Some(Stored::Travel(true)) => Some(RunningStitchPath::Inside),
        Some(Stored::Path(p)) => Some(p),
    })
}

/// Gradation pattern (PE-DESIGN's Gradation Pattern Setting): `steps` values, evenly spaced
/// across the rows from `max` % of the region's density at the start side to `min` % at the
/// far side, with the density interpolated between them. PE-DESIGN 11.50's default pattern
/// sews that way: its rows' density falls linearly from 53 % to 5 % across the region
/// (region-travel-u-gradation reference), not in equal bands. With evenly spaced step values
/// the steps do not change the density; they are kept for the pattern's own values. Without
/// `steps` the gradation is how designs saved before patterns (a single far-side percentage,
/// stored as a number) sew, and still read: from 100 % down to that percentage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gradation {
    /// Density at the start side, % of the region's density (5–100).
    pub max: f64,
    /// Density at the far side, % (5–100).
    pub min: f64,
    /// Number of step values (2–20); `None`: a gradation saved before patterns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
}

impl Gradation {
    /// PE-DESIGN 11's default pattern (the first one): Max 53 %, Min 5 %, nine steps
    /// (53/47/41/35/29/23/17/11/5 %), as its reference file sews it (fill-analysis).
    pub const PE_DEFAULT: Gradation = Gradation {
        max: 53.0,
        min: 5.0,
        steps: Some(9),
    };

    /// The gradation of designs saved before patterns: 100 % falling continuously to `far` %.
    pub fn continuous(far: f64) -> Self {
        Gradation {
            max: 100.0,
            min: far,
            steps: None,
        }
    }

    /// Density at `f` (0 = start side, 1 = far side) as a fraction of the region's density:
    /// linear between the evenly spaced step values, so linear from `max` to `min`.
    pub fn fraction(&self, f: f64) -> f64 {
        let t = f.clamp(0.0, 1.0);
        ((self.max + (self.min - self.max) * t) / 100.0).clamp(0.05, 1.0)
    }
}

impl<'de> Deserialize<'de> for Gradation {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Pattern {
            max: f64,
            min: f64,
            #[serde(default)]
            steps: Option<u32>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Far(f64),
            Pattern(Pattern),
        }
        Ok(match Stored::deserialize(d)? {
            Stored::Far(g) => Gradation::continuous(g),
            Stored::Pattern(p) => Gradation {
                max: p.max,
                min: p.min,
                steps: p.steps,
            },
        })
    }
}

/// Feathered edge: row ends shortened by a deterministic pseudo-random amount up to `width`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feather {
    pub side: FeatherSide,
    /// mm.
    pub width: f64,
}

/// Region sewing of a closed shape. mm and degrees; defaults are placeholders until calibrated
/// against PE-DESIGN reference output. Every attribute after `underlay` is optional and omitted
/// from JSON when unset, so designs that do not use it serialize as before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSewing {
    #[serde(rename = "type")]
    pub kind: RegionType,
    pub thread: Thread,
    /// Sewing direction, degrees counter-clockwise on screen as in PE-DESIGN (0 = horizontal
    /// rows, 45 = rows rising to the right). `.osd` version 1 stored it clockwise.
    pub angle: f64,
    /// Rows per mm.
    pub density: f64,
    /// Longest fill stitch, mm.
    pub stitch_length: f64,
    /// Row end extension, mm.
    pub pull_compensation: f64,
    /// Underlay (style in `underlay_style`).
    pub underlay: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub underlay_style: UnderlayStyle,
    /// Distance between underlay rows, mm (default 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlay_spacing: Option<f64>,
    /// Satin and fill: choose the direction from the region's shape (stitches across its
    /// narrow side) instead of `angle`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_direction: bool,
    /// Gradation: the density changes across the region, from `max` % of `density` at the
    /// start side to `min` % at the far side (see [`Gradation`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradation: Option<Gradation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feather: Option<Feather>,
    /// Satin: rows longer than this (mm) are split into staggered stitches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_length: Option<f64>,
    /// Move between separated parts with running stitches (Running Stitch Path); `None` jumps,
    /// as fill and satin did before (designs saved with `travel` off or unset).
    #[serde(
        default,
        alias = "travel",
        deserialize_with = "running_stitch_path",
        skip_serializing_if = "Option::is_none"
    )]
    pub running_stitch_path: Option<RunningStitchPath>,
    /// Pattern id: programmable fill, motif fill, net, zigzag net and decorative fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Pattern size, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_height: Option<f64>,
    /// Spacing, mm: cross size, stippling, net and flexible spiral spacing, motif gaps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// Running stitch length of line-based fills, mm (default 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_pitch: Option<f64>,
    /// Fill, piping and programmable-fill base: needle shift per row, % of a stitch (0-50).
    /// New fills take PE-DESIGN 11's 30; unset (older files) keeps the ⅓ stagger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<f64>,
    /// Programmable / motif fill: shift of every other tile row (column), % of the tile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_offset: Option<f64>,
    /// Concentric circle, spiral and flexible spiral start at the centre instead of outside.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub start_inside: bool,
    /// Spirals turn counter-clockwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub counterclockwise: bool,
    /// Cross stitch: each diagonal sewn three times.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub triple: bool,
    /// Decorative fill: random shift of needle points, % of the run pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_shift: Option<f64>,
    /// Programmable fill: sew a plain fill underneath (base sewing).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub base_sewing: bool,
    /// Region entry point (PE-DESIGN R1), 0.1 mm; snapped to the outline when sewn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Vec2>,
    /// Region exit point (PE-DESIGN R2), 0.1 mm; snapped to the outline when sewn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Vec2>,
    /// Centre point of concentric circle, radial and spiral stitches, 0.1 mm (default: the
    /// centre of the region's bounding box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<Vec2>,
    /// Reference point that anchors the pattern grid of programmable fill, motif fill, cross
    /// stitch, net fill, zigzag net fill and decorative fill, 0.1 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<Vec2>,
    /// Curved sewing direction ("round fill"): rows follow this open curve, shifted across
    /// the region. It must not turn back along its own chord.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction_curve: Option<Contour>,
    /// Embossing, engraving and stamps (fill, satin, programmable fill), applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stamps: Vec<crate::stamp::Stamp>,
    /// Several sewing-direction lines (Manual Punch): consecutive lines are the rungs of a
    /// column sewn along them (satin and fill). Each is [left end, right end].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direction_lines: Vec<[Vec2; 2]>,
    /// Curved Block (Manual Punch): the column's sides are smooth curves through the ends of
    /// [`RegionSewing::direction_lines`] instead of straight lines between them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub curved_lines: bool,
    /// Satin along a centre line with a width at each point: the stitch direction follows
    /// the line (variable-direction satin). Takes precedence over the other direction options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton: Option<crate::skeleton::Skeleton>,
}

impl RegionSewing {
    pub fn new(kind: RegionType, thread: Thread) -> Self {
        RegionSewing {
            kind,
            thread,
            // Guide: programmable fill "45° (default)"; the other types sew at 0°.
            angle: match kind {
                RegionType::ProgrammableFill => 45.0,
                _ => 0.0,
            },
            // PE-DESIGN 11.50 Sewing Attributes (reference files, fill-analysis): fill and satin
            // 4.5 lines/mm with under sewing on.
            density: match kind {
                RegionType::Fill | RegionType::Satin => 4.5,
                RegionType::ProgrammableFill => 2.5,
                RegionType::Piping => 1.0,
                _ => 1.5,
            },
            stitch_length: 4.0,
            pull_compensation: 0.0,
            underlay: matches!(
                kind,
                RegionType::Fill | RegionType::ProgrammableFill | RegionType::Satin
            ),
            underlay_style: match kind {
                RegionType::Fill | RegionType::ProgrammableFill => UnderlayStyle::EdgeFill,
                RegionType::Satin => UnderlayStyle::EdgeZigzag,
                _ => UnderlayStyle::Edge,
            },
            underlay_spacing: None,
            auto_direction: false,
            gradation: None,
            feather: None,
            split_length: None,
            running_stitch_path: Some(RunningStitchPath::Inside),
            pattern: None,
            pattern_width: None,
            pattern_height: None,
            spacing: None,
            run_pitch: None,
            // PE-DESIGN 11's Frequency default (its field shows 30 %).
            frequency: (kind == RegionType::Fill).then_some(30.0),
            row_offset: None,
            column_offset: None,
            start_inside: false,
            counterclockwise: false,
            triple: false,
            random_shift: None,
            base_sewing: false,
            entry: None,
            exit: None,
            center: None,
            reference: None,
            direction_curve: None,
            stamps: Vec::new(),
            direction_lines: Vec::new(),
            curved_lines: false,
            skeleton: None,
        }
    }

    /// Whether Running Stitch Path can be chosen: fill and satin without Auto direction or a
    /// feathered edge (and not on text, which the caller knows).
    pub fn running_stitch_path_available(&self) -> bool {
        matches!(self.kind, RegionType::Fill | RegionType::Satin)
            && !self.auto_direction
            && self.sewn_feather().is_none()
    }

    /// Why Gradation cannot be set on this region, if it cannot (the guide: a manual straight
    /// direction, no feathered edge; text, which the caller knows, is excluded too). PE-DESIGN
    /// also offers it for programmable fill, which OpenStitch does not sew yet.
    pub fn gradation_unavailable(&self) -> Option<&'static str> {
        if !matches!(self.kind, RegionType::Fill | RegionType::Satin) {
            Some("is only for fill and satin")
        } else if self.auto_direction {
            Some("is not available with Auto direction")
        } else if self.feather.is_some() {
            Some("is not available with a feathered edge")
        } else {
            None
        }
    }

    /// Why Feathered edge cannot be set on this region, if it cannot (the guide: not with
    /// gradation or Auto direction; text and hole sewing, which the caller knows, are excluded
    /// too). PE-DESIGN also offers it for programmable fill, which OpenStitch does not sew yet.
    pub fn feather_unavailable(&self) -> Option<&'static str> {
        if !matches!(self.kind, RegionType::Fill | RegionType::Satin) {
            Some("is only for fill and satin")
        } else if self.auto_direction {
            Some("is not available with Auto direction")
        } else if self.gradation.is_some() {
            Some("is not available with gradation")
        } else {
            None
        }
    }

    /// The gradation actually sewn: none where it is unavailable (another sew type, Auto
    /// direction). A stored gradation stays, so switching back restores it.
    pub fn sewn_gradation(&self) -> Option<&Gradation> {
        self.gradation.as_ref().filter(|_| {
            matches!(self.kind, RegionType::Fill | RegionType::Satin) && !self.auto_direction
        })
    }

    /// The feathered edge actually sewn: none where it is unavailable (another sew type, Auto
    /// direction, or with gradation, which wins in files that hold both).
    pub fn sewn_feather(&self) -> Option<Feather> {
        self.feather.filter(|_| {
            matches!(self.kind, RegionType::Fill | RegionType::Satin)
                && !self.auto_direction
                && self.gradation.is_none()
        })
    }

    /// The travel actually sewn between parts, `None` for jumps. Where the attribute is not
    /// available the region travels inside; gradation rules out Inside (Along outline then).
    pub fn travel_path(&self) -> Option<RunningStitchPath> {
        let chosen = self.running_stitch_path?;
        if !matches!(self.kind, RegionType::Fill | RegionType::Satin) {
            return Some(RunningStitchPath::Inside);
        }
        let path = if self.running_stitch_path_available() {
            chosen
        } else {
            RunningStitchPath::Inside
        };
        Some(match path {
            RunningStitchPath::Inside if self.sewn_gradation().is_some() => {
                RunningStitchPath::AlongOutline
            }
            p => p,
        })
    }

    /// Fill: needle shift from one row to the next, fraction of a stitch (default ⅓).
    pub fn fill_shift(&self) -> Option<f64> {
        self.frequency.map(|f| f.clamp(0.0, 50.0) / 100.0)
    }

    /// Spacing in mm, or the type's default.
    pub fn spacing_mm(&self) -> f64 {
        self.spacing.unwrap_or(match self.kind {
            RegionType::CrossStitch => 2.5,
            RegionType::Stippling => 2.0,
            RegionType::NetFill => 4.0,
            RegionType::ZigzagNetFill => 5.0,
            RegionType::FlexibleSpiral => 1.0,
            _ => 1.0,
        })
    }

    pub fn run_pitch_mm(&self) -> f64 {
        self.run_pitch.unwrap_or(2.0)
    }

    pub fn underlay_spacing_mm(&self) -> f64 {
        self.underlay_spacing.unwrap_or(2.0)
    }

    /// Pattern size in mm (width, height), defaults per type.
    pub fn pattern_size_mm(&self) -> (f64, f64) {
        let (w, h) = match self.kind {
            RegionType::MotifFill => (4.0, 3.0),
            RegionType::DecorativeFill => (10.0, 8.0),
            _ => (8.0, 8.0),
        };
        (
            self.pattern_width.unwrap_or(w),
            self.pattern_height.unwrap_or(h),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineType {
    Running,
    Triple,
    Zigzag,
    Motif,
    Stem,
    Candlewick,
    EStitch,
    VStitch,
    Chain,
    Cutting,
}

impl LineType {
    pub const ALL: [LineType; 10] = [
        LineType::Running,
        LineType::Triple,
        LineType::Zigzag,
        LineType::Motif,
        LineType::Stem,
        LineType::Candlewick,
        LineType::EStitch,
        LineType::VStitch,
        LineType::Chain,
        LineType::Cutting,
    ];

    pub fn id(self) -> &'static str {
        match self {
            LineType::Running => "running",
            LineType::Triple => "triple",
            LineType::Zigzag => "zigzag",
            LineType::Motif => "motif",
            LineType::Stem => "stem",
            LineType::Candlewick => "candlewick",
            LineType::EStitch => "eStitch",
            LineType::VStitch => "vStitch",
            LineType::Chain => "chain",
            LineType::Cutting => "cutting",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            LineType::Running => "Running",
            LineType::Triple => "Triple",
            LineType::Zigzag => "Zigzag",
            LineType::Motif => "Motif",
            LineType::Stem => "Stem",
            LineType::Candlewick => "Candlewicking",
            LineType::EStitch => "E stitch",
            LineType::VStitch => "V stitch",
            LineType::Chain => "Chain",
            LineType::Cutting => "Cutting",
        }
    }
}

/// How repeated elements (motifs, E/V strokes, chain links) sit on the line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Arrange {
    #[default]
    Normal,
    /// Flipped to the other side of the line.
    Mirror,
    /// Every other element flipped.
    Alternate,
}

/// How motif, stem, candlewicking and chain lines get from the entry point (L1) into the
/// pattern when the points were moved (PE-DESIGN's Start/End Type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartEnd {
    /// Start with the pattern, which begins and ends at the exit point (PE-DESIGN's default).
    #[default]
    Feed,
    /// Start at the entry point (L1) and run along the line to where the pattern begins.
    Run,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainShape {
    #[default]
    Triangle,
    Diamond,
}

/// Line (outline) sewing. Attributes after `density` are optional and omitted from JSON when
/// unset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSewing {
    #[serde(rename = "type")]
    pub kind: LineType,
    pub thread: Thread,
    /// Running/triple stitch length, mm.
    pub pitch: f64,
    /// Running passes (1–5); chain and E/V passes.
    pub run_times: u32,
    /// Width, mm: zigzag, motif, stem, candlewick size, E/V stroke, chain size.
    pub width: f64,
    /// Zigzag stitches per mm; candlewick spokes per knot ÷ 2.
    pub density: f64,
    /// Motif id from the library (motif lines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif: Option<String>,
    /// Motif height, mm (default 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Gap between motifs, stem stitches, knots or E/V strokes, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// Stem stitch angle to the line, degrees counter-clockwise on screen (45–135, default
    /// 45). `.osd` version 1 stored it clockwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Stem stitch: single instead of triple.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub single: bool,
    /// E stitch: times each stroke is sewn (1–3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_times: Option<u32>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub arrange: Arrange,
    #[serde(default, skip_serializing_if = "is_default")]
    pub chain_shape: ChainShape,
    /// Cutting lines: leave short uncut bridges so the fabric does not shift.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave_uncut: bool,
    /// Zigzag: pull every other inner-side point to the centre where a bend crowds them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub half_stitch: bool,
    /// Zigzag: under sewing, a running stitch a quarter of the width to each side of the line
    /// (PE-DESIGN's default). Older files have none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underlay: bool,
    /// Line entry point (PE-DESIGN L1), 0.1 mm: closed outlines start at the nearest point,
    /// open ones from the nearer end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Vec2>,
    /// Line exit point (PE-DESIGN L2), 0.1 mm, on the outline. The pattern of a closed stem,
    /// candlewick or chain line begins and ends there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Vec2>,
    /// Start/End Type of closed motif, stem, candlewick and chain lines with an entry point.
    #[serde(default, skip_serializing_if = "is_default")]
    pub start_end: StartEnd,
}

impl LineSewing {
    /// A line of `kind` with the attributes PE-DESIGN 11.50 gives that type (measured from
    /// compat/reference/stitches/line-*-default.pes): run pitch 2.0 mm for running, triple
    /// and E/V; zigzag 2.0 mm wide at 4.5 lines/mm with under sewing and half stitch; E/V
    /// strokes 2.5 mm every 5.0 mm; stem 2.0 mm every 1.5 mm at 45°; candlewicking knots of
    /// 5.0 mm (Medium: 24 points) 1.0 mm apart; chain triangles of 4.0 mm sewn 5 times.
    pub fn new(kind: LineType, thread: Thread) -> Self {
        let zigzag = kind == LineType::Zigzag;
        let stroke = matches!(kind, LineType::EStitch | LineType::VStitch);
        LineSewing {
            kind,
            thread,
            pitch: match kind {
                LineType::Running | LineType::Triple | LineType::Zigzag => 2.0,
                _ if stroke => 2.0,
                _ => 2.5,
            },
            run_times: if kind == LineType::Chain { 5 } else { 1 },
            width: match kind {
                LineType::Candlewick => 5.0,
                LineType::Chain => 4.0,
                _ if stroke => 2.5,
                _ => 2.0,
            },
            density: match kind {
                LineType::Zigzag => 4.5,
                LineType::Candlewick => 12.0,
                _ => 3.0,
            },
            motif: None,
            height: None,
            spacing: None,
            angle: None,
            single: false,
            stroke_times: None,
            arrange: Arrange::Normal,
            chain_shape: ChainShape::Triangle,
            leave_uncut: false,
            half_stitch: zigzag,
            underlay: zigzag,
            entry: None,
            exit: None,
            start_end: StartEnd::Feed,
        }
    }

    /// Spacing in mm, or the type's default.
    pub fn spacing_mm(&self) -> f64 {
        self.spacing.unwrap_or(match self.kind {
            LineType::Motif => 0.0,
            LineType::Stem => 1.5,
            LineType::Candlewick => 1.0,
            LineType::EStitch | LineType::VStitch => 5.0,
            _ => 1.0,
        })
    }
}
