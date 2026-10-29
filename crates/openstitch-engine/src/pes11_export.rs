//! The design's editable objects as PE-DESIGN 11 `#PES0110` objects (`openstitch_formats::pes11`),
//! for `design.export` with `pesVersion` 11: one `CEmbLine` per shape, one `CEmbNText` per text
//! (Lato Bold), a thread table in sewing order, the design's groups and page size. What the
//! object list cannot hold yet fails with the object named, so a user can fall back to version 6
//! (stitches) knowingly.

use openstitch_core::Thread;
use openstitch_design::text::{self, TextAlign, TextDirection, TextMetrics, TextSpec};
use openstitch_design::{
    Contour, Design, DesignObject, LineSewing, LineType, ObjectKind, Rect, RegionSewing,
    RegionType, RunningStitchPath, Transform,
};
use openstitch_formats::pes11::{
    KIND_CURVE_AFTER, KIND_CURVE_BEFORE, Letter, Objects, PAGE_ORIGIN, RegionSewType, Shape, Text,
    Vertex,
};

/// The `#PES0110` objects of `design`'s visible objects, in design order.
pub fn objects(design: &Design) -> Result<Objects, String> {
    let mut out = Objects {
        page_mm: Some((mm(design.page.width).max(1), mm(design.page.height).max(1))),
        ..Objects::default()
    };
    let mut groups: Vec<(u64, Vec<u16>)> = Vec::new();
    for (position, object) in design.objects.iter().filter(|o| o.visible).enumerate() {
        let index = u16::try_from(position).map_err(|_| "too many objects".to_string())?;
        if object.kind == ObjectKind::Text {
            out.texts
                .push(text_object(object, index, &mut out.threads)?);
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
    if out.shapes.is_empty() && out.texts.is_empty() {
        return Err("the design has no visible shapes or texts".into());
    }
    out.groups = groups.into_iter().map(|(_, m)| m).collect();
    Ok(out)
}

fn mm(units: f64) -> u16 {
    (units / 10.0).round().clamp(0.0, 65535.0) as u16
}

/// Index of `thread` in the table, appended when new (compared by colour and catalogue number).
fn thread_index(table: &mut Vec<Thread>, thread: &Thread) -> u16 {
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

fn shape(object: &DesignObject, table: &mut Vec<Thread>) -> Result<Shape, String> {
    let id = object.id;
    match object.kind {
        ObjectKind::Shape => {}
        ObjectKind::Text => unreachable!("texts go through text_object"),
        ObjectKind::Stitches => {
            return Err(format!(
                "object {id} is a stitch pattern: PES 0110 holds editable shapes only (use version 6)"
            ));
        }
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
    if spec.path.is_some() {
        return refuse("text fitted to an outline");
    }
    if spec.envelope.is_some() {
        return refuse("a Text Transform");
    }
    if spec.direction == TextDirection::Vertical {
        return refuse("vertical text");
    }
    if spec.metrics != TextMetrics::Pe {
        return refuse("a text sized by its capital height (an older OpenStitch file)");
    }
    if spec.letter_spacing != 0.0 || spec.line_spacing != 0.0 || spec.kerning {
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
        if c.is_whitespace() {
            return refuse("a space (no owner file shows how PE-DESIGN stores one)");
        }
        let ch = u16::try_from(u32::from(c))
            .map_err(|_| format!("object {id}: character {c:?} is outside UTF-16's first plane"))?;
        letters.push(Letter::with_defaults(ch));
    }
    if letters.is_empty() || breaks.last() == Some(&(letters.len() as u16)) {
        return refuse("an empty text or a trailing line break");
    }
    if !breaks.is_empty() && spec.align != TextAlign::Left {
        return refuse("centred or right-aligned text of several lines (the alignment field)");
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
    let shift = baseline_start(spec).map_err(|e| format!("object {id}: {e}"))?;
    // PE-DESIGN's text box runs from the font's ascent above the first baseline (1.59 × the size
    // for Lato: r12 at size 10, text-defaults-h at size 20) down to the lowest letter or the
    // baseline, and across the letters.
    let top = t.ty - ASCENT_PER_SIZE * spec.size * 10.0;
    Ok(Text {
        position,
        bounds: [
            page(b.min_x),
            page(top),
            page(b.max_x),
            page(b.max_y.max(t.ty)),
        ],
        letters,
        breaks,
        line_spacing: spec.line_spacing_percent as f32,
        vertical: false,
        transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        origin: (
            (t.tx + shift) as f32 + PAGE_ORIGIN,
            t.ty as f32 + PAGE_ORIGIN,
        ),
    })
}

/// How far above the baseline PE-DESIGN's text box starts, per mm of Lato text size.
const ASCENT_PER_SIZE: f64 = 1.59;

/// Where the first line starts, along x, relative to the text's alignment point: 0 for left
/// aligned text, minus half or all of the line's advance for centred or right-aligned text.
/// The advance is measured as how far the letters move between a left and a right aligned
/// layout of the same text.
fn baseline_start(spec: &TextSpec) -> Result<f64, String> {
    if spec.align == TextAlign::Left {
        return Ok(0.0);
    }
    let left_of = |align: TextAlign| -> Result<f64, String> {
        let probe = TextSpec {
            align,
            transform: Transform::IDENTITY,
            ..spec.clone()
        };
        let layout = text::layout(&probe, None).map_err(|e| e.to_string())?;
        Rect::enclosing(layout.contours().iter().flat_map(Contour::outline_points))
            .map(|r| r.min_x)
            .ok_or_else(|| "the text has no outline".to_string())
    };
    let advance = left_of(TextAlign::Left)? - left_of(TextAlign::Right)?;
    Ok(match spec.align {
        TextAlign::Left => 0.0,
        TextAlign::Center => -advance / 2.0,
        TextAlign::Right => -advance,
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
    fn text_becomes_letters_at_the_baseline_start() {
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

        // Centred: the origin moves left by half the line's advance.
        let mut centred = Design::new("t", openstitch_design::DesignPage::default());
        spec.align = TextAlign::Center;
        let (mut object, _) = DesignObject::text(spec.clone(), Some(region), None).expect("layout");
        object.region.as_mut().expect("region").underlay = false;
        centred.add(object);
        let c = &objects(&centred).expect("objects").texts[0];
        let half = 600.0 - c.origin.0;
        assert!(half > 50.0 && half < 120.0, "{half}");
        assert!((f32::from(c.bounds[0]) - c.origin.0).abs() < 3.0, "{:?}", c);

        // A space, a vertical text and a rotated text are refused with the reason.
        for (text, vertical, rotate) in [
            ("A B", false, false),
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
