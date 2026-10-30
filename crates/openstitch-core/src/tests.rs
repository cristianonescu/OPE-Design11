use super::*;

#[test]
fn charts_have_expected_sizes_and_entries() {
    assert_eq!(PEC_CHART.len(), 65);
    assert!(PEC_CHART[0].is_none());
    let red = PEC_CHART[5].as_ref().expect("entry 5");
    assert_eq!(
        (red.hex().as_str(), red.name.as_deref()),
        ("#ed171f", Some("Red"))
    );
    assert_eq!(JEF_CHART.len(), 79);
    assert_eq!(SEW_CHART.len(), 79);
    assert_eq!(
        SEW_CHART[0].as_ref().and_then(|t| t.name.as_deref()),
        Some("Unknown")
    );
    assert_eq!(
        SEW_CHART[1].as_ref().and_then(|t| t.name.as_deref()),
        Some("Black")
    );
}

#[test]
fn nearest_is_equal_weight_squared_euclidean_with_later_index_on_ties() {
    let chart = [
        None,
        Some(Thread::rgb(0x000000)),
        Some(Thread::rgb(0x000000)),
    ];
    assert_eq!(nearest_index(&Thread::rgb(0x010101), &chart), Some(2));
    assert_eq!(nearest_index(&Thread::rgb(0xEE1111), &PEC_CHART), Some(5));
    assert_eq!(nearest_index(&Thread::rgb(0x123456), &[None, None]), None);
    // No channel weighting: a red step costs the same as a blue step.
    let a = Thread::rgb(0x000000);
    assert_eq!(color_distance(&a, &Thread::rgb(0x0A0000)), 100);
    assert_eq!(color_distance(&a, &Thread::rgb(0x00000A)), 100);
    assert_eq!(color_distance(&a, &Thread::rgb(0xFFFFFF)), 195075);
    assert_eq!(city_block_distance(&a, &Thread::rgb(0xFFFFFF)), 765);
    // Where red-mean and equal weights disagree: (200,0,0) against (0,0,200) for (100,0,100).
    // Red-mean weights the blue step less for a dark colour and picked blue; equal weights tie
    // and keep the later entry.
    let chart = [Some(Thread::rgb(0xC80000)), Some(Thread::rgb(0x0000C8))];
    assert_eq!(nearest_index(&Thread::rgb(0x640064), &chart), Some(1));
}

#[test]
fn imported_colours_become_brother_chart_threads() {
    let t = nearest_brother_thread(&Thread::rgb(0xFF8000));
    assert_eq!(t, PEC_CHART[11].clone().expect("11"));
    assert_eq!(t.catalog_number.as_deref(), Some("11"));
    assert_eq!(brother_embroidery_code(11), Some("214"));
    // An exact chart colour stays that entry.
    let red = PEC_CHART[5].clone().expect("5");
    assert_eq!(nearest_brother_thread(&Thread::rgb(red.rgb_value())), red);
}

/// PE-DESIGN 11.50's picks for twelve RGB fills imported from an SVG (owner's capture
/// `compat/reference/pes/thread-match-swatches.pes`, 2026-10-29, issue #119): Brother chart
/// entry per fill. Equal-weight squared Euclidean against entries 1–61 reproduces all twelve;
/// red-mean missed blue and cyan, and the whole 64-entry chart would send orange to 62.
#[test]
fn pe_design_swatch_picks_are_reproduced() {
    let picks = [
        (0xFF0000, 5),  // 800 RED
        (0x00FF00, 14), // 513 LIME GREEN
        (0x0000FF, 2),  // 405 BLUE
        (0xFFFF00, 13), // 205 YELLOW
        (0x00FFFF, 32), // 019 SKY BLUE
        (0xFF00FF, 25), // 086 DEEP ROSE
        (0x808080, 39), // 817 GRAY
        (0xC89664, 35), // 348 KHAKI
        (0x326496, 4),  // 070 CORNFLOWER BLUE
        (0xF0C8DC, 57), // 124 FLESH PINK
        (0x145028, 38), // 415 PEACOCK BLUE
        (0xFF8000, 11), // 214 DEEP GOLD
    ];
    for (rgb, index) in picks {
        assert_eq!(
            nearest_index(&Thread::rgb(rgb), pec_basic_chart()),
            Some(index),
            "{rgb:06x}"
        );
    }
    assert_eq!(pec_basic_chart().len(), PEC_BASIC_COLORS + 1);
    // PE-DESIGN's thread table carries (253, 217, 222) for entry 57.
    assert_eq!(
        PEC_CHART[57].as_ref().map(|t| t.hex()),
        Some("#fdd9de".into())
    );
}

#[test]
fn approximate_match_follows_the_city_block_winner() {
    // Euclid² prefers one medium step over two smaller ones; L1 prefers the two smaller ones.
    // (0,0,0) → candidates (30,0,0): sq 900, L1 30; (12,12,12): sq 432, L1 36.
    let chart = [Some(Thread::rgb(0x0C0C0C)), Some(Thread::rgb(0x1E0000))];
    let c = Thread::rgb(0x000000);
    assert_eq!(nearest_index(&c, &chart), Some(0));
    assert_eq!(approximate_index(&c, &chart), Some(1));
    // Exact match: both minima are 0, the Euclidean winner is returned (the same entry).
    assert_eq!(approximate_index(&Thread::rgb(0x1E0000), &chart), Some(1));
    // Ties go to the later index on both accumulators, as in `nearest_index`.
    let chart = [
        None,
        Some(Thread::rgb(0x101010)),
        Some(Thread::rgb(0x101010)),
    ];
    assert_eq!(approximate_index(&Thread::rgb(0x000000), &chart), Some(2));
    assert_eq!(approximate_index(&c, &[None]), None);
}

#[test]
fn hex_round_trip() {
    assert_eq!(
        Thread::from_hex("#0a55A3").map(|t| t.hex()),
        Some("#0a55a3".to_string())
    );
    assert!(Thread::from_hex("#12345").is_none());
    assert!(Thread::from_hex("#zzzzzz").is_none());
}

#[test]
fn bounds_ignore_markers_and_blocks_split_on_colour_change() {
    let mut p = StitchPattern::new();
    p.stitch_to(10.0, 5.0);
    p.stitch_to(-20.0, 40.0);
    p.push(Command::ColorChange);
    p.jump_to(100.0, 100.0);
    p.stitch_to(100.0, 101.0);
    p.push(Command::End);
    let b = p.bounds();
    assert_eq!(
        (b.min_x, b.min_y, b.max_x, b.max_y),
        (-20.0, 5.0, 100.0, 101.0)
    );
    assert_eq!(p.color_block_count(), 2);
    assert_eq!(p.color_blocks().len(), 2);
    p.fix_thread_count(None);
    assert_eq!(p.threads.len(), 2);
}

#[test]
fn long_moves_are_split_evenly() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(500.0, -300.0);
    let q = p.interpolating_long_moves(121.0, 121.0);
    assert_eq!(q.stitches.len(), 6);
    assert!(
        q.stitches
            .windows(2)
            .all(|w| (w[1].x - w[0].x).abs() <= 121.0 && (w[1].y - w[0].y).abs() <= 121.0)
    );
}

#[test]
fn trims_inferred_from_jump_count_and_clipped() {
    let mut s = vec![
        Stitch::new(0.0, 0.0, Command::Stitch),
        Stitch::new(10.0, 0.0, Command::Stitch),
        Stitch::new(12.0, 2.0, Command::Jump),
        Stitch::new(8.0, -2.0, Command::Jump),
        Stitch::new(10.0, 0.0, Command::Jump),
        Stitch::new(60.0, 0.0, Command::Jump),
        Stitch::new(60.0, 10.0, Command::Stitch),
    ];
    trims::infer_trims(&mut s, Some(3), None, true);
    let cmds: Vec<_> = s.iter().map(|s| s.command).collect();
    assert_eq!(
        cmds,
        [
            Command::Stitch,
            Command::Stitch,
            Command::Trim,
            Command::Jump,
            Command::Stitch
        ]
    );
    assert_eq!((s[2].x, s[3].x), (10.0, 60.0));
}

#[test]
fn trims_inferred_from_distance_once() {
    let mut s = vec![
        Stitch::new(0.0, 0.0, Command::Stitch),
        Stitch::new(20.0, 0.0, Command::Jump),
        Stitch::new(60.0, 0.0, Command::Jump),
        Stitch::new(100.0, 0.0, Command::Jump),
        Stitch::new(100.0, 5.0, Command::Stitch),
    ];
    trims::infer_trims(&mut s, None, Some(30.0), true);
    let cmds: Vec<_> = s.iter().map(|s| s.command).collect();
    assert_eq!(
        cmds,
        [
            Command::Stitch,
            Command::Trim,
            Command::Jump,
            Command::Jump,
            Command::Jump,
            Command::Stitch
        ]
    );
}

#[test]
fn pattern_serializes_with_camel_case_commands() {
    let mut p = StitchPattern::new();
    p.push(Command::ColorChange);
    let json = serde_json::to_string(&p).expect("serialize");
    assert!(json.contains("\"colorChange\""));
    let back: StitchPattern = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, p);
}

/// The BROTHER EMBROIDERY codes of PEC entries 1–61 (pe-refs-4's reading of PE-DESIGN's chart,
/// matched by name): one code per entry, every code once, none for 62–64, and every pair PE
/// writes in the owner's own `#PES0110` files.
#[test]
fn brother_embroidery_codes_cover_the_basic_chart_once() {
    for (k, &(i, code, name)) in BROTHER_EMBROIDERY.iter().enumerate() {
        assert_eq!(i, k + 1);
        assert_eq!(code.len(), 3, "{code}");
        assert!(code.bytes().all(|b| b.is_ascii_digit()), "{code}");
        assert_eq!(pec_index_of_brother_code(code), Some(i), "{code} {name}");
        assert_eq!(brother_embroidery_code(i), Some(code));
        let pec = PEC_CHART[i]
            .as_ref()
            .and_then(|t| t.name.as_deref())
            .unwrap_or("");
        assert!(
            pec.eq_ignore_ascii_case(name)
                || pec
                    .replace(' ', "")
                    .eq_ignore_ascii_case(&name.replace(' ', ""))
                || (name == "VERMILLION" && pec == "Vermilion"),
            "{i}: PEC {pec} vs PE {name}"
        );
    }
    assert_eq!(BROTHER_EMBROIDERY.len(), PEC_BASIC_COLORS);
    for i in [0, 62, 63, 64, 65] {
        assert_eq!(brother_embroidery_code(i), None);
    }
    assert_eq!(pec_index_of_brother_code("7"), None);
    // Pairs from PE-DESIGN's own thread tables (owner files, issue #119 swatches).
    for (i, code) in [
        (1, "007"),
        (2, "405"),
        (3, "534"),
        (4, "070"),
        (5, "800"),
        (11, "214"),
        (12, "208"),
        (13, "205"),
        (14, "513"),
        (16, "005"),
        (20, "900"),
        (22, "869"),
        (24, "058"),
        (25, "086"),
        (28, "030"),
        (29, "001"),
        (32, "019"),
        (33, "126"),
        (35, "348"),
        (37, "509"),
        (38, "415"),
        (39, "817"),
        (48, "807"),
        (50, "519"),
        (51, "107"),
        (55, "614"),
        (56, "515"),
        (57, "124"),
        (58, "206"),
        (61, "027"),
    ] {
        assert_eq!(brother_embroidery_code(i), Some(code), "{i}");
    }
}
