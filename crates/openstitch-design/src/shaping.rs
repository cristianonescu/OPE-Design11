//! Text shaping for lettering: bidirectional ordering ([`unicode_bidi`]), script runs
//! ([`unicode_script`]) and OpenType shaping of TrueType/OpenType fonts ([`rustybuzz`], a
//! safe-Rust port of HarfBuzz's shaping algorithm).
//!
//! A line is split into level runs by the Unicode Bidirectional Algorithm (UAX #9) and put in
//! visual order; each level run is split further at script changes, so every piece is shaped
//! with its own script (Arabic joining, Indic reordering, mark positioning, …). Right-to-left
//! pieces come out of the shaper already left to right on the page.

use std::ops::Range;

use rustybuzz::{Direction, Feature, UnicodeBuffer, ttf_parser::Tag};
use unicode_bidi::BidiInfo;
use unicode_script::{Script, UnicodeScript};

/// One shaped glyph in font units, HarfBuzz conventions (+y up; the offsets move the glyph from
/// the pen, the advances move the pen).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shaped {
    /// Glyph id (or embroidery-font glyph index); `None` for `.notdef` (a character the font
    /// lacks).
    pub glyph: Option<u32>,
    /// Index (in characters, from the start of the text) of the first character the glyph
    /// stands for.
    pub cluster: usize,
    pub x_advance: f64,
    pub y_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

/// A piece of a line to shape: a byte range of the line and its direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub range: Range<usize>,
    pub rtl: bool,
}

/// The pieces of `line` in visual (left-to-right) order: bidi level runs split at script
/// changes. Characters of the Common and Inherited scripts (spaces, digits, punctuation,
/// combining marks) join the piece before them.
pub(crate) fn visual_pieces(line: &str) -> Vec<Piece> {
    if line.is_empty() {
        return Vec::new();
    }
    let bidi = BidiInfo::new(line, None);
    let mut out = Vec::new();
    for para in &bidi.paragraphs {
        let (levels, runs) = bidi.visual_runs(para, para.range.clone());
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            let mut pieces = script_pieces(line, run);
            if rtl {
                pieces.reverse();
            }
            out.extend(pieces.into_iter().map(|range| Piece { range, rtl }));
        }
    }
    out
}

/// `range` of `line` split where the script changes (logical order).
fn script_pieces(line: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut current: Option<Script> = None;
    let mut start = range.start;
    for (i, ch) in line[range.clone()].char_indices() {
        let at = range.start + i;
        let script = ch.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(s) if s != script => {
                out.push(start..at);
                start = at;
                current = Some(script);
            }
            Some(_) => {}
        }
    }
    out.push(start..range.end);
    out
}

/// How a piece is shaped.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub rtl: bool,
    pub vertical: bool,
    pub kerning: bool,
}

/// Shapes one piece: characters with their index in the whole text.
/// Standard ligatures are turned off so each letter stays its own glyph (embroidery digitizing
/// and per-character edits work letter by letter); required ligatures (Arabic lam-alef) and
/// contextual forms stay on. Kerning (`kern` feature, GPOS or the `kern` table) follows
/// `options.kerning`. `None` when the shaper panicked on a damaged font.
pub(crate) fn shape(
    face: &rustybuzz::Face<'_>,
    chars: &[(char, usize)],
    options: Options,
) -> Option<Vec<Shaped>> {
    let mut buffer = UnicodeBuffer::new();
    for &(ch, index) in chars {
        buffer.add(ch, index as u32);
    }
    buffer.guess_segment_properties();
    buffer.set_direction(match (options.vertical, options.rtl) {
        (true, _) => Direction::TopToBottom,
        (false, true) => Direction::RightToLeft,
        (false, false) => Direction::LeftToRight,
    });
    let mut features = vec![
        Feature::new(Tag::from_bytes(b"liga"), 0, ..),
        Feature::new(Tag::from_bytes(b"clig"), 0, ..),
    ];
    if !options.kerning {
        features.push(Feature::new(Tag::from_bytes(b"kern"), 0, ..));
    }
    // Font data is untrusted: a shaper panic loses the shaping, not the application.
    let shaped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustybuzz::shape(face, &features, buffer)
    }))
    .ok()?;
    Some(
        shaped
            .glyph_infos()
            .iter()
            .zip(shaped.glyph_positions())
            .map(|(info, pos)| Shaped {
                glyph: (info.glyph_id != 0).then_some(info.glyph_id),
                cluster: info.cluster as usize,
                x_advance: f64::from(pos.x_advance),
                y_advance: f64::from(pos.y_advance),
                x_offset: f64::from(pos.x_offset),
                y_offset: f64::from(pos.y_offset),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(line: &str) -> Vec<(&str, bool)> {
        visual_pieces(line)
            .into_iter()
            .map(|p| (&line[p.range], p.rtl))
            .collect()
    }

    #[test]
    fn left_to_right_text_is_one_piece() {
        assert_eq!(pieces("Hello, world 42"), vec![("Hello, world 42", false)]);
        assert!(pieces("").is_empty());
    }

    #[test]
    fn mixed_lines_are_in_visual_order() {
        // Hebrew inside Latin: the Hebrew run is right-to-left, between the Latin runs.
        let line = "ab \u{05D0}\u{05D1} cd";
        assert_eq!(
            pieces(line),
            vec![("ab ", false), ("\u{05D0}\u{05D1}", true), (" cd", false)]
        );
        // A right-to-left paragraph puts the Latin word on the left of the Hebrew one.
        let line = "\u{05D0}\u{05D1} ab";
        assert_eq!(
            pieces(line),
            vec![("ab", false), ("\u{05D0}\u{05D1} ", true)]
        );
        // An override makes Latin right-to-left.
        let p = pieces("\u{202E}abc");
        assert!(p.contains(&("abc", true)), "{p:?}");
    }

    #[test]
    fn script_changes_split_a_run() {
        let line = "abc \u{0915}\u{093F} def";
        assert_eq!(
            pieces(line),
            vec![
                ("abc ", false),
                ("\u{0915}\u{093F} ", false),
                ("def", false)
            ]
        );
    }
}
