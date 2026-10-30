//! Strokes of a letter: the centre-line graph of a glyph's outline, cut into the columns
//! PE-DESIGN 11.50 sews TrueType text in (docs/research/stroke-lettering.md).
//!
//! PE-DESIGN sews a TrueType letter stroke by stroke, each stroke a column whose crossings stay
//! across it (owner references `stroke-satin-*`, `stroke-fill-*`, round 5). This module finds
//! the strokes: the region is sampled on a grid, thinned to a one-cell skeleton (Zhang–Suen,
//! so the skeleton keeps the region's topology), and the skeleton is read as a graph whose
//! nodes are its free ends and junctions. Short spurs that thinning leaves at corners are pruned,
//! every branch between two nodes becomes a stroke, a branch is cut again where it turns sharply
//! (a V's foot, an A's apex; an L's right angle stays one stroke, as PE-DESIGN fans it), and a
//! closed loop with no junction (an O) is one closed stroke. Free ends reach out to the outline.
//!
//! Which stroke is sewn first, which stroke owns a junction and when two strokes through a
//! junction are sewn as one are routing decisions, made where the strokes are sewn. Own algorithm;
//! lengths are 0.1 mm.

use crate::fmath::Det;
use crate::geometry::{Contour, Rect, Vec2, v2};
use crate::skeleton::segment_distance;

/// Grid cells along the longer side of the region.
const GRID_CELLS: f64 = 200.0;
/// Most grid cells (a guard on degenerate input).
const MAX_CELLS: usize = 250_000;
/// Most outline edges.
const MAX_EDGES: usize = 20_000;
/// A free branch shorter than this many times its junction's clearance is a thinning spur.
const SPUR: f64 = 1.6;
/// Two junctions joined by a branch shorter than this many times their clearance are one
/// junction (thinning splits an X's crossing in two).
const CONTRACT: f64 = 1.5;
/// A branch turning by more than this (degrees) over about its width is cut there: an acute
/// V or A apex turns by 140° and more, an L's corner by 90°.
pub const SHARP_TURN: f64 = 115.0;

/// Where a stroke ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeEnd {
    /// At the outline (the stroke's end reaches the edge of the letter).
    Free,
    /// At junction `n` of [`GlyphStrokes::junctions`].
    Junction(usize),
    /// At a sharp turn, where the next stroke starts.
    Corner,
}

/// One stroke: its centre line (start to end), the region's width across it at each centre-line
/// point (0.1 mm), and how it ends. A closed stroke's centre line is a loop (first point not
/// repeated) and its ends are [`StrokeEnd::Free`].
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub centre: Vec<Vec2>,
    pub widths: Vec<f64>,
    pub closed: bool,
    pub ends: [StrokeEnd; 2],
}

impl Stroke {
    /// Length of the centre line.
    pub fn length(&self) -> f64 {
        let open: f64 = self.centre.windows(2).map(|w| w[0].distance(w[1])).sum();
        match (self.closed, self.centre.first(), self.centre.last()) {
            (true, Some(a), Some(b)) => open + a.distance(*b),
            _ => open,
        }
    }

    /// The centre line's bounding box.
    pub fn bounds(&self) -> Option<Rect> {
        Rect::enclosing(self.centre.iter().copied())
    }
}

/// Where strokes meet: the centre of the junction and the region's clearance there (half the
/// widest stroke through it).
#[derive(Clone, Debug, PartialEq)]
pub struct Junction {
    pub at: Vec2,
    pub radius: f64,
}

/// The strokes of one region (a glyph's outline, even-odd, holes allowed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphStrokes {
    pub strokes: Vec<Stroke>,
    pub junctions: Vec<Junction>,
}

/// The strokes of the closed region `contours`; `None` for an empty or degenerate region.
pub fn strokes(contours: &[Contour]) -> Option<GlyphStrokes> {
    let edges: Vec<(Vec2, Vec2)> = contours
        .iter()
        .filter(|c| c.closed)
        .flat_map(Contour::edges)
        .filter(|(a, b)| a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite())
        .collect();
    if edges.len() < 3 || edges.len() > MAX_EDGES {
        return None;
    }
    let b = Rect::enclosing(edges.iter().flat_map(|e| [e.0, e.1]))?;
    if b.width().max(b.height()) < 2.0 {
        return None;
    }
    let grid = Grid::new(&edges, &b)?;
    let skeleton = unstair(&grid, thin(&grid));
    let graph = Graph::trace(&grid, &skeleton);
    let graph = graph.pruned(&grid);
    Some(graph.into_strokes(&grid, &edges))
}

// ------------------------------------------------------------------ grid

struct Grid {
    origin: Vec2,
    step: f64,
    nx: usize,
    ny: usize,
    /// Distance to the outline; negative outside.
    clearance: Vec<f64>,
}

impl Grid {
    fn new(edges: &[(Vec2, Vec2)], b: &Rect) -> Option<Grid> {
        let step = (b.width().max(b.height()) / GRID_CELLS).max(0.2);
        // One cell of margin on every side, so the thinning never touches the border.
        let nx = (b.width() / step).ceil() as usize + 3;
        let ny = (b.height() / step).ceil() as usize + 3;
        if nx * ny > MAX_CELLS {
            return None;
        }
        let origin = v2(b.min_x - step, b.min_y - step);
        let mut clearance = vec![-1.0; nx * ny];
        let mut crossings = Vec::new();
        for j in 0..ny {
            let y = origin.y + (j as f64 + 0.5) * step;
            crossings.clear();
            for &(a, c) in edges {
                if (a.y > y) != (c.y > y) {
                    crossings.push(a.x + (y - a.y) * (c.x - a.x) / (c.y - a.y));
                }
            }
            crossings.sort_by(f64::total_cmp);
            for pair in crossings.as_chunks::<2>().0 {
                for i in 0..nx {
                    let x = origin.x + (i as f64 + 0.5) * step;
                    if x > pair[0] && x < pair[1] {
                        let p = v2(x, y);
                        clearance[j * nx + i] = edges
                            .iter()
                            .map(|&(a, c)| segment_distance(p, a, c))
                            .fold(f64::INFINITY, f64::min);
                    }
                }
            }
        }
        Some(Grid {
            origin,
            step,
            nx,
            ny,
            clearance,
        })
    }

    fn centre(&self, k: usize) -> Vec2 {
        v2(
            self.origin.x + ((k % self.nx) as f64 + 0.5) * self.step,
            self.origin.y + ((k / self.nx) as f64 + 0.5) * self.step,
        )
    }

    /// The 8 neighbours of cell `k`, clockwise from the one above (P2…P9 of Zhang–Suen);
    /// `None` outside the grid.
    fn ring(&self, k: usize) -> [Option<usize>; 8] {
        let (i, j) = ((k % self.nx) as i64, (k / self.nx) as i64);
        [
            (0, -1),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ]
        .map(|(di, dj)| {
            let (a, c) = (i + di, j + dj);
            (a >= 0 && c >= 0 && a < self.nx as i64 && c < self.ny as i64)
                .then(|| c as usize * self.nx + a as usize)
        })
    }
}

/// Zhang–Suen thinning of the inside cells: a one-cell-wide skeleton with the region's topology.
fn thin(grid: &Grid) -> Vec<bool> {
    let mut on: Vec<bool> = grid.clearance.iter().map(|&c| c >= 0.0).collect();
    let mut changed = true;
    let mut remove = Vec::new();
    while changed {
        changed = false;
        for pass in 0..2 {
            remove.clear();
            for k in 0..on.len() {
                if !on[k] {
                    continue;
                }
                let p = grid.ring(k).map(|n| n.is_some_and(|n| on[n]));
                let count = p.iter().filter(|&&x| x).count();
                if !(2..=6).contains(&count) {
                    continue;
                }
                let transitions = (0..8).filter(|&m| !p[m] && p[(m + 1) % 8]).count();
                if transitions != 1 {
                    continue;
                }
                // P2·P4·P6 and P4·P6·P8 (first pass); P2·P4·P8 and P2·P6·P8 (second pass).
                let (a, b) = if pass == 0 {
                    (p[0] && p[2] && p[4], p[2] && p[4] && p[6])
                } else {
                    (p[0] && p[2] && p[6], p[0] && p[4] && p[6])
                };
                if !a && !b {
                    remove.push(k);
                }
            }
            for &k in &remove {
                on[k] = false;
            }
            changed |= !remove.is_empty();
        }
    }
    on
}

/// Thinning leaves staircases: a cell whose two neighbours touch each other diagonally. Such a
/// cell carries nothing (its neighbours stay connected without it) and would read as a branch;
/// it is removed, so an ordinary skeleton cell has exactly two neighbours.
fn unstair(grid: &Grid, mut on: Vec<bool>) -> Vec<bool> {
    let mut changed = true;
    while changed {
        changed = false;
        for k in 0..on.len() {
            if !on[k] {
                continue;
            }
            let p = grid.ring(k).map(|n| n.is_some_and(|n| on[n]));
            // Orthogonal neighbours at 0 (up), 2 (right), 4 (down), 6 (left).
            let corner = [(0, 2, 1), (2, 4, 3), (4, 6, 5), (6, 0, 7)]
                .iter()
                .any(|&(a, b, between)| p[a] && p[b] && !p[between]);
            // The cell's neighbours, other than the corner pair, must keep the line connected:
            // only remove it when it has no third neighbour on the opposite side.
            let count = p.iter().filter(|&&x| x).count();
            if corner && count == 2 {
                on[k] = false;
                changed = true;
            }
        }
    }
    on
}

// ------------------------------------------------------------------ graph

/// A skeleton branch: its cells in order and the node at each end (`None` for a loop).
#[derive(Clone)]
struct Branch {
    cells: Vec<usize>,
    ends: [Option<usize>; 2],
}

#[derive(Clone)]
struct Node {
    cells: Vec<usize>,
    /// A free end (one neighbour) rather than a junction.
    free: bool,
    /// A skeleton cell with no neighbour at all: a blob thinned to one cell (the dot of an i).
    dot: bool,
}

struct Graph {
    nodes: Vec<Node>,
    branches: Vec<Branch>,
}

impl Graph {
    fn trace(grid: &Grid, on: &[bool]) -> Graph {
        let neighbours = |k: usize| -> Vec<usize> {
            grid.ring(k)
                .into_iter()
                .flatten()
                .filter(|&n| on[n])
                .collect()
        };
        // After `unstair`, an ordinary skeleton cell has exactly two neighbours: free ends have
        // one, junction cells three or more.
        let count: Vec<usize> = (0..on.len())
            .map(|k| if on[k] { neighbours(k).len() } else { 0 })
            .collect();
        let is_node = |k: usize| on[k] && count[k] != 2;
        let mut node_of = vec![usize::MAX; on.len()];
        let mut nodes: Vec<Node> = Vec::new();
        for k in 0..on.len() {
            if !is_node(k) || node_of[k] != usize::MAX {
                continue;
            }
            let id = nodes.len();
            let free = count[k] <= 1;
            let mut cells = vec![k];
            node_of[k] = id;
            if !free {
                let mut i = 0;
                while i < cells.len() {
                    for n in neighbours(cells[i]) {
                        if is_node(n) && count[n] >= 3 && node_of[n] == usize::MAX {
                            node_of[n] = id;
                            cells.push(n);
                        }
                    }
                    i += 1;
                }
            }
            nodes.push(Node {
                dot: count[k] == 0,
                cells,
                free,
            });
        }
        let mut used = vec![false; on.len()];
        let mut branches = Vec::new();
        let mut direct = std::collections::BTreeSet::new();
        for (id, node) in nodes.iter().enumerate() {
            for &c in &node.cells {
                for start in neighbours(c) {
                    let other = node_of[start];
                    if other == id {
                        continue;
                    }
                    if other != usize::MAX {
                        // Two nodes touching: a branch of no length.
                        if direct.insert((id.min(other), id.max(other))) {
                            branches.push(Branch {
                                cells: vec![c, start],
                                ends: [Some(id), Some(other)],
                            });
                        }
                        continue;
                    }
                    if used[start] {
                        continue;
                    }
                    used[start] = true;
                    let mut cells = vec![c, start];
                    let mut end = None;
                    loop {
                        let last = cells[cells.len() - 1];
                        let prev = cells[cells.len() - 2];
                        let next: Vec<usize> = neighbours(last)
                            .into_iter()
                            .filter(|&n| n != prev)
                            .collect();
                        if let Some(&n) = next.iter().find(|&&n| node_of[n] != usize::MAX) {
                            cells.push(n);
                            end = Some(node_of[n]);
                            break;
                        }
                        match next.into_iter().find(|&n| !used[n]) {
                            Some(n) => {
                                used[n] = true;
                                cells.push(n);
                            }
                            None => break,
                        }
                    }
                    branches.push(Branch {
                        cells,
                        ends: [Some(id), end],
                    });
                }
            }
        }
        // Loops with no node at all (an O).
        for k in 0..on.len() {
            if !on[k] || node_of[k] != usize::MAX || used[k] {
                continue;
            }
            let mut cells = vec![k];
            used[k] = true;
            while let Some(n) = neighbours(cells[cells.len() - 1])
                .into_iter()
                .find(|&n| !used[n] && node_of[n] == usize::MAX)
            {
                used[n] = true;
                cells.push(n);
            }
            if cells.len() > 2 {
                branches.push(Branch {
                    cells,
                    ends: [None, None],
                });
            }
        }
        Graph { nodes, branches }
    }

    fn degree(&self, id: usize) -> usize {
        self.branches
            .iter()
            .map(|b| b.ends.iter().filter(|&&e| e == Some(id)).count())
            .sum()
    }

    fn radius(&self, grid: &Grid, id: usize) -> f64 {
        self.nodes[id]
            .cells
            .iter()
            .map(|&k| grid.clearance[k])
            .fold(0.0, f64::max)
    }

    /// Spurs pruned (free branches shorter than [`SPUR`] times their junction's clearance) and
    /// the junctions they leave with two branches merged through.
    fn pruned(mut self, grid: &Grid) -> Graph {
        loop {
            let mut changed = false;
            let mut keep = Vec::with_capacity(self.branches.len());
            for b in std::mem::take(&mut self.branches) {
                let len = b.cells.len() as f64 * grid.step;
                let spur = match b.ends {
                    [Some(a), Some(c)] => {
                        let (free, junction) = if self.nodes[a].free { (a, c) } else { (c, a) };
                        self.nodes[free].free
                            && !self.nodes[junction].free
                            && len < SPUR * self.radius(grid, junction)
                    }
                    _ => false,
                };
                if spur {
                    changed = true;
                } else {
                    keep.push(b);
                }
            }
            self.branches = keep;
            // Two junctions a stub apart are one junction.
            if let Some(i) = (0..self.branches.len()).find(|&i| match self.branches[i].ends {
                [Some(a), Some(c)] if a != c && !self.nodes[a].free && !self.nodes[c].free => {
                    let len = self.branches[i].cells.len() as f64 * grid.step;
                    len < CONTRACT * self.radius(grid, a).max(self.radius(grid, c))
                }
                _ => false,
            }) {
                let b = self.branches.remove(i);
                let [Some(a), Some(c)] = b.ends else {
                    unreachable!("matched above")
                };
                let moved = std::mem::take(&mut self.nodes[c].cells);
                self.nodes[a].cells.extend(moved);
                self.nodes[a].cells.extend(b.cells);
                for br in &mut self.branches {
                    for e in &mut br.ends {
                        if *e == Some(c) {
                            *e = Some(a);
                        }
                    }
                }
                self.nodes[c].free = true;
                changed = true;
            }
            // A junction left with two branches: join them.
            for id in 0..self.nodes.len() {
                if self.nodes[id].free || self.degree(id) != 2 {
                    continue;
                }
                let at: Vec<usize> = (0..self.branches.len())
                    .filter(|&i| self.branches[i].ends.contains(&Some(id)))
                    .collect();
                if at.len() != 2 {
                    // A loop back to the same junction.
                    if at.len() == 1 {
                        self.branches[at[0]].ends = [None, None];
                        changed = true;
                    }
                    continue;
                }
                let (mut a, mut c) = (self.branches[at[0]].clone(), self.branches[at[1]].clone());
                if a.ends[1] != Some(id) {
                    a.cells.reverse();
                    a.ends.reverse();
                }
                if c.ends[0] != Some(id) {
                    c.cells.reverse();
                    c.ends.reverse();
                }
                a.cells.extend(c.cells.into_iter().skip(1));
                a.ends[1] = c.ends[1];
                // Both ends back at one junction that has no other branch: a closed loop.
                if let [Some(x), Some(y)] = a.ends
                    && x == y
                    && self.degree_without(x, &at) == 0
                {
                    a.ends = [None, None];
                }
                let (lo, hi) = (at[0].min(at[1]), at[0].max(at[1]));
                self.branches.remove(hi);
                self.branches.remove(lo);
                self.branches.push(a);
                self.nodes[id].free = true;
                self.nodes[id].cells.clear();
                changed = true;
            }
            if !changed {
                return self;
            }
        }
    }

    fn degree_without(&self, id: usize, skip: &[usize]) -> usize {
        self.branches
            .iter()
            .enumerate()
            .filter(|(i, _)| !skip.contains(i))
            .map(|(_, b)| b.ends.iter().filter(|&&e| e == Some(id)).count())
            .sum()
    }

    fn into_strokes(self, grid: &Grid, edges: &[(Vec2, Vec2)]) -> GlyphStrokes {
        // Junctions that still join three or more branches, renumbered.
        let mut junction_of = vec![None; self.nodes.len()];
        let mut junctions = Vec::new();
        for (id, n) in self.nodes.iter().enumerate() {
            if n.free || n.cells.is_empty() || self.degree(id) < 3 {
                continue;
            }
            let at = n
                .cells
                .iter()
                .map(|&k| grid.centre(k))
                .fold(v2(0.0, 0.0), |s, p| s + p)
                * (1.0 / n.cells.len() as f64);
            junction_of[id] = Some(junctions.len());
            junctions.push(Junction {
                at,
                radius: self.radius(grid, id),
            });
        }
        let clearance = |p: Vec2| {
            edges
                .iter()
                .map(|&(a, c)| segment_distance(p, a, c))
                .fold(f64::INFINITY, f64::min)
        };
        let mut strokes = Vec::new();
        for b in &self.branches {
            let closed = b.ends == [None, None];
            let mut pts: Vec<Vec2> = b.cells.iter().map(|&k| grid.centre(k)).collect();
            // Branch ends at a junction meet at its centre.
            let end_kind = |e: Option<usize>| match e.and_then(|id| junction_of[id]) {
                Some(j) => StrokeEnd::Junction(j),
                None => StrokeEnd::Free,
            };
            let ends = if closed {
                [StrokeEnd::Free, StrokeEnd::Free]
            } else {
                [end_kind(b.ends[0]), end_kind(b.ends[1])]
            };
            if let StrokeEnd::Junction(j) = ends[0] {
                pts[0] = junctions[j].at;
            }
            if let StrokeEnd::Junction(j) = ends[1] {
                let n = pts.len() - 1;
                pts[n] = junctions[j].at;
            }
            let pts = smooth(&pts, closed);
            let tolerance = grid.step;
            let mut centre: Vec<Vec2> = simplify(&pts, tolerance, closed);
            if centre.len() < 2 {
                continue;
            }
            // Free ends reach out to the outline along the stroke.
            if !closed {
                for (end, kind) in ends.iter().enumerate() {
                    if *kind != StrokeEnd::Free {
                        continue;
                    }
                    let (p, q) = if end == 0 {
                        (centre[0], centre[1])
                    } else {
                        (centre[centre.len() - 1], centre[centre.len() - 2])
                    };
                    let d = p - q;
                    let len = d.length();
                    if len <= 1e-9 {
                        continue;
                    }
                    let dir = d * (1.0 / len);
                    if let Some(t) = ray_to(p, dir, edges) {
                        let reach = p + dir * (t - grid.step * 0.5).max(0.0);
                        if end == 0 {
                            centre[0] = reach;
                        } else {
                            let n = centre.len() - 1;
                            centre[n] = reach;
                        }
                    }
                }
            }
            for piece in cut_sharp(&centre, closed, ends) {
                let centre = densify(&piece.0, piece.1);
                let widths = centre.iter().map(|&p| 2.0 * clearance(p)).collect();
                strokes.push(Stroke {
                    centre,
                    widths,
                    closed: piece.1,
                    ends: piece.2,
                });
            }
        }
        // A blob thinned to one cell (the dot of an i): a stroke across its longer side.
        for (id, n) in self.nodes.iter().enumerate() {
            if !n.dot || n.cells.len() != 1 || self.degree(id) != 0 {
                continue;
            }
            let k = n.cells[0];
            let mut blob = vec![k];
            let mut seen = std::collections::BTreeSet::from([k]);
            let mut i = 0;
            while i < blob.len() && blob.len() < MAX_CELLS {
                for m in grid.ring(blob[i]).into_iter().flatten() {
                    if grid.clearance[m] >= 0.0 && seen.insert(m) {
                        blob.push(m);
                    }
                }
                i += 1;
            }
            let Some(r) = Rect::enclosing(blob.iter().map(|&m| grid.centre(m))) else {
                continue;
            };
            let c = grid.centre(k);
            let half = grid.step * 0.5;
            let (a, b) = if r.width() >= r.height() {
                (v2(r.min_x - half, c.y), v2(r.max_x + half, c.y))
            } else {
                (v2(c.x, r.min_y - half), v2(c.x, r.max_y + half))
            };
            let centre = vec![a, b];
            let widths = centre.iter().map(|&p| 2.0 * clearance(p)).collect();
            strokes.push(Stroke {
                centre,
                widths,
                closed: false,
                ends: [StrokeEnd::Free, StrokeEnd::Free],
            });
        }
        // Deterministic order: by the top of each stroke, then its left.
        strokes.sort_by(|a, b| {
            let (ba, bb) = (a.bounds(), b.bounds());
            let key = |r: Option<Rect>| r.map_or((0.0, 0.0), |r| (r.min_y, r.min_x));
            let (ka, kb) = (key(ba), key(bb));
            ka.0.total_cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
        });
        GlyphStrokes { strokes, junctions }
    }
}

/// Most distance between two centre-line points, 0.1 mm: widths are measured at every point.
const MAX_SEGMENT: f64 = 20.0;

/// `pts` with points added so no segment is longer than [`MAX_SEGMENT`] (vertices kept).
fn densify(pts: &[Vec2], closed: bool) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(pts.len());
    let n = pts.len();
    let segments = if closed { n } else { n.saturating_sub(1) };
    for i in 0..n {
        out.push(pts[i]);
        if i < segments {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let pieces = (a.distance(b) / MAX_SEGMENT).ceil().max(1.0) as usize;
            out.extend((1..pieces).map(|k| a.lerp(b, k as f64 / pieces as f64)));
        }
    }
    out
}

/// A moving average over five points (ends kept for an open line).
fn smooth(pts: &[Vec2], closed: bool) -> Vec<Vec2> {
    let n = pts.len();
    if n < 5 {
        return pts.to_vec();
    }
    (0..n)
        .map(|i| {
            if !closed && (i < 2 || i + 2 >= n) {
                return pts[i];
            }
            let at = |d: i64| pts[((i as i64 + d).rem_euclid(n as i64)) as usize];
            (at(-2) + at(-1) + at(0) + at(1) + at(2)) * 0.2
        })
        .collect()
}

/// Douglas–Peucker; a closed line is split at its point farthest from the first.
fn simplify(pts: &[Vec2], tolerance: f64, closed: bool) -> Vec<Vec2> {
    fn walk(pts: &[Vec2], lo: usize, hi: usize, tolerance: f64, keep: &mut Vec<usize>) {
        let (far, d) = (lo + 1..hi)
            .map(|i| (i, segment_distance(pts[i], pts[lo], pts[hi])))
            .fold((lo, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        if d > tolerance {
            walk(pts, lo, far, tolerance, keep);
            walk(pts, far, hi, tolerance, keep);
        } else {
            keep.push(hi);
        }
    }
    if pts.len() < 3 {
        return pts.to_vec();
    }
    if closed {
        let far = (1..pts.len())
            .max_by(|&a, &b| pts[a].distance(pts[0]).total_cmp(&pts[b].distance(pts[0])))
            .unwrap_or(1);
        let mut first: Vec<Vec2> = pts[..=far].to_vec();
        let mut second: Vec<Vec2> = pts[far..].to_vec();
        second.push(pts[0]);
        first = simplify(&first, tolerance, false);
        second = simplify(&second, tolerance, false);
        first.pop();
        second.pop();
        first.extend(second);
        return first;
    }
    let mut keep = vec![0];
    walk(pts, 0, pts.len() - 1, tolerance, &mut keep);
    keep.into_iter().map(|i| pts[i]).collect()
}

/// The line cut where it turns by more than [`SHARP_TURN`], measured between the directions
/// over a stretch of the line on either side of each point (so a smooth curve's many small
/// turns do not add up to a cut). Returns (centre, closed, ends) per piece.
fn cut_sharp(
    centre: &[Vec2],
    closed: bool,
    ends: [StrokeEnd; 2],
) -> Vec<(Vec<Vec2>, bool, [StrokeEnd; 2])> {
    let n = centre.len();
    if n < 3 {
        return vec![(centre.to_vec(), closed, ends)];
    }
    let total: f64 = centre.windows(2).map(|w| w[0].distance(w[1])).sum();
    let reach = (total * 0.08).max(1e-9);
    // Direction over `reach` before and after point i.
    let dir_at = |i: usize, forward: bool| -> Option<Vec2> {
        let mut acc = 0.0;
        let mut j = i;
        loop {
            let next = if forward {
                if closed { (j + 1) % n } else { j + 1 }
            } else if closed {
                (j + n - 1) % n
            } else {
                j.checked_sub(1)?
            };
            if next >= n || next == i {
                break;
            }
            acc += centre[j].distance(centre[next]);
            j = next;
            if acc >= reach {
                break;
            }
        }
        let d = if forward {
            centre[j] - centre[i]
        } else {
            centre[i] - centre[j]
        };
        (d.length() > 1e-9).then(|| d * (1.0 / d.length()))
    };
    let mut cuts = Vec::new();
    for i in 0..n {
        if !closed && (i == 0 || i == n - 1) {
            continue;
        }
        let (Some(a), Some(b)) = (dir_at(i, false), dir_at(i, true)) else {
            continue;
        };
        let turn = (a.x * b.x + a.y * b.y)
            .clamp(-1.0, 1.0)
            .dacos()
            .to_degrees();
        // A turn sharp at this point and sharper than at its neighbours (one cut per corner).
        if turn > SHARP_TURN {
            let sharper = |j: usize| -> f64 {
                match (dir_at(j, false), dir_at(j, true)) {
                    (Some(a), Some(b)) => (a.x * b.x + a.y * b.y)
                        .clamp(-1.0, 1.0)
                        .dacos()
                        .to_degrees(),
                    _ => 0.0,
                }
            };
            let prev = if i > 0 {
                sharper(i - 1)
            } else if closed {
                sharper(n - 1)
            } else {
                0.0
            };
            let next = if i + 1 < n {
                sharper(i + 1)
            } else if closed {
                sharper(0)
            } else {
                0.0
            };
            if turn >= prev && turn >= next {
                cuts.push(i);
            }
        }
    }
    if cuts.is_empty() {
        return vec![(centre.to_vec(), closed, ends)];
    }
    let mut out = Vec::new();
    if closed {
        // A closed line with cuts becomes open pieces from cut to cut.
        for (k, &c) in cuts.iter().enumerate() {
            let next = cuts[(k + 1) % cuts.len()];
            let mut piece = vec![centre[c]];
            let mut j = c;
            loop {
                j = (j + 1) % n;
                piece.push(centre[j]);
                if j == next {
                    break;
                }
            }
            out.push((piece, false, [StrokeEnd::Corner, StrokeEnd::Corner]));
        }
        return out;
    }
    let mut start = 0;
    let mut start_end = ends[0];
    for &c in &cuts {
        out.push((
            centre[start..=c].to_vec(),
            false,
            [start_end, StrokeEnd::Corner],
        ));
        start = c;
        start_end = StrokeEnd::Corner;
    }
    out.push((centre[start..].to_vec(), false, [start_end, ends[1]]));
    out
}

/// Distance from `p` along `dir` (unit) to the nearest edge, if one is hit.
fn ray_to(p: Vec2, dir: Vec2, edges: &[(Vec2, Vec2)]) -> Option<f64> {
    edges
        .iter()
        .filter_map(|&(a, b)| {
            let e = b - a;
            let den = dir.x * e.y - dir.y * e.x;
            if den.abs() < 1e-12 {
                return None;
            }
            let w = a - p;
            let t = (w.x * e.y - w.y * e.x) / den;
            let u = (w.x * dir.y - w.y * dir.x) / den;
            (t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
        })
        .min_by(f64::total_cmp)
}

// ------------------------------------------------------------------ rungs

/// Outline points are taken at most this far apart (0.1 mm) when the sides are found.
const SIDE_STEP: f64 = 2.0;

/// Where `p` projects onto the polyline `line`: (arc length along it, distance, signed side:
/// positive on the left of its direction with +y down, i.e. counter-clockwise on screen).
fn project(line: &[Vec2], closed: bool, p: Vec2) -> (f64, f64, f64) {
    let n = line.len();
    let segments = if closed { n } else { n.saturating_sub(1) };
    let mut best = (0.0, f64::INFINITY, 0.0);
    let mut at = 0.0;
    for i in 0..segments {
        let (a, b) = (line[i], line[(i + 1) % n]);
        let d = b - a;
        let len2 = d.x * d.x + d.y * d.y;
        let t = if len2 > 0.0 {
            (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a + d * t;
        let dist = p.distance(q);
        let len = len2.sqrt();
        if dist < best.1 {
            let cross = d.x * (p.y - a.y) - d.y * (p.x - a.x);
            best = (at + t * len, dist, cross.signum());
        }
        at += len;
    }
    best
}

/// A polyline's point at arc length `s` (clamped to its ends).
fn point_at(line: &[Vec2], s: f64) -> Vec2 {
    let mut rest = s.max(0.0);
    for w in line.windows(2) {
        let len = w[0].distance(w[1]);
        if rest <= len {
            return if len > 0.0 {
                w[0].lerp(w[1], rest / len)
            } else {
                w[0]
            };
        }
        rest -= len;
    }
    line.last().copied().unwrap_or(v2(0.0, 0.0))
}

fn length(line: &[Vec2]) -> f64 {
    line.windows(2).map(|w| w[0].distance(w[1])).sum()
}

impl GlyphStrokes {
    /// Each stroke's two side edges (the outline that is nearer to this stroke than to any
    /// other, on either side of its centre line), each ordered along the stroke and kept over
    /// the stretch both sides cover. `[left, right]` per stroke; empty sides where the stroke
    /// has no outline of its own.
    pub fn sides(&self, contours: &[Contour]) -> Vec<[Vec<Vec2>; 2]> {
        let mut sides: Vec<[Vec<(f64, f64, Vec2)>; 2]> = self
            .strokes
            .iter()
            .map(|_| [Vec::new(), Vec::new()])
            .collect();
        for c in contours.iter().filter(|c| c.closed) {
            let pts = c.outline_points();
            let n = pts.len();
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let pieces = (a.distance(b) / SIDE_STEP).ceil().max(1.0) as usize;
                for k in 0..pieces {
                    let p = a.lerp(b, k as f64 / pieces as f64);
                    let Some((s, (at, dist, side))) = self
                        .strokes
                        .iter()
                        .enumerate()
                        .map(|(s, st)| (s, project(&st.centre, st.closed, p)))
                        .min_by(|x, y| x.1.1.total_cmp(&y.1.1))
                    else {
                        continue;
                    };
                    let which = usize::from(side < 0.0);
                    sides[s][which].push((at, dist, p));
                }
            }
        }
        sides
            .into_iter()
            .zip(&self.strokes)
            .map(|([l, r], st)| {
                let total = st.length();
                // An end edge projects onto the centre line's end point: keep only its corner
                // (the outermost point there) on each side, not the edge between the corners.
                let tidy = |mut v: Vec<(f64, f64, Vec2)>| -> Vec<(f64, f64, Vec2)> {
                    v.sort_by(|a, b| a.0.total_cmp(&b.0));
                    if st.closed {
                        return v;
                    }
                    let at_end = |at: f64| at <= 1e-6 || at >= total - 1e-6;
                    let corner = |pick_start: bool| {
                        v.iter()
                            .filter(|q| at_end(q.0) && (q.0 <= 1e-6) == pick_start)
                            .max_by(|a, b| a.1.total_cmp(&b.1))
                            .copied()
                    };
                    let (first, last) = (corner(true), corner(false));
                    let mut out: Vec<(f64, f64, Vec2)> = first.into_iter().collect();
                    out.extend(v.iter().copied().filter(|q| !at_end(q.0)));
                    out.extend(last);
                    out
                };
                let (l, r) = (tidy(l), tidy(r));
                // The stretch both sides cover.
                let (lo, hi) = match (l.first(), l.last(), r.first(), r.last()) {
                    (Some(a), Some(b), Some(c), Some(d)) => (a.0.max(c.0), b.0.min(d.0)),
                    _ => return [Vec::new(), Vec::new()],
                };
                let keep = |v: Vec<(f64, f64, Vec2)>| -> Vec<Vec2> {
                    v.into_iter()
                        .filter(|&(at, _, _)| at >= lo - SIDE_STEP && at <= hi + SIDE_STEP)
                        .map(|(_, _, p)| p)
                        .collect()
                };
                [keep(l), keep(r)]
            })
            .collect()
    }

    /// Each stroke's rungs `spacing` apart (0.1 mm along its longer side): the two side edges
    /// joined at equal fractions of their lengths, as PE-DESIGN 11.50 joins them (its Convert
    /// to Blocks lines on A's slanted right leg are about horizontal, not across the leg).
    pub fn rungs(&self, contours: &[Contour], spacing: f64) -> Vec<Vec<[Vec2; 2]>> {
        self.sides(contours)
            .into_iter()
            .map(|[l, r]| {
                if l.len() < 2 || r.len() < 2 || spacing <= 0.0 {
                    return Vec::new();
                }
                let (ll, lr) = (length(&l), length(&r));
                let n = (ll.max(lr) / spacing).ceil().clamp(1.0, 10_000.0) as usize;
                (0..=n)
                    .map(|k| {
                        let f = k as f64 / n as f64;
                        [point_at(&l, f * ll), point_at(&r, f * lr)]
                    })
                    .collect()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{DEFAULT_FONT, TextSpec, layout};

    /// Lato Bold `ch` at 20 mm, as PE-DESIGN's round-5 captures set it.
    fn glyph(ch: &str) -> (GlyphStrokes, Rect) {
        let spec = TextSpec::new(ch, DEFAULT_FONT, 20.0);
        let l = layout(&spec, None).expect("layout");
        let contours = l.contours();
        let b = Rect::enclosing(contours.iter().flat_map(Contour::outline_points)).expect("box");
        (strokes(&contours).expect("strokes"), b)
    }

    fn count(g: &GlyphStrokes) -> (usize, usize, usize) {
        let closed = g.strokes.iter().filter(|s| s.closed).count();
        (g.strokes.len(), g.junctions.len(), closed)
    }

    fn corners(g: &GlyphStrokes) -> usize {
        g.strokes
            .iter()
            .flat_map(|s| s.ends)
            .filter(|&e| e == StrokeEnd::Corner)
            .count()
            / 2
    }

    fn median(v: &[f64]) -> f64 {
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }

    /// The stroke structure PE-DESIGN 11.50 sews these letters with (round-5 captures,
    /// docs/research/stroke-lettering.md): strokes, junctions, closed loops and sharp corners.
    #[test]
    fn letters_have_pe_designs_stroke_structure() {
        // (letter, strokes, junctions, closed, corners)
        for (ch, want, corner) in [
            ("I", (1, 0, 0), 0),
            ("L", (1, 0, 0), 0), // one column round its corner, as PE-DESIGN fans it
            ("T", (3, 1, 0), 0), // the bar cut at the stem
            ("H", (5, 2, 0), 0), // the stems cut at the bar
            ("X", (4, 1, 0), 0), // four arms at one crossing
            ("V", (2, 0, 0), 1), // cut at its foot
            ("O", (1, 0, 1), 0), // one ring
            ("A", (5, 2, 0), 1), // legs cut at the apex and at the bar
            ("S", (1, 0, 0), 0),
            ("e", (2, 1, 0), 0), // the bowl round its counter and the tail
            ("i", (2, 0, 0), 0), // the stem and the dot
        ] {
            let (g, _) = glyph(ch);
            assert_eq!(count(&g), want, "{ch}: {g:?}");
            assert_eq!(corners(&g), corner, "{ch}");
        }
    }

    #[test]
    fn the_h_is_cut_at_its_bar_with_pe_designs_widths() {
        let (g, b) = glyph("H");
        // PE-DESIGN's satin H (stroke-satin-H-20): stems 3.8 mm wide, the bar 2.8 mm thick
        // between 8.8 and 11.6 mm below the top.
        let (bar, stems): (Vec<&Stroke>, Vec<&Stroke>) = g.strokes.iter().partition(|s| {
            let r = s.bounds().expect("bounds");
            r.width() > r.height()
        });
        assert_eq!((bar.len(), stems.len()), (1, 4));
        for j in &g.junctions {
            let below_top = j.at.y - b.min_y;
            assert!((below_top - 102.0).abs() < 8.0, "{j:?}");
        }
        for s in &stems {
            let w = median(&s.widths);
            assert!((w - 38.0).abs() < 4.0, "stem width {w}");
            let x = s.centre[0].x - b.min_x;
            assert!(
                (x - 19.0).abs() < 4.0 || (x - 155.0).abs() < 4.0,
                "stem at {x}"
            );
        }
        let w = median(&bar[0].widths);
        assert!((w - 28.0).abs() < 4.0, "bar width {w}");
    }

    #[test]
    fn sharp_feet_and_apexes_are_corners_right_angles_are_not() {
        // V: the two arms meet at its foot (PE-DESIGN sews them as two columns).
        let (g, b) = glyph("V");
        let foot = g.strokes[0]
            .centre
            .iter()
            .chain(&g.strokes[1].centre)
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            b.max_y - foot < 40.0,
            "the cut is at the foot: {foot} vs {}",
            b.max_y
        );
        // A: the apex at the top.
        let (g, b) = glyph("A");
        let apex = g
            .strokes
            .iter()
            .filter(|s| s.ends.contains(&StrokeEnd::Corner))
            .flat_map(|s| s.centre.iter().map(|p| p.y))
            .fold(f64::INFINITY, f64::min);
        assert!(
            apex - b.min_y < 30.0,
            "the cut is at the apex: {apex} vs {}",
            b.min_y
        );
        // L keeps its corner (PE-DESIGN fans it in one column).
        let (g, _) = glyph("L");
        assert!(g.strokes[0].ends == [StrokeEnd::Free, StrokeEnd::Free]);
        assert!(g.strokes[0].centre.len() >= 3);
    }

    #[test]
    fn strokes_are_deterministic_and_degenerate_input_is_refused() {
        let (a, _) = glyph("A");
        let (b, _) = glyph("A");
        assert_eq!(a, b);
        assert_eq!(strokes(&[]), None);
        let tiny = Contour::rectangle(0.0, 0.0, 1.0, 1.0);
        assert_eq!(strokes(&[tiny]), None);
        let open = Contour::new(vec![v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 50.0)], false);
        assert_eq!(strokes(&[open]), None);
        let nan = Contour::new(vec![v2(0.0, 0.0), v2(f64::NAN, 0.0), v2(100.0, 50.0)], true);
        assert!(strokes(&[nan]).is_none_or(|g| g.strokes.len() <= 1));
        // A huge region is refused rather than sampled at a huge size.
        let huge = Contour::rectangle(0.0, 0.0, 1.0e9, 1.0e9);
        assert!(strokes(&[huge]).is_some_and(|g| g.strokes.len() <= 4));
    }

    /// Angle of a rung, degrees in 0..180 (y down).
    fn angle(r: &[Vec2; 2]) -> f64 {
        let d = r[1] - r[0];
        d.y.datan2(d.x).to_degrees().rem_euclid(180.0)
    }

    /// Distance between two angles on the half circle.
    fn off(a: f64, b: f64) -> f64 {
        let d = (a - b).rem_euclid(180.0);
        d.min(180.0 - d)
    }

    #[test]
    fn rungs_join_the_sides_as_pe_designs_blocks_do() {
        // PE-DESIGN's Convert to Blocks of the H (stroke-blocks-H, read with the PES 0110
        // reader): horizontal lines across the stems from the top and the foot to the bar
        // (8.8 and 11.6 mm below the top), vertical lines across the bar from x 3.8 to 13.6.
        let spec = TextSpec::new("H", DEFAULT_FONT, 20.0);
        let contours = layout(&spec, None).expect("layout").contours();
        let b = Rect::enclosing(contours.iter().flat_map(Contour::outline_points)).expect("box");
        let g = strokes(&contours).expect("strokes");
        let rungs = g.rungs(&contours, 30.0);
        assert_eq!(rungs.len(), g.strokes.len());
        for (st, rs) in g.strokes.iter().zip(&rungs) {
            assert!(rs.len() >= 3, "{st:?}");
            let r = st.bounds().expect("bounds");
            let bar = r.width() > r.height();
            for rung in rs {
                let want = if bar { 90.0 } else { 0.0 };
                assert!(off(angle(rung), want) < 3.0, "{rung:?}");
                if bar {
                    let x = (rung[0].x - b.min_x) / 10.0;
                    assert!((3.4..=14.0).contains(&x), "bar rung at x {x}");
                } else {
                    // Every stem rung spans the stem: 3.8 mm.
                    assert!((rung[0].distance(rung[1]) - 38.0).abs() < 3.0, "{rung:?}");
                }
            }
            if !bar {
                // The stem's rungs stop near the bar's edges, where PE-DESIGN's do (8.8 and
                // 11.6 mm): at the junction's clearance, 8.3 and 12.2 mm (the junction square
                // is the order's to give to a stroke, step 4).
                let ys: Vec<f64> = rs.iter().map(|r| (r[0].y - b.min_y) / 10.0).collect();
                let near_bar = ys
                    .iter()
                    .map(|&y| (y - 8.8).abs().min((y - 11.6).abs()))
                    .fold(f64::INFINITY, f64::min);
                assert!(near_bar < 0.7, "{ys:?}");
            }
        }
        // A: the crossbar's lines are vertical, as PE-DESIGN's; the slanted right leg's lines
        // run near horizontal (PE-DESIGN: 161-180 degrees), not across the 60-degree leg
        // (150 degrees would be across it).
        let spec = TextSpec::new("A", DEFAULT_FONT, 20.0);
        let contours = layout(&spec, None).expect("layout").contours();
        let g = strokes(&contours).expect("strokes");
        let rungs = g.rungs(&contours, 30.0);
        let bar = g
            .strokes
            .iter()
            .position(|s| s.ends.iter().all(|e| matches!(e, StrokeEnd::Junction(_))))
            .expect("bar");
        assert!(
            rungs[bar].iter().all(|r| off(angle(r), 90.0) < 6.0),
            "{:?}",
            rungs[bar]
        );
        let right_upper = g
            .strokes
            .iter()
            .position(|s| {
                s.ends.contains(&StrokeEnd::Corner)
                    && s.centre.iter().map(|p| p.x).sum::<f64>() / s.centre.len() as f64 > 100.0
            })
            .expect("right leg");
        for r in &rungs[right_upper] {
            assert!(off(angle(r), 170.0) < 15.0, "{r:?} {}", angle(r));
        }
    }

    #[test]
    fn rungs_of_degenerate_strokes_are_empty() {
        let g = GlyphStrokes::default();
        assert!(g.rungs(&[], 30.0).is_empty());
        let spec = TextSpec::new("I", DEFAULT_FONT, 20.0);
        let contours = layout(&spec, None).expect("layout").contours();
        let g = strokes(&contours).expect("strokes");
        assert!(g.rungs(&contours, 0.0).iter().all(Vec::is_empty));
        assert!(g.rungs(&[], 30.0).iter().all(Vec::is_empty));
    }
}
