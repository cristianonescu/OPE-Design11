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
    // List positions by object id, for the outlines texts are fitted to.
    let positions: Vec<u64> = design
        .objects
        .iter()
        .filter(|o| o.visible)
        .map(|o| o.id)
        .collect();
    for (position, object) in design.objects.iter().filter(|o| o.visible).enumerate() {
        let index = u16::try_from(position).map_err(|_| "too many objects".to_string())?;
        if object.kind == ObjectKind::Text {
            out.texts
                .push(text_object(object, index, &positions, &mut out.threads)?);
        } else if object.kind == ObjectKind::Stitches {
            out.stitch_groups
                .push(stitch_group(object, index, &mut out.threads)?);
        } else {
            out.shapes.push(shape(object, &mut out.threads)?);
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
    out.groups = groups.into_iter().map(|(_, m)| m).collect();
    Ok(out)
}

fn mm(units: f64) -> u16 {
    (units / 10.0).round().clamp(0.0, 65535.0) as u16
}

/// Index of `thread` in the table, appended when new (compared by colour and catalogue number).
fn thread_index(table: &mut Vec<Thread>, thread: &Thread) -> u16 {
    // Written as PE-DESIGN writes the colour (BROTHER EMBROIDERY numbers for Brother chart
    // colours).
    let thread = &openstitch_formats::pes11::as_pe_writes(thread);
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

fn shape(object: &DesignObject, table: &mut Vec<Thread>) -> Result<Shape, String> {
    let id = object.id;
    match object.kind {
        ObjectKind::Shape => {}
        ObjectKind::Text => unreachable!("texts go through text_object"),
        ObjectKind::Stitches => unreachable!("stitch patterns go through stitch_group"),
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

fn region(id: u64, r: &RegionSewing, s: &mut Shape, table: &mut Vec<Thread>) -> Result<(), String> {
    s.region = match r.kind {
        RegionType::Fill => RegionSewType::Fill,
        RegionType::Satin => RegionSewType::Satin,
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
        other => {
            return Err(format!(
                "object {id}: line type {} is not mapped in PES 0110 yet (running, zigzag, triple, E and V are)",
                other.label()
            ));
        }
    };
    s.line_sewn = true;
    s.line_color = thread_index(table, &l.thread);
    s.zigzag_width = l.width as f32;
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
    let &(code, strength) = ENVELOPES
        .get(usize::from(preset).wrapping_sub(1))
        .ok_or_else(|| format!("Text Transform preset {preset} (1-26)"))?;
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
            let r = advance / ARCH_RADIANS;
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
