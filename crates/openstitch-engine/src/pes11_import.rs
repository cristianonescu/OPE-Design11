//! PE-DESIGN 11 `#PES0110` objects as the design's editable objects, for `import.pes`: the
//! inverse of [`crate::pes11_export`] for shapes. Each `CEmbLine` becomes a shape with its
//! outline, region and line sewing; a Manual Punch block (the Manual Punch tool's Straight and
//! Curve Blocks, and PE-DESIGN's Convert to Blocks) becomes a column sewn along its direction
//! lines; the file's groups stay groups. Texts (`CEmbNText`) and stitch groups (`CEmbOne`) are
//! not imported as objects yet and are counted in the result instead.

use crate::commands::{CommandSpec, Effect, opt, read_file, req, spec};
use openstitch_core::Thread;
use openstitch_design::{
    Bezier, Contour, DesignObject, LineSewing, LineType, RegionSewing, RegionType,
    RunningStitchPath, Vec2, v2,
};
use openstitch_formats::pes::ObjectSection;
use openstitch_formats::pes11::{KIND_CURVE_AFTER, PAGE_ORIGIN, RegionSewType, Shape, Vertex};
use serde_json::json;

/// Largest PES file accepted.
pub const MAX_PES_BYTES: usize = 64 << 20;

/// What a PES file's object list gives: its shapes as design objects with their list positions,
/// the groups (as indexes into `objects`), and what was not imported.
#[derive(Clone, Debug, Default)]
pub struct Imported {
    pub objects: Vec<DesignObject>,
    pub groups: Vec<Vec<usize>>,
    pub texts: usize,
    pub stitch_groups: usize,
    /// Shapes whose region or line type is not mapped: `(list position, reason)`.
    pub skipped: Vec<(u16, String)>,
}

/// The design objects of a `#PES0110` object list, in list order.
pub fn objects(section: &ObjectSection) -> Imported {
    let mut out = Imported {
        texts: section.texts.len(),
        stitch_groups: section.stitch_groups.len(),
        ..Imported::default()
    };
    // The shapes fill the list positions no text or stitch group takes, in order.
    let taken: std::collections::BTreeSet<u16> = section
        .texts
        .iter()
        .map(|t| t.position)
        .chain(section.stitch_groups.iter().map(|g| g.position))
        .collect();
    let mut positions = (0u16..).filter(|p| !taken.contains(p));
    let mut index_of = std::collections::BTreeMap::new();
    for s in &section.shapes {
        let Some(position) = positions.next() else {
            break;
        };
        match object(s, &section.threads) {
            Ok(o) => {
                index_of.insert(position, out.objects.len());
                out.objects.push(o);
            }
            Err(reason) => out.skipped.push((position, reason)),
        }
    }
    out.groups = section
        .groups
        .iter()
        .map(|g| g.iter().filter_map(|p| index_of.get(p).copied()).collect())
        .filter(|g: &Vec<usize>| !g.is_empty())
        .collect();
    out
}

/// Design coordinates (0.1 mm from the page's top-left) of a file point.
fn design_xy(x: f32, y: f32) -> Vec2 {
    v2(f64::from(x - PAGE_ORIGIN), f64::from(y - PAGE_ORIGIN))
}

fn thread(table: &[Thread], index: u16) -> Thread {
    table
        .get(usize::from(index))
        .cloned()
        .unwrap_or_else(|| Thread::rgb(0))
}

/// One shape as a design object.
fn object(s: &Shape, table: &[Thread]) -> Result<DesignObject, String> {
    if s.vertices.len() < 2 {
        return Err("fewer than two vertices".into());
    }
    let outline = contour(&s.vertices, s.closed);
    let mut region = match s.region {
        RegionSewType::Fill => Some(RegionType::Fill),
        RegionSewType::Satin => Some(RegionType::Satin),
        RegionSewType::NotSewn => None,
        other => return Err(format!("region type {other:?} is not mapped yet")),
    }
    .filter(|_| s.closed)
    .map(|kind| {
        let mut r = RegionSewing::new(kind, thread(table, s.region_color));
        r.density = f64::from(s.density);
        r.angle = f64::from(s.direction);
        r.underlay = s.under_sewing;
        if kind == RegionType::Fill {
            r.frequency = Some(f64::from(s.frequency));
            r.running_stitch_path = Some(match s.running_path {
                1 => RunningStitchPath::AlongOutline,
                2 => RunningStitchPath::OnOutline,
                _ => RunningStitchPath::Inside,
            });
        }
        r
    });
    if let Some(r) = region.as_mut().filter(|_| !s.punch.is_empty()) {
        r.direction_lines = punch_lines(s)?;
        // A Curve Block's sides are Bézier curves through the line ends (o-manual-curve).
        r.curved_lines = s.vertices.iter().any(|v| v.kind != 0);
    }
    let line = if s.line_sewn {
        let kind = match s.line_type {
            Shape::LINE_ZIGZAG => LineType::Zigzag,
            Shape::LINE_RUNNING => LineType::Running,
            Shape::LINE_TRIPLE => LineType::Triple,
            // E and V share the code; which one is not mapped (docs/FILE_FORMATS.md).
            Shape::LINE_E_OR_V => LineType::EStitch,
            Shape::LINE_CUTTING => LineType::Cutting,
            other => return Err(format!("line type {other} is not mapped yet")),
        };
        let mut l = LineSewing::new(kind, thread(table, s.line_color));
        l.width = f64::from(if kind == LineType::EStitch {
            s.ev_width
        } else {
            s.zigzag_width
        });
        l.density = f64::from(s.zigzag_density);
        l.pitch = f64::from(s.run_pitch);
        l.underlay = s.line_under_sewing;
        Some(l)
    } else {
        None
    };
    let mut o = DesignObject::shape(vec![outline], region, line);
    if !s.punch.is_empty() {
        o.name = Some("Manual punch".into());
    }
    Ok(o)
}

/// A block's direction lines: each `[a, b]` joins vertex a and vertex b.
fn punch_lines(s: &Shape) -> Result<Vec<[Vec2; 2]>, String> {
    s.punch
        .iter()
        .map(|&[a, b]| {
            let at = |k: u32| {
                s.vertices
                    .get(k as usize)
                    .map(|v| design_xy(v.x, v.y))
                    .ok_or_else(|| format!("direction line to vertex {k} of {}", s.vertices.len()))
            };
            Ok([at(a)?, at(b)?])
        })
        .collect()
}

/// The vertices as a contour: segment i is a Bézier curve when vertex i curves after it, with
/// vertex i's leaving handle and the next vertex's arriving handle.
fn contour(vertices: &[Vertex], closed: bool) -> Contour {
    let n = vertices.len();
    let mut c = Contour::new(
        vertices.iter().map(|v| design_xy(v.x, v.y)).collect(),
        closed,
    );
    for i in 0..c.segment_count() {
        let (a, b) = (&vertices[i], &vertices[(i + 1) % n]);
        if a.kind & KIND_CURVE_AFTER != 0 {
            c.set_curve(
                i,
                Some(Bezier {
                    c1: design_xy(a.handles[1].0, a.handles[1].1),
                    c2: design_xy(b.handles[0].0, b.handles[0].1),
                }),
            );
        }
    }
    c
}

pub fn commands() -> Vec<CommandSpec> {
    vec![spec(
        "import.pes",
        "Import a PE-DESIGN 11 PES file's shapes and Manual Punch blocks as editable objects",
        Effect::Edit,
        vec![
            req(
                "path",
                "string",
                "PES file (relative to the session directory)",
            ),
            opt("group", "bool", "Keep the file's groups (default true)"),
        ],
        |s, p| {
            let (_, data) = read_file(s, p, MAX_PES_BYTES)?;
            let section = openstitch_formats::pes::read_objects(&data)
                .map_err(|e| p.failed(e.to_string()))?;
            if section.version != "0110" {
                return Err(p.failed(format!(
                    "PES {} has no editable objects (PE-DESIGN 11's 0110 has); use stitches.import",
                    section.version
                )));
            }
            let imported = objects(&section);
            if imported.objects.is_empty() {
                return Err(p.failed("the file has no shapes OpenStitch can import"));
            }
            let mut ids: Vec<u64> = Vec::with_capacity(imported.objects.len());
            for o in imported.objects {
                ids.push(s.design.add(o));
            }
            let mut groups = Vec::new();
            if p.bool("group", Some(true))? {
                for members in imported.groups.iter().filter(|g| g.len() > 1) {
                    let g = s.design.new_group_id();
                    for &k in members {
                        if let Some(o) = s.design.objects.iter_mut().find(|o| o.id == ids[k]) {
                            o.group = Some(g);
                        }
                    }
                    groups.push(g);
                }
            }
            let skipped: Vec<_> = imported
                .skipped
                .iter()
                .map(|(at, why)| json!({ "position": at, "reason": why }))
                .collect();
            Ok(json!({
                "ids": ids,
                "groups": groups,
                "textsNotImported": imported.texts,
                "stitchGroupsNotImported": imported.stitch_groups,
                "skipped": skipped,
                "unparsed": section.unparsed,
            }))
        },
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> ObjectSection {
        let path = format!(
            "{}/../openstitch-formats/tests/fixtures/pe11/{name}.pes",
            env!("CARGO_MANIFEST_DIR")
        );
        let data = std::fs::read(&path).expect("fixture");
        openstitch_formats::pes::read_objects(&data).expect("objects")
    }

    fn blocks(i: &Imported) -> Vec<usize> {
        i.objects
            .iter()
            .filter_map(|o| o.region.as_ref())
            .map(|r| r.direction_lines.len())
            .filter(|&n| n > 0)
            .collect()
    }

    #[test]
    fn convert_to_blocks_imports_as_pe_designs_cut() {
        // PE-DESIGN 11.50's Convert to Blocks of Lato Bold H (stroke-blocks-H): the left stem
        // top (3 lines), the left stem bottom (4), the left junction square (2), the bar (4)
        // and the right stem whole (7), with a running piece between the first ones, all in one
        // group. A: left leg upper and lower, the junction, the bar, the right leg with the apex.
        let h = objects(&fixture("stroke-blocks-H"));
        assert!(h.skipped.is_empty(), "{:?}", h.skipped);
        assert_eq!(h.objects.len(), 8);
        assert_eq!(blocks(&h), [3, 4, 2, 4, 7]);
        assert_eq!(h.groups, [(0..8).collect::<Vec<usize>>()]);
        let a = objects(&fixture("stroke-blocks-A"));
        assert!(a.skipped.is_empty(), "{:?}", a.skipped);
        assert_eq!(blocks(&a).len(), 5);
        // The blocks are satin columns on the page (100 x 100 mm), every line inside the
        // block's own outline box.
        for o in h.objects.iter().chain(&a.objects) {
            let Some(r) = o.region.as_ref().filter(|r| !r.direction_lines.is_empty()) else {
                // The running pieces between blocks: open, line only.
                assert!(o.region.is_none() && o.line.is_some());
                continue;
            };
            assert_eq!(r.kind, RegionType::Satin);
            let b = o.bounds().expect("bounds");
            for p in r.direction_lines.iter().flatten() {
                assert!((0.0..=1000.0).contains(&p.x) && (0.0..=1000.0).contains(&p.y));
                assert!(b.min_x - 1.0 <= p.x && p.x <= b.max_x + 1.0, "{p:?} {b:?}");
                assert!(b.min_y - 1.0 <= p.y && p.y <= b.max_y + 1.0, "{p:?} {b:?}");
            }
        }
    }

    #[test]
    fn manual_punch_blocks_keep_their_lines_and_curves() {
        // The Manual Punch tool's Straight Block (o-manual-straight) and Curve Block
        // (o-manual-curve): the lines join vertex k and 2n - 1 - k.
        for (name, curved) in [("o-manual-straight", false), ("o-manual-curve", true)] {
            let section = fixture(name);
            let i = objects(&section);
            let s = &section.shapes[0];
            let o = &i.objects[0];
            let r = o.region.as_ref().expect("region");
            assert_eq!(r.direction_lines.len(), s.punch.len(), "{name}");
            assert_eq!(r.curved_lines, curved, "{name}");
            let n = s.vertices.len();
            let first = design_xy(s.vertices[0].x, s.vertices[0].y);
            let last = design_xy(s.vertices[n - 1].x, s.vertices[n - 1].y);
            assert_eq!(r.direction_lines[0], [first, last], "{name}");
            assert_eq!(o.contours[0].has_curves(), curved, "{name}");
        }
    }

    #[test]
    fn exported_shapes_import_back() {
        // The exporter's shapes import back as the same outline, region and line.
        use openstitch_design::Design;
        let mut d = Design::default();
        let square = Contour::new(
            vec![
                v2(100.0, 100.0),
                v2(500.0, 100.0),
                v2(500.0, 500.0),
                v2(100.0, 500.0),
            ],
            true,
        );
        let mut region = RegionSewing::new(RegionType::Fill, Thread::rgb(0x0000ff));
        region.angle = 30.0;
        region.density = 5.0;
        let line = LineSewing::new(LineType::Running, Thread::rgb(0xff0000));
        d.add(DesignObject::shape(
            vec![square.clone()],
            Some(region),
            Some(line),
        ));
        let pes = crate::pes11_export::objects(&d).expect("objects");
        let section = ObjectSection {
            version: "0110".into(),
            threads: pes.threads.clone(),
            shapes: pes.shapes.clone(),
            ..ObjectSection::default()
        };
        check(&objects(&section), &square);

        fn check(i: &Imported, square: &Contour) {
            let o = &i.objects[0];
            assert_eq!(o.contours[0].points, square.points);
            let r = o.region.as_ref().expect("region");
            assert_eq!((r.kind, r.angle, r.density), (RegionType::Fill, 30.0, 5.0));
            assert_eq!(r.thread.rgb_value(), 0x0000ff);
            let l = o.line.as_ref().expect("line");
            assert_eq!(
                (l.kind, l.thread.rgb_value()),
                (LineType::Running, 0xff0000)
            );
        }
    }
}
