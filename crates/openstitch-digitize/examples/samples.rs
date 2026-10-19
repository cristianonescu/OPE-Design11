//! Writes the synthetic sample images in `examples/digitize/` (run from the repository root):
//! `cargo run -p openstitch-digitize --example samples`. They are procedural, so the docs
//! images can be regenerated without third-party pictures.

use openstitch_digitize::Raster;

fn save(name: &str, r: &Raster) {
    let path = std::path::Path::new("examples/digitize").join(name);
    let png = r.encode_png().expect("encode");
    std::fs::write(&path, png).expect("write");
    println!("{}", path.display());
}

fn dist(x: u32, y: u32, cx: f64, cy: f64) -> f64 {
    ((f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2)).sqrt()
}

fn main() {
    // Badge: coloured areas with a dark outline, for Auto Punch and Cross Stitch.
    let badge = Raster::from_fn(240, 200, |x, y| {
        let (fx, fy) = (f64::from(x), f64::from(y));
        let d = dist(x, y, 90.0, 100.0);
        if (68.0..74.0).contains(&d) {
            [30, 30, 30, 255]
        } else if d < 68.0 {
            if (fy - 100.0).abs() < 10.0 + (fx - 90.0) * 0.1 {
                [210, 30, 40, 255]
            } else {
                [250, 200, 30, 255]
            }
        } else if (170..220).contains(&x) && (40..52).contains(&y) {
            [30, 70, 190, 255]
        } else if (170..220).contains(&x)
            && (70..160).contains(&y)
            && fx - 170.0 < (fy - 70.0) * 0.6
        {
            [40, 150, 70, 255]
        } else {
            [255, 255, 255, 255]
        }
    })
    .expect("size");
    save("badge.png", &badge);

    // Line drawing with one tinted area, for Design Center.
    let drawing = Raster::from_fn(240, 200, |x, y| {
        let (fx, fy) = (f64::from(x), f64::from(y));
        let ring = (dist(x, y, 80.0, 100.0) - 55.0).abs() < 2.5;
        let tri = {
            // Triangle (150,40) (220,160) (130,160), drawn as 5-px strokes.
            let edge = |ax: f64, ay: f64, bx: f64, by: f64| {
                let (dx, dy) = (bx - ax, by - ay);
                let t = (((fx - ax) * dx + (fy - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                ((fx - ax - t * dx).powi(2) + (fy - ay - t * dy).powi(2)).sqrt() < 2.5
            };
            edge(150.0, 40.0, 220.0, 160.0)
                || edge(220.0, 160.0, 130.0, 160.0)
                || edge(130.0, 160.0, 150.0, 40.0)
        };
        let stem = (78..83).contains(&x) && (45..155).contains(&y);
        if ring || tri || stem {
            [20, 20, 20, 255]
        } else if dist(x, y, 80.0, 100.0) < 52.0 && x > 82 {
            [120, 190, 240, 255]
        } else {
            [255, 255, 255, 255]
        }
    })
    .expect("size");
    save("drawing.png", &drawing);

    // A lit sphere on a graded background, for Photo Stitch.
    let photo = Raster::from_fn(200, 160, |x, y| {
        let d = dist(x, y, 100.0, 82.0);
        if d < 60.0 {
            let light = 1.0 - dist(x, y, 78.0, 60.0) / 110.0;
            let l = light.clamp(0.0, 1.0);
            [
                (250.0 * l + 40.0) as u8,
                (140.0 * l + 20.0) as u8,
                (40.0 * l + 10.0) as u8,
                255,
            ]
        } else {
            let t = f64::from(y) / 160.0;
            [
                (200.0 - 120.0 * t) as u8,
                (220.0 - 100.0 * t) as u8,
                (250.0 - 60.0 * t) as u8,
                255,
            ]
        }
    })
    .expect("size");
    save("sphere.png", &photo);
}
