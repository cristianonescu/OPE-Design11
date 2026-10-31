//! The design's editable objects as PE-DESIGN 11 `#PES0110` objects (`openstitch_formats::pes11`),
//! for `design.export` with `pesVersion` 11: one `CEmbLine` per shape, one `CEmbNText` per text
//! (Lato Bold), one `CEmbOne` per stitch pattern, a thread table in sewing order, the design's
//! groups and page size. What the
//! object list cannot hold yet fails with the object named, so a user can fall back to version 6
//! (stitches) knowingly.

use openstitch_core::{StitchPattern, Thread};
use openstitch_design::fmath::Det;
use openstitch_design::text::{
    self, Spread, TextAlign, TextDirection, TextMetrics, TextSpec, VAlign,
};
use openstitch_design::{
    Contour, Design, DesignObject, LineSewing, LineType, ObjectKind, Rect, RegionSewing,
    RegionType, RunningStitchPath, Transform,
};
use openstitch_formats::pes11::{
    Envelope, Fit, KIND_CURVE_AFTER, KIND_CURVE_BEFORE, Letter, Objects, PAGE_ORIGIN,
    RegionSewType, Shape, StitchGroup, Text, Vertex,
};

/// The `#PES0110` objects of `design`'s visible objects, in design order.
pub fn objects(design: &Design) -> Result<Objects, String> {
    let page = (mm(design.page.width).max(1), mm(design.page.height).max(1));
    // A 100 x 100 mm page is written as PE-DESIGN's own default, a Hoop Size page; other sizes
    // as a Custom Size page, which PE-DESIGN shows with a Hoop Settings tab and sections.
    let mut out = Objects {
        page_mm: Some(page),
        hoop_page: page == (100, 100),
        ..Objects::default()
    };
    let mut groups: Vec<(u64, Vec<u16>)> = Vec::new();
    // The list: every visible object. A hole-sewing object is paired with the shape it is sewn
    // around: that shape itself when it is not sewn (Modify Overlap > Set hole sewing: the
    // rings), else a Not Sewn copy of it placed after the outer object (a vector import's pair:
    // hole-sewing-pentagon-star, the pentagon, the star's copy, the star).
    let not_sewn = |o: &DesignObject| o.region.is_none() && o.line.is_none();
    let mut slots: Vec<(&DesignObject, Option<&DesignObject>)> = Vec::new();
    let mut direct: Vec<(u64, u64)> = Vec::new();
    for object in design.objects.iter().filter(|o| o.visible) {
        slots.push((object, None));
        if let Some(inner) = openstitch_design::holes::paired_inner(object, design) {
            if not_sewn(inner) && inner.visible {
                direct.push((object.id, inner.id));
            } else {
                slots.push((object, Some(inner)));
            }
        }
    }
    // List positions by object id, for the outlines texts are fitted to (a hole copy has none).
    let positions: Vec<u64> = slots
        .iter()
        .map(|(o, inner)| if inner.is_some() { u64::MAX } else { o.id })
        .collect();
    for (position, &(object, inner)) in slots.iter().enumerate() {
        let index = u16::try_from(position).map_err(|_| "too many objects".to_string())?;
        if let Some(inner) = inner {
            out.shapes.push(hole_copy(object, inner, &mut out.threads)?);
            if !out.holes.is_empty() {
                return Err(format!(
                    "object {}: a second hole sewing pair (PES 0110 holds one per file so far)",
                    object.id
                ));
            }
            out.holes.push([index - 1, index]);
            continue;
        }
        if object.kind == ObjectKind::Text {
            out.texts
                .push(text_object(object, index, &positions, &mut out.threads)?);
        } else if object.kind == ObjectKind::Stitches {
            out.stitch_groups
                .push(stitch_group(object, index, &mut out.threads)?);
        } else {
            out.shapes.push(shape(object, &mut out.threads)?);
            if let Some(r) = object
                .region
                .as_ref()
                .filter(|r| r.kind == RegionType::DecorativeFill)
            {
                let pattern = decorative_pattern(object.id, r, design)?;
                if let Some(s) = out.shapes.last_mut() {
                    s.decorative = Some(pattern);
                }
            }
            if let Some(r) = object
                .region
                .as_ref()
                .filter(|r| r.kind == RegionType::ProgrammableFill)
            {
                let pattern = fill_pattern(object.id, r, design)?;
                match out.fill_patterns.as_slice() {
                    [] => out.fill_patterns.push(pattern),
                    [only] if *only == pattern => {}
                    _ => {
                        return Err(format!(
                            "object {}: a second programmable fill pattern (PES 0110 holds one per file so far: which object field picks a pattern from the header's list is not mapped)",
                            object.id
                        ));
                    }
                }
            }
        }
        if let Some(g) = object.group {
            match groups.iter_mut().find(|(id, _)| *id == g) {
                Some((_, members)) => members.push(index),
                None => groups.push((g, vec![index])),
            }
        }
    }
    if out.shapes.is_empty() && out.texts.is_empty() && out.stitch_groups.is_empty() {
        return Err("the design has no visible objects".into());
    }
    // PE-DESIGN 11.50 groups every object of each owner file in one group (the Applique Wizard,
    // vector imports, Convert to Blocks, o-group-two-rects), and it refused a file whose one group
    // left an object out ("Unexpected file format": the appliqué export of 2026-10-30 grouped its
    // four lines without the square; the same file with all five grouped, or none, opens). So
    // only that layout is written: one group of every object. How PE-DESIGN stores other
    // grouping is not mapped, and such groups are left out of the file.
    let total = out.shapes.len() + out.texts.len() + out.stitch_groups.len();
    out.groups = match groups.as_slice() {
        [(_, members)] if members.len() == total => vec![members.clone()],
        _ => Vec::new(),
    };
    for (outer, inner) in direct {
        let at = |id: u64| positions.iter().position(|&p| p == id).map(|p| p as u16);
        if let (Some(o), Some(i)) = (at(outer), at(inner)) {
            if !out.holes.is_empty() {
                return Err(format!(
                    "object {outer}: a second hole sewing pair (PES 0110 holds one per file so far)"
                ));
            }
            out.holes.push([o, i]);
        }
    }
    if !out.holes.is_empty() && !groups.is_empty() {
        return Err(
            "a hole sewing pair in a design with groups: PES 0110's layout for both is not mapped yet"
                .into(),
        );
    }
    Ok(out)
}

/// The Not Sewn copy of `inner` that PE-DESIGN pairs with the hole-sewing `outer`: the inner
/// shape's outline, region and line not sewn, in the outer region's colour
/// (hole-sewing-pentagon-star's second object).
fn hole_copy(
    outer: &DesignObject,
    inner: &DesignObject,
    table: &mut Vec<Thread>,
) -> Result<Shape, String> {
    if inner.kind != ObjectKind::Shape {
        return Err(format!(
            "object {}: hole sewing around object {}, which is not a shape (not mapped in PES 0110)",
            outer.id, inner.id
        ));
    }
    let mut copy = inner.clone();
    copy.region = None;
    copy.line = None;
    copy.hole_sewing = false;
    copy.hole_inner = None;
    let mut s = shape(&copy, table)?;
    if let Some(r) = &outer.region {
        let colour = thread_index(table, &r.thread);
        s.region_color = colour;
        s.line_color = colour;
    }
    Ok(s)
}

fn mm(units: f64) -> u16 {
    (units / 10.0).round().clamp(0.0, 65535.0) as u16
}

/// Index of `thread` in the table, appended when new (compared by colour and catalogue number).
fn thread_index(table: &mut Vec<Thread>, thread: &Thread) -> u16 {
    // The Appliqué wizard's stage colours as PE-DESIGN's appliqué colours; any other thread
    // as PE-DESIGN writes the colour (BROTHER EMBROIDERY numbers for Brother chart colours).
    let thread =
        &applique_thread(thread).unwrap_or_else(|| openstitch_formats::pes11::as_pe_writes(thread));
    let same = |t: &Thread| {
        t.rgb_value() == thread.rgb_value() && t.catalog_number == thread.catalog_number
    };
    match table.iter().position(same) {
        Some(i) => i as u16,
        None => {
            table.push(thread.clone());
            (table.len() - 1) as u16
        }
    }
}

/// A stitch pattern as a `CEmbOne`: its needle points in runs, each colour block's thread added
/// to the table in sewing order (compat pes-0110-stitches, PE-DESIGN's re-saves of version 6
/// files).
fn stitch_group(
    object: &DesignObject,
    position: u16,
    table: &mut Vec<Thread>,
) -> Result<StitchGroup, String> {
    let id = object.id;
    let block = object
        .stitch_block
        .as_ref()
        .ok_or_else(|| format!("object {id} is a stitch pattern without stitches"))?;
    let pattern = StitchPattern {
        stitches: block.stitches.clone(),
        threads: block.threads.clone(),
        ..StitchPattern::default()
    };
    // A block past the last thread sews in the last one, as the compiler does; no thread is
    // black.
    let mut indices: Vec<u16> = block
        .threads
        .iter()
        .map(|t| thread_index(table, t))
        .collect();
    if indices.is_empty() {
        indices.push(thread_index(table, &Thread::rgb(0)));
    }
    let thread = |k: usize| indices.get(k).or(indices.last()).copied().unwrap_or(0);
    StitchGroup::from_pattern(&pattern, position, thread)
        .ok_or_else(|| format!("object {id} is a stitch pattern without stitches"))
}

/// The Appliqué wizard's stage threads as PE-DESIGN's Applique Wizard colours (catalogue
/// numbers 100 Applique Material, 101 Applique Position, 102 Applique; colour type 2, RGB 0, no
/// brand: a01-rect-applique). PE-DESIGN sews its tack-down and covering lines in 102; the
/// wizard's covering line keeps the design's colour.
fn applique_thread(t: &Thread) -> Option<Thread> {
    use crate::wizards::applique::{APPLIQUE, MATERIAL, POSITION, TACK_DOWN};
    let catalog = match t.name.as_deref()? {
        MATERIAL => "100",
        POSITION => "101",
        // TACK_DOWN: designs from before the tack-down took PE's Applique colour.
        APPLIQUE | TACK_DOWN => "102",
        _ => return None,
    };
    let mut pe = Thread::rgb(0);
    pe.catalog_number = Some(catalog.into());
    pe.kind = Some(openstitch_formats::pes11::APPLIQUE_KIND);
    Some(pe)
}

/// The decorative fill pattern of region `r` as PE-DESIGN keeps it in the object's pattern block
/// (deco-fill-1line-100mm): the free lines in a 1024-unit cell (the unit tile, y down, as
/// `.plf` sources are read) and the tile size in 0.01 mm. Row-chained decorative patterns and
/// tiles that aren't square are not mapped.
fn decorative_pattern(
    id: u64,
    r: &RegionSewing,
    design: &Design,
) -> Result<openstitch_formats::pes11::PesPattern, String> {
    use openstitch_design::patterns::{PatternKind, resolve};
    let pattern = resolve(
        &design.patterns,
        PatternKind::Decorative,
        r.pattern.as_deref(),
    );
    if !pattern.free_lines {
        return Err(format!(
            "object {id}: decorative pattern {} is chained in rows (PES 0110 holds free lines in a tile)",
            pattern.id
        ));
    }
    let (w, h) = r.pattern_size_mm();
    if (w - h).abs() > 1e-9 {
        return Err(format!(
            "object {id}: a decorative fill tile of {w} x {h} mm (PES 0110 stores one size; square tiles only)"
        ));
    }
    let cell = |v: f64| (v * 1024.0).round().clamp(-32768.0, 32767.0) as i16;
    Ok(openstitch_formats::pes11::PesPattern {
        size: (w * 100.0).round().clamp(0.0, 65535.0) as u16,
        polylines: pattern
            .paths
            .iter()
            .map(|path| path.iter().map(|p| (cell(p.x), cell(p.y))).collect())
            .collect(),
    })
}

/// The programmable fill pattern of region `r` as PE-DESIGN lists it in the header
/// (obj-progfill-owner-v): its tile's lines in a 1024-unit cell (a fill tile's unit square, y
/// down, as `.pas` sources are read), and the tile size in 0.01 mm (750 for the owner's
/// 7.5 mm V). Embossed or engraved areas, and a tile that isn't square, are not mapped.
fn fill_pattern(
    id: u64,
    r: &RegionSewing,
    design: &Design,
) -> Result<openstitch_formats::pes11::PesPattern, String> {
    use openstitch_design::patterns::{PatternKind, resolve};
    let pattern = resolve(&design.patterns, PatternKind::Fill, r.pattern.as_deref());
    if !pattern.regions.is_empty() {
        return Err(format!(
            "object {id}: programmable fill pattern {} has embossed or engraved areas (not mapped in PES 0110)",
            pattern.id
        ));
    }
    let (w, h) = r.pattern_size_mm();
    if (w - h).abs() > 1e-9 {
        return Err(format!(
            "object {id}: a programmable fill tile of {w} x {h} mm (PES 0110 stores one size; square tiles only)"
        ));
    }
    let cell = |v: f64| (v * 1024.0).round().clamp(-32768.0, 32767.0) as i16;
    Ok(openstitch_formats::pes11::PesPattern {
        size: (w * 100.0).round().clamp(0.0, 65535.0) as u16,
        polylines: pattern
            .paths
            .iter()
            .map(|path| path.iter().map(|p| (cell(p.x), cell(p.y))).collect())
            .collect(),
    })
}

fn shape(object: &DesignObject, table: &mut Vec<Thread>) -> Result<Shape, String> {
    let id = object.id;
    match object.kind {
        ObjectKind::Shape => {}
        ObjectKind::Text => unreachable!("texts go through text_object"),
        ObjectKind::Stitches => unreachable!("stitch patterns go through stitch_group"),
    }
    // A Manual Punch column (direction lines): a block object whose outline is the lines' first
    // ends in order, then their second ends back.
    if let Some(r) = object
        .region
        .as_ref()
        .filter(|r| r.direction_lines.len() >= 2)
    {
        return punch_block(object, r, table);
    }
    let [contour] = object.contours.as_slice() else {
        return Err(format!(
            "object {id} has {} outlines: a PES 0110 shape holds one (split it, or use version 6)",
            object.contours.len()
        ));
    };
    if contour.points.len() < 2 {
        return Err(format!("object {id} has fewer than two points"));
    }
    let mut s = Shape::with_defaults();
    s.closed = contour.closed;
    // Region first, then line: PE-DESIGN's thread table is in sewing order.
    match &object.region {
        Some(r) if contour.closed => region(id, r, &mut s, table)?,
        _ => s.region = RegionSewType::NotSewn,
    }
    match &object.line {
        Some(l) => line(id, l, &mut s, table)?,
        None => {
            s.line_sewn = false;
            s.line_type = Shape::LINE_NOT_SEWN;
        }
    }
    if s.region == RegionSewType::NotSewn {
        s.region_color = s.line_color;
    }
    if !s.line_sewn {
        s.line_color = s.region_color;
    }
    s.vertices = vertices(contour);
    s.bounds = bounds(contour);
    Ok(s)
}

/// A Manual Punch column as PE-DESIGN's block object: a closed `CEmbLine` with one vertex per
/// line end. A Straight Block's vertex is its own handle on both sides (m01, m03,
/// o-manual-straight); a Curved Block's sides are Bézier curves through the ends
/// ([`curve_side`]).
fn punch_block(
    object: &DesignObject,
    r: &RegionSewing,
    table: &mut Vec<Thread>,
) -> Result<Shape, String> {
    let id = object.id;
    let lines = &r.direction_lines;
    let k = u32::try_from(lines.len())
        .ok()
        .filter(|&k| k <= u32::from(u16::MAX) / 2)
        .ok_or_else(|| format!("object {id} has {} direction lines", lines.len()))?;
    let mut s = Shape::with_defaults();
    s.closed = true;
    // Line i joins vertex i and 2k − 1 − i; the sewing ends at the last line's second end.
    s.punch = (0..k).map(|i| [i, 2 * k - 1 - i]).collect();
    s.punch_exit = k as u16;
    region(id, r, &mut s, table)?;
    match &object.line {
        Some(l) => line(id, l, &mut s, table)?,
        None => {
            s.line_sewn = false;
            s.line_type = Shape::LINE_NOT_SEWN;
            s.line_color = s.region_color;
        }
    }
    let side = |pick: usize, reverse: bool| -> Vec<(f64, f64)> {
        let mut pts: Vec<(f64, f64)> = lines
            .iter()
            .map(|l| {
                let (x, y) = file_xy(l[pick]);
                (f64::from(x), f64::from(y))
            })
            .collect();
        if reverse {
            pts.reverse();
        }
        pts
    };
    for pts in [side(0, false), side(1, true)] {
        if r.curved_lines {
            s.vertices.extend(curve_side(&pts));
        } else {
            s.vertices.extend(pts.iter().map(|&(x, y)| {
                let (x, y) = (x as f32, y as f32);
                Vertex {
                    x,
                    y,
                    handles: [(x, y), (x, y)],
                    kind: 0,
                }
            }));
        }
    }
    // A curved side can bulge past its ends: the bounds are the outline's extent rounded
    // outward, as PE-DESIGN stores them (m10).
    let [x0, y0, x1, y1] = openstitch_formats::pes11::outline_extent(&s.vertices);
    s.bounds = [x0.floor(), y0.floor(), x1.ceil(), y1.ceil()].map(|v| v as i16);
    Ok(s)
}

/// One side of a Curved Block as PE-DESIGN draws it through the line ends `pts` (file units):
/// a Bézier curve whose handles at an inner point lie along the line from the point before to
/// the point after, each as long as the circular arc's handle over that segment, 2c / 3(1 +
/// cos a) for a chord c at angle a to the tangent; an end's handle is the mirror image of its
/// neighbour's across the segment's perpendicular bisector (a two-point side: a third of the
/// way along). Whole units, rounded. PE-DESIGN's own handles in m07, m09, m10, m11 and
/// o-manual-curve are all within one unit (0.1 mm) of these; how it rounds is not mapped. The
/// side's first vertex has kind `0x01`, its last `0x10`, the others `0x11`; the handles on the
/// straight segments joining the two sides are the vertex itself.
fn curve_side(pts: &[(f64, f64)]) -> Vec<Vertex> {
    let n = pts.len();
    let sub = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0, a.1 - b.1);
    let len = |v: (f64, f64)| v.0.hypot(v.1);
    // (arriving, leaving) handles, unrounded.
    let mut handles = vec![(pts[0], pts[0]); n];
    for i in 1..n.saturating_sub(1) {
        let d = sub(pts[i + 1], pts[i - 1]);
        let t = (d.0 / len(d), d.1 / len(d));
        let arc = |chord: (f64, f64)| {
            let c = len(chord);
            let cos = ((chord.0 * t.0 + chord.1 * t.1) / c).abs();
            2.0 * c / (3.0 * (1.0 + cos))
        };
        let (l_in, l_out) = (arc(sub(pts[i - 1], pts[i])), arc(sub(pts[i + 1], pts[i])));
        handles[i] = (
            (pts[i].0 - l_in * t.0, pts[i].1 - l_in * t.1),
            (pts[i].0 + l_out * t.0, pts[i].1 + l_out * t.1),
        );
    }
    // The ends: the neighbour's handle mirrored across the segment's perpendicular bisector.
    let mirror = |h: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        let ab = sub(b, a);
        let u = (ab.0 / len(ab), ab.1 / len(ab));
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let along = (h.0 - m.0) * u.0 + (h.1 - m.1) * u.1;
        (h.0 - 2.0 * along * u.0, h.1 - 2.0 * along * u.1)
    };
    if n == 2 {
        let third =
            |a: (f64, f64), b: (f64, f64)| (a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0);
        handles[0].1 = third(pts[0], pts[1]);
        handles[1] = (third(pts[1], pts[0]), pts[1]);
    } else if n > 2 {
        handles[0].1 = mirror(handles[1].0, pts[0], pts[1]);
        handles[n - 1] = (mirror(handles[n - 2].1, pts[n - 2], pts[n - 1]), pts[n - 1]);
    }
    let whole = |p: (f64, f64)| (p.0.round() as f32, p.1.round() as f32);
    (0..n)
        .map(|i| {
            let (x, y) = whole(pts[i]);
            Vertex {
                x,
                y,
                handles: [whole(handles[i].0), whole(handles[i].1)],
                kind: match i {
                    0 => KIND_CURVE_AFTER,
                    i if i == n - 1 => KIND_CURVE_BEFORE,
                    _ => KIND_CURVE_BEFORE | KIND_CURVE_AFTER,
                },
            }
        })
        .collect()
}

fn region(id: u64, r: &RegionSewing, s: &mut Shape, table: &mut Vec<Thread>) -> Result<(), String> {
    s.region = match r.kind {
        RegionType::Fill => RegionSewType::Fill,
        RegionType::Satin => RegionSewType::Satin,
        RegionType::ProgrammableFill => RegionSewType::ProgrammableFill,
        // Written at PE-DESIGN's default attributes: each file of these types at its defaults
        // differs from a fill only in the code (o-region-*); their own fields are not mapped.
        RegionType::CrossStitch => RegionSewType::Cross,
        RegionType::ConcentricCircle => RegionSewType::Concentric,
        RegionType::Radial => RegionSewType::Radial,
        RegionType::Spiral => RegionSewType::Spiral,
        RegionType::Piping => RegionSewType::Piping,
        RegionType::Stippling => RegionSewType::Stippling,
        RegionType::NetFill => RegionSewType::NetFill,
        RegionType::ZigzagNetFill => RegionSewType::ZigzagNet,
        RegionType::FlexibleSpiral => RegionSewType::FlexibleSpiral,
        RegionType::DecorativeFill => RegionSewType::Decorative,
        other => {
            return Err(format!(
                "object {id}: region type {} is not mapped in PES 0110 yet (fill and satin are)",
                other.label()
            ));
        }
    };
    s.region_color = thread_index(table, &r.thread);
    s.density = r.density as f32;
    s.direction = (r.angle.rem_euclid(360.0).round() as u32 % 360) as u16;
    s.frequency = r.frequency.unwrap_or(30.0).round().clamp(0.0, 100.0) as u16;
    s.under_sewing = r.underlay;
    s.gradation = r.gradation.is_some();
    s.running_path = match r.running_stitch_path {
        None | Some(RunningStitchPath::Inside) => 0,
        Some(RunningStitchPath::AlongOutline) => 1,
        Some(RunningStitchPath::OnOutline) => 2,
    };
    Ok(())
}

fn line(id: u64, l: &LineSewing, s: &mut Shape, table: &mut Vec<Thread>) -> Result<(), String> {
    s.line_type = match l.kind {
        LineType::Zigzag => Shape::LINE_ZIGZAG,
        LineType::Running => Shape::LINE_RUNNING,
        LineType::Triple => Shape::LINE_TRIPLE,
        LineType::EStitch | LineType::VStitch => Shape::LINE_E_OR_V,
        LineType::Cutting => Shape::LINE_CUTTING,
        other => {
            return Err(format!(
                "object {id}: line type {} is not mapped in PES 0110 yet (running, zigzag, triple, E and V are)",
                other.label()
            ));
        }
    };
    s.line_sewn = true;
    s.line_color = thread_index(table, &l.thread);
    // E and V keep their width in a field of their own; the zigzag width stays PE's default.
    if s.line_type == Shape::LINE_E_OR_V {
        s.ev_width = l.width as f32;
    } else {
        s.zigzag_width = l.width as f32;
    }
    s.zigzag_density = l.density as f32;
    s.run_pitch = l.pitch as f32;
    s.line_under_sewing = l.underlay;
    Ok(())
}

/// A text object as a `CEmbNText`: one `CTTFLetter` per character in Lato (PE-DESIGN stores
/// no outlines and lays the letters out again from the font), the size, the region and line
/// sewing, the line breaks and spacing, and the click point: the left end of the first baseline.
/// What the mapped fields cannot carry is refused with the reason.
fn text_object(
    object: &DesignObject,
    position: u16,
    positions: &[u64],
    table: &mut Vec<Thread>,
) -> Result<Text, String> {
    let id = object.id;
    let refuse = |what: &str| {
        Err(format!(
            "object {id}: {what} is not mapped in PES 0110 text yet (use version 6)"
        ))
    };
    let Some(spec) = &object.text else {
        return Err(format!("object {id} is text without its text attributes"));
    };
    if text::font(&spec.font).map(|f| f.id) != Some(text::DEFAULT_FONT) {
        return refuse(&format!("font {:?} (only Lato Bold is written)", spec.font));
    }
    // Fit Text to Outline: the shape it follows must come before the text in the list.
    let fit = match spec.path {
        None => None,
        Some(path) => {
            let at = positions
                .iter()
                .position(|&p| p == path)
                .filter(|&k| k < usize::from(position))
                .ok_or_else(|| {
                    format!(
                        "object {id}: the outline it is fitted to (object {path}) must be visible and come before the text"
                    )
                })?;
            let halign = match (spec.fit.spread, spec.align) {
                (Spread::None, TextAlign::Left) => 0,
                (Spread::None, TextAlign::Center) => 1,
                (Spread::None, TextAlign::Right) => 2,
                (Spread::Even, _) => 4,
                (Spread::Fill, _) => return refuse("text stretched along its outline"),
            };
            Some(Fit {
                path: at as u16,
                halign,
                valign: match spec.fit.valign {
                    VAlign::Bottom => 0,
                    VAlign::Middle => 1,
                    VAlign::Top => 2,
                },
                upright: spec.fit.upright,
                other_side: spec.fit.other_side,
            })
        }
    };
    if spec.direction == TextDirection::Vertical {
        return refuse("vertical text");
    }
    if spec.metrics != TextMetrics::Pe {
        return refuse("a text sized by its capital height (an older OpenStitch file)");
    }
    // Character spacing is mapped on fitted text (text-fit-spacing-m1/p2) only.
    if (spec.letter_spacing != 0.0 && fit.is_none()) || spec.line_spacing != 0.0 || spec.kerning {
        return refuse("letter spacing, extra line spacing in mm or font kerning");
    }
    if !spec.characters.is_empty() || spec.name_drop.is_some() || object.stitch_block.is_some() {
        return refuse("per-character edits, Name Drop or stitch glyphs");
    }
    let t = spec.transform;
    if (t.a - 1.0).abs() > 1e-9 || t.b.abs() > 1e-9 || t.c.abs() > 1e-9 || (t.d - 1.0).abs() > 1e-9
    {
        return refuse("a rotated, scaled or skewed text");
    }
    let mut letters = Vec::new();
    let mut breaks = Vec::new();
    for c in spec.text.chars() {
        if c == '\n' {
            if letters.is_empty() || breaks.last() == Some(&(letters.len() as u16)) {
                return refuse("an empty line");
            }
            breaks.push(letters.len() as u16);
            continue;
        }
        // A space is a letter like any other in PE-DESIGN's files (r27, `A B`); other white
        // space has no owner sample.
        if c.is_whitespace() && c != ' ' {
            return refuse(&format!("the white-space character {c:?}"));
        }
        let ch = u16::try_from(u32::from(c))
            .map_err(|_| format!("object {id}: character {c:?} is outside UTF-16's first plane"))?;
        letters.push(Letter::with_defaults(ch));
    }
    if letters.is_empty() || breaks.last() == Some(&(letters.len() as u16)) {
        return refuse("an empty text or a trailing line break");
    }
    let align = match (spec.align, &fit) {
        // A fitted text's alignment is the fit's; its Text Attributes alignment stays left.
        (_, Some(_)) => 0,
        (TextAlign::Left, None) => 0,
        (TextAlign::Center, None) => 1,
        (TextAlign::Right, None) => {
            return refuse("right-aligned text (no owner file shows its code)");
        }
    };
    if fit.is_some() && !breaks.is_empty() {
        return refuse("a text of several lines fitted to an outline");
    }
    if spec.envelope.is_some() && !breaks.is_empty() {
        return refuse("a Text Transform on several lines");
    }
    // Region first, then line: PE-DESIGN's thread table is in sewing order.
    let Some(r) = &object.region else {
        return refuse("text with its region not sewn");
    };
    let region = match r.kind {
        RegionType::Satin => RegionSewType::Satin,
        RegionType::Fill => RegionSewType::Fill,
        other => return refuse(&format!("region type {}", other.label())),
    };
    if r.underlay {
        return refuse("under sewing on text");
    }
    let region_color = thread_index(table, &r.thread);
    let (line_sewn, line_type, line_color) = match &object.line {
        None => (false, Shape::LINE_NOT_SEWN, region_color),
        Some(l) => {
            let code = match l.kind {
                LineType::Running if l.pitch == 2.0 => Shape::LINE_RUNNING,
                LineType::Triple if l.pitch == 2.0 => Shape::LINE_TRIPLE,
                LineType::Zigzag if l.width == 2.0 && l.density == 4.5 => Shape::LINE_ZIGZAG,
                _ => {
                    return refuse(&format!(
                        "a {} line on text (running, triple at 2.0 mm and zigzag 2.0 mm / 4.5 are)",
                        l.kind.label()
                    ));
                }
            };
            (true, code, thread_index(table, &l.thread))
        }
    };
    for l in &mut letters {
        l.size = spec.size as f32;
        l.density = r.density as f32;
        l.region = region;
        l.region_color = region_color;
        l.line_sewn = line_sewn;
        l.line_type = line_type;
        l.line_color = line_color;
    }
    let b = object
        .bounds()
        .ok_or_else(|| format!("object {id}: the text has no outline"))?;
    let page = |v: f64| {
        (v + f64::from(PAGE_ORIGIN))
            .round()
            .clamp(-32768.0, 32767.0) as i16
    };
    // PE-DESIGN's text box runs from the font's ascent above the first baseline (1.59 × the size
    // for Lato: r12 at size 10, text-defaults-h at size 20) down to the lowest letter or the
    // baseline, and across the letters. A transformed text's box is its letters' extent.
    let ascent_line = t.ty - ASCENT_PER_SIZE * spec.size * 10.0;
    let envelope = match spec.envelope {
        Some(preset) => Some(envelope(spec, preset).map_err(|e| format!("object {id}: {e}"))?),
        None => None,
    };
    let bounds = match (spec.envelope, &envelope) {
        // The arches: PE-DESIGN's straight box bent round the arch's circle.
        (Some(preset @ (1 | 2)), Some(e)) => {
            let [r, cx, _, _] = e.geometry.map(f64::from);
            let o = f64::from(PAGE_ORIGIN);
            bent_box(preset, r, cx - o, t.ty, spec.size * 10.0)
        }
        // A transformed text: its letters, its top at most the ascent line (presets 3, 13 …).
        (Some(_), _) => [
            b.min_x,
            b.min_y.min(ascent_line),
            b.max_x,
            b.max_y.max(t.ty),
        ],
        // A straight text: from the ascent line down to the baseline or the lowest letter.
        _ => [b.min_x, ascent_line, b.max_x, b.max_y.max(t.ty)],
    };
    Ok(Text {
        position,
        bounds: bounds.map(page),
        letters,
        breaks,
        line_spacing: spec.line_spacing_percent as f32,
        vertical: false,
        transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        align,
        // The Text tool's click: the alignment point of the first baseline (none when fitted).
        origin: if fit.is_some() {
            (0.0, 0.0)
        } else {
            (t.tx as f32 + PAGE_ORIGIN, t.ty as f32 + PAGE_ORIGIN)
        },
        envelope: if fit.is_some() { None } else { envelope },
        char_spacing: (spec.letter_spacing * 10.0) as f32,
        fit,
    })
}

/// How far above the baseline PE-DESIGN's text box starts, per mm of Lato text size.
const ASCENT_PER_SIZE: f64 = 1.59;

/// The box PE-DESIGN stores for an arched text (preset 1 over, 2 under; 0.1 mm, page
/// coordinates without the file offset): its straight box, from the ascent line down to the
/// baseline across the text's width (2 × r × [`ARCH_RADIANS`] / 2), bent round the arch's circle
/// of radius `r` centred on `cx`. It matches PE-DESIGN's own boxes to 1–2 mm (HHHHHHH and
/// OpenStitch, compat text-transform-1 and -2 and the round-4 captures); PE-DESIGN recomputes
/// the box when it opens the file (its re-save of pes-0110-editable-text-transform changed only
/// the box).
fn bent_box(preset: u8, r: f64, cx: f64, baseline: f64, size: f64) -> [f64; 4] {
    let ascent = ASCENT_PER_SIZE * size;
    let half = ARCH_RADIANS / 2.0;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for k in 0..=32 {
        let a = -half + 2.0 * half * f64::from(k) / 32.0;
        // Over: the baseline on the circle, the box outside it. Under: the size's top on the
        // circle, the box hanging below it.
        let (radii, centre_y, sign) = if preset == 1 {
            ([r, r + ascent], baseline + r, -1.0)
        } else {
            ([r - (ascent - size), r + size], baseline - size - r, 1.0)
        };
        for rho in radii {
            let (x, y) = (cx + rho * a.dsin(), centre_y + sign * rho * a.dcos());
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    [x0, y0, x1, y1]
}

/// PE-DESIGN's code and default-level strength of Text Transform presets 1–26 (its palette
/// order, as [`TextSpec::envelope`] numbers them): the same in every owner file
/// (compat/reference/text/text-transform-N, the round-4 captures on two other texts).
const ENVELOPES: [(u16, f32); 26] = [
    (24, 30.0),
    (25, 30.0),
    (0, 50.0),
    (1, 50.0),
    (8, 20.0),
    (9, 20.0),
    (4, 50.0),
    (5, 50.0),
    (10, 10.0),
    (11, 10.0),
    (3, 50.0),
    (2, 50.0),
    (7, 30.0),
    (6, 30.0),
    (18, 20.0),
    (19, 20.0),
    (12, 50.0),
    (14, 50.0),
    (20, 20.0),
    (21, 20.0),
    (17, 50.0),
    (16, 50.0),
    (22, 20.0),
    (23, 20.0),
    (13, 50.0),
    (15, 50.0),
];

/// The arch presets bend the text round a circle whose radius is the straight text's advance
/// width over this angle (HHHHHHH, OpenStitch and Agjy 123 in the owner files: 1.887).
const ARCH_RADIANS: f64 = 1.887;

/// The `CEmbDText` transform of preset `preset` at the default level. The arches (1 over, 2
/// under) carry their circle: the radius, the straight text's centre x and the circle's centre
/// y, a radius below the baseline for 1 and a radius above the size's top for 2. The fans (13,
/// 14) carry 1000 and zeros, as in the owner file made before any arch; the other presets leave
/// PE-DESIGN's last arch numbers there, which OpenStitch writes as zeros.
fn envelope(spec: &TextSpec, preset: u8) -> Result<Envelope, String> {
    let &(code, default) = ENVELOPES
        .get(usize::from(preset).wrapping_sub(1))
        .ok_or_else(|| format!("Text Transform preset {preset} (1-26)"))?;
    // The text's own level (Text Transform's handle), else the preset's default.
    let strength = spec.envelope_level.map_or(default, |l| l as f32);
    let geometry = match preset {
        1 | 2 => {
            let straight = TextSpec {
                envelope: None,
                transform: Transform::IDENTITY,
                ..spec.clone()
            };
            // The straight text's advance width, as PE-DESIGN measures its box: the shift of the
            // letters between a left and a right aligned layout.
            let min_x = |align: TextAlign| -> Result<f64, String> {
                let probe = TextSpec {
                    align,
                    ..straight.clone()
                };
                let layout = text::layout(&probe, None).map_err(|e| e.to_string())?;
                Rect::enclosing(layout.contours().iter().flat_map(Contour::outline_points))
                    .map(|b| b.min_x)
                    .ok_or_else(|| "the text has no outline".to_string())
            };
            let advance = min_x(TextAlign::Left)? - min_x(TextAlign::Right)?;
            let start = match spec.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => -advance / 2.0,
                TextAlign::Right => -advance,
            };
            let (tx, ty) = (spec.transform.tx, spec.transform.ty);
            // The level is the arch's half-sweep: the radius keeps the text length.
            let r = advance / (ARCH_RADIANS * f64::from(strength) / f64::from(default));
            let cx = tx + start + advance / 2.0 + f64::from(PAGE_ORIGIN);
            let baseline = ty + f64::from(PAGE_ORIGIN);
            let cy = if preset == 1 {
                baseline + r
            } else {
                baseline - spec.size * 10.0 - r
            };
            [r as f32, cx as f32, cy as f32, 0.0]
        }
        13 | 14 => [1000.0, 0.0, 0.0, 0.0],
        _ => [0.0; 4],
    };
    Ok(Envelope {
        code,
        strength,
        geometry,
    })
}

/// File coordinates: 0.1 mm from the page's top-left corner, offset by [`PAGE_ORIGIN`].
fn file_xy(p: openstitch_design::Vec2) -> (f32, f32) {
    (p.x as f32 + PAGE_ORIGIN, p.y as f32 + PAGE_ORIGIN)
}

/// The contour's vertices with the Bézier handles of the segments before and after each.
fn vertices(c: &Contour) -> Vec<Vertex> {
    let n = c.points.len();
    (0..n)
        .map(|i| {
            let (x, y) = file_xy(c.points[i]);
            // Segment i leaves vertex i; segment i-1 (wrapping when closed) arrives at it.
            let after = c.curve(i).filter(|_| c.closed || i + 1 < n);
            let before = if i > 0 {
                c.curve(i - 1)
            } else if c.closed {
                c.curve(n - 1)
            } else {
                None
            };
            let mut kind = 0;
            let mut handles = [(0.0, 0.0); 2];
            if let Some(b) = before {
                kind |= KIND_CURVE_BEFORE;
                handles[0] = file_xy(b.c2);
            }
            if let Some(a) = after {
                kind |= KIND_CURVE_AFTER;
                handles[1] = file_xy(a.c1);
            }
            Vertex {
                x,
                y,
                handles,
                kind,
            }
        })
        .collect()
}

/// The outline's extent in file units, curves flattened.
fn bounds(c: &Contour) -> [i16; 4] {
    let pts = c.outline_points();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1].map(|v| {
        (v + f64::from(PAGE_ORIGIN))
            .round()
            .clamp(-32768.0, 32767.0) as i16
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_core::PEC_CHART;
    use openstitch_design::v2;

    fn red() -> Thread {
        PEC_CHART[5].clone().expect("chart colour")
    }

    #[test]
    fn a_rectangle_with_fill_and_zigzag_maps_to_pe_designs_fields() {
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let region = RegionSewing::new(RegionType::Fill, PEC_CHART[16].clone().expect("silver"));
        let line = LineSewing::new(LineType::Zigzag, red());
        design.add(DesignObject::shape(
            vec![Contour::rectangle(100.0, 100.0, 100.0, 100.0)],
            Some(region),
            Some(line),
        ));
        let o = objects(&design).expect("objects");
        assert_eq!(o.page_mm, Some((100, 100)));
        assert_eq!(o.threads.len(), 2, "region silver first, then line red");
        let s = &o.shapes[0];
        assert_eq!(s.bounds, [600, 600, 700, 700]);
        assert_eq!((s.region_color, s.line_color), (0, 1));
        assert_eq!(
            (s.region, s.line_type),
            (RegionSewType::Fill, Shape::LINE_ZIGZAG)
        );
        assert!(s.closed && s.line_sewn && s.under_sewing && s.line_under_sewing);
        assert_eq!(s.vertices.len(), 4);
        assert!(
            s.vertices
                .iter()
                .all(|v| v.kind == 0 && v.handles == [(0.0, 0.0); 2])
        );
        assert_eq!((s.vertices[0].x, s.vertices[0].y), (600.0, 600.0));
    }

    #[test]
    fn a_stitch_pattern_becomes_a_stitch_group_with_its_threads() {
        use openstitch_core::{Command, Stitch};
        use openstitch_design::StitchBlock;
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        design.add(DesignObject::shape(
            vec![Contour::rectangle(0.0, 0.0, 50.0, 50.0)],
            Some(RegionSewing::new(RegionType::Fill, red())),
            None,
        ));
        let blue = Thread::rgb(0x00_00_FF);
        let stitches = [
            (100.0, 100.0, Command::Stitch),
            (200.0, 100.0, Command::Stitch),
            (200.0, 150.0, Command::ColorChange),
            (250.0, 150.0, Command::Stitch),
        ]
        .map(|(x, y, c)| Stitch::new(x, y, c));
        design.add(DesignObject::stitches(StitchBlock {
            stitches: stitches.to_vec(),
            threads: vec![red(), blue.clone()],
        }));
        let o = objects(&design).expect("objects");
        assert_eq!(o.shapes.len(), 1);
        let [g] = o.stitch_groups.as_slice() else {
            panic!("one stitch group: {:?}", o.stitch_groups);
        };
        assert_eq!(g.position, 1);
        // Red is the shape's thread already; blue is appended.
        assert_eq!(o.threads.len(), 2);
        assert_eq!(o.threads[1].rgb_value(), blue.rgb_value());
        assert_eq!(g.color_log.first().map(|&(_, c)| c), Some(0));
        assert_eq!(g.color_log.last().map(|&(_, c)| c), Some(1));
        assert_eq!(g.bounds, [600, 600, 750, 650]);
    }

    #[test]
    fn a_manual_punch_column_becomes_a_straight_block() {
        // m01-straight-1's clicks: (10,10) (10,30), (30,10) (30,30) mm.
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let mut r = RegionSewing::new(RegionType::Satin, red());
        r.direction_lines = vec![
            [v2(100.0, 100.0), v2(100.0, 300.0)],
            [v2(300.0, 100.0), v2(300.0, 300.0)],
        ];
        let outline = Contour::new(
            vec![
                v2(100.0, 100.0),
                v2(300.0, 100.0),
                v2(300.0, 300.0),
                v2(100.0, 300.0),
            ],
            true,
        );
        design.add(DesignObject::shape(vec![outline], Some(r.clone()), None));
        let o = objects(&design).expect("objects");
        let s = &o.shapes[0];
        assert_eq!(s.punch, [[0, 3], [1, 2]]);
        assert_eq!(s.punch_exit, 2);
        assert_eq!(s.bounds, [600, 600, 800, 800]);
        let xy: Vec<(f32, f32)> = s.vertices.iter().map(|v| (v.x, v.y)).collect();
        assert_eq!(
            xy,
            [
                (600.0, 600.0),
                (800.0, 600.0),
                (800.0, 800.0),
                (600.0, 800.0)
            ]
        );
        assert!(
            s.vertices
                .iter()
                .all(|v| v.handles == [(v.x, v.y); 2] && v.kind == 0)
        );
        assert_eq!(
            (s.region, s.line_sewn, s.line_type),
            (RegionSewType::Satin, false, Shape::LINE_NOT_SEWN)
        );
    }

    #[test]
    fn a_curved_block_has_pe_designs_handles_to_a_unit() {
        // PE-DESIGN's Curve Blocks (tests/fixtures/pe11): the same clicks as a column of
        // direction lines give the same outline, kinds and, within one file unit, handles.
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../openstitch-formats/tests/fixtures/pe11"
        );
        for name in [
            "m07-curve-1",
            "m09-curve-bent",
            "m10-curve-uneven",
            "m11-curve-4",
            "o-manual-curve",
        ] {
            let bytes = std::fs::read(format!("{dir}/{name}.pes")).expect("fixture");
            let pe = openstitch_formats::pes::read_objects(&bytes).expect("objects");
            let pe = &pe.shapes[0];
            let at = |k: u32| {
                let v = &pe.vertices[k as usize];
                v2(f64::from(v.x - PAGE_ORIGIN), f64::from(v.y - PAGE_ORIGIN))
            };
            let mut r = RegionSewing::new(RegionType::Satin, red());
            r.direction_lines = pe.punch.iter().map(|&[a, b]| [at(a), at(b)]).collect();
            r.curved_lines = true;
            let mut design = Design::new("t", openstitch_design::DesignPage::default());
            design.add(DesignObject::shape(
                vec![Contour::rectangle(0.0, 0.0, 10.0, 10.0)],
                Some(r),
                None,
            ));
            let o = objects(&design).expect("objects");
            let ours = &o.shapes[0];
            assert_eq!(
                (&ours.punch, ours.punch_exit),
                (&pe.punch, pe.punch_exit),
                "{name}"
            );
            // The bounds: the curve's extent rounded outward (ours from our handles).
            for (a, b) in ours.bounds.iter().zip(&pe.bounds) {
                assert!(
                    (a - b).abs() <= 1,
                    "{name}: bounds {:?} against {:?}",
                    ours.bounds,
                    pe.bounds
                );
            }
            for (k, (a, b)) in ours.vertices.iter().zip(&pe.vertices).enumerate() {
                assert_eq!((a.x, a.y, a.kind), (b.x, b.y, b.kind), "{name} vertex {k}");
                for (h, g) in a.handles.iter().zip(&b.handles) {
                    assert!(
                        (h.0 - g.0).abs() <= 1.0 && (h.1 - g.1).abs() <= 1.0,
                        "{name} vertex {k}: {h:?} against PE-DESIGN's {g:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn applique_stages_become_pe_designs_applique_lines_and_colours() {
        use crate::wizards::applique::{MATERIAL, POSITION, TACK_DOWN};
        let stage = |name: &'static str| {
            let mut t = red();
            t.name = Some(name.into());
            t.catalog_number = None;
            t.brand = None;
            t
        };
        let square = || vec![Contour::rectangle(100.0, 100.0, 200.0, 200.0)];
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        for (kind, name) in [
            (LineType::Cutting, MATERIAL),
            (LineType::Running, POSITION),
            (LineType::VStitch, TACK_DOWN),
        ] {
            let mut line = LineSewing::new(kind, stage(name));
            line.width = 2.0;
            design.add(DesignObject::shape(square(), None, Some(line)));
        }
        let o = objects(&design).expect("objects");
        let catalogs: Vec<_> = o.threads.iter().map(|t| t.catalog_number.clone()).collect();
        assert_eq!(
            catalogs,
            [Some("100".into()), Some("101".into()), Some("102".into())]
        );
        assert!(o.threads.iter().all(
            |t| t.kind == Some(openstitch_formats::pes11::APPLIQUE_KIND) && t.rgb_value() == 0
        ));
        let types: Vec<u16> = o.shapes.iter().map(|s| s.line_type).collect();
        assert_eq!(
            types,
            [Shape::LINE_CUTTING, Shape::LINE_RUNNING, Shape::LINE_E_OR_V]
        );
        // The V tack-down's width is the E/V width; the zigzag width stays PE's default.
        assert_eq!((o.shapes[2].ev_width, o.shapes[2].zigzag_width), (2.0, 2.0));
    }

    #[test]
    fn only_a_group_of_every_object_is_written() {
        let square = |x: f64| {
            DesignObject::shape(
                vec![Contour::rectangle(x, 100.0, 100.0, 100.0)],
                Some(RegionSewing::new(RegionType::Fill, red())),
                None,
            )
        };
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let ids: Vec<u64> = [100.0, 300.0, 500.0]
            .map(|x| design.add(square(x)))
            .to_vec();
        let group = |design: &mut Design, members: &[u64], g: u64| {
            for o in &mut design.objects {
                if members.contains(&o.id) {
                    o.group = Some(g);
                }
            }
        };
        // Two of three grouped: PE-DESIGN refused such a file, so the group is left out.
        let mut partial = design.clone();
        group(&mut partial, &ids[..2], 1);
        assert!(objects(&partial).expect("objects").groups.is_empty());
        // Two groups: not mapped either.
        let mut two = design.clone();
        group(&mut two, &ids[..2], 1);
        group(&mut two, &ids[2..], 2);
        assert!(objects(&two).expect("objects").groups.is_empty());
        // Every object in one group: written.
        let mut all = design.clone();
        group(&mut all, &ids, 1);
        assert_eq!(objects(&all).expect("objects").groups, [vec![0, 1, 2]]);
    }

    #[test]
    fn hole_sewing_writes_the_inner_copy_and_the_pair() {
        // A 40 mm square sewn around a 15 mm square inside it, as PE-DESIGN's vector import
        // writes a pair: the outer shape, a Not Sewn copy of the inner one, the inner one.
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let outer = design.add(DesignObject::shape(
            vec![Contour::rectangle(100.0, 100.0, 400.0, 400.0)],
            Some(RegionSewing::new(RegionType::Fill, red())),
            None,
        ));
        let blue = Thread::rgb(0x00_00_FF);
        let inner = design.add(DesignObject::shape(
            vec![Contour::rectangle(225.0, 225.0, 150.0, 150.0)],
            Some(RegionSewing::new(RegionType::Fill, blue.clone())),
            None,
        ));
        let i = design.index_of(outer).expect("outer");
        design.objects[i].hole_sewing = true;
        design.objects[i].hole_inner = Some(inner);
        let o = objects(&design).expect("objects");
        assert_eq!(o.holes, [[0, 1]]);
        assert_eq!(o.shapes.len(), 3);
        let copy = &o.shapes[1];
        assert_eq!(copy.region, RegionSewType::NotSewn);
        assert!(!copy.line_sewn);
        assert_eq!(copy.vertices, o.shapes[2].vertices);
        assert_eq!(copy.region_color, o.shapes[0].region_color);
        assert_eq!(o.shapes[2].region, RegionSewType::Fill);
        // An inner shape that is not sewn is paired itself, with no copy (the rings).
        let mut direct = design.clone();
        let j = direct.index_of(inner).expect("inner");
        direct.objects[j].region = None;
        let d = objects(&direct).expect("objects");
        assert_eq!((d.holes.clone(), d.shapes.len()), (vec![[0, 1]], 2));
        assert_eq!(d.shapes[1].region, RegionSewType::NotSewn);
        // A design with a group as well is refused: the layout of both is not mapped.
        let mut grouped = design.clone();
        grouped.objects[i].group = Some(7);
        let err = objects(&grouped).expect_err("hole pair and group");
        assert!(err.contains("hole sewing"), "{err}");
    }

    #[test]
    fn programmable_fill_writes_its_pattern_into_the_header_list() {
        use openstitch_design::patterns::{PatternKind, resolve};
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let mut r = RegionSewing::new(RegionType::ProgrammableFill, red());
        r.pattern_width = Some(7.5);
        r.pattern_height = Some(7.5);
        design.add(DesignObject::shape(
            vec![Contour::rectangle(200.0, 200.0, 300.0, 200.0)],
            Some(r.clone()),
            None,
        ));
        let o = objects(&design).expect("objects");
        assert_eq!(o.shapes[0].region, RegionSewType::ProgrammableFill);
        let [p] = o.fill_patterns.as_slice() else {
            panic!("one pattern: {:?}", o.fill_patterns)
        };
        assert_eq!(p.size, 750);
        let tile = resolve(&design.patterns, PatternKind::Fill, None);
        assert_eq!(p.polylines.len(), tile.paths.len());
        assert!(
            p.polylines
                .iter()
                .flatten()
                .all(|&(x, y)| (0..=1024).contains(&x) && (0..=1024).contains(&y))
        );
        // The same pattern twice is listed once; a non-square tile is refused.
        design.add(DesignObject::shape(
            vec![Contour::rectangle(600.0, 200.0, 100.0, 100.0)],
            Some(r.clone()),
            None,
        ));
        assert_eq!(objects(&design).expect("objects").fill_patterns.len(), 1);
        let mut oblong = Design::new("t", openstitch_design::DesignPage::default());
        let mut r2 = r;
        r2.pattern_height = Some(5.0);
        oblong.add(DesignObject::shape(
            vec![Contour::rectangle(200.0, 200.0, 300.0, 200.0)],
            Some(r2),
            None,
        ));
        let err = objects(&oblong).expect_err("non-square tile");
        assert!(err.contains("square"), "{err}");
    }

    #[test]
    fn region_types_map_to_pe_designs_codes() {
        for (kind, code) in [
            (RegionType::CrossStitch, RegionSewType::Cross),
            (RegionType::ConcentricCircle, RegionSewType::Concentric),
            (RegionType::Radial, RegionSewType::Radial),
            (RegionType::Spiral, RegionSewType::Spiral),
            (RegionType::Piping, RegionSewType::Piping),
            (RegionType::Stippling, RegionSewType::Stippling),
            (RegionType::NetFill, RegionSewType::NetFill),
            (RegionType::ZigzagNetFill, RegionSewType::ZigzagNet),
            (RegionType::FlexibleSpiral, RegionSewType::FlexibleSpiral),
        ] {
            let mut design = Design::new("t", openstitch_design::DesignPage::default());
            design.add(DesignObject::shape(
                vec![Contour::rectangle(200.0, 200.0, 300.0, 300.0)],
                Some(RegionSewing::new(kind, red())),
                None,
            ));
            let o = objects(&design).expect("objects");
            assert_eq!(o.shapes[0].region, code, "{kind:?}");
            assert!(o.shapes[0].decorative.is_none());
        }
        // Motif fill and contour fill are refused with the reason.
        for kind in [RegionType::MotifFill, RegionType::Contour] {
            let mut design = Design::new("t", openstitch_design::DesignPage::default());
            design.add(DesignObject::shape(
                vec![Contour::rectangle(200.0, 200.0, 300.0, 300.0)],
                Some(RegionSewing::new(kind, red())),
                None,
            ));
            assert!(objects(&design).is_err(), "{kind:?}");
        }
    }

    #[test]
    fn open_line_text_and_unmapped_types_are_reported() {
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let line = LineSewing::new(LineType::Running, red());
        design.add(DesignObject::shape(
            vec![Contour::new(
                vec![v2(100.0, 100.0), v2(300.0, 100.0)],
                false,
            )],
            None,
            Some(line.clone()),
        ));
        let o = objects(&design).expect("objects");
        let s = &o.shapes[0];
        assert!(!s.closed && s.line_sewn);
        assert_eq!(
            (s.region, s.line_type),
            (RegionSewType::NotSewn, Shape::LINE_RUNNING)
        );
        assert_eq!((s.region_color, s.line_color), (0, 0));

        let mut motif = design.clone();
        motif.add(DesignObject::shape(
            vec![Contour::rectangle(0.0, 0.0, 50.0, 50.0)],
            None,
            Some(LineSewing::new(LineType::Motif, red())),
        ));
        let err = objects(&motif).expect_err("motif lines are not mapped");
        assert!(err.contains("Motif"), "{err}");

        let mut two = design.clone();
        two.add(DesignObject::shape(
            vec![
                Contour::rectangle(0.0, 0.0, 50.0, 50.0),
                Contour::rectangle(10.0, 10.0, 10.0, 10.0),
            ],
            Some(RegionSewing::new(RegionType::Fill, red())),
            None,
        ));
        let err = objects(&two).expect_err("two outlines");
        assert!(err.contains("2 outlines"), "{err}");
    }

    #[test]
    fn text_becomes_letters_at_the_alignment_point() {
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let mut spec = TextSpec::new("AB", text::DEFAULT_FONT, 10.0);
        spec.transform = Transform::translation(100.0, 200.0);
        let region = RegionSewing::new(RegionType::Satin, red());
        let (mut object, _) =
            DesignObject::text(spec.clone(), Some(region.clone()), None).expect("layout");
        object.region.as_mut().expect("region").underlay = false;
        design.add(object);
        let o = objects(&design).expect("objects");
        let t = &o.texts[0];
        assert_eq!(t.position, 0);
        assert_eq!(
            t.origin,
            (600.0, 700.0),
            "left aligned: the alignment point"
        );
        let chars: Vec<u16> = t.letters.iter().map(|l| l.ch).collect();
        assert_eq!(chars, [65, 66]);
        assert!(t.letters.iter().all(|l| l.size == 10.0 && !l.line_sewn));
        assert_eq!(t.bounds[3], 700, "letters stand on the baseline");
        assert_eq!(t.bounds[1], 541, "the font's ascent above it, as r12");

        // Centred: alignment 1 and the click at the middle of the line, as PE-DESIGN writes it.
        let mut centred = Design::new("t", openstitch_design::DesignPage::default());
        spec.align = TextAlign::Center;
        let (mut object, _) =
            DesignObject::text(spec.clone(), Some(region.clone()), None).expect("layout");
        object.region.as_mut().expect("region").underlay = false;
        centred.add(object);
        let c = &objects(&centred).expect("objects").texts[0];
        assert_eq!((c.align, c.origin), (1, (600.0, 700.0)));
        assert!(f32::from(c.bounds[0]) < 600.0 && f32::from(c.bounds[2]) > 600.0);

        // Text Transform preset 1 (arch over): PE-DESIGN's code 24 at strength 30, and the
        // circle a radius below the baseline.
        let mut arched = Design::new("t", openstitch_design::DesignPage::default());
        spec.envelope = Some(1);
        let (mut object, _) =
            DesignObject::text(spec.clone(), Some(region.clone()), None).expect("layout");
        object.region.as_mut().expect("region").underlay = false;
        arched.add(object);
        let a = objects(&arched).expect("objects").texts.remove(0);
        let e = a.envelope.expect("CEmbDText");
        assert_eq!((e.code, e.strength), (24, 30.0));
        assert!(e.geometry[0] > 50.0, "{:?}", e.geometry);
        assert!((e.geometry[2] - (700.0 + e.geometry[0])).abs() < 0.01);
        // The arch's box: its apex at the ascent line above the baseline, its ends below it.
        assert_eq!(a.bounds[1], 541);
        assert!(a.bounds[3] > 700, "{:?}", a.bounds);
        // At its own level the arch writes that half-sweep, and the radius keeps the text
        // length (twice the sweep, half the radius).
        let mut steep = Design::new("t", openstitch_design::DesignPage::default());
        spec.envelope_level = Some(60.0);
        let (mut object, _) =
            DesignObject::text(spec.clone(), Some(region.clone()), None).expect("layout");
        object.region.as_mut().expect("region").underlay = false;
        steep.add(object);
        let s = objects(&steep).expect("objects").texts.remove(0);
        let se = s.envelope.expect("CEmbDText");
        assert_eq!((se.code, se.strength), (24, 60.0));
        assert!(
            (se.geometry[0] * 2.0 - e.geometry[0]).abs() < 0.01,
            "{se:?} {e:?}"
        );
        spec.envelope_level = None;
        spec.envelope = None;

        // A tab, a vertical text and a rotated text are refused with the reason.
        for (text, vertical, rotate) in [
            ("A	B", false, false),
            ("AB", true, false),
            ("AB", false, true),
        ] {
            let mut s = TextSpec::new(text, text::DEFAULT_FONT, 10.0);
            if vertical {
                s.direction = TextDirection::Vertical;
            }
            if rotate {
                s.transform = Transform::rotation(30.0, openstitch_design::v2(0.0, 0.0));
            }
            let (object, _) =
                DesignObject::text(s, Some(RegionSewing::new(RegionType::Satin, red())), None)
                    .expect("layout");
            let mut d = Design::new("t", openstitch_design::DesignPage::default());
            d.add(object);
            assert!(objects(&d).is_err(), "{text} {vertical} {rotate}");
        }
    }

    #[test]
    fn curves_become_handles_with_kind_bits() {
        let mut design = Design::new("t", openstitch_design::DesignPage::default());
        let mut c = Contour::rectangle(100.0, 100.0, 100.0, 100.0);
        c.set_curve(
            0,
            Some(openstitch_design::Bezier {
                c1: v2(120.0, 80.0),
                c2: v2(180.0, 80.0),
            }),
        );
        design.add(DesignObject::shape(
            vec![c],
            None,
            Some(LineSewing::new(LineType::Running, red())),
        ));
        let o = objects(&design).expect("objects");
        let v = &o.shapes[0].vertices;
        assert_eq!(v[0].kind, KIND_CURVE_AFTER);
        assert_eq!(v[0].handles[1], (620.0, 580.0));
        assert_eq!(v[1].kind, KIND_CURVE_BEFORE);
        assert_eq!(v[1].handles[0], (680.0, 580.0));
        assert_eq!(v[2].kind, 0);
        // The flattened curve bulges above the vertices.
        assert!(o.shapes[0].bounds[1] < 600, "{:?}", o.shapes[0].bounds);
    }
}
