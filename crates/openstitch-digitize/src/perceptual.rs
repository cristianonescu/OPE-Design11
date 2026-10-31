//! Perceptual colour for the "OpenStitch" image mode (docs/research/beyond/image-digitizing.md):
//! sRGB to CIELAB (D65), the CIEDE2000 colour difference, and a weighted k-means in CIELAB.
//! PE-DESIGN mode never uses this module; its colour arithmetic stays in RGB (#119, #127).

use crate::palette::Rgb;

/// CIELAB coordinates (L*, a*, b*), D65 white.
pub type Lab = [f64; 3];

fn linear(v: u8) -> f64 {
    let c = f64::from(v) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB to CIELAB (D65).
pub fn lab(c: Rgb) -> Lab {
    let (r, g, b) = (linear(c[0]), linear(c[1]), linear(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIELAB back to sRGB, clamped to the gamut.
pub fn rgb(l: Lab) -> Rgb {
    let fy = (l[0] + 16.0) / 116.0;
    let (fx, fz) = (fy + l[1] / 500.0, fy - l[2] / 200.0);
    let inv = |t: f64| {
        if t * t * t > 216.0 / 24389.0 {
            t * t * t
        } else {
            (116.0 * t - 16.0) * 27.0 / 24389.0
        }
    };
    let (x, y, z) = (inv(fx) * 0.95047, inv(fy), inv(fz) * 1.08883);
    let lin = [
        3.2406 * x - 1.5372 * y - 0.4986 * z,
        -0.9689 * x + 1.8758 * y + 0.0415 * z,
        0.0557 * x - 0.2040 * y + 1.0570 * z,
    ];
    lin.map(|c| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

/// The CIEDE2000 colour difference.
pub fn de2000(p: Lab, q: Lab) -> f64 {
    let (l1, a1, b1) = (p[0], p[1], p[2]);
    let (l2, a2, b2) = (q[0], q[1], q[2]);
    let cb7 = ((a1.hypot(b1) + a2.hypot(b2)) / 2.0).powi(7);
    let g = 0.5 * (1.0 - (cb7 / (cb7 + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let (dlp, dcp) = (l2 - l1, c2p - c1p);
    let mut dh = h2p - h1p;
    if c1p * c2p == 0.0 {
        dh = 0.0;
    } else if dh > 180.0 {
        dh -= 360.0;
    } else if dh < -180.0 {
        dh += 360.0;
    }
    let dhp = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).to_radians().sin();
    let (lbp, cbp) = ((l1 + l2) / 2.0, (c1p + c2p) / 2.0);
    let hbp = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() > 180.0 {
        if h1p + h2p < 360.0 {
            (h1p + h2p + 360.0) / 2.0
        } else {
            (h1p + h2p - 360.0) / 2.0
        }
    } else {
        (h1p + h2p) / 2.0
    };
    let t = 1.0 - 0.17 * (hbp - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hbp).to_radians().cos()
        + 0.32 * (3.0 * hbp + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hbp - 63.0).to_radians().cos();
    let sl = 1.0 + 0.015 * (lbp - 50.0).powi(2) / (20.0 + (lbp - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let cbp7 = cbp.powi(7);
    let rt = -2.0
        * (cbp7 / (cbp7 + 25f64.powi(7))).sqrt()
        * (60.0 * (-((hbp - 275.0) / 25.0).powi(2)).exp())
            .to_radians()
            .sin();
    ((dlp / sl).powi(2) + (dcp / sc).powi(2) + (dhp / sh).powi(2) + rt * (dcp / sc) * (dhp / sh))
        .sqrt()
}

/// Squared Euclidean distance in CIELAB (the k-means objective).
fn sq(p: Lab, q: Lab) -> f64 {
    (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)
}

/// Weighted k-means (Lloyd) in CIELAB, started from `seeds`: each colour joins its nearest
/// centre, each centre moves to its members' weighted mean, for at most `rounds` rounds or
/// until nothing moves. An empty centre keeps its place. Deterministic.
pub fn kmeans(colors: &[(Lab, f64)], seeds: &[Lab], rounds: usize) -> Vec<Lab> {
    let mut centres = seeds.to_vec();
    let mut owner = vec![usize::MAX; colors.len()];
    for _ in 0..rounds {
        let mut moved = false;
        for (i, (c, _)) in colors.iter().enumerate() {
            let best = nearest(&centres, *c);
            if owner[i] != best {
                owner[i] = best;
                moved = true;
            }
        }
        if !moved {
            break;
        }
        let mut sum = vec![([0.0; 3], 0.0); centres.len()];
        for (i, (c, w)) in colors.iter().enumerate() {
            let s = &mut sum[owner[i]];
            for (total, v) in s.0.iter_mut().zip(c) {
                *total += v * w;
            }
            s.1 += w;
        }
        for (centre, (s, w)) in centres.iter_mut().zip(sum) {
            if w > 0.0 {
                *centre = s.map(|v| v / w);
            }
        }
    }
    centres
}

/// Index of the centre nearest `c` in CIELAB (ties to the lower index).
pub fn nearest(centres: &[Lab], c: Lab) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (i, &m) in centres.iter().enumerate() {
        let d = sq(m, c);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}
