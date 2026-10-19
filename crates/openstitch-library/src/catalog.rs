//! Catalogues of designs (guide: "Outputting a Catalog of Embroidery Designs"): printable
//! pages as PDF or PNG, an HTML page with an image folder, and CSV.
//!
//! Pages are laid out once as a display list in points (1/72 inch, origin top left) and
//! drawn by two back ends: a small PDF writer (standard Helvetica fonts, Flate-compressed
//! images, no dates, so output is deterministic) and a tiny-skia rasteriser that sets text
//! with OpenStitch's bundled Lato font.

use crate::convert::write_file;
use crate::info::{self, DesignInfo, thread_label};
use crate::{LibraryError, Limits, Result, io_err, thumb};
use openstitch_core::{StitchPattern, Thread};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    /// Portrait width and height in points.
    pub fn size(self) -> (f64, f64) {
        match self {
            Paper::A4 => (595.28, 841.89),
            Paper::Letter => (612.0, 792.0),
        }
    }
}

/// Print layout (guide: Catalog style 1, 4, 12; Instruction Guide style).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Style {
    One,
    #[default]
    Four,
    Twelve,
    /// One design per page with its whole colour sequence as a numbered list.
    Instructions,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::One, Style::Four, Style::Twelve, Style::Instructions];

    pub fn label(self) -> &'static str {
        match self {
            Style::One => "Catalog style 1",
            Style::Four => "Catalog style 4",
            Style::Twelve => "Catalog style 12",
            Style::Instructions => "Instruction guide",
        }
    }

    /// Columns and rows per page.
    fn grid(self, landscape: bool) -> (usize, usize) {
        match (self, landscape) {
            (Style::One | Style::Instructions, _) => (1, 1),
            (Style::Four, _) => (2, 2),
            (Style::Twelve, false) => (3, 4),
            (Style::Twelve, true) => (4, 3),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorList {
    /// Every colour block in sewing order.
    #[default]
    Changes,
    /// Each distinct thread once.
    Totals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

impl Units {
    pub fn format(self, mm: f64) -> String {
        match self {
            Units::Mm => format!("{mm:.1} mm"),
            Units::Inch => format!("{:.2} in", mm / 25.4),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Setup {
    pub paper: Paper,
    pub landscape: bool,
    pub style: Style,
    pub color_list: ColorList,
    /// Label threads with brand and number instead of the colour name.
    pub brand_numbers: bool,
    pub units: Units,
    /// Shown at the top of each page and as the HTML title.
    pub title: String,
}

/// A design to put in a catalogue.
#[derive(Clone, Debug)]
pub struct Item {
    pub info: DesignInfo,
    pub pattern: Option<StitchPattern>,
}

/// Reads designs for a catalogue; files that are not design files are left out.
pub fn load_items(paths: &[PathBuf], limits: &Limits) -> Vec<Item> {
    paths
        .iter()
        .filter_map(|p| info::inspect(p, limits).ok())
        .map(|(info, pattern)| Item { info, pattern })
        .collect()
}

fn colours<'a>(item: &'a Item, setup: &Setup) -> &'a [Thread] {
    match (&item.info.summary, setup.color_list) {
        (Ok(s), ColorList::Changes) => &s.sequence,
        (Ok(s), ColorList::Totals) => &s.totals,
        (Err(_), _) => &[],
    }
}

/// Facts shown under each picture.
fn facts(item: &Item, setup: &Setup) -> Vec<String> {
    match &item.info.summary {
        Ok(s) => vec![
            format!(
                "{} × {}",
                setup.units.format(s.width_mm),
                setup.units.format(s.height_mm)
            ),
            format!(
                "{} stitches · about {:.0} min",
                s.stitches,
                s.sewing_minutes.ceil()
            ),
            format!(
                "{} colour changes · {} colours",
                s.color_changes(),
                s.total_colors()
            ),
        ],
        Err(e) => vec![format!("Cannot be read: {e}")],
    }
}

// MARK: layout

#[derive(Clone, Debug, PartialEq)]
enum Op {
    /// Baseline at `y`.
    Text {
        x: f64,
        y: f64,
        size: f64,
        bold: bool,
        text: String,
    },
    Fill {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgb: [u8; 3],
    },
    Frame {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
    /// The item's stitches fitted into the box.
    Design {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        item: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Page {
    ops: Vec<Op>,
}

const MARGIN: f64 = 36.0;

/// Approximate Helvetica advance, used to shorten text that would not fit.
fn fit(text: &str, size: f64, width: f64) -> String {
    let max = ((width / (size * 0.55)).floor() as usize).max(1);
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn page_size(setup: &Setup) -> (f64, f64) {
    let (w, h) = setup.paper.size();
    if setup.landscape { (h, w) } else { (w, h) }
}

fn layout(items: &[Item], setup: &Setup) -> Vec<Page> {
    let (pw, ph) = page_size(setup);
    let (cols, rows) = setup.style.grid(setup.landscape);
    let per_page = cols * rows;
    let count = items.len().div_ceil(per_page).max(1);
    let header = if setup.title.trim().is_empty() {
        0.0
    } else {
        28.0
    };
    let footer = 20.0;
    let (gx, gy) = (12.0, 12.0);
    let cw = (pw - 2.0 * MARGIN - gx * (cols as f64 - 1.0)) / cols as f64;
    let ch = (ph - 2.0 * MARGIN - header - footer - gy * (rows as f64 - 1.0)) / rows as f64;
    let mut pages = Vec::new();
    for p in 0..count {
        let mut ops = Vec::new();
        if header > 0.0 {
            ops.push(Op::Text {
                x: MARGIN,
                y: MARGIN + 16.0,
                size: 16.0,
                bold: true,
                text: fit(&setup.title, 16.0, pw - 2.0 * MARGIN),
            });
        }
        ops.push(Op::Text {
            x: pw - MARGIN - 70.0,
            y: ph - MARGIN + 4.0,
            size: 8.0,
            bold: false,
            text: format!("Page {} of {count}", p + 1),
        });
        for slot in 0..per_page {
            let i = p * per_page + slot;
            let Some(item) = items.get(i) else { break };
            let (c, r) = (slot % cols, slot / cols);
            let x = MARGIN + c as f64 * (cw + gx);
            let y = MARGIN + header + r as f64 * (ch + gy);
            if setup.style == Style::Instructions {
                instruction_page(&mut ops, item, i, setup, x, y, cw, ch);
            } else {
                cell(&mut ops, item, i, setup, x, y, cw, ch);
            }
        }
        pages.push(Page { ops });
    }
    pages
}

#[allow(clippy::too_many_arguments)]
fn cell(ops: &mut Vec<Op>, item: &Item, i: usize, setup: &Setup, x: f64, y: f64, w: f64, h: f64) {
    ops.push(Op::Frame { x, y, w, h });
    let pad = 8.0;
    let size = if w < 200.0 { 7.5 } else { 9.0 };
    let line = size * 1.35;
    let text_w = w - 2.0 * pad;
    let facts = facts(item, setup);
    let colours = colours(item, setup);
    // Picture takes what the text leaves, at most a square.
    let text_lines = 1 + facts.len() + colours.len().min(6);
    let pic_h = (h - 2.0 * pad - text_lines as f64 * line - 6.0)
        .min(text_w)
        .max(h * 0.4);
    ops.push(Op::Design {
        x: x + pad,
        y: y + pad,
        w: text_w,
        h: pic_h,
        item: i,
    });
    let mut ty = y + pad + pic_h + 6.0 + size;
    let bottom = y + h - pad;
    ops.push(Op::Text {
        x: x + pad,
        y: ty,
        size,
        bold: true,
        text: fit(&item.info.file_name(), size, text_w),
    });
    for f in facts {
        ty += line;
        if ty > bottom {
            return;
        }
        ops.push(Op::Text {
            x: x + pad,
            y: ty,
            size,
            bold: false,
            text: fit(&f, size, text_w),
        });
    }
    swatches(
        ops,
        colours,
        setup,
        x + pad,
        ty,
        text_w,
        bottom,
        size,
        false,
    );
}

/// Colour list from below `top` down to `bottom`; ends with "+N more" when it does not fit.
#[allow(clippy::too_many_arguments)]
fn swatches(
    ops: &mut Vec<Op>,
    colours: &[Thread],
    setup: &Setup,
    x: f64,
    top: f64,
    w: f64,
    bottom: f64,
    size: f64,
    numbered: bool,
) {
    let line = size * 1.35;
    let mut ty = top;
    for (n, t) in colours.iter().enumerate() {
        ty += line;
        let last_room = ty + line > bottom;
        if ty > bottom || (last_room && n + 1 < colours.len()) {
            ops.push(Op::Text {
                x,
                y: ty.min(bottom),
                size,
                bold: false,
                text: format!("+{} more", colours.len() - n),
            });
            return;
        }
        ops.push(Op::Fill {
            x,
            y: ty - size * 0.8,
            w: size,
            h: size,
            rgb: [t.red, t.green, t.blue],
        });
        let label = thread_label(t, setup.brand_numbers);
        let text = if numbered {
            format!("{}. {label}", n + 1)
        } else {
            label
        };
        ops.push(Op::Text {
            x: x + size * 1.5,
            y: ty,
            size,
            bold: false,
            text: fit(&text, size, w - size * 1.5),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn instruction_page(
    ops: &mut Vec<Op>,
    item: &Item,
    i: usize,
    setup: &Setup,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let size = 10.0;
    let line = size * 1.4;
    let pic = (h * 0.5).min(w);
    ops.push(Op::Frame { x, y, w, h: pic });
    ops.push(Op::Design {
        x: x + 6.0,
        y: y + 6.0,
        w: w - 12.0,
        h: pic - 12.0,
        item: i,
    });
    let mut ty = y + pic + 20.0;
    ops.push(Op::Text {
        x,
        y: ty,
        size: 13.0,
        bold: true,
        text: fit(&item.info.file_name(), 13.0, w),
    });
    for f in facts(item, setup) {
        ty += line;
        ops.push(Op::Text {
            x,
            y: ty,
            size,
            bold: false,
            text: fit(&f, size, w),
        });
    }
    if let Ok(s) = &item.info.summary {
        for (k, label) in info::PROPERTY_KEYS {
            if let Some(v) = s.properties.get(k) {
                ty += line;
                ops.push(Op::Text {
                    x,
                    y: ty,
                    size,
                    bold: false,
                    text: fit(&format!("{label}: {v}"), size, w),
                });
            }
        }
    }
    ty += line * 0.6;
    // Two columns of numbered colours.
    let colours = colours(item, setup);
    let half = colours.len().div_ceil(2);
    let col_w = w / 2.0 - 6.0;
    swatches(
        ops,
        &colours[..half],
        setup,
        x,
        ty,
        col_w,
        y + h,
        size,
        true,
    );
    let mut right = Vec::new();
    swatches(
        &mut right,
        &colours[half..],
        setup,
        x + w / 2.0,
        ty,
        col_w,
        y + h,
        size,
        true,
    );
    // Number the right column after the left one.
    for op in &mut right {
        if let Op::Text { text, .. } = op
            && let Some((n, rest)) = text.split_once(". ")
            && let Ok(n) = n.parse::<usize>()
        {
            *text = format!("{}. {rest}", n + half);
        }
    }
    ops.extend(right);
}

// MARK: PDF

/// A PDF with one page per sheet.
pub fn pdf(items: &[Item], setup: &Setup) -> Vec<u8> {
    let pages = layout(items, setup);
    let (pw, ph) = page_size(setup);
    let mut w = PdfWriter::default();
    // 1 catalog, 2 page tree, 3 and 4 fonts; then per page: page, content, images.
    w.reserve(4);
    let mut kids = Vec::new();
    for page in &pages {
        let page_id = w.reserve(1);
        let content_id = w.reserve(1);
        let mut images = Vec::new();
        let mut content = String::new();
        for op in &page.ops {
            draw_pdf(op, items, ph, &mut content, &mut images, &mut w);
        }
        let xobjects: String = images
            .iter()
            .map(|(name, id)| format!("/{name} {id} 0 R "))
            .collect();
        w.set(
            page_id,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> /XObject << {xobjects}>> >> /Contents {content_id} 0 R >>"
            )
            .into_bytes(),
        );
        w.set_stream(content_id, "", content.into_bytes());
        kids.push(format!("{page_id} 0 R"));
    }
    w.set(1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    w.set(
        2,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            kids.len()
        )
        .into_bytes(),
    );
    for (id, font) in [(3, "Helvetica"), (4, "Helvetica-Bold")] {
        w.set(
            id,
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{font} /Encoding /WinAnsiEncoding >>"
            )
            .into_bytes(),
        );
    }
    w.finish()
}

fn draw_pdf(
    op: &Op,
    items: &[Item],
    ph: f64,
    out: &mut String,
    images: &mut Vec<(String, usize)>,
    w: &mut PdfWriter,
) {
    match op {
        Op::Text {
            x,
            y,
            size,
            bold,
            text,
        } => {
            let font = if *bold { "F2" } else { "F1" };
            let _ = writeln!(
                out,
                "0 0 0 rg BT /{font} {size:.2} Tf {x:.2} {:.2} Td ({}) Tj ET",
                ph - y,
                pdf_string(text)
            );
        }
        Op::Fill { x, y, w, h, rgb } => {
            let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
            let _ = writeln!(
                out,
                "{r:.3} {g:.3} {b:.3} rg {x:.2} {:.2} {w:.2} {h:.2} re f 0.5 G 0.3 w {x:.2} {:.2} {w:.2} {h:.2} re S",
                ph - y - h,
                ph - y - h
            );
        }
        Op::Frame { x, y, w, h } => {
            let _ = writeln!(
                out,
                "0.75 G 0.6 w {x:.2} {:.2} {w:.2} {h:.2} re S",
                ph - y - h
            );
        }
        Op::Design {
            x,
            y,
            w: bw,
            h: bh,
            item,
        } => {
            let Some(pattern) = items.get(*item).and_then(|i| i.pattern.as_ref()) else {
                return;
            };
            // 150 dpi.
            let px = |v: f64| ((v * 150.0 / 72.0).round() as u32).clamp(1, 2400);
            let img = thumb::render_rect(pattern, px(*bw), px(*bh), [255, 255, 255]);
            let data = miniz_oxide::deflate::compress_to_vec_zlib(&img.rgb(), 6);
            let id = w.reserve(1);
            w.set_stream(
                id,
                &format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
                    img.width, img.height
                ),
                data,
            );
            let name = format!("Im{}", images.len() + 1);
            let _ = writeln!(
                out,
                "q {bw:.2} 0 0 {bh:.2} {x:.2} {:.2} cm /{name} Do Q",
                ph - y - bh
            );
            images.push((name, id));
        }
    }
}

/// A PDF literal string in WinAnsiEncoding; characters outside it become `?`.
fn pdf_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let byte: u8 = match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
                continue;
            }
            ' '..='~' => c as u8,
            '\u{a0}'..='\u{ff}' => c as u32 as u8,
            '€' => 0x80,
            '…' => 0x85,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            _ => b'?',
        };
        if byte.is_ascii() {
            out.push(byte as char);
        } else {
            let _ = write!(out, "\\{byte:03o}");
        }
    }
    out
}

#[derive(Default)]
struct PdfWriter {
    objects: Vec<Vec<u8>>,
}

impl PdfWriter {
    /// Reserves `n` object numbers and returns the first (numbers start at 1).
    fn reserve(&mut self, n: usize) -> usize {
        let first = self.objects.len() + 1;
        self.objects.extend(std::iter::repeat_n(Vec::new(), n));
        first
    }

    fn set(&mut self, id: usize, body: Vec<u8>) {
        self.objects[id - 1] = body;
    }

    fn set_stream(&mut self, id: usize, dict: &str, data: Vec<u8>) {
        let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend(data);
        body.extend(b"\nendstream");
        self.set(id, body);
    }

    fn finish(self) -> Vec<u8> {
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
            out.extend(body);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1);
        for o in offsets {
            let _ = writeln!(tail, "{o:010} 00000 n ");
        }
        let _ = write!(
            tail,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.objects.len() + 1
        );
        out.extend(tail.into_bytes());
        out
    }
}

// MARK: PNG pages

/// One PNG per page at `dpi` (clamped to 36–300).
pub fn png_pages(items: &[Item], setup: &Setup, dpi: f64) -> Vec<Vec<u8>> {
    let dpi = dpi.clamp(36.0, 300.0);
    let k = dpi / 72.0;
    let (pw, ph) = page_size(setup);
    let (w, h) = ((pw * k).round() as u32, (ph * k).round() as u32);
    layout(items, setup)
        .iter()
        .map(|page| {
            let Some(mut pix) = tiny_skia::Pixmap::new(w, h) else {
                return Vec::new();
            };
            pix.fill(tiny_skia::Color::WHITE);
            for op in &page.ops {
                draw_png(op, items, k, &mut pix);
            }
            let rgb: Vec<u8> = pix
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            thumb::encode_png(w, h, &rgb)
        })
        .collect()
}

fn draw_png(op: &Op, items: &[Item], k: f64, pix: &mut tiny_skia::Pixmap) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Rect, Stroke, Transform};
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    let rect = |x: f64, y: f64, w: f64, h: f64| {
        Rect::from_xywh(
            (x * k) as f32,
            (y * k) as f32,
            (w * k) as f32,
            (h * k) as f32,
        )
    };
    match op {
        Op::Text {
            x, y, size, text, ..
        } => {
            // Lato's capital height is about 0.72 em; text sizes are in points, layout in mm.
            let cap_mm = size * 0.72 * 25.4 / 72.0;
            let spec = openstitch_design::text::TextSpec::new(
                text.clone(),
                openstitch_design::text::DEFAULT_FONT,
                cap_mm,
            );
            let Ok(laid) = openstitch_design::text::layout(&spec, None) else {
                return;
            };
            let mut pb = PathBuilder::new();
            for c in laid.contours() {
                let mut pts = c.path().into_iter();
                let Some(first) = pts.next() else { continue };
                // Contours are in 0.1 mm with the baseline at y = 0.
                let at = |v: openstitch_design::geometry::Vec2| {
                    (
                        ((x + v.x / 10.0 * 72.0 / 25.4) * k) as f32,
                        ((y + v.y / 10.0 * 72.0 / 25.4) * k) as f32,
                    )
                };
                let (fx, fy) = at(first);
                pb.move_to(fx, fy);
                for p in pts {
                    let (px, py) = at(p);
                    pb.line_to(px, py);
                }
                pb.close();
            }
            if let Some(path) = pb.finish() {
                paint.set_color_rgba8(0, 0, 0, 255);
                pix.fill_path(
                    &path,
                    &paint,
                    FillRule::EvenOdd,
                    Transform::identity(),
                    None,
                );
            }
        }
        Op::Fill { x, y, w, h, rgb } => {
            if let Some(r) = rect(*x, *y, *w, *h) {
                paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
                pix.fill_rect(r, &paint, Transform::identity(), None);
                paint.set_color_rgba8(128, 128, 128, 255);
                let stroke = Stroke {
                    width: (0.3 * k) as f32,
                    ..Stroke::default()
                };
                pix.stroke_path(
                    &PathBuilder::from_rect(r),
                    &paint,
                    &stroke,
                    Transform::identity(),
                    None,
                );
            }
        }
        Op::Frame { x, y, w, h } => {
            if let Some(r) = rect(*x, *y, *w, *h) {
                paint.set_color_rgba8(191, 191, 191, 255);
                let stroke = Stroke {
                    width: (0.6 * k) as f32,
                    ..Stroke::default()
                };
                pix.stroke_path(
                    &PathBuilder::from_rect(r),
                    &paint,
                    &stroke,
                    Transform::identity(),
                    None,
                );
            }
        }
        Op::Design { x, y, w, h, item } => {
            let Some(pattern) = items.get(*item).and_then(|i| i.pattern.as_ref()) else {
                return;
            };
            let (pw, ph) = (
                ((w * k).round() as u32).max(1),
                ((h * k).round() as u32).max(1),
            );
            let img = thumb::render_rect(pattern, pw, ph, [255, 255, 255]);
            let Some(size) = tiny_skia::IntSize::from_wh(img.width, img.height) else {
                return;
            };
            if let Some(src) = tiny_skia::Pixmap::from_vec(img.rgba, size) {
                pix.draw_pixmap(
                    (x * k).round() as i32,
                    (y * k).round() as i32,
                    src.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}

// MARK: HTML

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Percent-encodes one path segment for a relative URL.
fn url_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Writes `html_path` and a sibling folder named after it holding one PNG per design
/// (guide: "Outputting as Image and HTML Files"). Existing output is replaced only when
/// `overwrite` is set. Returns every file written.
pub fn write_html(
    items: &[Item],
    setup: &Setup,
    html_path: &Path,
    overwrite: bool,
) -> Result<Vec<PathBuf>> {
    let stem = html_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LibraryError::InvalidName(html_path.display().to_string()))?;
    let dir = html_path.parent().unwrap_or(Path::new("."));
    let images = dir.join(&stem);
    if !overwrite {
        for p in [html_path, images.as_path()] {
            if std::fs::symlink_metadata(p).is_ok() {
                return Err(LibraryError::Exists(p.to_path_buf()));
            }
        }
    }
    match std::fs::create_dir(&images) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && images.is_dir() => {}
        Err(e) => return Err(io_err(&images)(e)),
    }
    let columns = match setup.style {
        Style::One | Style::Instructions => 1,
        Style::Four => 2,
        Style::Twelve => 4,
    };
    let title = if setup.title.trim().is_empty() {
        "Embroidery designs".to_string()
    } else {
        setup.title.clone()
    };
    let mut written = Vec::new();
    let mut cards = String::new();
    for (n, item) in items.iter().enumerate() {
        let name = item.info.file_name();
        let mut img = String::new();
        if let Some(p) = &item.pattern {
            let file = format!("{:04}.png", n + 1);
            let path = images.join(&file);
            write_file(
                &path,
                &thumb::render(p, 320, [255, 255, 255]).png(),
                overwrite,
            )?;
            written.push(path);
            img = format!(
                "<img src=\"{}/{}\" width=\"320\" height=\"320\" alt=\"{}\">",
                url_segment(&stem),
                file,
                html_escape(&name)
            );
        }
        let facts: String = facts(item, setup)
            .iter()
            .map(|f| format!("<li>{}</li>", html_escape(f)))
            .collect();
        let swatches: String = colours(item, setup)
            .iter()
            .map(|t| {
                format!(
                    "<li><span class=\"sw\" style=\"background:{}\"></span>{}</li>",
                    t.hex(),
                    html_escape(&thread_label(t, setup.brand_numbers))
                )
            })
            .collect();
        let _ = writeln!(
            cards,
            "<figure>{img}<figcaption><strong>{}</strong><ul>{facts}</ul><ul class=\"colours\">{swatches}</ul></figcaption></figure>",
            html_escape(&name)
        );
    }
    let page = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title><style>\
body{{font-family:sans-serif;margin:24px;color:#222}}\
main{{display:grid;grid-template-columns:repeat({columns},minmax(0,1fr));gap:16px}}\
figure{{margin:0;border:1px solid #ccc;padding:8px}}img{{max-width:100%;height:auto}}\
ul{{list-style:none;padding:0;margin:4px 0}}.sw{{display:inline-block;width:10px;height:10px;border:1px solid #888;margin-right:6px}}\
</style></head><body><h1>{t}</h1><main>\n{cards}</main></body></html>\n",
        t = html_escape(&title)
    );
    write_file(html_path, page.as_bytes(), overwrite)?;
    written.push(html_path.to_path_buf());
    Ok(written)
}

// MARK: CSV

/// A CSV cell: quoted when needed; text starting with `=`, `+`, `-`, `@` or a tab gets a
/// leading apostrophe so spreadsheets do not run it as a formula.
fn csv_cell(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// One row per design (guide: "Outputting as a CSV file"): name, size, stitches, colours,
/// path, PES properties and the colour list, comma separated with CRLF line ends.
pub fn csv(items: &[Item], setup: &Setup) -> String {
    let unit = match setup.units {
        Units::Mm => "mm",
        Units::Inch => "in",
    };
    let mut header = vec![
        "File name".to_string(),
        format!("Width ({unit})"),
        format!("Height ({unit})"),
        "Stitches".into(),
        "Color changes".into(),
        "Total colors".into(),
        "Sewing time (min)".into(),
        "Format".into(),
        "File size (bytes)".into(),
        "Folder".into(),
    ];
    header.extend(info::PROPERTY_KEYS.iter().map(|(_, l)| l.to_string()));
    header.push("Colors".into());
    let mut out = header
        .iter()
        .map(|h| csv_cell(h))
        .collect::<Vec<_>>()
        .join(",");
    out.push_str("\r\n");
    let size = |mm: f64| match setup.units {
        Units::Mm => format!("{mm:.1}"),
        Units::Inch => format!("{:.2}", mm / 25.4),
    };
    for item in items {
        let i = &item.info;
        let mut row = vec![i.file_name()];
        match &i.summary {
            Ok(s) => row.extend([
                size(s.width_mm),
                size(s.height_mm),
                s.stitches.to_string(),
                s.color_changes().to_string(),
                s.total_colors().to_string(),
                format!("{:.1}", s.sewing_minutes),
            ]),
            Err(_) => row.extend(std::iter::repeat_n(String::new(), 6)),
        }
        row.push(i.kind.extension().to_uppercase());
        row.push(i.bytes.to_string());
        row.push(i.folder().display().to_string());
        for (k, _) in info::PROPERTY_KEYS {
            row.push(
                i.summary
                    .as_ref()
                    .ok()
                    .and_then(|s| s.properties.get(k).cloned())
                    .unwrap_or_default(),
            );
        }
        row.push(
            colours(item, setup)
                .iter()
                .map(|t| thread_label(t, setup.brand_numbers))
                .collect::<Vec<_>>()
                .join("; "),
        );
        out.push_str(
            &row.iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    out
}

/// Number of pages a catalogue of `items` designs takes.
pub fn page_count(items: usize, setup: &Setup) -> usize {
    let (c, r) = setup.style.grid(setup.landscape);
    items.div_ceil(c * r).max(1)
}
