//! A minimal PDF 1.4 writer: pages of vector paths, text in the standard Helvetica font and RGB
//! images, with Flate-compressed streams. Written here rather than taken from a crate because
//! the output needs only these few operators, must be exact in scale (1 mm = 72 / 25.4 pt) and
//! deterministic (no dates or ids), and the workspace keeps its dependencies few.

use miniz_oxide::deflate::compress_to_vec_zlib;
use std::fmt::Write;

/// Points per millimetre.
pub const PT_PER_MM: f64 = 72.0 / 25.4;

/// An RGB image placed on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, rows top to bottom.
    pub rgb: Vec<u8>,
}

/// One page: its size in points and its content stream (PDF operators, origin bottom-left).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub width: f64,
    pub height: f64,
    pub content: String,
    pub images: Vec<Image>,
}

/// Number formatting for content streams: up to 3 decimals, no exponent.
pub fn n(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Text as a PDF literal string in WinAnsi (Latin-1 subset); other characters become `?`.
pub fn literal(text: &str) -> String {
    let mut out = String::from("(");
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            '\u{a0}'..='\u{ff}' => {
                let _ = write!(out, "\\{:03o}", c as u32);
            }
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

/// Approximate Helvetica advance width (per unit font size) for centring and right alignment.
pub fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | '\'' | '.' | ',' | ':' | ';' | '|' | '!' => 0.24,
            ' ' | 'f' | 't' | 'r' | 'I' | '(' | ')' | '[' | ']' => 0.3,
            'm' | 'w' | 'M' | 'W' => 0.85,
            'A'..='Z' => 0.68,
            '0'..='9' => 0.556,
            _ => 0.52,
        })
        .sum::<f64>()
        * size
}

/// Serialises pages into a PDF file.
pub fn write(pages: &[Page], title: &str) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    // 1: catalog, 2: pages, 3: font, 4: info; then per page: page, content, images.
    let font = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
        .to_vec();
    let info = format!("<< /Title {} /Producer (OpenStitch) >>", literal(title)).into_bytes();
    let mut page_ids = Vec::new();
    let mut next = 5;
    let mut page_objects: Vec<(usize, Vec<u8>)> = Vec::new();
    for page in pages {
        let page_id = next;
        let content_id = next + 1;
        let image_ids: Vec<usize> = (0..page.images.len()).map(|i| next + 2 + i).collect();
        next += 2 + page.images.len();
        page_ids.push(page_id);
        let xobjects: String = image_ids
            .iter()
            .enumerate()
            .map(|(i, id)| format!("/Im{i} {id} 0 R "))
            .collect();
        let page_dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {content_id} 0 R /Resources << /Font << /F1 3 0 R >> /XObject << {xobjects}>> >> >>",
            n(page.width),
            n(page.height)
        );
        page_objects.push((page_id, page_dict.into_bytes()));
        page_objects.push((content_id, stream("", page.content.as_bytes())));
        for (img, id) in page.images.iter().zip(&image_ids) {
            let dict = format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 ",
                img.width, img.height
            );
            page_objects.push((*id, stream(&dict, &img.rgb)));
        }
    }
    let kids: String = page_ids.iter().map(|id| format!("{id} 0 R ")).collect();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", pages.len()).into_bytes());
    objects.push(font);
    objects.push(info);
    page_objects.sort_by_key(|(id, _)| *id);
    objects.extend(page_objects.into_iter().map(|(_, o)| o));

    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let packed = compress_to_vec_zlib(data, 6);
    let mut out = format!(
        "<< {dict}/Length {} /Filter /FlateDecode >>\nstream\n",
        packed.len()
    )
    .into_bytes();
    out.extend(packed);
    out.extend(b"\nendstream");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_strings_are_pdf_safe() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(2.83464567), "2.835");
        assert_eq!(n(-0.0001), "0");
        assert_eq!(n(f64::NAN), "0");
        assert_eq!(literal("a(b)\\c"), "(a\\(b\\)\\\\c)");
        assert_eq!(literal("é✓"), "(\\351?)");
    }

    #[test]
    fn document_structure_and_xref_offsets_are_valid() {
        let page = Page {
            width: 595.0,
            height: 842.0,
            content: "0 0 m 100 100 l S".into(),
            images: vec![Image {
                width: 2,
                height: 1,
                rgb: vec![255, 0, 0, 0, 0, 255],
            }],
        };
        let pdf = write(&[page.clone(), page], "Test");
        let find = |needle: &[u8], from: usize| {
            pdf[from..]
                .windows(needle.len())
                .position(|w| w == needle)
                .map(|i| i + from)
        };
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(find(b"/Count 2", 0).is_some());
        assert!(find(b"/MediaBox [0 0 595 842]", 0).is_some());
        // Every xref entry points at "<n> 0 obj".
        let sx = find(b"startxref\n", 0).expect("startxref") + 10;
        let tail = String::from_utf8_lossy(&pdf[sx..]).into_owned();
        let xref_at: usize = tail
            .lines()
            .next()
            .and_then(|s| s.parse().ok())
            .expect("offset");
        let table = String::from_utf8_lossy(&pdf[xref_at..]).into_owned();
        let entries: Vec<usize> = table
            .lines()
            .skip(3)
            .take_while(|l| l.ends_with(" n "))
            .map(|l| l[..10].parse().expect("offset"))
            .collect();
        assert_eq!(entries.len(), 4 + 2 * 3);
        for (i, off) in entries.iter().enumerate() {
            assert!(
                pdf[*off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()),
                "object {}",
                i + 1
            );
        }
        // The first page's content stream decompresses to its operators.
        let start = find(b"stream\n", 0).expect("stream") + 7;
        let end = find(b"\nendstream", start).expect("end");
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[start..end]).expect("inflate");
        assert_eq!(raw, b"0 0 m 100 100 l S");
    }
}
