//! Brother's BROTHER EMBROIDERY thread chart as PE-DESIGN 11.50 shows and writes it: the
//! catalogue code and name of each PEC chart entry 1–61 (issues #119, #136).
//!
//! Sources, all black-box: the chart's 61 rows read from PE-DESIGN 11.50's Sewing Attributes ▸
//! Color tab (code and name, brand `EMBROIDERY` on every row; pe-refs-4, 2026-10-29), matched
//! to [`crate::PEC_CHART`] by name, and cross-checked with the 25 (PEC index, code) pairs PE
//! writes in the thread tables of the owner's own `#PES0110` files (all agree). Two rows needed
//! more than a name: pyembroidery's PEC table calls entries 42 and 57 both "Flesh Pink"; PE's
//! files put 124 FLESH PINK at 57, so 307 LINEN, the only code without a name match, is 42.
//! The chart has no entry for PEC 62–64, which PE-DESIGN never matches a colour to.
//!
//! The dialog's own first column (1 BLACK, 2 WHITE, 3 RED, …) is not the PEC index and is not
//! used here.

use crate::Thread;
use std::borrow::Cow;

/// Brand string PE-DESIGN writes for a BROTHER EMBROIDERY thread.
pub const BROTHER_EMBROIDERY_BRAND: &str = "EMBROIDERY";

/// `(PEC chart index, catalogue code, name as PE-DESIGN shows it)` for PEC entries 1–61, in
/// PEC order. The comment names the evidence for the pair: an owner file, the name, or
/// elimination.
pub const BROTHER_EMBROIDERY: [(usize, &str, &str); 61] = [
    (1, "007", "PRUSSIAN BLUE"),    // owner file
    (2, "405", "BLUE"),             // owner file
    (3, "534", "TEAL GREEN"),       // owner file
    (4, "070", "CORNFLOWER BLUE"),  // owner file
    (5, "800", "RED"),              // owner file
    (6, "337", "REDDISH BROWN"),    // name
    (7, "620", "MAGENTA"),          // name
    (8, "810", "LIGHT LILAC"),      // name
    (9, "612", "LILAC"),            // name
    (10, "502", "MINT GREEN"),      // name
    (11, "214", "DEEP GOLD"),       // owner file
    (12, "208", "ORANGE"),          // owner file
    (13, "205", "YELLOW"),          // owner file
    (14, "513", "LIME GREEN"),      // owner file
    (15, "328", "BRASS"),           // name
    (16, "005", "SILVER"),          // owner file
    (17, "330", "RUSSET BROWN"),    // name
    (18, "010", "CREAM BROWN"),     // name
    (19, "704", "PEWTER"),          // name
    (20, "900", "BLACK"),           // owner file
    (21, "406", "ULTRA MARINE"),    // name
    (22, "869", "ROYAL PURPLE"),    // owner file
    (23, "707", "DARK GRAY"),       // name
    (24, "058", "DARK BROWN"),      // owner file
    (25, "086", "DEEP ROSE"),       // owner file
    (26, "323", "LIGHT BROWN"),     // name
    (27, "079", "SALMON PINK"),     // name
    (28, "030", "VERMILLION"),      // owner file
    (29, "001", "WHITE"),           // owner file
    (30, "613", "VIOLET"),          // name
    (31, "542", "SEACREST"),        // name
    (32, "019", "SKY BLUE"),        // owner file
    (33, "126", "PUMPKIN"),         // owner file
    (34, "812", "CREAM YELLOW"),    // name
    (35, "348", "KHAKI"),           // owner file
    (36, "339", "CLAY BROWN"),      // name
    (37, "509", "LEAF GREEN"),      // owner file
    (38, "415", "PEACOCK BLUE"),    // owner file
    (39, "817", "GRAY"),            // owner file
    (40, "399", "WARM GRAY"),       // name
    (41, "517", "DARK OLIVE"),      // name
    (42, "307", "LINEN"),           // elimination
    (43, "085", "PINK"),            // name
    (44, "808", "DEEP GREEN"),      // name
    (45, "804", "LAVENDER"),        // name
    (46, "607", "WISTERIA VIOLET"), // name
    (47, "843", "BEIGE"),           // name
    (48, "807", "CARMINE"),         // owner file
    (49, "333", "AMBER RED"),       // name
    (50, "519", "OLIVE GREEN"),     // owner file
    (51, "107", "DARK FUCHSIA"),    // owner file
    (52, "209", "TANGERINE"),       // name
    (53, "017", "LIGHT BLUE"),      // name
    (54, "507", "EMERALD GREEN"),   // name
    (55, "614", "PURPLE"),          // owner file
    (56, "515", "MOSS GREEN"),      // owner file
    (57, "124", "FLESH PINK"),      // owner file
    (58, "206", "HARVEST GOLD"),    // owner file
    (59, "420", "ELECTRIC BLUE"),   // name
    (60, "202", "LEMON YELLOW"),    // name
    (61, "027", "FRESH GREEN"),     // owner file
];

/// The BROTHER EMBROIDERY catalogue code of PEC chart entry `pec_index` (1–61), as PE-DESIGN
/// writes it in a thread table: `brother_embroidery_code(5) == Some("800")`.
pub fn brother_embroidery_code(pec_index: usize) -> Option<&'static str> {
    BROTHER_EMBROIDERY
        .get(pec_index.checked_sub(1)?)
        .map(|&(_, code, _)| code)
}

/// The PEC chart entry of a BROTHER EMBROIDERY catalogue code: `pec_index_of_brother_code("800")
/// == Some(5)`. Leading zeros matter, as in PE-DESIGN (`"007"`, not `"7"`).
pub fn pec_index_of_brother_code(code: &str) -> Option<usize> {
    BROTHER_EMBROIDERY
        .iter()
        .find(|&&(_, c, _)| c == code)
        .map(|&(i, _, _)| i)
}

/// Brand PE-DESIGN 11.50 writes for a Ver4 Option Color thread.
pub const OPTION_COLOR_BRAND: &str = "Option Color";

/// Colour type PE-DESIGN 11.50 writes for a Ver4 Option Color thread (BROTHER EMBROIDERY's is
/// `0x0A`, [`Thread::CUSTOM_KIND`]).
pub const OPTION_COLOR_KIND: u32 = 0x0B;

/// PE-DESIGN 11.50's "Ver4 Option Color" thread chart: `(sew palette id, name, RGB)` for its
/// 29 rows, in row order. Read from PE-DESIGN's own file of the 29 colours (reference session,
/// 2026-10-30; `openstitch-formats` fixture `pe11/ver4-option-color-29.pes`): empty code, type
/// [`OPTION_COLOR_KIND`], brand [`OPTION_COLOR_BRAND`] and the id string 66–94 (row n is 65 + n)
/// where a BROTHER EMBROIDERY thread has its PEC index. Every channel is 0, 85, 170 or 255.
pub const VER4_OPTION_COLORS: [(usize, &str, u32); 29] = [
    (66, "prussian blue2", 0x00_00AA),
    (67, "blue2", 0x00_00FF),
    (68, "electric blue2", 0x00_55FF),
    (69, "moss green2", 0x00_AA00),
    (70, "sky blue3", 0x00_AAFF),
    (71, "lime green3", 0x00_FF00),
    (72, "leaf green3", 0x00_FF55),
    (73, "sky blue5", 0x00_FFAA),
    (74, "sky blue6", 0x00_FFFF),
    (75, "dark brown2", 0x55_0000),
    (76, "purple2", 0x55_00FF),
    (77, "cornflower blue2", 0x55_55FF),
    (78, "sky blue2", 0x55_AAAA),
    (79, "sky blue4", 0x55_AAFF),
    (80, "lime green2", 0x55_FF00),
    (81, "leaf green2", 0x55_FF55),
    (82, "seacrest2", 0x55_FFAA),
    (83, "light blue2", 0x55_FFFF),
    (84, "red2", 0xAA_0000),
    (85, "magenta2", 0xAA_00AA),
    (86, "magenta3", 0xAA_00FF),
    (87, "lilac2", 0xAA_55FF),
    (88, "lavender2", 0xAA_AAFF),
    (89, "yellow2", 0xAA_FF00),
    (90, "dark fuchsia2", 0xFF_0055),
    (91, "deep rose2", 0xFF_00AA),
    (92, "deep rose3", 0xFF_00FF),
    (93, "light lilac2", 0xFF_55FF),
    (94, "flesh pink2", 0xFF_AAFF),
];

/// Sew palette ids PE-DESIGN reserves and never shows (issue #136: 61 basic + 4 reserved + 29).
pub const SEW_PALETTE_RESERVED: std::ops::RangeInclusive<usize> = 62..=65;

/// The 94-entry sew palette of issue #136, indexed by its 1-based id (index 0 unused): ids 1–61
/// are the BROTHER EMBROIDERY chart threads ([`crate::PEC_CHART`], their codes in
/// [`BROTHER_EMBROIDERY`]), 62–65 reserved (`None`), 66–94 the Ver4 Option Color threads.
pub fn sew_palette() -> &'static [Option<Thread>] {
    static PALETTE: std::sync::LazyLock<Vec<Option<Thread>>> = std::sync::LazyLock::new(|| {
        let mut v: Vec<Option<Thread>> = crate::pec_basic_chart().to_vec();
        v.extend(SEW_PALETTE_RESERVED.map(|_| None));
        v.extend(VER4_OPTION_COLORS.iter().map(|&(_, name, rgb)| {
            let mut t = Thread::rgb(rgb);
            t.name = Some(Cow::Borrowed(name));
            t.brand = Some(Cow::Borrowed(OPTION_COLOR_BRAND));
            t.kind = Some(OPTION_COLOR_KIND);
            Some(t)
        }));
        v
    });
    &PALETTE
}

/// The sew palette id of `color` (issue #119/#136's "approximate" match: the dual squared
/// Euclidean / city-block minima of [`crate::approximate_index`]; reserved ids never match).
/// `basic_only` matches against ids 1–61 alone. The rule is the issue's: no PE-DESIGN capture
/// shows where PE uses this palette or how it matches to it.
pub fn sew_palette_index(color: &Thread, basic_only: bool) -> Option<usize> {
    let palette = sew_palette();
    let candidates = if basic_only {
        &palette[..=crate::PEC_BASIC_COLORS]
    } else {
        palette
    };
    crate::approximate_index(color, candidates)
}
