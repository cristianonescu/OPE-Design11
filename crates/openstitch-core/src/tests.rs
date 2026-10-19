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
fn nearest_prefers_later_index_on_ties() {
    let chart = [
        None,
        Some(Thread::rgb(0x000000)),
        Some(Thread::rgb(0x000000)),
    ];
    assert_eq!(nearest_index(&Thread::rgb(0x010101), &chart), Some(2));
    assert_eq!(nearest_index(&Thread::rgb(0xEE1111), &PEC_CHART), Some(5));
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
