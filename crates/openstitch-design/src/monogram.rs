//! Monograms (PE-DESIGN's Monogram text tool, Tutorial 4) and decorative frames.
//!
//! A monogram is one to three capital letters on one line. The letter styles and every frame
//! are OpenStitch's own designs, built here from geometry (no Brother font or pattern):
//!
//! - *Block* and *Script*: text objects (editable text). The middle letter is full height and
//!   the side letters smaller; Script letters overlap, like joined initials.
//! - *Diamond* and *Circle*: each letter is stretched to fill a vertical band of a diamond or
//!   circle and clipped to its outline, so together the letters form the silhouette. They
//!   are shape objects.
//!
//! Frames are built around a box (the monogram or any selection); with "maintain aspect
//! ratio" a frame keeps its natural proportions and grows to enclose the box.

use crate::boolean::{self, BoolOp};
use crate::fmath::Det;
use crate::fonts::DesignFont;
use crate::geometry::{Contour, Rect, Transform, Vec2, v2};
use crate::shapes::{self, ArcMode};
use crate::text::{self, TextAlign, TextError, TextSpec};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MonogramStyle {
    Block,
    Script,
    Diamond,
    Circle,
}

impl MonogramStyle {
    pub const ALL: [MonogramStyle; 4] = [
        MonogramStyle::Block,
        MonogramStyle::Script,
        MonogramStyle::Diamond,
        MonogramStyle::Circle,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MonogramStyle::Block => "block",
            MonogramStyle::Script => "script",
            MonogramStyle::Diamond => "diamond",
            MonogramStyle::Circle => "circle",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            MonogramStyle::Block => "Block (larger middle letter)",
            MonogramStyle::Script => "Script (overlapping initials)",
            MonogramStyle::Diamond => "Diamond (letters fill a diamond)",
            MonogramStyle::Circle => "Circle (letters fill a circle)",
        }
    }

    /// Font used when none is given.
    pub fn default_font(self) -> &'static str {
        match self {
            MonogramStyle::Script => "pacifico",
            _ => text::DEFAULT_FONT,
        }
    }

    /// Whether the letters stay text objects (else they are clipped shapes).
    pub fn is_text(self) -> bool {
        matches!(self, MonogramStyle::Block | MonogramStyle::Script)
    }
}

/// What a monogram was made from; kept on its first letter so it can be regenerated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonogramSpec {
    /// One to three capital letters.
    pub letters: String,
    pub style: MonogramStyle,
    pub font: String,
    /// Height of the middle letter (or of the silhouette), 0.1 mm.
    pub height: f64,
    /// Frame id from [`FRAMES`], if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// Space between letters and frame, 0.1 mm.
    #[serde(default)]
    pub margin: f64,
    #[serde(default = "yes")]
    pub maintain_aspect: bool,
    /// Centre of the letters on the Design Page, 0.1 mm; moves with the monogram.
    pub center: Vec2,
}

fn yes() -> bool {
    true
}

/// The letters a monogram may hold: 1–3 letters or digits, upper-cased.
pub fn normalize_letters(s: &str) -> Option<String> {
    let out: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    let n = out.chars().count();
    ((1..=3).contains(&n) && out.chars().all(char::is_alphanumeric)).then_some(out)
}

/// One letter of a laid-out monogram.
#[derive(Clone, Debug, PartialEq)]
pub enum Letter {
    /// A text object's spec (Block, Script).
    Text(Box<TextSpec>),
    /// Clipped outlines (Diamond, Circle).
    Shape { ch: char, contours: Vec<Contour> },
}

/// Relative height of the side letters (Block, Script).
const SIDE: f64 = 0.7;

/// Lays out the letters of `spec` centred on its centre (0.1 mm).
pub fn letters(spec: &MonogramSpec, fonts: &[DesignFont]) -> Result<Vec<Letter>, TextError> {
    let center = spec.center;
    let chars: Vec<char> = spec.letters.chars().collect();
    let h = spec.height;
    if spec.style.is_text() {
        let n = chars.len();
        let mid = n / 2;
        let sizes: Vec<f64> = (0..n)
            .map(|i| if n == 3 && i != mid { h * SIDE } else { h })
            .collect();
        // Widths and vertical extents at the origin, then placed side by side.
        let mut boxes = Vec::new();
        for (i, &c) in chars.iter().enumerate() {
            let mut t = TextSpec::new(c.to_string(), &spec.font, sizes[i] / 10.0);
            // Monogram letter heights are capital heights.
            t.metrics = text::TextMetrics::CapHeight;
            t.align = TextAlign::Center;
            let l = text::layout_in(&t, None, fonts)?;
            let b = Rect::enclosing(l.contours().iter().flat_map(Contour::outline_points))
                .ok_or_else(|| TextError::BadFont(format!("no glyph for {c}")))?;
            boxes.push((t, b));
        }
        let gap = match spec.style {
            MonogramStyle::Script => -0.12 * h,
            _ => 0.08 * h,
        };
        let total: f64 = boxes.iter().map(|(_, b)| b.width()).sum::<f64>() + gap * (n as f64 - 1.0);
        let mut x = center.x - total / 2.0;
        let mut out = Vec::new();
        for (mut t, b) in boxes {
            // Each letter's box centred vertically on the centre line.
            let dx = x - b.min_x;
            let dy = center.y - b.center().y;
            t.transform = Transform::translation(dx, dy);
            out.push(Letter::Text(Box::new(t)));
            x += b.width() + gap;
        }
        return Ok(out);
    }

    let w = h * if spec.style == MonogramStyle::Diamond {
        1.15
    } else {
        1.0
    };
    let silhouette = silhouette(spec.style, center, w, h);
    let n = chars.len();
    // Band edges across the silhouette: the middle letter gets the widest band.
    let bands: Vec<(f64, f64)> = match n {
        1 => vec![(-0.5, 0.5)],
        2 => vec![(-0.5, -0.025), (0.025, 0.5)],
        _ => vec![(-0.5, -0.2), (-0.17, 0.17), (0.2, 0.5)],
    };
    let mut out = Vec::new();
    for (&c, &(a, b)) in chars.iter().zip(&bands) {
        let (contours, _) = text::glyph_outline(&spec.font, fonts, c, 10.0)?
            .filter(|(c, _)| !c.is_empty())
            .ok_or_else(|| TextError::BadFont(format!("no glyph for {c}")))?;
        let gb = Rect::enclosing(contours.iter().flat_map(Contour::outline_points))
            .ok_or_else(|| TextError::BadFont(format!("no glyph for {c}")))?;
        let (x0, x1) = (center.x + a * w, center.x + b * w);
        // A side band is as tall as the silhouette at its inner edge, so the letter loses only
        // its outer side to the clip.
        let near = if a < 0.0 && b > 0.0 {
            0.0
        } else {
            a.abs().min(b.abs())
        };
        let share = match spec.style {
            MonogramStyle::Circle => (1.0 - (2.0 * near).powi(2)).max(0.0).sqrt(),
            _ => 1.0 - 2.0 * near,
        };
        let (y0, y1) = (center.y - share * h / 2.0, center.y + share * h / 2.0);
        let t = Transform::translation(-gb.min_x, -gb.min_y)
            .then(&Transform::scaling(
                (x1 - x0) / gb.width().max(1e-9),
                (y1 - y0) / gb.height().max(1e-9),
                v2(0.0, 0.0),
            ))
            .then(&Transform::translation(x0, y0));
        let stretched: Vec<Contour> = contours.iter().map(|k| k.transformed(&t)).collect();
        let clipped = boolean::contours(&boolean::combine(
            &stretched,
            std::slice::from_ref(&silhouette),
            BoolOp::Intersection,
        ));
        if !clipped.is_empty() {
            out.push(Letter::Shape {
                ch: c,
                contours: clipped,
            });
        }
    }
    Ok(out)
}

fn silhouette(style: MonogramStyle, c: Vec2, w: f64, h: f64) -> Contour {
    match style {
        MonogramStyle::Circle => shapes::arc(c, w / 2.0, h / 2.0, 0.0, 360.0, ArcMode::Arc),
        _ => Contour::new(
            vec![
                v2(c.x, c.y - h / 2.0),
                v2(c.x + w / 2.0, c.y),
                v2(c.x, c.y + h / 2.0),
                v2(c.x - w / 2.0, c.y),
            ],
            true,
        ),
    }
}

/// A decorative frame (OpenStitch's own artwork).
pub struct FrameInfo {
    pub id: &'static str,
    pub name: &'static str,
    /// Natural width / height, kept with "maintain aspect ratio"; `None` follows the box.
    pub aspect: Option<f64>,
}

pub static FRAMES: [FrameInfo; 8] = [
    FrameInfo {
        id: "ring",
        name: "Ring",
        aspect: Some(1.0),
    },
    FrameInfo {
        id: "double-ring",
        name: "Double ring",
        aspect: Some(1.0),
    },
    FrameInfo {
        id: "diamond",
        name: "Diamond band",
        aspect: Some(1.15),
    },
    FrameInfo {
        id: "scallop",
        name: "Scalloped circle",
        aspect: Some(1.0),
    },
    FrameInfo {
        id: "laurel",
        name: "Laurel wreath",
        aspect: Some(1.0),
    },
    FrameInfo {
        id: "shield",
        name: "Shield",
        aspect: Some(0.85),
    },
    FrameInfo {
        id: "stars",
        name: "Ring of stars",
        aspect: Some(1.0),
    },
    FrameInfo {
        id: "corners",
        name: "Square with corner diamonds",
        aspect: None,
    },
];

pub fn frame_info(id: &str) -> Option<&'static FrameInfo> {
    FRAMES.iter().find(|f| f.id == id)
}

/// One piece of a frame: filled (a region; holes by even-odd) or a line.
#[derive(Clone, Debug, PartialEq)]
pub struct FramePart {
    pub contours: Vec<Contour>,
    pub filled: bool,
}

/// The box a frame occupies around `inner` (`margin` added on every side): the smallest box
/// whose frame's inner edge clears the corners of the margin box. With `keep_aspect` a frame
/// keeps its natural proportions.
pub fn frame_box(info: &FrameInfo, inner: Rect, margin: f64, keep_aspect: bool) -> Rect {
    let (hw, hh) = (inner.width() / 2.0 + margin, inner.height() / 2.0 + margin);
    let aspect = info.aspect.filter(|_| keep_aspect);
    // Half sizes of the frame's inner edge, then the inner edge as a share of the frame.
    let ((rx, ry), inner_share) = match info.id {
        "ring" | "double-ring" | "scallop" | "laurel" | "stars" => {
            let inner_share = match info.id {
                "ring" => 0.9,
                "double-ring" => 0.85,
                "scallop" => 0.8,
                "laurel" => 0.74,
                _ => 0.82,
            };
            let r = match aspect {
                // A circle through the corners.
                Some(_) => (hw.dhypot(hh), hw.dhypot(hh)),
                // An ellipse of the box's proportions through its corners.
                None => (hw * std::f64::consts::SQRT_2, hh * std::f64::consts::SQRT_2),
            };
            (r, inner_share)
        }
        "diamond" => {
            let r = match aspect {
                // |x|/rx + |y|/ry = 1 through (hw, hh) with rx = a·ry.
                Some(a) => (a * (hw / a + hh), hw / a + hh),
                None => (2.0 * hw, 2.0 * hh),
            };
            (r, 0.86)
        }
        "shield" => {
            // The pointed bottom needs more room below the box.
            let (mut w, mut h) = (hw / 0.88, hh / 0.62);
            if let Some(a) = aspect {
                if w / h > a {
                    h = w / a;
                } else {
                    w = h * a;
                }
            }
            ((w, h), 1.0)
        }
        _ => ((hw, hh), 0.82),
    };
    let (rx, ry) = (rx / inner_share, ry / inner_share);
    let c = inner.center();
    Rect {
        min_x: c.x - rx,
        min_y: c.y - ry,
        max_x: c.x + rx,
        max_y: c.y + ry,
    }
}

/// The frame `id` filling `b`.
pub fn frame(id: &str, b: Rect) -> Vec<FramePart> {
    let c = b.center();
    let (rx, ry) = (b.width() / 2.0, b.height() / 2.0);
    let band = 0.1 * rx.min(ry);
    let ellipse = |s: f64| shapes::arc(c, rx * s, ry * s, 0.0, 360.0, ArcMode::Arc);
    let ring = |outer: f64, inner: f64| FramePart {
        contours: vec![ellipse(outer), ellipse(inner)],
        filled: true,
    };
    let at = |a: f64, s: f64| v2(c.x + rx * s * a.dcos(), c.y + ry * s * a.dsin());
    match id {
        "ring" => vec![ring(1.0, 1.0 - band / rx.min(ry))],
        "double-ring" => vec![
            ring(1.0, 1.0 - 0.6 * band / rx.min(ry)),
            FramePart {
                contours: vec![ellipse(1.0 - 1.4 * band / rx.min(ry))],
                filled: false,
            },
        ],
        "diamond" => {
            let d = |s: f64| {
                Contour::new(
                    vec![
                        v2(c.x, c.y - ry * s),
                        v2(c.x + rx * s, c.y),
                        v2(c.x, c.y + ry * s),
                        v2(c.x - rx * s, c.y),
                    ],
                    true,
                )
            };
            vec![FramePart {
                contours: vec![d(1.0), d(1.0 - 1.4 * band / rx.min(ry))],
                filled: true,
            }]
        }
        "scallop" => {
            let n = 16;
            let mut outer = Vec::new();
            for i in 0..n * 8 {
                let a = i as f64 / (n * 8) as f64 * TAU;
                // Half-sine bumps between the valleys.
                let bump = ((a * n as f64 / 2.0).dsin()).abs();
                outer.push(at(a, 0.9 + 0.1 * bump));
            }
            vec![FramePart {
                contours: vec![Contour::new(outer, true), ellipse(0.9 - band / rx.min(ry))],
                filled: true,
            }]
        }
        "laurel" => {
            let mut parts = Vec::new();
            for side in [-1.0, 1.0] {
                // A stem from the bottom up each side, open at the top.
                let stem: Vec<Vec2> = (0..=40)
                    .map(|i| {
                        let a = FRAC_PI_2
                            - side * (0.15 + 0.7 * i as f64 / 40.0) * std::f64::consts::PI;
                        at(a, 0.88)
                    })
                    .collect();
                parts.push(FramePart {
                    contours: vec![Contour::new(stem, false)],
                    filled: false,
                });
                for k in 0..9 {
                    let t = 0.2 + 0.68 * k as f64 / 8.0;
                    let a = FRAC_PI_2 - side * t * std::f64::consts::PI;
                    for out in [-1.0, 1.0] {
                        let base = at(a, 0.88);
                        let tip = at(a - side * 0.09, 0.88 + out * 0.11);
                        parts.push(FramePart {
                            contours: vec![leaf(base, tip, 0.35)],
                            filled: true,
                        });
                    }
                }
            }
            parts
        }
        "shield" => {
            let s = |k: f64| {
                let (w, h) = (rx * k, ry * k);
                let top = c.y - h;
                let mut cont = Contour::new(
                    vec![
                        v2(c.x - w, top),
                        v2(c.x + w, top),
                        v2(c.x + w, c.y + 0.1 * h),
                        v2(c.x, c.y + h),
                        v2(c.x - w, c.y + 0.1 * h),
                    ],
                    true,
                );
                cont.set_curve(
                    2,
                    Some(crate::geometry::Bezier {
                        c1: v2(c.x + w, c.y + 0.55 * h),
                        c2: v2(c.x + 0.45 * w, c.y + 0.85 * h),
                    }),
                );
                cont.set_curve(
                    3,
                    Some(crate::geometry::Bezier {
                        c1: v2(c.x - 0.45 * w, c.y + 0.85 * h),
                        c2: v2(c.x - w, c.y + 0.55 * h),
                    }),
                );
                cont
            };
            vec![FramePart {
                contours: vec![s(1.0), s(1.0 - band / rx.min(ry))],
                filled: true,
            }]
        }
        "stars" => (0..12)
            .map(|i| {
                let a = i as f64 / 12.0 * TAU - FRAC_PI_2;
                let p = at(a, 0.92);
                let r = 0.08 * rx.min(ry);
                FramePart {
                    contours: vec![
                        shapes::star(5, 0.45).transformed(
                            &Transform::scaling(r, r, v2(0.0, 0.0))
                                .then(&Transform::translation(p.x, p.y)),
                        ),
                    ],
                    filled: true,
                }
            })
            .collect(),
        _ => {
            // "corners": a running square with a small diamond at each corner.
            let inset = 1.4 * band;
            let r = Rect {
                min_x: b.min_x + inset,
                min_y: b.min_y + inset,
                max_x: b.max_x - inset,
                max_y: b.max_y - inset,
            };
            let mut parts = vec![FramePart {
                contours: vec![Contour::rectangle(r.min_x, r.min_y, r.width(), r.height())],
                filled: false,
            }];
            for p in [
                v2(r.min_x, r.min_y),
                v2(r.max_x, r.min_y),
                v2(r.max_x, r.max_y),
                v2(r.min_x, r.max_y),
            ] {
                let d = 1.4 * band;
                parts.push(FramePart {
                    contours: vec![Contour::new(
                        vec![
                            v2(p.x, p.y - d),
                            v2(p.x + d, p.y),
                            v2(p.x, p.y + d),
                            v2(p.x - d, p.y),
                        ],
                        true,
                    )],
                    filled: true,
                });
            }
            parts
        }
    }
}

/// A pointed leaf from `base` to `tip`, `width` of its length wide at the middle.
fn leaf(base: Vec2, tip: Vec2, width: f64) -> Contour {
    let d = tip - base;
    let n = v2(-d.y, d.x) * (width / 2.0);
    let mid = base.lerp(tip, 0.5);
    let mut c = Contour::new(vec![base, tip], true);
    c.set_curve(
        0,
        Some(crate::geometry::Bezier {
            c1: mid + n - d * 0.25,
            c2: mid + n + d * 0.25,
        }),
    );
    c.set_curve(
        1,
        Some(crate::geometry::Bezier {
            c1: mid - n + d * 0.25,
            c2: mid - n - d * 0.25,
        }),
    );
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::contains;

    fn spec(letters: &str, style: MonogramStyle) -> MonogramSpec {
        MonogramSpec {
            letters: letters.into(),
            style,
            font: style.default_font().into(),
            height: 300.0,
            frame: None,
            margin: 0.0,
            maintain_aspect: true,
            center: v2(0.0, 0.0),
        }
    }

    fn at(mut s: MonogramSpec, c: Vec2) -> MonogramSpec {
        s.center = c;
        s
    }

    fn bounds(c: &[Contour]) -> Rect {
        Rect::enclosing(c.iter().flat_map(Contour::outline_points)).expect("bounds")
    }

    #[test]
    fn letters_are_normalized() {
        assert_eq!(normalize_letters("a b c").as_deref(), Some("ABC"));
        assert_eq!(normalize_letters("jd").as_deref(), Some("JD"));
        assert_eq!(normalize_letters("abcd"), None);
        assert_eq!(normalize_letters(""), None);
        assert_eq!(normalize_letters("A!"), None);
    }

    #[test]
    fn block_letters_have_a_larger_middle_letter() {
        let l = letters(
            &at(spec("ABC", MonogramStyle::Block), v2(500.0, 500.0)),
            &[],
        )
        .expect("l");
        let boxes: Vec<Rect> = l
            .iter()
            .map(|x| match x {
                Letter::Text(t) => bounds(&text::layout(t, None).expect("layout").contours()),
                Letter::Shape { .. } => panic!("block letters are text"),
            })
            .collect();
        assert_eq!(boxes.len(), 3);
        assert!((boxes[1].height() - 300.0).abs() < 2.0, "{:?}", boxes[1]);
        assert!((boxes[0].height() - 210.0).abs() < 2.0);
        assert!(boxes[0].max_x < boxes[1].min_x && boxes[1].max_x < boxes[2].min_x);
        for b in &boxes {
            assert!((b.center().y - 500.0).abs() < 1.0, "centred: {b:?}");
        }
        let all = Rect::enclosing(
            boxes
                .iter()
                .flat_map(|b| [v2(b.min_x, 0.0), v2(b.max_x, 0.0)]),
        )
        .expect("all");
        assert!((all.center().x - 500.0).abs() < 1.0);
        // Script letters overlap.
        let s = letters(&spec("ABC", MonogramStyle::Script), &[]).expect("s");
        let sb: Vec<Rect> = s
            .iter()
            .map(|x| match x {
                Letter::Text(t) => bounds(&text::layout(t, None).expect("layout").contours()),
                Letter::Shape { .. } => panic!("script letters are text"),
            })
            .collect();
        assert!(sb[0].max_x > sb[1].min_x, "overlap");
    }

    #[test]
    fn diamond_and_circle_letters_fill_their_silhouette() {
        for style in [MonogramStyle::Diamond, MonogramStyle::Circle] {
            let c = v2(500.0, 500.0);
            let l = letters(&at(spec("AMW", style), c), &[]).expect("letters");
            assert_eq!(l.len(), 3);
            let w = if style == MonogramStyle::Diamond {
                345.0
            } else {
                300.0
            };
            let sil = silhouette(style, c, w, 300.0);
            let mut all = Vec::new();
            for x in &l {
                let Letter::Shape { contours, .. } = x else {
                    panic!("clipped letters are shapes")
                };
                // Every point lies inside (or on) the silhouette.
                for p in contours.iter().flat_map(Contour::outline_points) {
                    let inward = c + (p - c) * 0.995;
                    assert!(contains(inward, std::slice::from_ref(&sil)), "{p:?}");
                }
                all.extend(contours.iter().cloned());
            }
            let b = bounds(&all);
            // The letters span the silhouette (an M leaves the diamond's top tip empty).
            assert!(
                b.height() > 0.6 * 300.0 && b.width() > 0.85 * w,
                "{style:?} {b:?}"
            );
        }
        let one = letters(&spec("Q", MonogramStyle::Circle), &[]).expect("one");
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn frames_enclose_the_box_and_keep_their_aspect() {
        let inner = Rect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 400.0,
            max_y: 200.0,
        };
        for f in &FRAMES {
            let b = frame_box(f, inner, 20.0, true);
            assert!(b.min_x < inner.min_x && b.max_y > inner.max_y, "{}", f.id);
            if let Some(a) = f.aspect {
                assert!((b.width() / b.height() - a).abs() < 1e-9, "{}", f.id);
            }
            let parts = frame(f.id, b);
            assert!(!parts.is_empty(), "{}", f.id);
            let pb = bounds(
                &parts
                    .iter()
                    .flat_map(|p| p.contours.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            // Drawn within its box, and nothing of it covers the box's centre.
            assert!(
                pb.min_x >= b.min_x - 1.0 && pb.max_x <= b.max_x + 1.0,
                "{}",
                f.id
            );
            let centre = inner.center();
            for p in parts.iter().filter(|p| p.filled) {
                assert!(!contains(centre, &p.contours), "{} covers the centre", f.id);
            }
        }
        let stretched = frame_box(frame_info("ring").expect("ring"), inner, 0.0, false);
        assert!(stretched.width() > stretched.height());
    }
}
