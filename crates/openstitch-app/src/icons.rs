//! OpenStitch's own command icons. Each one is an SVG in `assets/icons/commands`, generated for
//! OpenStitch with Higgsfield (prompts in `prompts.tsv` there), rasterised at the exact pixel
//! size with its charcoal outline swapped for the theme's text colour. The vector shapes drawn
//! below are the fallback when an SVG cannot be rendered. No artwork from any other product is
//! used.

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};
use openstitch_app::editor::ribbon::Action;

/// Accent colours of the icon set.
const BLUE: Color32 = Color32::from_rgb(0x2a, 0x7a, 0xd8);
const RED: Color32 = Color32::from_rgb(0xd8, 0x43, 0x3a);
const GREEN: Color32 = Color32::from_rgb(0x2e, 0x9d, 0x58);
const GOLD: Color32 = Color32::from_rgb(0xe0, 0xa2, 0x1b);
const PURPLE: Color32 = Color32::from_rgb(0x8a, 0x52, 0xc8);

#[derive(Clone, Copy)]
enum Glyph {
    Page,
    Folder,
    Disk,
    DiskPen,
    Printer,
    Info,
    Gear,
    Palette,
    Wand,
    Export,
    Exit,
    Undo,
    Redo,
    Help,
    Database,
    Cursor,
    PointEdit,
    SelectAll,
    Scissors,
    Copy,
    Clipboard,
    Duplicate,
    Trash,
    ArrangeCopy,
    Dropper,
    Text,
    Shapes,
    Rect,
    Ellipse,
    Polygon,
    Curve,
    Pencil,
    OpenLine,
    ZoomIn,
    ZoomOut,
    Zoom,
    Hand,
    Ruler,
    Import,
    Align,
    Rotate,
    Size,
    Group,
    Ungroup,
    Stamp,
    Overlap,
    Frame,
    TextPath,
    Wizard(Color32),
    Needle,
    Image,
    Cross,
    Photo,
    Eye,
    Pane,
    Grid,
    Magnet,
    Guide,
    Font,
    Swatch(bool),
    SewType(bool),
    Blocks,
    Outline,
    Split,
    Direction,
    Point,
    Offset,
    Flower,
    EntryExit,
    Hoop,
}

fn glyph(a: Action) -> Glyph {
    use Action::*;
    use Glyph as G;
    match a {
        New => G::Page,
        Open => G::Folder,
        Save => G::Disk,
        SaveAs => G::DiskPen,
        Print => G::Printer,
        DesignProperty => G::Info,
        DesignSettings | Options => G::Gear,
        FabricSelector | SelectColorPalette | UserThreadCharts => G::Palette,
        Wizard => G::Wand,
        Export => G::Export,
        Exit => G::Exit,
        Undo => G::Undo,
        Redo => G::Redo,
        Help => G::Help,
        DesignDatabase => G::Database,
        StitchCreator | StitchDesignFactory => G::Needle,
        FontCreator | TrueTypeAttributes => G::Font,
        AttributeSets => G::SewType(true),
        SelectTool => G::Cursor,
        PointTool | EditPoint => G::PointEdit,
        SelectAll => G::SelectAll,
        Cut => G::Scissors,
        Copy => G::Copy,
        Paste => G::Clipboard,
        Duplicate => G::Duplicate,
        Delete | DeleteImage => G::Trash,
        ArrangeCopy => G::ArrangeCopy,
        CopyAttributes | PasteAttributes => G::Dropper,
        TextTools | EditText => G::Text,
        ShapesTools => G::Shapes,
        Rectangle | EdgeRadius => G::Rect,
        Ellipse => G::Ellipse,
        ClosedStraight | StraightBlock | CurvedBlock => G::Polygon,
        ClosedCurve | SatinSkeleton => G::Curve,
        SkeletonWidth => G::Size,
        DeriveSkeleton => G::Outline,
        EditCharacter => G::Text,
        PencilClosed | PencilOpen => G::Pencil,
        OpenStraight | OpenCurve | Running | Feed => G::OpenLine,
        ZoomIn => G::ZoomIn,
        ZoomOut => G::ZoomOut,
        OtherZoom => G::Zoom,
        Pan => G::Hand,
        Measure | Ruler => G::Ruler,
        ImportPatterns | ImportSvg => G::Import,
        Arrange => G::Align,
        RotateFlip | ImageRotateRight | ImageRotateLeft => G::Rotate,
        Size => G::Size,
        Group => G::Group,
        Ungroup => G::Ungroup,
        StampEmboss => G::Stamp,
        ModifyOverlap => G::Overlap,
        DecorativeFrame => G::Frame,
        FitTextToOutline => G::TextPath,
        BackgroundFill => G::Wizard(GREEN),
        Applique => G::Wizard(RED),
        Patch => G::Wizard(GOLD),
        Cutwork => G::Wizard(PURPLE),
        SendEmbroidery => G::Needle,
        OpenImage | PasteImage | SaveImage | ModifyImage | ImageDensity => G::Image,
        AutoPunch | DesignCenter => G::Wand,
        CrossStitch => G::Cross,
        PhotoStitch1 | PhotoStitch2 => G::Photo,
        SolidView | StitchView | RealisticView | RealisticSettings | ViewThreadTrimming => G::Eye,
        PaneSewingOrder | PaneSimulator | PaneAttributes | PaneImport | ReferenceWindow => G::Pane,
        ShowGrid | GridAxes | GridSpacing => G::Grid,
        SnapToGrid => G::Magnet,
        Guideline => G::Guide,
        Font | TextSize | FontFilter | FontFilterSettings => G::Font,
        LineColor => G::Swatch(false),
        RegionColor | ChangeColor => G::Swatch(true),
        LineSewType => G::SewType(false),
        RegionSewType => G::SewType(true),
        ConvertToBlocks => G::Blocks,
        ConvertToStitches => G::Needle,
        ConvertToOutline | ConvertToOutlines | ConvertRegionToLine => G::Outline,
        SplitOutline | SplitStitchData | SplitAtPoint | DivideByColor | Connect => G::Split,
        AddDirectionLine | DeleteDirectionLine | ReverseDirectionLine => G::Direction,
        DeletePoint | SewingPoints => G::Point,
        OffsetLines => G::Offset,
        FloralPattern => G::Flower,
        EntryExit => G::EntryExit,
        MoveSections | ExportSections => G::Hoop,
    }
}

/// The name and source of the generated SVG of `g`.
fn svg(g: Glyph) -> (&'static str, &'static str) {
    use Glyph::*;
    macro_rules! svg {
        ($n:literal) => {
            (
                $n,
                include_str!(concat!("../../../assets/icons/commands/", $n, ".svg")),
            )
        };
    }
    match g {
        Page => svg!("page"),
        Folder => svg!("folder"),
        Disk => svg!("disk"),
        DiskPen => svg!("disk_pen"),
        Printer => svg!("printer"),
        Info => svg!("info"),
        Gear => svg!("gear"),
        Palette => svg!("palette"),
        Wand => svg!("wand"),
        Export => svg!("export"),
        Exit => svg!("exit"),
        Undo => svg!("undo"),
        Redo => svg!("redo"),
        Help => svg!("help"),
        Database => svg!("database"),
        Cursor => svg!("cursor"),
        PointEdit => svg!("point_edit"),
        SelectAll => svg!("select_all"),
        Scissors => svg!("scissors"),
        Copy => svg!("copy"),
        Clipboard => svg!("clipboard"),
        Duplicate => svg!("duplicate"),
        Trash => svg!("trash"),
        ArrangeCopy => svg!("arrange_copy"),
        Dropper => svg!("dropper"),
        Text => svg!("text"),
        Font => svg!("font"),
        TextPath => svg!("text_path"),
        Shapes => svg!("shapes"),
        Rect => svg!("rect"),
        Ellipse => svg!("ellipse"),
        Polygon => svg!("polygon"),
        Curve => svg!("curve"),
        Pencil => svg!("pencil"),
        OpenLine => svg!("open_line"),
        ZoomIn => svg!("zoom_in"),
        ZoomOut => svg!("zoom_out"),
        Zoom => svg!("zoom"),
        Hand => svg!("hand"),
        Ruler => svg!("ruler"),
        Import => svg!("import"),
        Align => svg!("align"),
        Rotate => svg!("rotate"),
        Size => svg!("size"),
        Group => svg!("group"),
        Ungroup => svg!("ungroup"),
        Stamp => svg!("stamp"),
        Overlap => svg!("overlap"),
        Frame => svg!("frame"),
        Wizard(c) if c == GREEN => svg!("wizard_background"),
        Wizard(c) if c == RED => svg!("wizard_applique"),
        Wizard(c) if c == GOLD => svg!("wizard_patch"),
        Wizard(_) => svg!("wizard_cutwork"),
        Needle => svg!("needle"),
        Image => svg!("image"),
        Photo => svg!("photo"),
        Cross => svg!("cross"),
        Eye => svg!("eye"),
        Pane => svg!("pane"),
        Grid => svg!("grid"),
        Magnet => svg!("magnet"),
        Guide => svg!("guide"),
        Swatch(false) => svg!("swatch_line"),
        Swatch(true) => svg!("swatch_region"),
        SewType(false) => svg!("sew_line"),
        SewType(true) => svg!("sew_region"),
        Blocks => svg!("blocks"),
        Outline => svg!("outline"),
        Split => svg!("split"),
        Direction => svg!("direction"),
        Point => svg!("point"),
        Offset => svg!("offset"),
        Flower => svg!("flower"),
        EntryExit => svg!("entry_exit"),
        Hoop => svg!("hoop"),
    }
}

/// The outline colour of the SVGs, replaced by the theme's text colour.
const OUTLINE: &str = "#2b2f36";

/// Rendered icons by (name, size in pixels, outline colour); `None` when the SVG failed.
#[derive(Clone, Default)]
struct Textures(HashMap<(&'static str, u32, [u8; 4]), Option<egui::TextureHandle>>);

fn texture(ctx: &egui::Context, g: Glyph, px: u32, fg: Color32) -> Option<egui::TextureHandle> {
    let (name, src) = svg(g);
    let key = (name, px, fg.to_array());
    let id = egui::Id::unique("openstitch-icons");
    if let Some(t) = ctx.data(|d| {
        d.get_temp::<Textures>(id)
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return t;
    }
    let t = rasterize(src, px, fg).map(|img| {
        ctx.load_texture(
            format!("icon-{name}-{px}"),
            img,
            egui::TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Textures>(id)
            .0
            .insert(key, t.clone())
    });
    t
}

/// Renders `src` into a `px`-square image with the outline drawn in `fg`.
pub(crate) fn rasterize(src: &str, px: u32, fg: Color32) -> Option<egui::ColorImage> {
    use resvg::{tiny_skia, usvg};
    let [r, g, b, _] = fg.to_array();
    let src = src.replace(OUTLINE, &format!("#{r:02x}{g:02x}{b:02x}"));
    let tree = usvg::Tree::from_str(&src, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let k = px as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(k, k),
        &mut pixmap.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize; 2],
        pixmap.data(),
    ))
}

/// Draws the icon of `a` filling the square `r`.
pub fn paint(painter: &egui::Painter, r: Rect, a: Action, fg: Color32, enabled: bool) {
    let px = (r.width() * painter.ctx().pixels_per_point()).round() as u32;
    if let Some(t) = texture(painter.ctx(), glyph(a), px, fg) {
        let tint = if enabled {
            Color32::WHITE
        } else {
            Color32::WHITE.gamma_multiply(0.35)
        };
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(t.id(), r, uv, tint);
        return;
    }
    paint_shapes(painter, r, a, fg, enabled);
}

/// Draws the code-drawn fallback icon of `a` filling the square `r`.
fn paint_shapes(painter: &egui::Painter, r: Rect, a: Action, fg: Color32, enabled: bool) {
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.35) };
    let fg = fade(fg);
    let (blue, red, green, gold) = (fade(BLUE), fade(RED), fade(GREEN), fade(GOLD));
    let s = r.width();
    let w = (s / 14.0).max(1.0);
    let line = Stroke::new(w, fg);
    let p = |x: f32, y: f32| pos2(r.min.x + x * s, r.min.y + y * s);
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(x0, y0), p(x1, y1));
    let poly = |pts: &[(f32, f32)], closed: bool, stroke: Stroke| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        if closed {
            painter.add(Shape::closed_line(v, stroke));
        } else {
            painter.add(Shape::line(v, stroke));
        }
    };
    let fillpoly = |pts: &[(f32, f32)], fill: Color32| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        painter.add(Shape::convex_polygon(v, fill, Stroke::new(w, fg)));
    };
    let boxed = |x0, y0, x1, y1, fill: Color32| {
        painter.rect(
            rr(x0, y0, x1, y1),
            CornerRadius::same((s / 16.0) as u8),
            fill,
            line,
            StrokeKind::Inside,
        );
    };
    let page = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let f = (x1 - x0) * 0.3;
        fillpoly(
            &[(x0, y0), (x1 - f, y0), (x1, y0 + f), (x1, y1), (x0, y1)],
            fade(Color32::WHITE),
        );
        poly(&[(x1 - f, y0), (x1 - f, y0 + f), (x1, y0 + f)], false, line);
    };
    let circle = |x: f32, y: f32, rad: f32, fill: Color32| {
        painter.circle(p(x, y), rad * s, fill, line);
    };
    let ring = |x: f32, y: f32, rad: f32, c: Color32| {
        painter.circle_stroke(p(x, y), rad * s, Stroke::new(w * 1.4, c));
    };
    let arrow = |from: (f32, f32), to: (f32, f32), c: Color32| {
        let st = Stroke::new(w * 1.5, c);
        poly(&[from, to], false, st);
        let d = vec2(to.0 - from.0, to.1 - from.1).normalized() * 0.16;
        let n = vec2(-d.y, d.x) * 0.7;
        poly(
            &[
                (to.0 - d.x + n.x, to.1 - d.y + n.y),
                to,
                (to.0 - d.x - n.x, to.1 - d.y - n.y),
            ],
            false,
            st,
        );
    };
    let magnifier = |sign: Option<bool>| {
        ring(0.42, 0.42, 0.26, fg);
        poly(&[(0.6, 0.6), (0.9, 0.9)], false, Stroke::new(w * 2.2, fg));
        if let Some(plus) = sign {
            let st = Stroke::new(w * 1.4, blue);
            poly(&[(0.3, 0.42), (0.54, 0.42)], false, st);
            if plus {
                poly(&[(0.42, 0.3), (0.42, 0.54)], false, st);
            }
        }
    };
    let letter = |t: &str, c: Color32, size: f32| {
        painter.text(
            p(0.5, 0.52),
            egui::Align2::CENTER_CENTER,
            t,
            egui::FontId::proportional(size * s),
            c,
        );
    };
    let stitches = |y: f32, c: Color32| {
        let st = Stroke::new(w * 1.3, c);
        for k in 0..4 {
            let x = 0.14 + k as f32 * 0.2;
            poly(&[(x, y), (x + 0.12, y)], false, st);
        }
    };
    match glyph(a) {
        Glyph::Page => {
            page(0.2, 0.1, 0.8, 0.9);
            poly(
                &[(0.62, 0.62), (0.62, 0.86)],
                false,
                Stroke::new(w * 1.6, green),
            );
            poly(
                &[(0.5, 0.74), (0.74, 0.74)],
                false,
                Stroke::new(w * 1.6, green),
            );
        }
        Glyph::Folder => {
            fillpoly(
                &[
                    (0.1, 0.25),
                    (0.4, 0.25),
                    (0.48, 0.33),
                    (0.9, 0.33),
                    (0.9, 0.8),
                    (0.1, 0.8),
                ],
                gold,
            );
        }
        Glyph::Disk | Glyph::DiskPen => {
            boxed(0.14, 0.14, 0.86, 0.86, blue);
            boxed(0.3, 0.14, 0.7, 0.4, fade(Color32::WHITE));
            boxed(0.26, 0.56, 0.74, 0.86, fade(Color32::from_gray(230)));
            if matches!(glyph(a), Glyph::DiskPen) {
                poly(
                    &[(0.58, 0.98), (0.98, 0.58)],
                    false,
                    Stroke::new(w * 2.4, red),
                );
            }
        }
        Glyph::Printer => {
            boxed(0.12, 0.38, 0.88, 0.74, fade(Color32::from_gray(200)));
            page(0.28, 0.1, 0.72, 0.4);
            page(0.28, 0.62, 0.72, 0.92);
        }
        Glyph::Info => {
            circle(0.5, 0.5, 0.38, blue);
            letter("i", fade(Color32::WHITE), 0.6);
        }
        Glyph::Gear => {
            for k in 0..8 {
                let t = k as f32 * std::f32::consts::TAU / 8.0;
                let (sn, cs) = t.sin_cos();
                poly(
                    &[
                        (0.5 + cs * 0.22, 0.5 + sn * 0.22),
                        (0.5 + cs * 0.4, 0.5 + sn * 0.4),
                    ],
                    false,
                    Stroke::new(w * 2.6, fg),
                );
            }
            circle(0.5, 0.5, 0.26, fade(Color32::from_gray(170)));
            circle(0.5, 0.5, 0.1, fade(Color32::WHITE));
        }
        Glyph::Palette => {
            circle(0.5, 0.52, 0.38, fade(Color32::from_rgb(0xf3, 0xe2, 0xc3)));
            for (x, y, c) in [
                (0.36, 0.38, red),
                (0.56, 0.32, gold),
                (0.7, 0.48, green),
                (0.4, 0.6, blue),
            ] {
                painter.circle_filled(p(x, y), 0.07 * s, c);
            }
        }
        Glyph::Wand => {
            poly(
                &[(0.14, 0.86), (0.62, 0.38)],
                false,
                Stroke::new(w * 2.4, fg),
            );
            for (x, y) in [(0.74, 0.18), (0.86, 0.36), (0.6, 0.12)] {
                painter.circle_filled(p(x, y), 0.06 * s, gold);
            }
        }
        Glyph::Export => {
            page(0.12, 0.1, 0.62, 0.9);
            arrow((0.44, 0.5), (0.94, 0.5), green);
        }
        Glyph::Exit => {
            boxed(0.14, 0.12, 0.56, 0.88, fade(Color32::from_gray(220)));
            arrow((0.4, 0.5), (0.92, 0.5), red);
        }
        Glyph::Undo | Glyph::Redo => {
            let undo = matches!(glyph(a), Glyph::Undo);
            let m = |x: f32| if undo { x } else { 1.0 - x };
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|k| {
                    let t = std::f32::consts::PI * (k as f32 / 12.0);
                    (m(0.5 + 0.28 * t.cos()), 0.56 - 0.28 * t.sin())
                })
                .collect();
            poly(&pts, false, Stroke::new(w * 1.8, blue));
            arrow((m(0.22), 0.5), (m(0.22), 0.74), blue);
        }
        Glyph::Help => {
            circle(0.5, 0.5, 0.38, blue);
            letter("?", fade(Color32::WHITE), 0.6);
        }
        Glyph::Database => {
            for y in [0.24, 0.46, 0.68] {
                painter.add(Shape::ellipse_filled(
                    p(0.5, y + 0.06),
                    vec2(0.34, 0.12) * s,
                    blue,
                ));
                painter.add(Shape::ellipse_stroke(p(0.5, y), vec2(0.34, 0.12) * s, line));
            }
        }
        Glyph::Cursor => {
            fillpoly(
                &[(0.3, 0.12), (0.3, 0.78), (0.46, 0.64), (0.72, 0.62)],
                fade(Color32::WHITE),
            );
            poly(
                &[(0.44, 0.62), (0.58, 0.9)],
                false,
                Stroke::new(w * 1.6, fg),
            );
        }
        Glyph::PointEdit => {
            poly(
                &[(0.12, 0.8), (0.4, 0.22), (0.86, 0.5)],
                false,
                Stroke::new(w * 1.4, blue),
            );
            for (x, y) in [(0.12, 0.8), (0.4, 0.22), (0.86, 0.5)] {
                boxed(x - 0.07, y - 0.07, x + 0.07, y + 0.07, fade(Color32::WHITE));
            }
        }
        Glyph::SelectAll => {
            let dash = Stroke::new(w, blue);
            painter.add(Shape::dashed_line(
                &[
                    p(0.12, 0.12),
                    p(0.88, 0.12),
                    p(0.88, 0.88),
                    p(0.12, 0.88),
                    p(0.12, 0.12),
                ],
                dash,
                0.1 * s,
                0.06 * s,
            ));
            boxed(0.28, 0.3, 0.52, 0.54, red);
            circle(0.64, 0.64, 0.12, gold);
        }
        Glyph::Scissors => {
            ring(0.28, 0.74, 0.12, red);
            ring(0.72, 0.74, 0.12, red);
            poly(
                &[(0.36, 0.64), (0.7, 0.12)],
                false,
                Stroke::new(w * 1.6, fg),
            );
            poly(
                &[(0.64, 0.64), (0.3, 0.12)],
                false,
                Stroke::new(w * 1.6, fg),
            );
        }
        Glyph::Copy | Glyph::Duplicate => {
            page(0.12, 0.08, 0.6, 0.66);
            page(0.4, 0.34, 0.88, 0.92);
            if matches!(glyph(a), Glyph::Duplicate) {
                poly(
                    &[(0.64, 0.5), (0.64, 0.76)],
                    false,
                    Stroke::new(w * 1.6, green),
                );
                poly(
                    &[(0.51, 0.63), (0.77, 0.63)],
                    false,
                    Stroke::new(w * 1.6, green),
                );
            }
        }
        Glyph::Clipboard => {
            boxed(0.16, 0.16, 0.84, 0.92, gold);
            boxed(0.34, 0.08, 0.66, 0.24, fade(Color32::from_gray(200)));
            page(0.3, 0.34, 0.74, 0.86);
        }
        Glyph::Trash => {
            boxed(0.24, 0.3, 0.76, 0.9, fade(Color32::from_gray(210)));
            poly(
                &[(0.14, 0.24), (0.86, 0.24)],
                false,
                Stroke::new(w * 1.8, red),
            );
            poly(
                &[(0.4, 0.24), (0.42, 0.12), (0.58, 0.12), (0.6, 0.24)],
                false,
                line,
            );
            for x in [0.4, 0.5, 0.6] {
                poly(&[(x, 0.4), (x, 0.8)], false, line);
            }
        }
        Glyph::ArrangeCopy => {
            for (x, y) in [(0.1, 0.1), (0.55, 0.1), (0.1, 0.55), (0.55, 0.55)] {
                boxed(x, y, x + 0.35, y + 0.35, blue);
            }
        }
        Glyph::Dropper => {
            poly(&[(0.2, 0.8), (0.62, 0.38)], false, Stroke::new(w * 2.6, fg));
            circle(0.72, 0.28, 0.13, red);
            stitches(0.92, blue);
        }
        Glyph::Text => letter("A", blue, 0.9),
        Glyph::Font => {
            letter("F", fg, 0.62);
            poly(
                &[(0.16, 0.9), (0.84, 0.9)],
                false,
                Stroke::new(w * 1.6, blue),
            );
        }
        Glyph::TextPath => {
            let pts: Vec<(f32, f32)> = (0..=10)
                .map(|k| {
                    let t = k as f32 / 10.0;
                    (0.1 + 0.8 * t, 0.78 - 0.4 * (t * std::f32::consts::PI).sin())
                })
                .collect();
            poly(&pts, false, Stroke::new(w * 1.2, blue));
            letter("A", fg, 0.5);
        }
        Glyph::Shapes => {
            boxed(0.08, 0.36, 0.56, 0.84, blue);
            circle(0.64, 0.38, 0.26, gold);
        }
        Glyph::Rect => boxed(0.14, 0.24, 0.86, 0.76, blue),
        Glyph::Ellipse => {
            painter.add(Shape::ellipse_filled(
                p(0.5, 0.5),
                vec2(0.38, 0.28) * s,
                gold,
            ));
            painter.add(Shape::ellipse_stroke(
                p(0.5, 0.5),
                vec2(0.38, 0.28) * s,
                line,
            ));
        }
        Glyph::Polygon => fillpoly(
            &[
                (0.5, 0.1),
                (0.9, 0.42),
                (0.74, 0.88),
                (0.26, 0.88),
                (0.1, 0.42),
            ],
            green,
        ),
        Glyph::Curve => {
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|k| {
                    let t = k as f32 / 24.0 * std::f32::consts::TAU;
                    (
                        0.5 + 0.36 * t.cos(),
                        0.5 + 0.26 * (t.sin() + 0.25 * (2.0 * t).sin()),
                    )
                })
                .collect();
            poly(&pts, true, Stroke::new(w * 1.6, blue));
            boxed(0.8, 0.44, 0.94, 0.58, fade(Color32::WHITE));
        }
        Glyph::Pencil => {
            fillpoly(
                &[
                    (0.16, 0.84),
                    (0.24, 0.62),
                    (0.72, 0.14),
                    (0.86, 0.28),
                    (0.38, 0.76),
                ],
                gold,
            );
        }
        Glyph::OpenLine => {
            poly(
                &[(0.1, 0.8), (0.36, 0.3), (0.62, 0.66), (0.9, 0.2)],
                false,
                Stroke::new(w * 1.6, blue),
            );
        }
        Glyph::ZoomIn => magnifier(Some(true)),
        Glyph::ZoomOut => magnifier(Some(false)),
        Glyph::Zoom => magnifier(None),
        Glyph::Hand => {
            boxed(
                0.26,
                0.46,
                0.74,
                0.9,
                fade(Color32::from_rgb(0xf1, 0xc9, 0x9e)),
            );
            for (k, x) in [0.3, 0.42, 0.54, 0.66].into_iter().enumerate() {
                let top = if k == 0 || k == 3 { 0.24 } else { 0.12 };
                boxed(
                    x - 0.04,
                    top,
                    x + 0.06,
                    0.56,
                    fade(Color32::from_rgb(0xf1, 0xc9, 0x9e)),
                );
            }
        }
        Glyph::Ruler => {
            fillpoly(
                &[(0.08, 0.66), (0.66, 0.08), (0.92, 0.34), (0.34, 0.92)],
                gold,
            );
            for k in 1..6 {
                let t = k as f32 / 6.0;
                let (x, y) = (0.08 + 0.58 * t, 0.66 - 0.58 * t);
                let l = if k % 2 == 0 { 0.14 } else { 0.08 };
                poly(&[(x, y), (x + l, y + l)], false, line);
            }
        }
        Glyph::Import => {
            page(0.38, 0.1, 0.88, 0.9);
            arrow((0.06, 0.5), (0.58, 0.5), green);
        }
        Glyph::Align => {
            poly(&[(0.5, 0.08), (0.5, 0.92)], false, Stroke::new(w, red));
            boxed(0.22, 0.18, 0.78, 0.4, blue);
            boxed(0.32, 0.56, 0.68, 0.8, gold);
        }
        Glyph::Rotate => {
            let pts: Vec<(f32, f32)> = (0..=16)
                .map(|k| {
                    let t = std::f32::consts::PI * 1.5 * (k as f32 / 16.0);
                    (0.5 + 0.32 * t.cos(), 0.5 + 0.32 * t.sin())
                })
                .collect();
            poly(&pts, false, Stroke::new(w * 1.6, blue));
            arrow((0.36, 0.18), (0.56, 0.18), blue);
        }
        Glyph::Size => {
            boxed(0.1, 0.4, 0.6, 0.9, fade(Color32::from_gray(210)));
            arrow((0.4, 0.6), (0.88, 0.12), blue);
        }
        Glyph::Group | Glyph::Ungroup => {
            let dash = Stroke::new(w, fg);
            if matches!(glyph(a), Glyph::Group) {
                painter.add(Shape::dashed_line(
                    &[
                        p(0.08, 0.08),
                        p(0.92, 0.08),
                        p(0.92, 0.92),
                        p(0.08, 0.92),
                        p(0.08, 0.08),
                    ],
                    dash,
                    0.1 * s,
                    0.06 * s,
                ));
                boxed(0.2, 0.2, 0.54, 0.54, blue);
                circle(0.64, 0.64, 0.18, gold);
            } else {
                boxed(0.08, 0.08, 0.46, 0.46, blue);
                circle(0.7, 0.7, 0.2, gold);
            }
        }
        Glyph::Stamp => {
            circle(0.5, 0.56, 0.34, blue);
            fillpoly(
                &[
                    (0.5, 0.3),
                    (0.58, 0.5),
                    (0.78, 0.52),
                    (0.62, 0.64),
                    (0.68, 0.84),
                    (0.5, 0.72),
                    (0.32, 0.84),
                    (0.38, 0.64),
                    (0.22, 0.52),
                    (0.42, 0.5),
                ],
                fade(Color32::WHITE),
            );
        }
        Glyph::Overlap => {
            circle(0.38, 0.5, 0.28, blue.gamma_multiply(0.8));
            circle(0.62, 0.5, 0.28, gold.gamma_multiply(0.8));
        }
        Glyph::Frame => {
            boxed(0.08, 0.08, 0.92, 0.92, gold);
            boxed(0.24, 0.24, 0.76, 0.76, fade(Color32::WHITE));
        }
        Glyph::Wizard(c) => {
            boxed(0.12, 0.12, 0.88, 0.88, fade(c));
            poly(
                &[(0.28, 0.72), (0.62, 0.38)],
                false,
                Stroke::new(w * 2.2, fade(Color32::WHITE)),
            );
            painter.circle_filled(p(0.7, 0.3), 0.07 * s, fade(Color32::WHITE));
        }
        Glyph::Needle => {
            poly(&[(0.5, 0.06), (0.5, 0.7)], false, Stroke::new(w * 1.6, fg));
            ring(0.5, 0.16, 0.04, fg);
            stitches(0.86, red);
        }
        Glyph::Image | Glyph::Photo => {
            boxed(
                0.08,
                0.16,
                0.92,
                0.84,
                fade(Color32::from_rgb(0xbf, 0xe0, 0xf6)),
            );
            fillpoly(
                &[
                    (0.12, 0.8),
                    (0.4, 0.42),
                    (0.6, 0.66),
                    (0.72, 0.52),
                    (0.88, 0.8),
                ],
                green,
            );
            painter.circle_filled(p(0.72, 0.32), 0.07 * s, gold);
            if matches!(glyph(a), Glyph::Photo) {
                stitches(0.92, red);
            }
        }
        Glyph::Cross => {
            for (x, y) in [(0.2, 0.2), (0.52, 0.2), (0.2, 0.52), (0.52, 0.52)] {
                let st = Stroke::new(w * 1.4, red);
                poly(&[(x, y), (x + 0.26, y + 0.26)], false, st);
                poly(&[(x + 0.26, y), (x, y + 0.26)], false, st);
            }
        }
        Glyph::Eye => {
            let up: Vec<(f32, f32)> = (0..=12)
                .map(|k| {
                    let t = k as f32 / 12.0;
                    (
                        0.08 + 0.84 * t,
                        0.5 - 0.3 * (t * std::f32::consts::PI).sin(),
                    )
                })
                .collect();
            let mut all = up.clone();
            all.extend(up.iter().rev().map(|&(x, y)| (x, 1.0 - y)));
            poly(&all, true, Stroke::new(w * 1.4, fg));
            circle(0.5, 0.5, 0.15, blue);
        }
        Glyph::Pane => {
            boxed(0.08, 0.14, 0.92, 0.86, fade(Color32::WHITE));
            painter.rect_filled(rr(0.08, 0.14, 0.34, 0.86), CornerRadius::ZERO, blue);
            painter.rect_stroke(
                rr(0.08, 0.14, 0.92, 0.86),
                CornerRadius::ZERO,
                line,
                StrokeKind::Inside,
            );
        }
        Glyph::Grid => {
            for k in 0..4 {
                let t = 0.14 + k as f32 * 0.24;
                poly(&[(t, 0.1), (t, 0.9)], false, line);
                poly(&[(0.1, t), (0.9, t)], false, line);
            }
        }
        Glyph::Magnet => {
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|k| {
                    let t = std::f32::consts::PI * (k as f32 / 12.0);
                    (0.5 + 0.28 * t.cos(), 0.5 + 0.28 * t.sin())
                })
                .collect();
            poly(&pts, false, Stroke::new(w * 3.0, red));
            poly(
                &[(0.22, 0.5), (0.22, 0.2)],
                false,
                Stroke::new(w * 3.0, red),
            );
            poly(
                &[(0.78, 0.5), (0.78, 0.2)],
                false,
                Stroke::new(w * 3.0, red),
            );
        }
        Glyph::Guide => {
            painter.add(Shape::dashed_line(
                &[p(0.5, 0.06), p(0.5, 0.94)],
                Stroke::new(w * 1.2, blue),
                0.08 * s,
                0.05 * s,
            ));
            painter.add(Shape::dashed_line(
                &[p(0.06, 0.5), p(0.94, 0.5)],
                Stroke::new(w * 1.2, blue),
                0.08 * s,
                0.05 * s,
            ));
        }
        Glyph::Swatch(region) => {
            if region {
                boxed(0.14, 0.14, 0.86, 0.86, red);
            } else {
                boxed(0.14, 0.14, 0.86, 0.86, fade(Color32::WHITE));
                painter.rect_stroke(
                    rr(0.14, 0.14, 0.86, 0.86),
                    CornerRadius::ZERO,
                    Stroke::new(w * 2.6, blue),
                    StrokeKind::Inside,
                );
            }
        }
        Glyph::SewType(region) => {
            if region {
                for k in 0..5 {
                    let x = 0.14 + k as f32 * 0.16;
                    poly(
                        &[(x, 0.86), (x + 0.12, 0.14)],
                        false,
                        Stroke::new(w * 1.2, green),
                    );
                }
            } else {
                poly(
                    &[
                        (0.08, 0.6),
                        (0.24, 0.36),
                        (0.4, 0.6),
                        (0.56, 0.36),
                        (0.72, 0.6),
                        (0.9, 0.36),
                    ],
                    false,
                    Stroke::new(w * 1.6, blue),
                );
            }
        }
        Glyph::Blocks => {
            fillpoly(&[(0.1, 0.2), (0.5, 0.1), (0.5, 0.5), (0.1, 0.6)], blue);
            fillpoly(&[(0.5, 0.5), (0.9, 0.4), (0.9, 0.8), (0.5, 0.9)], gold);
        }
        Glyph::Outline => {
            painter.add(Shape::ellipse_stroke(
                p(0.5, 0.5),
                vec2(0.36, 0.3) * s,
                Stroke::new(w * 1.8, blue),
            ));
            stitches(0.92, fg);
        }
        Glyph::Split => {
            boxed(0.08, 0.2, 0.44, 0.8, blue);
            boxed(0.56, 0.2, 0.92, 0.8, gold);
            painter.add(Shape::dashed_line(
                &[p(0.5, 0.06), p(0.5, 0.94)],
                Stroke::new(w, red),
                0.08 * s,
                0.05 * s,
            ));
        }
        Glyph::Direction => {
            boxed(0.1, 0.1, 0.9, 0.9, fade(Color32::from_gray(225)));
            arrow((0.22, 0.78), (0.78, 0.22), red);
        }
        Glyph::Point => {
            poly(
                &[(0.1, 0.8), (0.5, 0.3), (0.9, 0.7)],
                false,
                Stroke::new(w * 1.4, blue),
            );
            boxed(0.42, 0.22, 0.58, 0.38, red);
        }
        Glyph::Offset => {
            for (k, c) in [blue, green, gold].into_iter().enumerate() {
                let d = k as f32 * 0.12;
                painter.rect_stroke(
                    rr(0.12 + d, 0.12 + d, 0.88 - d, 0.88 - d),
                    CornerRadius::same((s / 8.0) as u8),
                    Stroke::new(w * 1.2, c),
                    StrokeKind::Inside,
                );
            }
        }
        Glyph::Flower => {
            for k in 0..6 {
                let t = k as f32 * std::f32::consts::TAU / 6.0;
                circle(
                    0.5 + 0.22 * t.cos(),
                    0.5 + 0.22 * t.sin(),
                    0.14,
                    fade(Color32::from_rgb(0xf2, 0x8c, 0xb8)),
                );
            }
            circle(0.5, 0.5, 0.12, gold);
        }
        Glyph::EntryExit => {
            ring(0.5, 0.5, 0.34, blue);
            painter.circle_filled(p(0.22, 0.32), 0.08 * s, green);
            painter.circle_filled(p(0.78, 0.68), 0.08 * s, red);
        }
        Glyph::Hoop => {
            painter.rect_stroke(
                rr(0.1, 0.1, 0.9, 0.9),
                CornerRadius::same((s / 6.0) as u8),
                Stroke::new(w * 2.2, fg),
                StrokeKind::Inside,
            );
            painter.add(Shape::dashed_line(
                &[p(0.1, 0.5), p(0.9, 0.5)],
                Stroke::new(w, red),
                0.08 * s,
                0.05 * s,
            ));
        }
    }
}

/// The OpenStitch logo (generated with Higgsfield, cleaned by tools/clean_logo.py): the mark
/// alone, and the wordmark for light and dark backgrounds.
pub const MARK: &str = include_str!("../../../assets/brand/openstitch-mark.svg");
pub const WORDMARK: &str = include_str!("../../../assets/brand/openstitch-wordmark.svg");
pub const WORDMARK_DARK: &str = include_str!("../../../assets/brand/openstitch-wordmark-dark.svg");

/// Logo SVG `src` rasterised `w` pixels wide (height from its aspect), cached per `name`.
pub fn logo_texture(
    ctx: &egui::Context,
    name: &'static str,
    src: &str,
    w: u32,
) -> Option<egui::TextureHandle> {
    let key = (name, w, [0; 4]);
    let id = egui::Id::unique("openstitch-icons");
    if let Some(t) = ctx.data(|d| {
        d.get_temp::<Textures>(id)
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return t;
    }
    let t = rasterize_logo(src, w).map(|img| {
        ctx.load_texture(
            format!("logo-{name}-{w}"),
            img,
            egui::TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Textures>(id)
            .0
            .insert(key, t.clone())
    });
    t
}

fn rasterize_logo(src: &str, w: u32) -> Option<egui::ColorImage> {
    use resvg::{tiny_skia, usvg};
    // Below 48 px the thread would be thinner than a pixel: outline the needle and thread in
    // their own colour so the mark stays readable (tools/gen_icons.py does the same).
    let src = if w < 48 {
        let scale: f32 = src
            .split("scale(")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let width = 1.2 * 1024.0 / (w as f32 * scale);
        let mut out = src.to_string();
        for c in ["#e2445c", "#f6f1e7"] {
            out = out.replace(
                &format!("<path fill=\"{c}\""),
                &format!("<path fill=\"{c}\" stroke=\"{c}\" stroke-width=\"{width:.0}\" stroke-linejoin=\"round\""),
            );
        }
        out
    } else {
        src.to_string()
    };
    let tree = usvg::Tree::from_str(&src, &usvg::Options::default()).ok()?;
    let k = w as f32 / tree.size().width();
    let h = (tree.size().height() * k).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(k, k),
        &mut pixmap.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [w as usize, h as usize],
        pixmap.data(),
    ))
}

/// Draws logo SVG `src` (cached as `name`) into `r`, at the screen's pixel density.
pub fn paint_svg_logo(painter: &egui::Painter, r: Rect, name: &'static str, src: &str) -> bool {
    let px = (r.width() * painter.ctx().pixels_per_point()).round() as u32;
    let Some(t) = logo_texture(painter.ctx(), name, src, px) else {
        return false;
    };
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(t.id(), r, uv, Color32::WHITE);
    true
}

/// The OpenStitch mark for the Application button; a code-drawn needle and running stitch
/// if the SVG cannot be rendered.
pub fn paint_logo(painter: &egui::Painter, r: Rect, bg: Color32) {
    if paint_svg_logo(painter, r, "mark", MARK) {
        return;
    }
    painter.rect_filled(r, CornerRadius::same(4), bg);
    let s = r.width();
    let p = |x: f32, y: f32| pos2(r.min.x + x * s, r.min.y + y * s);
    let white = Stroke::new(s / 12.0, Color32::WHITE);
    painter.line_segment([p(0.28, 0.74), p(0.74, 0.2)], white);
    painter.circle_stroke(p(0.7, 0.25), s / 18.0, Stroke::new(s / 22.0, bg));
    for k in 0..3 {
        let x = 0.16 + k as f32 * 0.24;
        painter.line_segment(
            [p(x, 0.84), p(x + 0.14, 0.84)],
            Stroke::new(s / 14.0, Color32::from_rgb(0xff, 0xd0, 0x6a)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_rendered_svg_and_every_svg_is_used() {
        let mut used = std::collections::BTreeSet::new();
        for &a in Action::ALL {
            let (name, src) = svg(glyph(a));
            assert!(src.contains(OUTLINE), "{name}: no outline colour");
            let img = rasterize(src, 32, Color32::WHITE).unwrap_or_else(|| panic!("{name}"));
            let inked = img.pixels.iter().filter(|p| p.a() > 128).count();
            assert!(inked >= 8, "{name}: draws nothing");
            used.insert(name.to_string());
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icons/commands");
        let files: std::collections::BTreeSet<String> = std::fs::read_dir(dir)
            .expect("icons folder")
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .into_string()
                    .ok()?
                    .strip_suffix(".svg")
                    .map(str::to_string)
            })
            .collect();
        assert_eq!(used, files);
    }

    #[test]
    fn the_logo_renders_at_every_size() {
        for (name, src) in [
            ("mark", MARK),
            ("wordmark", WORDMARK),
            ("dark", WORDMARK_DARK),
        ] {
            for w in [16, 20, 64, 512] {
                let img = rasterize_logo(src, w).unwrap_or_else(|| panic!("{name} {w}"));
                let inked = img.pixels.iter().filter(|p| p.a() > 128).count();
                assert!(
                    inked * 20 >= img.pixels.len(),
                    "{name} {w}: draws too little"
                );
            }
        }
    }
}
