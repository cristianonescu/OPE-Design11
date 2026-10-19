//! Cross Stitch from an image: the image's colours are reduced to N PEC threads, each cell of
//! a square grid takes the thread most of its pixels have, and each thread becomes one stitch
//! object of crosses.
//!
//! Specified from PE-DESIGN 11's public guide (Cross Stitch converts an image into cross
//! stitches on a grid with a chosen cell size and colour count); the sewing order is this
//! project's own: each run of neighbouring cells in a row is sewn with "/" diagonals going
//! right and "\" diagonals coming back, stepping along the shared cell sides.

use crate::DigitizeError;
use crate::autopunch::border_majority;
use crate::palette::Palette;
use crate::raster::{Frame, Raster};
use crate::regions::NONE;
use openstitch_core::{Command, Stitch, Thread};
use openstitch_design::{DesignObject, StitchBlock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossType {
    /// Full cross: both diagonals.
    Full,
    /// Half cross: "/" only.
    Half,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrossStitch {
    /// Grid cell side, mm.
    pub cell_mm: f64,
    pub colors: usize,
    pub kind: CrossType,
    pub skip_background: bool,
}

impl Default for CrossStitch {
    fn default() -> Self {
        CrossStitch {
            cell_mm: 2.5,
            colors: 8,
            kind: CrossType::Full,
            skip_background: true,
        }
    }
}

/// Most grid cells one conversion may sample.
pub const MAX_CELLS: usize = 160_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    /// One stitch object per thread, in sewing order (ids unassigned).
    pub objects: Vec<DesignObject>,
    pub threads: Vec<Thread>,
    /// Crosses per object.
    pub cells: Vec<usize>,
    pub columns: usize,
    pub rows: usize,
    pub background: Option<Thread>,
}

/// `frame` places the source image; its width in mm fixes the grid.
pub fn convert(image: &Raster, frame: &Frame, p: &CrossStitch) -> Result<Converted, DigitizeError> {
    let cell = p.cell_mm.clamp(1.0, 10.0) * 10.0;
    let width = f64::from(image.width()) * frame.scale;
    let height = f64::from(image.height()) * frame.scale;
    let cols = (width / cell).round().max(1.0);
    let rows = (height / cell).round().max(1.0);
    if cols * rows > MAX_CELLS as f64 {
        return Err(DigitizeError::TooLarge(format!(
            "{cols}×{rows} cells (limit {MAX_CELLS}); use larger cells or a smaller width"
        )));
    }
    let (cols, rows) = (cols as usize, rows as usize);
    // Classify a working copy (about 8 pixels per cell side) against the reduced palette;
    // each cell takes the thread most of its pixels have, and is empty when most of its
    // pixels are transparent.
    let side = (cols.max(rows) * 8).clamp(64, 1024) as u32;
    let work = image.fit(side);
    let mut colors = Vec::new();
    for y in 0..work.height() {
        for x in 0..work.width() {
            let [r, g, b, a] = work.pixel(x, y);
            if a >= 128 {
                colors.push([r, g, b]);
            }
        }
    }
    if colors.is_empty() {
        return Err(DigitizeError::Empty(
            "the image is fully transparent".into(),
        ));
    }
    let palette = Palette::build(&colors, p.colors.clamp(1, 64));
    let (ww, wh) = (work.width() as usize, work.height() as usize);
    let mut labels: Vec<u16> = Vec::with_capacity(cols * rows);
    let mut votes = vec![0usize; palette.threads.len() + 1];
    for y in 0..rows {
        let (y0, y1) = (
            y * wh / rows,
            ((y + 1) * wh / rows).max(y * wh / rows + 1).min(wh),
        );
        for x in 0..cols {
            let (x0, x1) = (
                x * ww / cols,
                ((x + 1) * ww / cols).max(x * ww / cols + 1).min(ww),
            );
            votes.iter_mut().for_each(|v| *v = 0);
            for py in y0..y1 {
                for px in x0..x1 {
                    let [r, g, b, a] = work.pixel(px as u32, py as u32);
                    let k = if a >= 128 {
                        palette.classify([r, g, b])
                    } else {
                        palette.threads.len()
                    };
                    votes[k] += 1;
                }
            }
            // Most votes; ties to the lower thread, transparency last.
            let best = (0..votes.len())
                .max_by_key(|&k| (votes[k], std::cmp::Reverse(k)))
                .unwrap_or(0);
            labels.push(if best == palette.threads.len() {
                NONE
            } else {
                best as u16
            });
        }
    }
    let background = if p.skip_background {
        border_majority(&labels, cols, rows)
    } else {
        None
    };
    let mut count = vec![0usize; palette.threads.len()];
    for &l in &labels {
        if l != NONE && Some(l) != background {
            count[l as usize] += 1;
        }
    }
    let mut order: Vec<usize> = (0..palette.threads.len())
        .filter(|&t| count[t] > 0)
        .collect();
    if order.is_empty() {
        return Err(DigitizeError::Empty("every cell is background".into()));
    }
    order.sort_by_key(|&t| (std::cmp::Reverse(count[t]), t));
    let mut objects = Vec::new();
    for &t in &order {
        let mut stitches = Vec::new();
        for y in 0..rows {
            let mut x = 0;
            while x < cols {
                if labels[y * cols + x] as usize != t {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < cols && labels[y * cols + x] as usize == t {
                    x += 1;
                }
                sew_run(&mut stitches, frame, cell, y, start, x, p.kind);
            }
        }
        let thread = palette.threads[t].clone();
        let mut o = DesignObject::stitches(StitchBlock {
            stitches,
            threads: vec![thread.clone()],
        });
        o.name = Some(format!(
            "Cross Stitch {}",
            thread.name.as_deref().unwrap_or("thread")
        ));
        objects.push(o);
    }
    Ok(Converted {
        objects,
        threads: order.iter().map(|&t| palette.threads[t].clone()).collect(),
        cells: order.iter().map(|&t| count[t]).collect(),
        columns: cols,
        rows,
        background: background.map(|b| palette.threads[b as usize].clone()),
    })
}

/// Cells `a..b` of grid row `row`: jump to the run's bottom-left corner, "/" diagonals
/// left to right, then (full crosses) "\" diagonals right to left.
fn sew_run(
    out: &mut Vec<Stitch>,
    frame: &Frame,
    cell: f64,
    row: usize,
    a: usize,
    b: usize,
    kind: CrossType,
) {
    let at = |col: usize, r: usize| {
        let x = frame.origin.x + col as f64 * cell;
        let y = frame.origin.y + r as f64 * cell;
        (x, y)
    };
    let mut put = |(x, y): (f64, f64), command| out.push(Stitch::new(x, y, command));
    let (top, bottom) = (row, row + 1);
    put(at(a, bottom), Command::Jump);
    put(at(a, bottom), Command::Stitch);
    for i in a..b {
        put(at(i + 1, top), Command::Stitch);
        if i + 1 < b || kind == CrossType::Full {
            put(at(i + 1, bottom), Command::Stitch);
        }
    }
    if kind == CrossType::Full {
        for i in (a..b).rev() {
            put(at(i, top), Command::Stitch);
            put(at(i, bottom), Command::Stitch);
        }
    }
}
