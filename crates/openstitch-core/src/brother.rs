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
