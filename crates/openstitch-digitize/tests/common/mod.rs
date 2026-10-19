#![allow(dead_code)]

use openstitch_design::{Contour, DesignObject};
use openstitch_digitize::Raster;

pub const WHITE: [u8; 4] = [255, 255, 255, 255];
pub const RED: [u8; 4] = [220, 20, 30, 255];
pub const BLUE: [u8; 4] = [20, 40, 200, 255];
pub const YELLOW: [u8; 4] = [250, 200, 20, 255];
pub const BLACK: [u8; 4] = [0, 0, 0, 255];
pub const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// `w`×`h` image of `bg` with filled rectangles `(x0, y0, x1, y1, colour)` (later on top).
pub fn rects(w: u32, h: u32, bg: [u8; 4], rs: &[(u32, u32, u32, u32, [u8; 4])]) -> Raster {
    Raster::from_fn(w, h, |x, y| {
        rs.iter()
            .rev()
            .find(|r| x >= r.0 && x < r.2 && y >= r.1 && y < r.3)
            .map_or(bg, |r| r.4)
    })
    .expect("valid size")
}

pub fn png(r: &Raster) -> Vec<u8> {
    let mut data = Vec::new();
    for y in 0..r.height() {
        for x in 0..r.width() {
            data.extend_from_slice(&r.pixel(x, y));
        }
    }
    let img = image::RgbaImage::from_raw(r.width(), r.height(), data).expect("size");
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("encode");
    out.into_inner()
}

/// Every flattened contour point of every object lies within the box (0.1 mm), with `slack`.
pub fn all_inside(
    objects: &[DesignObject],
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    slack: f64,
) -> bool {
    objects
        .iter()
        .flat_map(|o| o.contours.iter())
        .all(|c: &Contour| {
            c.path().iter().all(|p| {
                p.x >= x0 - slack && p.x <= x1 + slack && p.y >= y0 - slack && p.y <= y1 + slack
            })
        })
}
