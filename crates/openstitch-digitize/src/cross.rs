//! Cross Stitch from an image: the image's colours are reduced to N PEC threads through the
//! colour-reduction prepass Auto Punch uses (3×3 modal colours, census capped at 100 tracked
//! colours, median cut; #127 #132), each cell of a square grid takes the thread most of its
//! modal pixels have, and each thread becomes one stitch object of crosses.
//!
//! Defaults, documented and configurable (#132), as PE-DESIGN 11.50's Cross Stitch wizard
//! shows them (round-4 capture `crossstitch-default.pes`): Cross size 2.5 mm (0.4 cells per
//! mm), Max. Number of Colors automatic from the image (0; PE showed 5 for four shapes on
//! white), full crosses, Cross Stitch Times Single (`triple` sews each
//! full diagonal three times, as the Cross stitch region type's Triple does), the
//! border-majority colour left out (PE omits the white ground the same way). The grid is
//! `round(width / cell) × round(height / cell)` cells over the placed image. Colour → cell:
//! the cell's pixels (about 8 per cell side on the working raster) vote with their modal
//! colour's thread; most votes wins, ties go to the lower thread, a cell most of whose pixels
//! are transparent stays empty. On the owner's four-shape test image this grid gives PE's cells
//! to within one per colour on three of the four colours.
//!
//! Specified from PE-DESIGN 11's public guide (Cross Stitch converts an image into cross
//! stitches on a grid with a chosen cell size and colour count) and PE-DESIGN 11.50's output:
//! each cell is six stitches on its diagonals (see `sew_run`); the order of the runs is this
//! project's own (each run of a row, top to bottom, with a jump between runs, where PE-DESIGN
//! travels along the diagonals instead).

use crate::DigitizeError;
use crate::autopunch::border_majority;
use crate::palette::{self, Palette};
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
    /// Grid cell side, mm (1–10); the grid has `1 / cell_mm` cells per mm.
    pub cell_mm: f64,
    /// Thread colours to reduce to (1–64), or 0 for the automatic count.
    pub colors: usize,
    pub kind: CrossType,
    /// Each diagonal sewn three times instead of once (3 or 1 stitches per diagonal).
    pub triple: bool,
    /// Leave out the colour covering most of the grid's border.
    pub skip_background: bool,
}

impl CrossStitch {
    /// Default grid cell, mm.
    pub const CELL_MM: f64 = 2.5;
    /// Default thread colours: 0, the automatic count PE-DESIGN fills in from the image
    /// (5 for the owner's four shapes on white).
    pub const COLORS: usize = 0;

    /// Stitches per cell for these settings: a full cross is two full diagonals (1 or 3
    /// stitches each) and four half diagonals; a half cross one "/" and a step along the bottom.
    pub fn stitches_per_cell(&self) -> usize {
        let full = if self.triple { 3 } else { 1 };
        match self.kind {
            CrossType::Full => 2 * full + 4,
            CrossType::Half => full + 1,
        }
    }
}

impl Default for CrossStitch {
    fn default() -> Self {
        CrossStitch {
            cell_mm: Self::CELL_MM,
            colors: Self::COLORS,
            kind: CrossType::Full,
            triple: false,
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
    let work = image.fit_mode(side);
    let census = palette::census(&work);
    if census.colors.is_empty() {
        return Err(DigitizeError::Empty(
            "the image is fully transparent".into(),
        ));
    }
    let colors = match p.colors {
        0 => census.auto_colors(),
        n => n.min(64),
    };
    let palette = Palette::from_census(&census, colors);
    let (ww, wh) = (work.width() as usize, work.height() as usize);
    // Thread of every working pixel from its modal colour; the transparent ones vote "empty".
    let pixel_thread: Vec<usize> = census
        .modal
        .iter()
        .map(|m| m.map_or(palette.threads.len(), |c| palette.classify(c)))
        .collect();
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
                    votes[pixel_thread[py * ww + px]] += 1;
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
    // PE-DESIGN's colour order: brightest channel sum first.
    let order = palette::brightness_order(
        &palette,
        (0..palette.threads.len()).filter(|&t| count[t] > 0),
    );
    if order.is_empty() {
        return Err(DigitizeError::Empty("every cell is background".into()));
    }
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
                sew_run(&mut stitches, frame, cell, y, start, x, p.kind, p.triple);
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

/// Cells `a..b` of grid row `row`. Full crosses are sewn as PE-DESIGN 11.50 sews them
/// (reference `crossstitch-default.pes`): every needle point is a cell corner or a cell
/// centre and every stitch lies on a diagonal, six per cell. Going right from the run's
/// top-left corner, each cell is sewn top-left → centre → bottom-left (two half diagonals),
/// then "/" in full to its top-right corner, the next cell's top-left; coming back from the
/// last cell's top-right corner, each cell is sewn top-right → centre → bottom-right, then
/// "\" in full to its top-left corner. Each diagonal is so covered twice, once whole and once
/// in halves, and nothing runs along a cell side. With `triple` every full diagonal is sewn
/// there, back and there again. Half crosses are one "/" per cell, stepping along the bottom.
#[allow(clippy::too_many_arguments)]
fn sew_run(
    out: &mut Vec<Stitch>,
    frame: &Frame,
    cell: f64,
    row: usize,
    a: usize,
    b: usize,
    kind: CrossType,
    triple: bool,
) {
    let at = |col: f64, r: f64| (frame.origin.x + col * cell, frame.origin.y + r * cell);
    let put = |out: &mut Vec<Stitch>, (x, y): (f64, f64), command| {
        out.push(Stitch::new(x, y, command));
    };
    // A full diagonal from the needle's current point `from` to `to`, once or three times.
    let diagonal = |out: &mut Vec<Stitch>, from: (f64, f64), to: (f64, f64)| {
        put(out, to, Command::Stitch);
        if triple {
            put(out, from, Command::Stitch);
            put(out, to, Command::Stitch);
        }
    };
    let (top, bottom) = (row as f64, row as f64 + 1.0);
    if kind == CrossType::Half {
        put(out, at(a as f64, bottom), Command::Jump);
        put(out, at(a as f64, bottom), Command::Stitch);
        for i in a..b {
            let i = i as f64;
            diagonal(out, at(i, bottom), at(i + 1.0, top));
            if i + 1.0 < b as f64 {
                put(out, at(i + 1.0, bottom), Command::Stitch);
            }
        }
        return;
    }
    put(out, at(a as f64, top), Command::Jump);
    put(out, at(a as f64, top), Command::Stitch);
    for i in a..b {
        let i = i as f64;
        put(out, at(i + 0.5, top + 0.5), Command::Stitch);
        put(out, at(i, bottom), Command::Stitch);
        diagonal(out, at(i, bottom), at(i + 1.0, top));
    }
    for i in (a..b).rev() {
        let i = i as f64;
        put(out, at(i + 0.5, top + 0.5), Command::Stitch);
        put(out, at(i + 1.0, bottom), Command::Stitch);
        diagonal(out, at(i + 1.0, bottom), at(i, top));
    }
}
