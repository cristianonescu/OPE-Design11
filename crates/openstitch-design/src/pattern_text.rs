//! Text pattern sources in the `@DIRECTIVE` layout PE-DESIGN's pattern families use (`.pas`
//! programmable-fill/stamp tiles, `.pmf` motifs, `.plf` decorative-fill rows), imported into
//! [`crate::patterns::Pattern`].
//!
//! Only files a user made in their own copy of the Programmable Stitch Creator are read here;
//! Brother's bundled patterns are copyrighted content and are never read, copied or shipped.
//! The grammar below is the documented layout (#131): one directive per line, `\` at the end of
//! a line continues it, CRLF or LF, ASCII. Geometry is authored in a 0…1024 cell.
//!
//! ```text
//! @VERSION 40000
//! @UNIT 11
//! @COOD 2 0 0 8000 8000
//! @SYSENV 0                 PAS: fill subtype
//! @SYSENV 1024              PMF: motif height in cell units
//! @SYSENV 75 75 0           PLF: tile width, height (0.1 mm) and repeat flag
//! @TILE 2                   number of tile lines that follow
//! @TILELINE 0 2 0 0 1024 1024
//! @TILELINE 1 3 0 1024 512 512 1024 1024
//! @TILEAREA 4 0 0 1024 0 1024 1024 0 1024   a stamp outline (kept as a closed path)
//! @LINE 3 0 512 512 0 1024 512              a motif stroke
//! ```
//!
//! Mapping into the unit box: a cell x of 0…1024 becomes 0…1 along the tile; cell y grows
//! downwards like the Design Page. A fill tile keeps the cell as the unit tile. A motif's
//! stroke runs from its first to its last point along the sewing line and is scaled so those
//! two points sit at (0, 0) and (1, 0); across the line the cell's 1024 units span the motif
//! height (y −1…1 is ±512). A decorative row keeps y in 0…1 and is chained left to right. Which
//! of these PE-DESIGN's own Programmable Stitch Creator writes, and at which physical size the
//! `@SYSENV` numbers sew, is pinned only by owner-made files; until then the physical sizes are
//! returned as read and the Design's own tile size applies.

use crate::geometry::{Vec2, v2};
use crate::patterns::{MAX_PATHS, MAX_POINTS, Pattern, PatternKind};

/// Cell size the geometry is authored in.
pub const CELL: f64 = 1024.0;
/// Largest text source read (bytes).
pub const MAX_TEXT: usize = 1 << 20;
/// Most directives one file may hold.
pub const MAX_DIRECTIVES: usize = 10_000;

/// The role a `@SYSENV` line gives a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    /// `.pas`: a fill or stamp tile; the number is the fill subtype as written.
    Fill { subtype: i64 },
    /// `.pmf`: a motif; the number is its height in cell units as written.
    Motif { height: i64 },
    /// `.plf`: a decorative-fill row; width and height in 0.1 mm and the repeat flag, as written.
    Decorative {
        width: i64,
        height: i64,
        repeat: i64,
    },
}

/// A parsed text pattern source: the pattern plus what the header said.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPattern {
    pub version: Option<i64>,
    pub unit: Option<i64>,
    pub role: TextRole,
    pub pattern: Pattern,
    /// `@TILEAREA` outlines, in cell units (closed polygons).
    pub areas: Vec<Vec<Vec2>>,
}

/// Lines of a source joined by `\` continuations, blanks dropped.
fn directives(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut pending = String::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        let (body, cont) = match line.strip_suffix('\\') {
            Some(b) => (b, true),
            None => (line, false),
        };
        pending.push_str(body);
        pending.push(' ');
        if cont {
            continue;
        }
        let words: Vec<String> = pending.split_whitespace().map(str::to_string).collect();
        pending.clear();
        if words.is_empty() {
            continue;
        }
        if out.len() >= MAX_DIRECTIVES {
            return Err(format!("more than {MAX_DIRECTIVES} directives"));
        }
        out.push(words);
    }
    if !pending.trim().is_empty() {
        return Err("the file ends inside a continued line".into());
    }
    Ok(out)
}

fn ints(words: &[String], what: &str) -> Result<Vec<i64>, String> {
    words
        .iter()
        .map(|w| {
            w.parse::<i64>()
                .map_err(|_| format!("{what}: '{w}' is not an integer"))
        })
        .collect()
}

/// `<npts> <x0> <y0> …` into cell points.
fn point_list(words: &[String], what: &str) -> Result<Vec<Vec2>, String> {
    let v = ints(words, what)?;
    let Some((&n, rest)) = v.split_first() else {
        return Err(format!("{what}: missing point count"));
    };
    if n < 2 || n as usize > MAX_POINTS {
        return Err(format!("{what}: {n} points is outside 2…{MAX_POINTS}"));
    }
    if rest.len() != 2 * n as usize {
        return Err(format!(
            "{what}: {n} points announced, {} coordinates given",
            rest.len()
        ));
    }
    let pts: Vec<Vec2> = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| v2(c[0] as f64, c[1] as f64))
        .collect();
    if pts
        .iter()
        .any(|p| !(0.0..=CELL).contains(&p.x) || !(0.0..=CELL).contains(&p.y))
    {
        return Err(format!("{what}: a coordinate is outside the 0…1024 cell"));
    }
    Ok(pts)
}

fn one_int(rest: &[String], what: &str) -> Result<i64, String> {
    let v = ints(rest, what)?;
    if v.len() != 1 {
        return Err(format!("{what} takes one number"));
    }
    Ok(v[0])
}

/// Parses one source. `id` and `name` name the pattern (the file carries none); the role comes
/// from `@SYSENV`'s arity (1 number: fill, unless the file has one `@LINE` and no tile
/// geometry, then motif; 3 numbers: decorative).
pub fn parse(text: &str, id: &str, name: &str) -> Result<TextPattern, String> {
    if text.len() > MAX_TEXT {
        return Err(format!("text pattern larger than {MAX_TEXT} bytes"));
    }
    if !text.is_ascii() {
        return Err("text pattern sources are ASCII".into());
    }
    let mut version = None;
    let mut unit = None;
    let mut sysenv: Option<Vec<i64>> = None;
    let mut tile_count: Option<i64> = None;
    let mut tile_lines: Vec<(i64, Vec<Vec2>)> = Vec::new();
    let mut areas: Vec<Vec<Vec2>> = Vec::new();
    let mut lines: Vec<Vec<Vec2>> = Vec::new();
    let mut seen_any = false;
    for d in directives(text)? {
        let (head, rest) = d.split_first().expect("non-empty");
        if !head.starts_with('@') {
            return Err(format!("'{head}' is not a @DIRECTIVE"));
        }
        seen_any = true;
        match head.as_str() {
            "@VERSION" => version = Some(one_int(rest, "@VERSION")?),
            "@UNIT" => unit = Some(one_int(rest, "@UNIT")?),
            "@COOD" => {
                ints(rest, "@COOD")?;
            }
            "@SYSENV" => {
                let v = ints(rest, "@SYSENV")?;
                if v.len() != 1 && v.len() != 3 {
                    return Err(
                        "@SYSENV takes one number (fill, motif) or three (decorative)".into(),
                    );
                }
                if sysenv.replace(v).is_some() {
                    return Err("@SYSENV given twice".into());
                }
            }
            "@TILE" => {
                let n = one_int(rest, "@TILE")?;
                if n < 0 || n as usize > MAX_PATHS {
                    return Err(format!("@TILE takes a count of 0…{MAX_PATHS}"));
                }
                if tile_count.replace(n).is_some() {
                    return Err("@TILE given twice".into());
                }
            }
            "@TILELINE" => {
                let Some((idx, pts)) = rest.split_first() else {
                    return Err("@TILELINE needs an index".into());
                };
                let idx: i64 = idx
                    .parse()
                    .map_err(|_| format!("@TILELINE index '{idx}' is not an integer"))?;
                if tile_lines.len() >= MAX_PATHS {
                    return Err(format!("more than {MAX_PATHS} tile lines"));
                }
                tile_lines.push((idx, point_list(pts, "@TILELINE")?));
            }
            "@TILEAREA" => {
                if areas.len() >= MAX_PATHS {
                    return Err(format!("more than {MAX_PATHS} tile areas"));
                }
                let pts = point_list(rest, "@TILEAREA")?;
                if pts.len() < 3 {
                    return Err("@TILEAREA needs at least 3 points".into());
                }
                areas.push(pts);
            }
            "@LINE" => {
                if lines.len() >= MAX_PATHS {
                    return Err(format!("more than {MAX_PATHS} lines"));
                }
                lines.push(point_list(rest, "@LINE")?);
            }
            other => return Err(format!("unknown directive {other}")),
        }
    }
    if !seen_any {
        return Err("no directives".into());
    }
    let Some(sysenv) = sysenv else {
        return Err("no @SYSENV line: the file's role is unknown".into());
    };
    if let Some(n) = tile_count
        && n as usize != tile_lines.len()
    {
        return Err(format!(
            "@TILE announces {n} lines, {} @TILELINE given",
            tile_lines.len()
        ));
    }
    for (i, (idx, _)) in tile_lines.iter().enumerate() {
        if *idx != i as i64 {
            return Err(format!("@TILELINE {idx} out of order (expected {i})"));
        }
    }
    let total: usize = tile_lines.iter().map(|(_, p)| p.len()).sum::<usize>()
        + areas.iter().map(Vec::len).sum::<usize>()
        + lines.iter().map(Vec::len).sum::<usize>();
    if total > MAX_POINTS || total + areas.len() > MAX_POINTS {
        return Err(format!("more than {MAX_POINTS} points"));
    }
    let unit_pt = |p: Vec2| v2(p.x / CELL, p.y / CELL);
    let (role, kind, paths): (TextRole, PatternKind, Vec<Vec<Vec2>>) = match sysenv.as_slice() {
        [w, h, r] => {
            if *w <= 0 || *h <= 0 {
                return Err("decorative @SYSENV width and height must be positive".into());
            }
            let mut paths = Vec::new();
            for p in tile_lines.iter().map(|(_, p)| p).chain(&lines) {
                let q: Vec<Vec2> = p.iter().map(|&p| unit_pt(p)).collect();
                // A decorative row chains tiles left to right: its path runs from x = 0 to
                // x = 1 at one height.
                let (a, b) = (q[0], *q.last().expect("2+ points"));
                if a.x != 0.0 || b.x != 1.0 || (a.y - b.y).abs() > 1e-9 {
                    return Err(
                        "a decorative row must run from the left edge to the right edge at one height"
                            .into(),
                    );
                }
                paths.push(q);
            }
            (
                TextRole::Decorative {
                    width: *w,
                    height: *h,
                    repeat: *r,
                },
                PatternKind::Decorative,
                paths,
            )
        }
        [n] if tile_lines.is_empty() && lines.len() == 1 && areas.is_empty() => {
            let stroke = &lines[0];
            let (a, b) = (stroke[0], *stroke.last().expect("2+ points"));
            let along = b - a;
            let len = along.length();
            if len < 1e-9 {
                return Err("a motif stroke must end away from where it starts".into());
            }
            let (ux, uy) = (along.x / len, along.y / len);
            let path: Vec<Vec2> = stroke
                .iter()
                .map(|&p| {
                    let d = p - a;
                    // Along the stroke 0…1; across in motif heights (±512 cell units = ±1).
                    v2(
                        (d.x * ux + d.y * uy) / len,
                        ((-d.x * uy + d.y * ux) / (CELL / 2.0)).clamp(-1.0, 1.0),
                    )
                })
                .collect();
            (
                TextRole::Motif { height: *n },
                PatternKind::Motif,
                vec![path],
            )
        }
        [n] => {
            let mut paths: Vec<Vec<Vec2>> = tile_lines
                .iter()
                .map(|(_, p)| p.iter().map(|&p| unit_pt(p)).collect())
                .collect();
            for a in &areas {
                let mut q: Vec<Vec2> = a.iter().map(|&p| unit_pt(p)).collect();
                if q[0].distance(*q.last().expect("3+ points")) > 1e-9 {
                    q.push(q[0]);
                }
                paths.push(q);
            }
            if paths.is_empty() {
                return Err("a fill tile needs at least one @TILELINE or @TILEAREA".into());
            }
            (TextRole::Fill { subtype: *n }, PatternKind::Fill, paths)
        }
        _ => unreachable!("arity checked"),
    };
    let kind_name = match kind {
        PatternKind::Motif => "motif",
        PatternKind::Fill => "fill",
        PatternKind::Decorative => "decorative",
    };
    let doc = serde_json::json!({
        "format": "openstitch.pattern", "version": 1,
        "kind": kind_name,
        "id": id, "name": name,
        "paths": paths.iter().map(|p| p.iter().map(|q| [round6(q.x), round6(q.y)]).collect::<Vec<_>>()).collect::<Vec<_>>(),
    });
    let pattern = crate::patterns::parse(&doc.to_string())?;
    Ok(TextPattern {
        version,
        unit,
        role,
        pattern,
        areas,
    })
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// Whether `bytes` look like a text pattern source (its first non-blank byte is `@`).
pub fn looks_like(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'@')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILL: &str = "@VERSION 40000\r\n@UNIT 11\r\n@COOD 2 0 0 8000 8000\r\n@SYSENV 0\r\n@TILE 2\r\n@TILELINE 0 2 0 0 1024 1024\r\n@TILELINE 1 3 0 1024 \\\r\n512 512 1024 0\r\n";
    const MOTIF: &str = "@VERSION 40000\n@SYSENV 1024\n@LINE 3 0 512 512 0 1024 512\n";
    const DECO: &str = "@VERSION 40000\n@SYSENV 75 75 0\n@TILELINE 0 3 0 512 512 1024 1024 512\n";
    const STAMP: &str = "@VERSION 40000\n@SYSENV 2\n@TILEAREA 4 0 0 1024 0 1024 1024 0 1024\n";

    #[test]
    fn the_three_roles_parse_into_patterns() {
        let f = parse(FILL, "my-fill", "Fill").expect("fill");
        assert_eq!(f.role, TextRole::Fill { subtype: 0 });
        assert_eq!(f.version, Some(40000));
        assert_eq!(f.unit, Some(11));
        assert_eq!(f.pattern.kind, PatternKind::Fill);
        assert_eq!(f.pattern.paths.len(), 2);
        assert_eq!(f.pattern.paths[1][1], v2(0.5, 0.5));

        let m = parse(MOTIF, "my-motif", "Motif").expect("motif");
        assert_eq!(m.role, TextRole::Motif { height: 1024 });
        assert_eq!(m.pattern.kind, PatternKind::Motif);
        let p = &m.pattern.paths[0];
        assert_eq!((p[0], p[2]), (v2(0.0, 0.0), v2(1.0, 0.0)));
        assert!(
            (p[1].x - 0.5).abs() < 1e-9 && (p[1].y + 1.0).abs() < 1e-9,
            "{p:?}"
        );

        let d = parse(DECO, "my-deco", "Deco").expect("deco");
        assert_eq!(
            d.role,
            TextRole::Decorative {
                width: 75,
                height: 75,
                repeat: 0
            }
        );
        assert_eq!(d.pattern.kind, PatternKind::Decorative);
        assert_eq!(d.pattern.paths[0][1], v2(0.5, 1.0));

        let s = parse(STAMP, "my-stamp", "Stamp").expect("stamp");
        assert_eq!(s.role, TextRole::Fill { subtype: 2 });
        assert_eq!(s.areas.len(), 1);
        assert_eq!(s.pattern.paths[0].len(), 5, "closed");
        assert!(looks_like(b"  \r\n@VERSION 1") && !looks_like(b"{\"a\":1}") && !looks_like(b""));
    }

    #[test]
    fn damaged_sources_are_refused() {
        let bad = [
            "",
            "VERSION 1\n@SYSENV 0\n@TILELINE 0 2 0 0 1 1\n",
            "@SYSENV 0\n",
            "@SYSENV 0 1\n@TILELINE 0 2 0 0 1 1\n",
            "@SYSENV 0\n@TILE 2\n@TILELINE 0 2 0 0 1 1\n",
            "@SYSENV 0\n@TILELINE 1 2 0 0 1 1\n",
            "@SYSENV 0\n@TILELINE 0 2 0 0 1 1025\n",
            "@SYSENV 0\n@TILELINE 0 3 0 0 1 1\n",
            "@SYSENV 0\n@TILELINE 0 x 0 0 1 1\n",
            "@SYSENV 0\n@TILELINE 0 2 0 0 1 1 \\",
            "@SYSENV 0\n@BOGUS 1\n@TILELINE 0 2 0 0 1 1\n",
            "@SYSENV 0\n@SYSENV 0\n@TILELINE 0 2 0 0 1 1\n",
            "@SYSENV 1024\n@LINE 2 5 5 5 5\n",
            "@SYSENV 75 75 0\n@TILELINE 0 2 0 0 1024 512\n",
            "@SYSENV 0 75 0\n@TILELINE 0 2 0 0 1024 0\n",
            "@SYSENV 0\n@TILELINE 0 2 0 0 1 1\n\u{e9}",
            "@SYSENV 0\n@TILEAREA 2 0 0 1 1\n",
        ];
        for b in bad {
            assert!(parse(b, "x", "X").is_err(), "{b:?}");
        }
        let huge = "@".repeat(MAX_TEXT + 1);
        assert!(parse(&huge, "x", "X").is_err());
        let many: String = std::iter::repeat_n("@COOD 1\n", MAX_DIRECTIVES + 1).collect();
        assert!(parse(&many, "x", "X").is_err());
        // The pattern's own rules still apply.
        assert!(parse(FILL, "Bad Id", "X").is_err());
    }
}
