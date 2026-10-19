//! Importing artwork: SVG vector paths as editable Bézier shapes (`import.svg`), and a raster
//! image as a background reference layer (`background.*`, with the `digitize` feature, which
//! brings the size-limited image decoder).
//!
//! SVG is parsed by `usvg` (resvg project, MIT / Apache-2.0) without text, fonts or compressed
//! SVG, and with external and embedded `<image>` references disabled, so a file can never make
//! the engine read outside the sandbox. usvg resolves CSS, units, `viewBox` and transforms and
//! turns rectangles, circles, ellipses, lines and polygons into paths. Each visible path becomes
//! one shape object: subpaths become contours (quadratic segments are raised to cubics, so
//! curves stay exact), a solid fill becomes a fill region in that colour, a solid stroke a
//! running line. Gradient and pattern paints use the default colour. SVG's `nonzero` fill rule
//! is approximated by OpenStitch's even-odd rule.

use crate::commands::{CommandSpec, Effect, MM, opt, read_file, req, spec};
use crate::params::{CommandError, Params};
use crate::session::EngineState;
use openstitch_core::Thread;
use openstitch_design::{
    Bezier, Contour, DesignObject, LineSewing, LineType, Rect, RegionSewing, RegionType, Transform,
    Vec2, v2,
};
use serde_json::json;

/// Largest SVG file accepted.
pub const MAX_SVG_BYTES: usize = 16 << 20;
/// Most shape objects one import may add.
pub const MAX_SVG_PATHS: usize = 5_000;
/// Most vertices one import may add.
pub const MAX_SVG_POINTS: usize = 500_000;

/// One SVG unit (CSS pixel, 96 per inch) in 0.1 mm.
const PX: f64 = 254.0 / 96.0;

/// A shape read from an SVG, in 0.1 mm with the SVG's own origin.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgShape {
    pub contours: Vec<Contour>,
    pub fill: Option<Thread>,
    pub stroke: Option<Thread>,
}

/// Why an SVG could not be imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SvgError {
    Parse(String),
    TooLarge(String),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "not a usable SVG: {e}"),
            Self::TooLarge(e) => write!(f, "SVG too large: {e}"),
        }
    }
}

fn options() -> usvg::Options<'static> {
    usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

fn paint_thread(paint: &usvg::Paint) -> Option<Thread> {
    match paint {
        usvg::Paint::Color(c) => Some(Thread::rgb(
            u32::from(c.red) << 16 | u32::from(c.green) << 8 | u32::from(c.blue),
        )),
        _ => None,
    }
}

/// Parses `data` into shapes, in document order.
pub fn parse_svg(data: &[u8]) -> Result<Vec<SvgShape>, SvgError> {
    if data.len() > MAX_SVG_BYTES {
        return Err(SvgError::TooLarge(format!("{} bytes", data.len())));
    }
    let tree =
        usvg::Tree::from_data(data, &options()).map_err(|e| SvgError::Parse(e.to_string()))?;
    let mut shapes = Vec::new();
    let mut points = 0;
    collect(tree.root(), &mut shapes, &mut points)?;
    Ok(shapes)
}

fn collect(
    group: &usvg::Group,
    out: &mut Vec<SvgShape>,
    points: &mut usize,
) -> Result<(), SvgError> {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, out, points)?,
            usvg::Node::Path(path) if path.is_visible() => {
                let fill = path.fill();
                let stroke = path.stroke();
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                let contours = contours(path.data(), &path.abs_transform(), fill.is_some());
                if contours.is_empty() {
                    continue;
                }
                *points += contours.iter().map(|c| c.points.len()).sum::<usize>();
                if out.len() >= MAX_SVG_PATHS || *points > MAX_SVG_POINTS {
                    return Err(SvgError::TooLarge(format!(
                        "more than {MAX_SVG_PATHS} paths or {MAX_SVG_POINTS} points"
                    )));
                }
                let default = |t: Option<Thread>| t.unwrap_or_else(|| Thread::rgb(0));
                out.push(SvgShape {
                    contours,
                    fill: fill.map(|f| default(paint_thread(f.paint()))),
                    stroke: stroke.map(|s| default(paint_thread(s.paint()))),
                });
            }
            _ => {} // images, text (not parsed), hidden paths
        }
    }
    Ok(())
}

/// Subpaths as contours in 0.1 mm. Filled paths close every subpath, as SVG fills do.
fn contours(data: &usvg::tiny_skia_path::Path, t: &usvg::Transform, filled: bool) -> Vec<Contour> {
    use usvg::tiny_skia_path::PathSegment;
    let map = |p: usvg::tiny_skia_path::Point| {
        let (x, y) = (f64::from(p.x), f64::from(p.y));
        v2(
            (f64::from(t.sx) * x + f64::from(t.kx) * y + f64::from(t.tx)) * PX,
            (f64::from(t.ky) * x + f64::from(t.sy) * y + f64::from(t.ty)) * PX,
        )
    };
    let mut out = Vec::new();
    let mut current: Option<Contour> = None;
    let mut last = Vec2::default();
    let finish = |c: Option<Contour>, closed: bool, out: &mut Vec<Contour>| {
        let Some(mut c) = c else { return };
        let closed = closed || filled;
        if closed {
            // A final vertex on top of the first is the closing segment's end.
            if c.points.len() > 1 && c.points[c.points.len() - 1].distance(c.points[0]) < 1e-6 {
                c.points.pop();
            } else {
                c.curves.push(None);
            }
        }
        c.closed = closed;
        if !c.curves.iter().any(Option::is_some) {
            c.curves.clear();
        }
        let enough = if closed {
            c.points.len() >= 3 || (c.points.len() >= 2 && !c.curves.is_empty())
        } else {
            c.points.len() >= 2
        };
        if enough && c.points.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
            out.push(c);
        }
    };
    for segment in data.segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                finish(current.take(), false, &mut out);
                last = map(p);
                current = Some(Contour::new(vec![last], false));
            }
            PathSegment::LineTo(p) => {
                if let Some(c) = &mut current {
                    last = map(p);
                    c.curves.push(None);
                    c.points.push(last);
                }
            }
            PathSegment::QuadTo(q, p) => {
                if let Some(c) = &mut current {
                    let (q, end) = (map(q), map(p));
                    let c1 = last + (q - last) * (2.0 / 3.0);
                    let c2 = end + (q - end) * (2.0 / 3.0);
                    c.curves.push(Some(Bezier { c1, c2 }));
                    c.points.push(end);
                    last = end;
                }
            }
            PathSegment::CubicTo(a, b, p) => {
                if let Some(c) = &mut current {
                    last = map(p);
                    c.curves.push(Some(Bezier {
                        c1: map(a),
                        c2: map(b),
                    }));
                    c.points.push(last);
                }
            }
            PathSegment::Close => {
                finish(current.take(), true, &mut out);
            }
        }
    }
    finish(current.take(), false, &mut out);
    out
}

fn bounds(shapes: &[SvgShape]) -> Option<Rect> {
    Rect::enclosing(
        shapes
            .iter()
            .flat_map(|s| s.contours.iter())
            .flat_map(Contour::outline_points),
    )
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    let mut v = vec![spec(
        "import.svg",
        "Import an SVG's paths as editable shapes (fill → fill region, stroke → running line), grouped",
        Effect::Edit,
        vec![
            req(
                "path",
                "string",
                "SVG file (relative to the session directory)",
            ),
            opt(
                "width",
                "mm",
                "Scale the artwork to this width (default: the SVG's own size)",
            ),
            opt("x", "mm", "Left edge (default: centred on the Design Page)"),
            opt("y", "mm", "Top edge (default: centred on the Design Page)"),
        ],
        |s, p| {
            let (_, data) = read_file(s, p, MAX_SVG_BYTES)?;
            let shapes = parse_svg(&data).map_err(|e| p.failed(e.to_string()))?;
            let Some(b) = bounds(&shapes) else {
                return Err(p.failed("the SVG has no visible filled or stroked paths"));
            };
            place(s, p, shapes, b)
        },
    )];
    #[cfg(feature = "digitize")]
    v.extend(background_commands());
    v
}

fn place(s: &mut EngineState, p: &Params, shapes: Vec<SvgShape>, b: Rect) -> crate::CommandResult {
    let natural = b.width().max(1e-6);
    let width = p.opt("width", (natural / MM).clamp(0.1, 2000.0), 0.1, 2000.0)? * MM;
    let scale = width / natural;
    let height = b.height() * scale;
    if height > 2000.0 * MM {
        return Err(p.invalid("width", "makes the artwork taller than 2000 mm"));
    }
    let page = s.design.page;
    let x = p.opt("x", (page.width - width) / 2.0 / MM, -5000.0, 5000.0)? * MM;
    let y = p.opt("y", (page.height - height) / 2.0 / MM, -5000.0, 5000.0)? * MM;
    let t = Transform::translation(-b.min_x, -b.min_y)
        .then(&Transform::scaling(scale, scale, v2(0.0, 0.0)))
        .then(&Transform::translation(x, y));
    let group = (shapes.len() > 1).then(|| s.design.new_group_id());
    let mut ids = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let closed = shape.contours.iter().all(|c| c.closed);
        let region = shape
            .fill
            .clone()
            .filter(|_| closed)
            .map(|t| RegionSewing::new(RegionType::Fill, t));
        let line = shape
            .stroke
            .or_else(|| (!closed).then(|| shape.fill.clone()).flatten())
            .map(|t| LineSewing::new(LineType::Running, t));
        let contours = shape.contours.iter().map(|c| c.transformed(&t)).collect();
        let mut object = DesignObject::shape(contours, region, line);
        object.group = group;
        ids.push(s.design.add(object));
    }
    Ok(json!({ "ids": ids, "group": group }))
}

#[cfg(feature = "digitize")]
fn background_commands() -> Vec<CommandSpec> {
    use crate::sandbox::Access;
    use openstitch_design::{BackgroundImage, ImageAdjust};
    use openstitch_digitize::Raster;
    use openstitch_digitize::raster::MAX_FILE_BYTES;

    fn opacity(p: &Params, default: f64) -> Result<f64, CommandError> {
        p.opt("opacity", default, 0.0, 1.0)
    }

    fn background<'a>(
        s: &'a mut EngineState,
        p: &Params,
    ) -> Result<&'a mut BackgroundImage, CommandError> {
        s.design
            .background
            .as_mut()
            .ok_or_else(|| p.failed("no background image"))
    }

    /// The background image's pixels as shown on the Design Page.
    fn shown(s: &EngineState, p: &Params) -> Result<Raster, CommandError> {
        let b = s
            .design
            .background
            .as_ref()
            .ok_or_else(|| p.failed("no background image"))?;
        let path = s.resolve(&b.path, Access::Read, &p.command)?;
        let meta = std::fs::metadata(&path)
            .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
        if meta.len() > MAX_FILE_BYTES as u64 {
            return Err(p.failed("background image too large"));
        }
        let data = std::fs::read(&path)
            .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
        let image = Raster::decode(&data).map_err(|e| p.failed(e.to_string()))?;
        Ok(image.with_adjust(&b.adjust))
    }

    vec![
        spec(
            "background.set",
            "Place a PNG, JPEG or BMP image behind the design as a tracing reference (not sewn)",
            Effect::Edit,
            vec![
                req(
                    "path",
                    "string",
                    "Image file (relative to the session directory)",
                ),
                opt(
                    "width",
                    "mm",
                    "Width on the page (default: fit the Design Page)",
                ),
                opt("x", "mm", "Left edge (default: centred on the Design Page)"),
                opt("y", "mm", "Top edge (default: centred on the Design Page)"),
                opt("opacity", "0–1", "Default 0.5"),
            ],
            |s, p| {
                let (path, data) = read_file(s, p, MAX_FILE_BYTES)?;
                let image = Raster::decode(&data).map_err(|e| p.failed(e.to_string()))?;
                let (pw, ph) = (f64::from(image.width()), f64::from(image.height()));
                let page = s.design.page;
                let fit = (page.width / pw).min(page.height / ph) * pw / MM;
                let width = p.opt("width", fit.clamp(1.0, 2000.0), 1.0, 2000.0)? * MM;
                let height = width * ph / pw;
                if height > 2000.0 * MM {
                    return Err(p.invalid("width", "makes the image taller than 2000 mm"));
                }
                let x = p.opt("x", (page.width - width) / 2.0 / MM, -5000.0, 5000.0)? * MM;
                let y = p.opt("y", (page.height - height) / 2.0 / MM, -5000.0, 5000.0)? * MM;
                // Stored absolute, so the reference survives opening the project elsewhere.
                let path = std::fs::canonicalize(&path).unwrap_or(path);
                let background = BackgroundImage {
                    path: path.display().to_string(),
                    pixel_width: image.width(),
                    pixel_height: image.height(),
                    x,
                    y,
                    width,
                    height,
                    opacity: opacity(p, 0.5)?,
                    visible: true,
                    adjust: ImageAdjust::default(),
                };
                let out = json!({
                    "path": background.path,
                    "pixels": [image.width(), image.height()],
                    "widthMM": width / MM,
                    "heightMM": height / MM,
                });
                s.design.background = Some(background);
                Ok(out)
            },
        ),
        spec(
            "background.show",
            "Show or hide the background image, or change its opacity",
            Effect::Edit,
            vec![
                opt("visible", "bool", "Show (default: unchanged)"),
                opt("opacity", "0–1", "Opacity (default: unchanged)"),
            ],
            |s, p| {
                let Some(b) = s.design.background.as_mut() else {
                    return Err(p.failed("no background image"));
                };
                let visible = p.bool("visible", Some(b.visible))?;
                let o = opacity(p, b.opacity)?;
                b.visible = visible;
                b.opacity = o;
                Ok(json!({ "visible": visible, "opacity": o }))
            },
        ),
        spec(
            "background.rotate",
            "Turn the background image 90° about its centre (Image ▸ Rotate Right / Left)",
            Effect::Edit,
            vec![req("direction", "\"right\" | \"left\"", "Right = clockwise")],
            |s, p| {
                let right = match p.string("direction", None)?.as_str() {
                    "right" => true,
                    "left" => false,
                    _ => return Err(p.invalid("direction", "must be \"right\" or \"left\"")),
                };
                let b = background(s, p)?;
                let [l, t, r, bt] = b.adjust.crop;
                // The crop belongs to the turned image, so its sides turn with it.
                b.adjust.crop = if right { [bt, l, t, r] } else { [t, r, bt, l] };
                b.adjust.rotation = (b.adjust.rotation + if right { 1 } else { 3 }) % 4;
                let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
                (b.width, b.height) = (b.height, b.width);
                (b.x, b.y) = (cx - b.width / 2.0, cy - b.height / 2.0);
                Ok(json!({ "rotation": u32::from(b.adjust.rotation) * 90 }))
            },
        ),
        spec(
            "background.modify",
            "Move, scale, crop or adjust the colours of the background image (Image ▸ Modify Image); the image file is not changed",
            Effect::Edit,
            vec![
                opt("x", "mm", "Left edge (default: unchanged)"),
                opt("y", "mm", "Top edge (default: unchanged)"),
                opt(
                    "width",
                    "mm",
                    "Width; the height follows the image's proportions (default: unchanged)",
                ),
                opt(
                    "crop",
                    "[left, top, right, bottom] %",
                    "Cut off each side of the turned image, 0–90 % of its width or height; at least 5 % stays. The kept part stays where it was",
                ),
                opt("brightness", "-100–100", "0 = as in the file"),
                opt("contrast", "-100–100", "0 = as in the file"),
                opt("saturation", "-100–100", "-100 = greys; 0 = as in the file"),
            ],
            |s, p| {
                let crop = match p.get("crop") {
                    None => None,
                    Some(_) => {
                        let v = p.array("crop")?;
                        let mut c = [0.0; 4];
                        if v.len() != 4 {
                            return Err(p.invalid("crop", "must be [left, top, right, bottom]"));
                        }
                        for (i, x) in v.iter().enumerate() {
                            let n = x
                                .as_f64()
                                .filter(|n| (0.0..=ImageAdjust::MAX_CROP * 100.0).contains(n))
                                .ok_or_else(|| p.invalid("crop", "each side must be 0–90 %"))?;
                            c[i] = n / 100.0;
                        }
                        let keep = 1.0 - ImageAdjust::MIN_KEPT + 1e-9;
                        if c[0] + c[2] > keep || c[1] + c[3] > keep {
                            return Err(p.invalid("crop", "must keep at least 5 % of the image"));
                        }
                        Some(c)
                    }
                };
                let level = |key: &str, now: f64| p.opt(key, now, -100.0, 100.0);
                let b = background(s, p)?;
                let brightness = level("brightness", b.adjust.brightness)?;
                let contrast = level("contrast", b.adjust.contrast)?;
                let saturation = level("saturation", b.adjust.saturation)?;
                let mut n = b.clone();
                if let Some(c) = crop {
                    // Scale stays: the whole turned image would cover `full`.
                    let [l, t, r, bt] = n.adjust.crop;
                    let fw = n.width / (1.0 - l - r);
                    let fh = n.height / (1.0 - t - bt);
                    let (fx, fy) = (n.x - l * fw, n.y - t * fh);
                    n.x = fx + c[0] * fw;
                    n.y = fy + c[1] * fh;
                    n.width = fw * (1.0 - c[0] - c[2]);
                    n.height = fh * (1.0 - c[1] - c[3]);
                    n.adjust.crop = c;
                }
                if p.has("width") {
                    let w = p.opt("width", n.width / MM, 1.0, 2000.0)? * MM;
                    n.height *= w / n.width;
                    n.width = w;
                }
                if n.height > 2000.0 * MM || n.width > 2000.0 * MM {
                    return Err(p.invalid("width", "makes the image larger than 2000 mm"));
                }
                n.x = p.opt("x", n.x / MM, -5000.0, 5000.0)? * MM;
                n.y = p.opt("y", n.y / MM, -5000.0, 5000.0)? * MM;
                n.adjust.brightness = brightness;
                n.adjust.contrast = contrast;
                n.adjust.saturation = saturation;
                let out = json!({
                    "x": n.x / MM, "y": n.y / MM, "widthMM": n.width / MM, "heightMM": n.height / MM,
                    "crop": n.adjust.crop.map(|c| c * 100.0),
                    "brightness": brightness, "contrast": contrast, "saturation": saturation,
                });
                *b = n;
                Ok(out)
            },
        ),
        spec(
            "background.save",
            "Save the background image as shown (turned, cropped, colours adjusted) to a PNG, BMP or JPEG file, chosen by the extension",
            Effect::File,
            vec![req(
                "path",
                "string",
                "Image file to write: .png, .bmp or .jpg (relative to the session directory)",
            )],
            |s, p| {
                let target = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
                let ext = target
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default()
                    .to_string();
                let image = shown(s, p)?;
                let bytes = image.encode_as(&ext).map_err(|e| p.invalid("path", e.to_string()))?;
                std::fs::write(&target, bytes).map_err(|e| p.failed(e.to_string()))?;
                Ok(json!({
                    "path": target.display().to_string(),
                    "pixels": [image.width(), image.height()],
                }))
            },
        ),
        spec(
            "background.clear",
            "Remove the background image",
            Effect::Edit,
            vec![],
            |s, p| {
                if s.design.background.take().is_none() {
                    return Err(p.failed("no background image"));
                }
                Ok(json!({}))
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_circle_and_path_become_contours() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">
            <rect x="0" y="0" width="48" height="24" fill="#ff0000"/>
            <circle cx="48" cy="48" r="24" fill="none" stroke="#0000ff"/>
            <path d="M 0 96 Q 48 48 96 96" fill="none" stroke="green"/>
            <g transform="translate(10 10)"><path d="M0 0 L10 0 L10 10 Z M2 2 L8 2 L8 8 Z" fill="#00ff00"/></g>
            <text x="0" y="0">ignored</text>
            <image href="/etc/passwd" width="10" height="10"/>
        </svg>"##;
        let shapes = parse_svg(svg).expect("parse");
        assert_eq!(shapes.len(), 4);
        // 48 px = 12.7 mm = 127 units.
        let r = &shapes[0];
        assert_eq!(r.fill.as_ref().map(Thread::hex).as_deref(), Some("#ff0000"));
        assert!(r.stroke.is_none());
        assert_eq!(r.contours.len(), 1);
        assert!(r.contours[0].closed);
        let rb = Rect::enclosing(r.contours[0].points.iter().copied()).expect("bounds");
        assert!((rb.width() - 127.0).abs() < 1e-6 && (rb.height() - 63.5).abs() < 1e-6);
        // The circle is four exact cubic segments, stroked only.
        let c = &shapes[1];
        assert!(c.fill.is_none());
        assert_eq!(
            c.stroke.as_ref().map(Thread::hex).as_deref(),
            Some("#0000ff")
        );
        assert!(c.contours[0].closed && c.contours[0].has_curves());
        // The quadratic is raised to one cubic; open stroke.
        let q = &shapes[2].contours[0];
        assert!(!q.closed);
        assert_eq!(q.segment_count(), 1);
        assert!(q.curve(0).is_some());
        // Transformed group: two closed subpaths (outer and hole).
        let g = &shapes[3];
        assert_eq!(g.contours.len(), 2);
        assert!((g.contours[0].points[0].x - 10.0 * PX).abs() < 1e-6);
    }

    #[test]
    fn hostile_svgs_fail_cleanly() {
        for bad in [
            &b""[..],
            b"not xml",
            b"<svg",
            b"<svg xmlns='http://www.w3.org/2000/svg'><path d='M 0 0 L nan nan'/></svg>",
            b"<?xml version='1.0'?><!DOCTYPE x [<!ENTITY a 'aaaaaaaaaa'><!ENTITY b '&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;'><!ENTITY c '&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;'>]><svg xmlns='http://www.w3.org/2000/svg'><text>&c;</text></svg>",
        ] {
            let _ = parse_svg(bad);
        }
        let mut many = String::from("<svg xmlns='http://www.w3.org/2000/svg'>");
        for _ in 0..=MAX_SVG_PATHS {
            many.push_str("<rect width='1' height='1'/>");
        }
        many.push_str("</svg>");
        assert!(matches!(
            parse_svg(many.as_bytes()),
            Err(SvgError::TooLarge(_))
        ));
    }
}
