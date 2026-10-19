//! What gets printed, laid out in millimetres and placed on paper: the design at actual size
//! (tiled over several sheets when it is larger than the paper) with a sewing information page,
//! a reduced one-page print, positioning sheets for each hooping of a split / multi-position /
//! jumbo design, and Print and Stitch (the background image, optionally mirrored, plus an
//! alignment sheet). Every actual-size page carries a 50 mm scale bar to check the printer.

use crate::pdf::{self, Image, PT_PER_MM, Page, n};
use openstitch_core::{Command, StitchPattern, Thread};
use openstitch_design::sections::{self, Section, SectionKind};
use openstitch_design::{Compiler, Design, estimate};
use std::fmt::Write;

pub type Rgb = [u8; 3];
const BLACK: Rgb = [0, 0, 0];
const RED: Rgb = [0xE5, 0x39, 0x35];
const GREEN: Rgb = [0x2E, 0x9E, 0x4F];
const GREY: Rgb = [0x90, 0x90, 0x90];

/// Paper sizes in mm (portrait).
pub const PAPERS: [(&str, f64, f64); 4] = [
    ("a4", 210.0, 297.0),
    ("letter", 215.9, 279.4),
    ("a3", 297.0, 420.0),
    ("legal", 215.9, 355.6),
];
/// Unprinted border on every side, mm.
pub const MARGIN: f64 = 10.0;
/// Space for the heading above the drawing, mm.
const HEADER: f64 = 16.0;
/// Space for the scale bar and footer below the drawing, mm.
const FOOTER: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
}

/// One drawing primitive; coordinates in mm, +y down.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Line {
        a: (f64, f64),
        b: (f64, f64),
        width: f64,
        rgb: Rgb,
        dashed: bool,
    },
    Polyline {
        points: Vec<(f64, f64)>,
        width: f64,
        rgb: Rgb,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stroke: Option<(Rgb, f64)>,
        fill: Option<Rgb>,
    },
    Circle {
        c: (f64, f64),
        r: f64,
        width: f64,
        rgb: Rgb,
    },
    /// Text size in points, independent of scale.
    Text {
        at: (f64, f64),
        size: f64,
        text: String,
        rgb: Rgb,
        align: Align,
    },
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: Image,
        mirror: bool,
    },
}

/// Drawing content and the part of it that must be printed (mm).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    pub items: Vec<Item>,
    pub extent: (f64, f64, f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Paper width and height in mm, portrait; the orientation that fits best is used.
    pub paper: (f64, f64),
    pub sewing_area_box: bool,
    pub center_mark: bool,
    pub template_grid: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            paper: (210.0, 297.0),
            sewing_area_box: true,
            center_mark: true,
            template_grid: false,
        }
    }
}

fn mm(v: f64) -> f64 {
    v / 10.0
}

fn rgb(t: Option<&Thread>) -> Rgb {
    t.map_or(BLACK, |t| [t.red, t.green, t.blue])
}

fn lighten(c: Rgb, k: f64) -> Rgb {
    c.map(|v| (f64::from(v) + (255.0 - f64::from(v)) * k).round() as u8)
}

/// Sewn stitches as polylines in thread colours (mm), stitches after cuts not joined.
pub fn stitch_items(pattern: &StitchPattern, light: bool) -> Vec<Item> {
    let mut items = Vec::new();
    for (thread, stitches) in pattern.color_blocks() {
        let colour = if light {
            lighten(rgb(thread), 0.55)
        } else {
            rgb(thread)
        };
        let mut run: Vec<(f64, f64)> = Vec::new();
        let flush = |run: &mut Vec<(f64, f64)>, items: &mut Vec<Item>| {
            if run.len() > 1 {
                items.push(Item::Polyline {
                    points: std::mem::take(run),
                    width: 0.3,
                    rgb: colour,
                });
            } else {
                run.clear();
            }
        };
        for s in &stitches {
            match s.command {
                Command::Stitch => run.push((mm(s.x), mm(s.y))),
                Command::Jump | Command::Trim | Command::ColorChange => {
                    flush(&mut run, &mut items);
                    if s.command == Command::Jump {
                        run.push((mm(s.x), mm(s.y)));
                    }
                }
                Command::Stop | Command::End => {}
            }
        }
        flush(&mut run, &mut items);
    }
    items
}

fn line(a: (f64, f64), b: (f64, f64), width: f64, rgb: Rgb) -> Item {
    Item::Line {
        a,
        b,
        width,
        rgb,
        dashed: false,
    }
}

fn text(at: (f64, f64), size: f64, s: impl Into<String>, rgb: Rgb) -> Item {
    Item::Text {
        at,
        size,
        text: s.into(),
        rgb,
        align: Align::Left,
    }
}

/// Box, centre axes, centre mark and template grid for an area `x0..x1, y0..y1` (mm).
fn area_guides(r: (f64, f64, f64, f64), opts: &Options, items: &mut Vec<Item>) {
    let (x0, y0, x1, y1) = r;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    if opts.template_grid {
        let mut x = cx - ((cx - x0) / 10.0).floor() * 10.0;
        while x <= x1 + 1e-9 {
            items.push(line((x, y0), (x, y1), 0.1, GREEN));
            x += 10.0;
        }
        let mut y = cy - ((cy - y0) / 10.0).floor() * 10.0;
        while y <= y1 + 1e-9 {
            items.push(line((x0, y), (x1, y), 0.1, GREEN));
            y += 10.0;
        }
    }
    if opts.sewing_area_box {
        items.push(Item::Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
            stroke: Some((BLACK, 0.25)),
            fill: None,
        });
        items.push(line((cx, y0), (cx, y1), 0.15, BLACK));
        items.push(line((x0, cy), (x1, cy), 0.15, BLACK));
    }
    if opts.center_mark {
        items.push(Item::Circle {
            c: (cx, cy),
            r: 2.5,
            width: 0.25,
            rgb: BLACK,
        });
        items.push(line((cx - 5.0, cy), (cx + 5.0, cy), 0.25, BLACK));
        items.push(line((cx, cy - 5.0), (cx, cy + 5.0), 0.25, BLACK));
    }
}

/// Sewing information lines for the info page or the reduced print.
pub fn info_lines(design: &Design, pattern: &StitchPattern) -> Vec<String> {
    let b = pattern.bounds();
    let e = estimate(pattern, &design.machine);
    let hoop = design.machine.hoop.as_deref().unwrap_or("custom");
    vec![
        format!(
            "Design Page {:.1} x {:.1} mm   Hoop {hoop}",
            mm(design.page.width),
            mm(design.page.height)
        ),
        format!(
            "Design size {:.1} x {:.1} mm   Stitches {}   Colour changes {}",
            mm(b.width()),
            mm(b.height()),
            pattern.count(Command::Stitch),
            pattern.count(Command::ColorChange)
        ),
        format!(
            "Thread about {:.1} m, sewing time about {:.0} min",
            e.needle_total() / 1000.0,
            (e.seconds / 60.0).ceil()
        ),
    ]
}

/// The colour list: one swatch and name per colour change, in sewing order.
fn color_list(pattern: &StitchPattern, at: (f64, f64), items: &mut Vec<Item>) -> f64 {
    let mut y = at.1;
    for (i, (t, _)) in pattern.color_blocks().into_iter().enumerate() {
        items.push(Item::Rect {
            x: at.0,
            y: y - 3.0,
            w: 5.0,
            h: 3.5,
            stroke: Some((GREY, 0.1)),
            fill: Some(rgb(t)),
        });
        let name = t.and_then(|t| t.name.clone()).unwrap_or_default();
        let hex = t.map(Thread::hex).unwrap_or_default();
        items.push(text(
            (at.0 + 7.0, y),
            9.0,
            format!("{}. {name} {hex}", i + 1),
            BLACK,
        ));
        y += 5.0;
    }
    y
}

// MARK: placing drawings on paper

/// Pages for `d`: at `scale` 1 a drawing larger than the paper is tiled over several sheets
/// (each labelled, with corner marks to tape them together); `None` fits it on one page.
pub fn place(d: &Drawing, title: &str, scale: Option<f64>, paper: (f64, f64)) -> Vec<Page> {
    let (x0, y0, x1, y1) = d.extent;
    let (cw, ch) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    // Orientation: the one that needs fewer sheets (or, when fitting, scales larger).
    let area = |(pw, ph): (f64, f64)| (pw - 2.0 * MARGIN, ph - 2.0 * MARGIN - HEADER - FOOTER);
    let portrait = paper;
    let landscape = (paper.1, paper.0);
    let sheets = |p: (f64, f64)| {
        let (aw, ah) = area(p);
        ((cw / aw).ceil() * (ch / ah).ceil(), -(aw / cw).min(ah / ch))
    };
    let (sp, sl) = (sheets(portrait), sheets(landscape));
    // Actual size: landscape only when it needs fewer sheets; fitted: when it prints larger.
    let landscape_wins = match scale {
        Some(_) => sl.0 < sp.0,
        None => sl.1 < sp.1,
    };
    let paper = if landscape_wins { landscape } else { portrait };
    let (aw, ah) = area(paper);
    match scale {
        None => {
            let s = (aw / cw).min(ah / ch);
            vec![page(
                d,
                title,
                "",
                paper,
                s,
                (x0, y0),
                (x0 + aw / s, y0 + ah / s),
                false,
            )]
        }
        Some(s) => {
            let (cols, rows) = (
                ((cw * s) / aw).ceil() as usize,
                ((ch * s) / ah).ceil() as usize,
            );
            let tiled = cols * rows > 1;
            let mut pages = Vec::new();
            for r in 0..rows {
                for c in 0..cols {
                    let tx0 = x0 + c as f64 * aw / s;
                    let ty0 = y0 + r as f64 * ah / s;
                    let label = if tiled {
                        format!(
                            "Sheet {} of {} (row {}, column {})",
                            r * cols + c + 1,
                            cols * rows,
                            r + 1,
                            c + 1
                        )
                    } else {
                        String::new()
                    };
                    pages.push(page(
                        d,
                        title,
                        &label,
                        paper,
                        s,
                        (tx0, ty0),
                        (tx0 + aw / s, ty0 + ah / s),
                        tiled,
                    ));
                }
            }
            pages
        }
    }
}

/// One page showing drawing region `from..to` (mm) at `scale`, under a heading.
#[allow(clippy::too_many_arguments)]
fn page(
    d: &Drawing,
    title: &str,
    subtitle: &str,
    paper: (f64, f64),
    scale: f64,
    from: (f64, f64),
    to: (f64, f64),
    crop_marks: bool,
) -> Page {
    let (pw, ph) = paper;
    let (aw, ah) = (pw - 2.0 * MARGIN, ph - 2.0 * MARGIN - HEADER - FOOTER);
    // Centre the visible content in the drawing area.
    let (vx0, vy0, vx1, vy1) = (
        from.0.max(d.extent.0),
        from.1.max(d.extent.1),
        to.0.min(d.extent.2),
        to.1.min(d.extent.3),
    );
    let ox = MARGIN + (aw - (vx1 - vx0) * scale) / 2.0;
    let oy = MARGIN + HEADER + (ah - (vy1 - vy0) * scale) / 2.0;
    let k = PT_PER_MM;
    let px = |x: f64| (ox + (x - vx0) * scale) * k;
    let py = |y: f64| (ph - (oy + (y - vy0) * scale)) * k;
    let mut c = String::new();
    let mut images = Vec::new();
    let color = |c: &mut String, rgb: Rgb, op: &str| {
        let _ = write!(
            c,
            "{} {} {} {op} ",
            n(f64::from(rgb[0]) / 255.0),
            n(f64::from(rgb[1]) / 255.0),
            n(f64::from(rgb[2]) / 255.0)
        );
    };
    // Clip to the visible region (tiles) — headings are drawn after the clip is restored.
    let _ = writeln!(
        c,
        "q {} {} {} {} re W n",
        n(px(vx0)),
        n(py(vy1)),
        n((vx1 - vx0) * scale * k),
        n((vy1 - vy0) * scale * k)
    );
    for item in &d.items {
        match item {
            Item::Line {
                a,
                b,
                width,
                rgb,
                dashed,
            } => {
                color(&mut c, *rgb, "RG");
                let dash = if *dashed { "[3 2] 0 d" } else { "[] 0 d" };
                let _ = writeln!(
                    c,
                    "{} w {dash} {} {} m {} {} l S",
                    n(width * k),
                    n(px(a.0)),
                    n(py(a.1)),
                    n(px(b.0)),
                    n(py(b.1))
                );
            }
            Item::Polyline { points, width, rgb } => {
                color(&mut c, *rgb, "RG");
                let _ = write!(c, "{} w [] 0 d 1 J 1 j ", n((width * scale).max(0.1) * k));
                for (i, p) in points.iter().enumerate() {
                    let _ = write!(
                        c,
                        "{} {} {} ",
                        n(px(p.0)),
                        n(py(p.1)),
                        if i == 0 { "m" } else { "l" }
                    );
                }
                c.push_str("S\n");
            }
            Item::Rect {
                x,
                y,
                w,
                h,
                stroke,
                fill,
            } => {
                let re = format!(
                    "{} {} {} {} re",
                    n(px(*x)),
                    n(py(y + h)),
                    n(w * scale * k),
                    n(h * scale * k)
                );
                if let Some(f) = fill {
                    color(&mut c, *f, "rg");
                    let _ = writeln!(c, "{re} f");
                }
                if let Some((s, width)) = stroke {
                    color(&mut c, *s, "RG");
                    let _ = writeln!(c, "{} w [] 0 d {re} S", n(width * k));
                }
            }
            Item::Circle {
                c: ctr,
                r,
                width,
                rgb,
            } => {
                color(&mut c, *rgb, "RG");
                // Four Bézier quarters.
                let (x, y, rr) = (px(ctr.0), py(ctr.1), r * scale * k);
                let m = rr * 0.552_284_75;
                let _ = writeln!(
                    c,
                    "{} w [] 0 d {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c S",
                    n(width * k),
                    n(x + rr),
                    n(y),
                    n(x + rr),
                    n(y + m),
                    n(x + m),
                    n(y + rr),
                    n(x),
                    n(y + rr),
                    n(x - m),
                    n(y + rr),
                    n(x - rr),
                    n(y + m),
                    n(x - rr),
                    n(y),
                    n(x - rr),
                    n(y - m),
                    n(x - m),
                    n(y - rr),
                    n(x),
                    n(y - rr),
                    n(x + m),
                    n(y - rr),
                    n(x + rr),
                    n(y - m),
                    n(x + rr),
                    n(y)
                );
            }
            Item::Text {
                at,
                size,
                text,
                rgb,
                align,
            } => {
                color(&mut c, *rgb, "rg");
                let dx = match align {
                    Align::Left => 0.0,
                    Align::Center => -pdf::text_width(text, *size) / 2.0,
                };
                let _ = writeln!(
                    c,
                    "BT /F1 {} Tf {} {} Td {} Tj ET",
                    n(*size),
                    n(px(at.0) + dx),
                    n(py(at.1)),
                    pdf::literal(text)
                );
            }
            Item::Image {
                x,
                y,
                w,
                h,
                image,
                mirror,
            } => {
                let (iw, ih) = (w * scale * k, h * scale * k);
                let (a, e) = if *mirror {
                    (-iw, px(x + w))
                } else {
                    (iw, px(*x))
                };
                let _ = writeln!(
                    c,
                    "q {} 0 0 {} {} {} cm /Im{} Do Q",
                    n(a),
                    n(ih),
                    n(e),
                    n(py(y + h)),
                    images.len()
                );
                images.push(image.clone());
            }
        }
    }
    c.push_str("Q\n");
    // Heading.
    let _ = writeln!(
        c,
        "0 0 0 rg BT /F1 12 Tf {} {} Td {} Tj ET",
        n(MARGIN * k),
        n((ph - MARGIN - 6.0) * k),
        pdf::literal(title)
    );
    let scale_text = if (scale - 1.0).abs() < 1e-9 {
        "Actual size (100 %): print without page scaling".to_string()
    } else {
        format!("Reduced to {:.0} %", scale * 100.0)
    };
    let sub = if subtitle.is_empty() {
        scale_text.clone()
    } else {
        format!("{subtitle}   {scale_text}")
    };
    let _ = writeln!(
        c,
        "0.3 0.3 0.3 rg BT /F1 9 Tf {} {} Td {} Tj ET",
        n(MARGIN * k),
        n((ph - MARGIN - 12.0) * k),
        pdf::literal(&sub)
    );
    // Scale bar: 50 mm at the printed scale, to check the printout with a ruler.
    if (scale - 1.0).abs() < 1e-9 {
        let y = (MARGIN + 4.0) * k;
        let (bx0, bx1) = (MARGIN * k, (MARGIN + 50.0) * k);
        let _ = writeln!(
            c,
            "0 0 0 RG 0.6 w [] 0 d {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S",
            n(bx0),
            n(y),
            n(bx1),
            n(y),
            n(bx0),
            n(y - 3.0),
            n(bx0),
            n(y + 3.0),
            n(bx1),
            n(y - 3.0),
            n(bx1),
            n(y + 3.0)
        );
        let _ = writeln!(
            c,
            "0 0 0 rg BT /F1 8 Tf {} {} Td (50 mm) Tj ET",
            n(bx1 + 6.0),
            n(y - 3.0)
        );
    }
    if crop_marks {
        // Corner marks of the tile, to trim and tape sheets together.
        let (l, r, t, b) = (px(vx0), px(vx1), py(vy0), py(vy1));
        let m = 6.0 * k;
        let _ = writeln!(
            c,
            "0 0 0 RG 0.3 w {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S",
            n(l - m),
            n(t),
            n(l - 1.0),
            n(t),
            n(l),
            n(t + m),
            n(l),
            n(t + 1.0),
            n(r + m),
            n(t),
            n(r + 1.0),
            n(t),
            n(r),
            n(t + m),
            n(r),
            n(t + 1.0),
            n(l - m),
            n(b),
            n(l - 1.0),
            n(b),
            n(l),
            n(b - m),
            n(l),
            n(b - 1.0),
            n(r + m),
            n(b),
            n(r + 1.0),
            n(b),
            n(r),
            n(b - m),
            n(r),
            n(b - 1.0)
        );
    }
    Page {
        width: pw * k,
        height: ph * k,
        content: c,
        images,
    }
}

/// A page of text lines and the colour list.
fn info_page(design: &Design, pattern: &StitchPattern, paper: (f64, f64)) -> Page {
    let mut items = Vec::new();
    let mut y = 0.0;
    for l in info_lines(design, pattern) {
        items.push(text((0.0, y), 10.0, l, BLACK));
        y += 6.0;
    }
    y += 4.0;
    items.push(text((0.0, y), 11.0, "Colour order", BLACK));
    let end = color_list(pattern, (0.0, y + 7.0), &mut items);
    let d = Drawing {
        items,
        extent: (0.0, -5.0, paper.0 - 2.0 * MARGIN, end.max(1.0)),
    };
    let mut p = place(
        &d,
        &format!("{}: sewing information", design.name),
        Some(1.0),
        paper,
    );
    p.truncate(1);
    p.remove(0)
}

// MARK: print types

fn compiled(design: &Design) -> StitchPattern {
    Compiler::for_design(design).compile(design)
}

/// Actual Size: the Design Page at 1:1 with the sewing area box, centre axes, centre mark and
/// optional template grid, then a sewing information page.
pub fn actual_size(design: &Design, opts: &Options) -> Vec<Page> {
    let pattern = compiled(design);
    let r = (0.0, 0.0, mm(design.page.width), mm(design.page.height));
    let mut items = Vec::new();
    area_guides(r, opts, &mut items);
    items.extend(stitch_items(&pattern, false));
    let d = Drawing { items, extent: r };
    let mut pages = place(&d, &design.name, Some(1.0), opts.paper);
    pages.push(info_page(design, &pattern, opts.paper));
    pages
}

/// Reduced Size: the design fitted on one page with the information below.
pub fn reduced(design: &Design, opts: &Options) -> Vec<Page> {
    let pattern = compiled(design);
    let (w, h) = (mm(design.page.width), mm(design.page.height));
    let mut items = Vec::new();
    area_guides((0.0, 0.0, w, h), opts, &mut items);
    items.extend(stitch_items(&pattern, false));
    // Information under the design; spacing follows the drawing's scale so lines of 9 pt
    // text stay apart once the page is reduced to fit.
    let unit = w.max(h) / 150.0;
    let mut y = h + 8.0 * unit;
    let colours = pattern
        .color_blocks()
        .into_iter()
        .enumerate()
        .map(|(i, (t, _))| {
            let name = t.and_then(|t| t.name.clone()).unwrap_or_default();
            format!(
                "{}. {name} {}",
                i + 1,
                t.map(Thread::hex).unwrap_or_default()
            )
        });
    for l in info_lines(design, &pattern).into_iter().chain(colours) {
        items.push(text((0.0, y), 9.0, l, BLACK));
        y += 5.0 * unit;
    }
    let d = Drawing {
        items,
        extent: (0.0, 0.0, w, y),
    };
    place(&d, &design.name, None, opts.paper)
}

/// Positioning sheets: an overview of all hoopings, then one actual-size sheet per hooping with
/// its area, centre lines ending in punch marks, the centre mark, corner points A–D (upper
/// left, upper right, lower right, lower left) and its part of the design.
pub fn positioning_sheets(design: &Design, opts: &Options) -> Vec<Page> {
    let pattern = compiled(design);
    let plan = sections::plan(design);
    let (w, h) = (mm(design.page.width), mm(design.page.height));
    // Overview.
    let mut items = vec![Item::Rect {
        x: 0.0,
        y: 0.0,
        w,
        h,
        stroke: Some((BLACK, 0.3)),
        fill: None,
    }];
    items.extend(stitch_items(&pattern, false));
    for s in &plan.sections {
        let r = &s.rect;
        items.push(Item::Rect {
            x: mm(r.min_x),
            y: mm(r.min_y),
            w: mm(r.width()),
            h: mm(r.height()),
            stroke: Some((RED, 0.4)),
            fill: None,
        });
        items.push(text(
            (mm(r.min_x) + 2.0, mm(r.min_y) + 6.0),
            10.0,
            s.label.clone(),
            RED,
        ));
    }
    let b = pattern.bounds();
    let overview = Drawing {
        items,
        extent: (0.0, 0.0, w, h),
    };
    let title = format!(
        "{}: {} hooping(s), design {:.1} x {:.1} mm",
        design.name,
        plan.sections.len(),
        mm(b.width()),
        mm(b.height())
    );
    let mut pages = place(&overview, &title, None, opts.paper);
    for s in &plan.sections {
        pages.extend(section_sheet(design, &pattern, s, opts));
    }
    pages
}

fn section_sheet(design: &Design, full: &StitchPattern, s: &Section, opts: &Options) -> Vec<Page> {
    // The section's stitches centred on its hoop (turned 180° for jumbo Side 2).
    let part = sections::section_pattern(design, full, s);
    let (hw, hh) = (mm(s.rect.width()) / 2.0, mm(s.rect.height()) / 2.0);
    let mut items = Vec::new();
    items.extend(stitch_items(&part, true));
    items.push(Item::Rect {
        x: -hw,
        y: -hh,
        w: 2.0 * hw,
        h: 2.0 * hh,
        stroke: Some((RED, 0.3)),
        fill: None,
    });
    // Centre lines run past the area; their ends are the points to punch and mark.
    let ext = 8.0;
    items.push(line((-hw - ext, 0.0), (hw + ext, 0.0), 0.2, BLACK));
    items.push(line((0.0, -hh - ext), (0.0, hh + ext), 0.2, BLACK));
    for (x, y) in [
        (-hw - ext, 0.0),
        (hw + ext, 0.0),
        (0.0, -hh - ext),
        (0.0, hh + ext),
    ] {
        items.push(Item::Circle {
            c: (x, y),
            r: 1.2,
            width: 0.3,
            rgb: BLACK,
        });
    }
    if opts.center_mark {
        items.push(Item::Circle {
            c: (0.0, 0.0),
            r: 2.5,
            width: 0.25,
            rgb: BLACK,
        });
    }
    if opts.template_grid {
        area_guides(
            (-hw, -hh, hw, hh),
            &Options {
                sewing_area_box: false,
                center_mark: false,
                ..opts.clone()
            },
            &mut items,
        );
    }
    for (label, x, y) in [
        ("A", -hw, -hh),
        ("B", hw, -hh),
        ("C", hw, hh),
        ("D", -hw, hh),
    ] {
        let dx = if x < 0.0 { -4.0 } else { 1.5 };
        let dy = if y < 0.0 { -1.5 } else { 4.0 };
        items.push(text((x + dx, y + dy), 11.0, label, RED));
    }
    let kind = match s.kind {
        SectionKind::Split => "Section",
        SectionKind::Position => "Hoop position",
        SectionKind::JumboSide => "Jumbo frame",
    };
    let turned = if s.rotate180 {
        " (frame turned 180°)"
    } else {
        ""
    };
    let d = Drawing {
        items,
        extent: (
            -hw - ext - 3.0,
            -hh - ext - 3.0,
            hw + ext + 3.0,
            hh + ext + 3.0,
        ),
    };
    place(
        &d,
        &format!("{}: {kind} {}{turned}", design.name, s.label),
        Some(1.0),
        opts.paper,
    )
}

/// Print and Stitch: the background image at actual size where it lies on the Design Page
/// (mirrored for iron-on transfer sheets when asked), then an alignment sheet with the Design
/// Page, its centre mark and the embroidery area over the image outline.
pub fn print_and_stitch(
    design: &Design,
    image: Image,
    mirror: bool,
    opts: &Options,
) -> Result<Vec<Page>, String> {
    let bg = design
        .background
        .as_ref()
        .ok_or("the design has no background image")?;
    let (w, h) = (mm(design.page.width), mm(design.page.height));
    let (ix, iy, iw, ih) = (mm(bg.x), mm(bg.y), mm(bg.width), mm(bg.height));
    let page_rect = (
        0.0f64.min(ix),
        0.0f64.min(iy),
        w.max(ix + iw),
        h.max(iy + ih),
    );
    let mut items = vec![Item::Image {
        x: ix,
        y: iy,
        w: iw,
        h: ih,
        image,
        mirror,
    }];
    // Small corner ticks of the Design Page so the print can be placed in the hoop.
    for (x, y) in [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)] {
        items.push(line((x - 3.0, y), (x + 3.0, y), 0.2, GREY));
        items.push(line((x, y - 3.0), (x, y + 3.0), 0.2, GREY));
    }
    let image_sheet = Drawing {
        items,
        extent: page_rect,
    };
    let mut pages = place(
        &image_sheet,
        &format!(
            "{}: image{}",
            design.name,
            if mirror { " (mirrored)" } else { "" }
        ),
        Some(1.0),
        opts.paper,
    );
    let pattern = compiled(design);
    let mut items = Vec::new();
    items.push(Item::Rect {
        x: ix,
        y: iy,
        w: iw,
        h: ih,
        stroke: Some((GREY, 0.2)),
        fill: None,
    });
    area_guides(
        (0.0, 0.0, w, h),
        &Options {
            template_grid: false,
            ..opts.clone()
        },
        &mut items,
    );
    if pattern.count(Command::Stitch) > 0 {
        let b = pattern.bounds();
        let (x0, y0, x1, y1) = (mm(b.min_x), mm(b.min_y), mm(b.max_x), mm(b.max_y));
        for (a, c) in [
            ((x0, y0), (x1, y0)),
            ((x1, y0), (x1, y1)),
            ((x1, y1), (x0, y1)),
            ((x0, y1), (x0, y0)),
        ] {
            items.push(Item::Line {
                a,
                b: c,
                width: 0.25,
                rgb: RED,
                dashed: true,
            });
        }
    }
    let align = Drawing {
        items,
        extent: page_rect,
    };
    pages.extend(place(
        &align,
        &format!(
            "{}: alignment sheet (lay on the printed image, align the needle with the centre mark)",
            design.name
        ),
        Some(1.0),
        opts.paper,
    ));
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_design::sections::SectionSettings;
    use openstitch_design::{Contour, DesignObject, DesignPage, LineSewing, LineType, v2};

    fn design(w: f64, h: f64) -> Design {
        let mut d = Design::new(
            "Test",
            DesignPage {
                width: w * 10.0,
                height: h * 10.0,
            },
        );
        d.add(DesignObject::shape(
            vec![Contour::new(
                vec![v2(100.0, 100.0), v2((w - 10.0) * 10.0, (h - 10.0) * 10.0)],
                false,
            )],
            None,
            Some(LineSewing::new(LineType::Running, Thread::rgb(0x0000FF))),
        ));
        d
    }

    /// The numbers in a line of PDF operators.
    fn numbers(s: &str) -> Vec<f64> {
        s.split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect()
    }

    #[test]
    fn actual_size_is_exact_and_has_a_scale_bar() {
        let d = Drawing {
            items: vec![line((0.0, 0.0), (100.0, 0.0), 0.2, BLACK)],
            extent: (0.0, 0.0, 100.0, 10.0),
        };
        let pages = place(&d, "t", Some(1.0), (210.0, 297.0));
        assert_eq!(pages.len(), 1);
        let p = &pages[0];
        assert!((p.width - 595.276).abs() < 0.01 && (p.height - 841.89).abs() < 0.01);
        let seg = p
            .content
            .lines()
            .find(|l| l.contains(" m ") && l.ends_with(" l S"))
            .expect("line");
        let v = numbers(seg);
        // "… x0 y0 m x1 y1 l S": 100 mm is 283.465 pt.
        let (x0, x1) = (v[v.len() - 4], v[v.len() - 2]);
        assert!((x1 - x0 - 283.465).abs() < 0.002, "{seg}");
        assert!(p.content.contains("(50 mm) Tj"));
    }

    #[test]
    fn large_drawings_tile_and_reduced_prints_fit_one_page() {
        let d = Drawing {
            items: vec![line((0.0, 0.0), (400.0, 300.0), 0.2, BLACK)],
            extent: (0.0, 0.0, 400.0, 300.0),
        };
        let tiles = place(&d, "big", Some(1.0), (210.0, 297.0));
        // Landscape A4 holds 277 × 233 mm of drawing: 2 × 2 sheets.
        assert_eq!(tiles.len(), 4);
        assert!(tiles[0].width > tiles[0].height, "landscape");
        assert!(tiles[3].content.contains("Sheet 4 of 4"));
        let one = place(&d, "big", None, (210.0, 297.0));
        assert_eq!(one.len(), 1);
        assert!(one[0].content.contains("Reduced to"));
    }

    #[test]
    fn print_types_produce_the_expected_pages() {
        let d = design(100.0, 100.0);
        let opts = Options::default();
        assert_eq!(actual_size(&d, &opts).len(), 2, "design + information");
        assert_eq!(reduced(&d, &opts).len(), 1);
        let mut split = design(230.0, 160.0);
        split.machine.sections = Some(SectionSettings {
            hoop: "130x180".into(),
            ..Default::default()
        });
        let sheets = positioning_sheets(&split, &opts);
        assert_eq!(sheets.len(), 3, "overview + 2 sections");
        assert!(sheets[1].content.contains("Section Aa"));
        for corner in ["(A) Tj", "(B) Tj", "(C) Tj", "(D) Tj"] {
            assert!(sheets[2].content.contains(corner));
        }
        let bytes = crate::write(&sheets, &split.name);
        assert!(bytes.starts_with(b"%PDF-1.4"));
    }

    #[test]
    fn print_and_stitch_mirrors_the_image_on_request() {
        let mut d = design(100.0, 100.0);
        assert!(
            print_and_stitch(
                &d,
                Image {
                    width: 1,
                    height: 1,
                    rgb: vec![0; 3]
                },
                false,
                &Options::default()
            )
            .is_err()
        );
        d.background = Some(openstitch_design::BackgroundImage {
            path: "x.png".into(),
            pixel_width: 2,
            pixel_height: 1,
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 400.0,
            opacity: 1.0,
            visible: true,
            adjust: Default::default(),
        });
        let img = Image {
            width: 2,
            height: 1,
            rgb: vec![255, 0, 0, 0, 0, 255],
        };
        let plain = print_and_stitch(&d, img.clone(), false, &Options::default()).expect("pages");
        let mirrored = print_and_stitch(&d, img, true, &Options::default()).expect("pages");
        assert_eq!(plain.len(), 2, "image + alignment sheet");
        let cm = |p: &Page| {
            let l = p
                .content
                .lines()
                .find(|l| l.contains(" cm /Im0 Do"))
                .expect("image")
                .to_string();
            numbers(&l)[0]
        };
        // 80 mm wide = 226.77 pt; mirrored flips the sign.
        assert!((cm(&plain[0]) - 226.772).abs() < 0.01);
        assert!((cm(&mirrored[0]) + 226.772).abs() < 0.01);
        assert_eq!(plain[0].images.len(), 1);
    }
}
