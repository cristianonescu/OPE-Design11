//! Codec tests: oracle round trips, structural checks and malformed input.

mod common;

use common::{assert_golden, data, hexes, stitches};
use openstitch_core::{Bounds, Command, PEC_CHART, Stitch, StitchPattern, Thread};
use openstitch_formats::{
    FileType, FormatError, dst, exp, hus, jef, lzh, pcs, pec, pes, phc, sew, shv, vip, xxx,
};

const JEF_DATE: &str = "20261018120000";

fn sewn(p: &StitchPattern) -> Vec<[f64; 2]> {
    p.stitches
        .iter()
        .filter(|s| s.command == Command::Stitch)
        .map(|s| [s.x, s.y])
        .collect()
}

fn with_threads(rgbs: &[u32]) -> StitchPattern {
    StitchPattern {
        threads: rgbs.iter().map(|&c| Thread::rgb(c)).collect(),
        ..StitchPattern::new()
    }
}

// MARK: DST

#[test]
fn dst_every_delta_round_trips() {
    for dx in -121..=121 {
        for dy in (-121..=121).step_by(11) {
            let r = dst::encode(dx, dy, false).expect("encode");
            let (x, y) = dst::decode(r[0], r[1], r[2]);
            assert_eq!((i64::from(x), i64::from(y)), (dx, dy));
        }
    }
}

#[test]
fn dst_rejects_oversized_delta() {
    assert!(matches!(
        dst::encode(122, 0, false),
        Err(FormatError::ValueOutOfRange(_))
    ));
}

#[test]
fn dst_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_dst_read.json");
    let pattern = dst::read(&data("oracle.dst")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(pattern.name(), Some("oracle"));
}

#[test]
fn dst_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.dst");
    assert_eq!(
        dst::write(&dst::read(&original).expect("read")).expect("write"),
        original
    );
}

#[test]
fn dst_long_stitches_are_split() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(500.0, -300.0);
    p.push(Command::End);
    let back = dst::read(&dst::write(&p).expect("write")).expect("read");
    let s = sewn(&back);
    assert_eq!(s.last(), Some(&[500.0, -300.0]));
    assert_eq!(s.len(), 6); // 0,0 + ceil(500/121)=5 segments
}

#[test]
fn dst_trim_jump_count_is_configurable() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(10.0, 0.0);
    p.push(Command::Trim);
    p.jump_to(50.0, 0.0);
    p.stitch_to(50.0, 10.0);
    p.push(Command::End);
    for n in [2, 3, 5] {
        let data = dst::write_with(
            &p,
            dst::WriteOptions {
                trim_jumps: n,
                ..Default::default()
            },
        )
        .expect("write");
        assert_eq!((data.len() - 512) / 3, 3 + 1 + n + 1); // stitches + jump + trim jumps + end
        let back = dst::read_with(&data, n).expect("read");
        assert_eq!(back.count(Command::Trim), 1);
        assert_eq!(back.count(Command::Jump), 1);
    }
    assert_eq!(
        dst::trim_jump_sequence(4).iter().map(|d| d.0).sum::<i64>(),
        0
    );
}

#[test]
fn dst_extended_header_round_trips_metadata() {
    let mut p = with_threads(&[0x123456]);
    p.threads[0] = p.threads[0].clone().with_name("Teal");
    p.metadata.insert("author".into(), "Ada".into());
    p.stitch_to(0.0, 0.0);
    p.stitch_to(5.0, 5.0);
    p.push(Command::End);
    let data = dst::write_with(
        &p,
        dst::WriteOptions {
            extended_header: true,
            ..Default::default()
        },
    )
    .expect("write");
    let back = dst::read(&data).expect("read");
    assert_eq!(back.metadata.get("author").map(String::as_str), Some("Ada"));
    assert_eq!(hexes(&back.threads), ["#123456"]);
    assert_eq!(back.threads[0].name.as_deref(), Some("Teal"));
}

#[test]
fn distance_rule_only_trims_once_until_next_stitch() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.jump_to(20.0, 0.0);
    p.jump_to(60.0, 0.0); // run now exceeds 30
    p.jump_to(100.0, 0.0);
    p.stitch_to(100.0, 5.0);
    p.infer_trims(None, Some(30.0), true);
    let commands: Vec<Command> = p.stitches.iter().map(|s| s.command).collect();
    assert_eq!(
        commands,
        [
            Command::Stitch,
            Command::Trim,
            Command::Jump,
            Command::Jump,
            Command::Jump,
            Command::Stitch
        ]
    );
    assert_eq!(p.stitches[1].x, 0.0);
}

// MARK: PEC / PES

#[test]
fn pec_pes_read_oracle_files_like_reference() {
    for ext in ["pes", "pec"] {
        let expected = stitches(&format!("oracle_{ext}_read.json"));
        let t = FileType::from_extension(ext).expect("type");
        let pattern = t.read(&data(&format!("oracle.{ext}"))).expect("read");
        assert_eq!(pattern.stitches, expected.stitches, "{ext}");
        assert_eq!(hexes(&pattern.threads), expected.threads, "{ext}");
    }
}

#[test]
fn pec_pes_rewrite_is_byte_identical_to_reference() {
    for ext in ["pes", "pec"] {
        let t = FileType::from_extension(ext).expect("type");
        let original = data(&format!("oracle.{ext}"));
        let mut pattern = t.read(&original).expect("read");
        pattern.set_name("oracle");
        // oracle.pes is pyembroidery's `1t`; the default PES is version 6 with objects.
        let bytes = match t {
            FileType::Pes => {
                pes::write_with(&pattern, &pes::WriteOptions::parse("1t").expect("1t"))
            }
            _ => t.write(&pattern),
        };
        assert_eq!(bytes.expect("write"), original, "{ext}");
    }
}

#[test]
fn default_pes_is_version_6_with_objects_and_thread_table() {
    let pattern = pes::read(&data("oracle_v6.pes")).expect("read");
    assert_eq!(
        pes::WriteOptions::default(),
        pes::WriteOptions::parse("6").expect("6")
    );
    assert_eq!(
        pes::WriteOptions::parse(pes::WriteOptions::DEFAULT_NAME),
        Some(pes::WriteOptions::default())
    );
    let bytes = FileType::Pes.write(&pattern).expect("write");
    assert_eq!(bytes, data("oracle_v6.pes"));
    let objects = pes::read_objects(&bytes).expect("objects");
    assert_eq!(objects.version, "0060");
    assert!(!objects.objects.is_empty());
    assert_eq!(objects.threads.len(), 2);
}

#[test]
fn pes_v6_thread_chart_and_kind_round_trip_byte_identically() {
    let original = data("oracle_v6_chart.pes");
    let pattern = pes::read(&original).expect("read");
    let charts: Vec<_> = pattern.threads.iter().map(|t| t.chart.as_deref()).collect();
    assert_eq!(charts, [Some("Test chart A"), Some("Test chart B")]);
    assert_eq!(pattern.threads[0].brand.as_deref(), Some("Acme"));
    assert_eq!(pattern.threads[0].catalog_number.as_deref(), Some("800"));
    assert_eq!(pattern.threads[0].kind, None); // 0x0A, custom colour
    let options = pes::WriteOptions::parse("6").expect("6");
    assert_eq!(
        pes::write_with(&pattern, &options).expect("write"),
        original
    );

    // A non-default colour type is kept; unknown charts stay empty.
    let mut p = pattern.clone();
    p.threads[1].kind = Some(0x0B);
    p.threads[1].chart = None;
    let back = pes::read(&pes::write_with(&p, &options).expect("write")).expect("read");
    assert_eq!(back.threads[1].kind, Some(0x0B));
    assert_eq!(back.threads[1].chart, None);
    assert_eq!(back.threads, p.threads);
}

#[test]
fn pes_versions_with_objects_read_like_reference() {
    for v in ["1", "6", "6t"] {
        let expected = stitches(&format!("oracle_v{v}_pes_read.json"));
        let pattern = pes::read(&data(&format!("oracle_v{v}.pes"))).expect("read");
        assert_eq!(pattern.stitches, expected.stitches, "v{v}");
        assert_eq!(hexes(&pattern.threads), expected.threads, "v{v}");
    }
}

#[test]
fn pes_versions_with_objects_rewrite_byte_identically() {
    for v in ["1", "6", "6t"] {
        let original = data(&format!("oracle_v{v}.pes"));
        let pattern = pes::read(&original).expect("read");
        let options = pes::WriteOptions::parse(v).expect("version");
        assert_eq!(
            pes::write_with(&pattern, &options).expect("write"),
            original,
            "v{v}"
        );
    }
    assert_eq!(pes::WriteOptions::parse("6x"), None);
    assert_ne!(
        pes::WriteOptions::default(),
        pes::WriteOptions::parse("1t").expect("1t")
    );
}

#[test]
fn pes_v6_keeps_rgb_threads_names_and_metadata() {
    let mut p = with_threads(&[0x123456, 0xABCDEF]);
    p.threads[0] = p.threads[0].clone().with_name("Night");
    p.threads[0].catalog_number = Some("042".into());
    p.threads[0].brand = Some("Acme".into());
    p.set_name("A long design name");
    p.metadata.insert("author".into(), "Ana".into());
    p.stitch_to(0.0, 0.0);
    p.stitch_to(50.0, 20.0);
    p.push(Command::ColorChange);
    p.stitch_to(80.0, -30.0);
    p.push(Command::End);
    let bytes = pes::write_with(&p, &pes::WriteOptions::parse("6").expect("v6")).expect("write");
    let back = pes::read(&bytes).expect("read");
    assert_eq!(back.threads, p.threads);
    assert_eq!(back.name(), Some("A long design name"));
    assert_eq!(back.metadata.get("author").map(String::as_str), Some("Ana"));
    assert_eq!(sewn(&back), sewn(&p));

    let objects = pes::read_objects(&bytes).expect("objects");
    assert_eq!(objects.version, "0060");
    assert_eq!(objects.objects.len(), 1);
    let one = &objects.objects[0];
    assert_eq!((one.width, one.height), (80, 50));
    // Two stitch runs in colours 0 and 1 (indexes into the thread table), bottom-left origin.
    let runs: Vec<(u16, u16, usize)> = one
        .segments
        .iter()
        .map(|s| (s.flag, s.color, s.points.len()))
        .collect();
    assert_eq!(runs, [(0, 0, 2), (0, 1, 1)]);
    assert_eq!(one.segments[0].points[1], (50, 0));
    assert_eq!(one.color_log, [(0, 0), (1, 1)]);
    assert_eq!(objects.unparsed, None);
}

#[test]
fn pes_unknown_object_classes_are_reported_not_fatal() {
    let mut bytes = data("oracle_v1.pes");
    let at = bytes
        .windows(7)
        .position(|w| w == b"CEmbOne")
        .expect("CEmbOne");
    bytes[at..at + 7].copy_from_slice(b"CEmbRec");
    let objects = pes::read_objects(&bytes).expect("objects");
    assert_eq!(objects.unparsed.as_deref(), Some("CEmbRec"));
    assert!(pes::read(&bytes).is_ok());
}

#[test]
fn pes_version_is_recorded() {
    let pattern = pes::read(&data("oracle.pes")).expect("read");
    assert_eq!(
        pattern.metadata.get("pes.version").map(String::as_str),
        Some("0001")
    );
}

#[test]
fn distinct_colours_get_distinct_chart_entries() {
    let (red, also_red) = (Thread::rgb(0xE01010), Thread::rgb(0xEE1111));
    let idx = pec::pec_color_indexes(&[red.clone(), also_red, red]);
    assert_ne!(idx[0], idx[1]);
    assert_eq!(idx[0], idx[2]);
    assert_eq!(
        PEC_CHART[idx[0]].as_ref().and_then(|t| t.name.as_deref()),
        Some("Red")
    );
}

#[test]
fn pec_long_deltas_round_trip() {
    let mut p = with_threads(&[0]);
    p.stitch_to(0.0, 0.0);
    p.stitch_to(63.0, -64.0); // first values needing the long form
    p.stitch_to(2063.0, -2111.0); // forces interpolation past ±2047
    p.push(Command::Trim);
    p.jump_to(100.0, 100.0);
    p.stitch_to(101.0, 100.0);
    p.push(Command::End);
    let back = pes::read(&pes::write(&p).expect("write")).expect("read");
    let s = sewn(&back);
    assert!(s.contains(&[63.0, -64.0]) && s.contains(&[2063.0, -2111.0]));
    assert_eq!(s.last(), Some(&[101.0, 100.0]));
    assert_eq!(back.count(Command::Trim), 1);
}

#[test]
fn icon_frame_and_design_pixels() {
    let pts = [
        Stitch::new(0.0, 0.0, Command::Stitch),
        Stitch::new(100.0, 100.0, Command::Stitch),
    ];
    let icon = pec::render_icon(
        &pts,
        &Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        },
        5.0,
    );
    assert_eq!(icon.len(), 6 * 38);
    assert_ne!(icon, pec::blank_icon());
}

#[test]
fn blank_icon_matches_reference_template() {
    assert_eq!(common::data("pec_blank_icon.bin"), pec::blank_icon());
}

#[test]
fn non_ascii_name_keeps_pec_header_aligned() {
    let mut p = with_threads(&[0xFF0000]);
    p.set_name("Ünïcødé–label");
    p.stitch_to(0.0, 0.0);
    p.stitch_to(10.0, 10.0);
    p.push(Command::End);
    let data = pec::write(&p).expect("write");
    assert_eq!(&data[8 + 0x200..8 + 0x202], &[0, 0]);
    assert!(pec::read(&data).is_ok());
}

// MARK: EXP

#[test]
fn exp_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_exp_read.json");
    assert_eq!(
        exp::read(&data("oracle.exp")).expect("read").stitches,
        expected.stitches
    );
}

#[test]
fn exp_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.exp");
    assert_eq!(
        exp::write(&exp::read(&original).expect("read")).expect("write"),
        original
    );
}

fn long_move_pattern() -> StitchPattern {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.jump_to(400.0, -300.0);
    p.stitch_to(401.0, -300.0);
    p
}

#[test]
fn exp_long_moves_are_split_within_signed_byte_range() {
    let back = exp::read(&exp::write(&long_move_pattern()).expect("write")).expect("read");
    assert_eq!(sewn(&back).last(), Some(&[401.0, -300.0]));
    assert_eq!(back.count(Command::Jump), 4); // ceil(400 / 127)
}

// MARK: JEF

#[test]
fn jef_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_jef_read.json");
    let pattern = jef::read(&data("oracle.jef")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
}

#[test]
fn jef_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.jef");
    let options = jef::WriteOptions {
        date: Some(JEF_DATE.into()),
        ..Default::default()
    };
    assert_eq!(
        jef::write_with(&jef::read(&original).expect("read"), &options).expect("write"),
        original
    );
}

#[test]
fn jef_stop_is_encoded_as_colour_zero_and_read_back() {
    let mut p = with_threads(&[0xFF0000]);
    p.stitch_to(0.0, 0.0);
    p.stitch_to(10.0, 10.0);
    p.push(Command::Stop);
    p.stitch_to(20.0, 0.0);
    p.push(Command::End);
    let data = jef::write_with(
        &p,
        &jef::WriteOptions {
            date: Some(JEF_DATE.into()),
            ..Default::default()
        },
    )
    .expect("write");
    assert!(jef::jef_palette(&p).contains(&0));
    let back = jef::read(&data).expect("read");
    assert_eq!(back.count(Command::Stop), 1);
    assert_eq!(back.count(Command::ColorChange), 0);
}

#[test]
fn jef_hoop_code_follows_design_size() {
    assert_eq!(jef::Hoop::fitting(400, 400), jef::Hoop::H50x50);
    assert_eq!(jef::Hoop::fitting(1000, 1000), jef::Hoop::H126x110);
    assert_eq!(jef::Hoop::fitting(1300, 1900), jef::Hoop::H140x200);
    assert_eq!(jef::Hoop::fitting(2500, 100), jef::Hoop::H110x110);
}

#[test]
fn jef_default_date_is_fourteen_digits_and_bad_dates_are_rejected() {
    let p = long_move_pattern();
    let data = jef::write(&p).expect("write");
    assert!(data[8..22].iter().all(u8::is_ascii_digit));
    let bad = jef::WriteOptions {
        date: Some("2026".into()),
        ..Default::default()
    };
    assert!(matches!(
        jef::write_with(&p, &bad),
        Err(FormatError::ValueOutOfRange(_))
    ));
}

#[test]
fn jef_command_trims_write_zero_jumps() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.push(Command::Trim);
    p.stitch_to(1.0, 0.0);
    let options = jef::WriteOptions {
        date: Some(JEF_DATE.into()),
        command_trims: true,
        trim_jumps: 2,
    };
    let data = jef::write_with(&p, &options).expect("write");
    let records = &data[0x74 + 8..];
    assert_eq!(
        records,
        [0, 0, 0x80, 0x02, 0, 0, 0x80, 0x02, 0, 0, 1, 0, 0x80, 0x10]
    );
}

// MARK: HUS

#[test]
fn hus_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_hus_read.json");
    let pattern = hus::read(&data("oracle.hus")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
}

#[test]
fn hus_written_by_openstitch_reads_back_identically_in_the_reference() {
    // openstitch.hus is this writer's output (LZ77 + Huffman streams); the oracle decoder
    // (pyembroidery's EmbCompress) read it into openstitch_hus_read.json.
    let written = hus::write(&hus::read(&data("oracle.hus")).expect("read")).expect("write");
    assert_golden("openstitch.hus", &written);
    let reference = stitches("openstitch_hus_read.json");
    let original = stitches("oracle_hus_read.json");
    assert_eq!(reference.stitches, original.stitches);
    assert_eq!(reference.threads, original.threads);
    assert_eq!(
        hus::read(&written).expect("reread").stitches,
        original.stitches
    );
}

#[test]
fn hus_streams_are_compressed() {
    let mut p = with_threads(&[0x000000]);
    for i in 0..5000 {
        p.stitch_to(f64::from(i % 2) * 40.0, f64::from(i / 2) * 3.0);
    }
    p.push(Command::End);
    let bytes = hus::write(&p).expect("write");
    assert!(bytes.len() < 3 * 5000 / 4, "{} bytes", bytes.len());
    assert_eq!(sewn(&hus::read(&bytes).expect("read")), sewn(&p));
}

#[test]
fn hus_long_moves_split_and_commands_round_trip() {
    let mut p = with_threads(&[0xFF0000, 0x00FF00]);
    p.stitch_to(0.0, 0.0);
    p.stitch_to(400.0, -300.0);
    p.push(Command::Trim);
    p.jump_to(-500.0, 200.0);
    p.stitch_to(-500.0, 200.0);
    p.push(Command::ColorChange);
    p.stitch_to(-480.0, 210.0);
    p.push(Command::End);
    let back = hus::read(&hus::write(&p).expect("write")).expect("read");
    assert_eq!(back.count(Command::Trim), 1);
    assert_eq!(back.count(Command::ColorChange), 1);
    assert_eq!(sewn(&back).last(), Some(&[-480.0, 210.0]));
    assert_eq!(hexes(&back.threads), ["#ff0000", "#00c600"]); // nearest HUS chart entries
}

// MARK: VIP

#[test]
fn vip_round_trips_rgb_colours_and_stitches() {
    let original = hus::read(&data("oracle.hus")).expect("read");
    let mut p = original.clone();
    p.threads = vec![Thread::rgb(0xE01010), Thread::rgb(0x1040C0)];
    let bytes = vip::write(&p).expect("write");
    assert_eq!(&bytes[..4], &vip::MAGIC.to_le_bytes());
    assert_golden("openstitch.vip", &bytes);
    let back = vip::read(&bytes).expect("read");
    assert_eq!(back.stitches, original.stitches);
    assert_eq!(hexes(&back.threads), ["#e01010", "#1040c0"]);
}

#[test]
fn vip_refuses_more_colours_than_its_key_covers() {
    let mut p = StitchPattern::new();
    for i in 0..=vip::MAX_COLORS {
        p.threads.push(Thread::rgb(i as u32));
        p.stitch_to(i as f64, 0.0);
        p.push(Command::ColorChange);
    }
    assert!(matches!(
        vip::write(&p),
        Err(FormatError::ValueOutOfRange(_))
    ));
}

#[test]
fn lzh_reads_libembroidery_style_literal_streams() {
    // pyembroidery's EmbCompress.compress / libembroidery's hus_compress: a 6-byte header
    // (little-endian size) and the bytes as 8-bit literal codes.
    let mut stored = vec![5, 0, 0x02, 0xA0, 0x01, 0xFE];
    stored.extend_from_slice(&[0x80, 0x80, 0x81, 0x84, 0x90]);
    assert_eq!(
        lzh::expand(&stored, 5).expect("expand"),
        [0x80, 0x80, 0x81, 0x84, 0x90]
    );
}

// MARK: SHV

#[test]
fn shv_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_shv_read.json");
    let pattern = shv::read(&data("oracle.shv")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
    assert_eq!(pattern.name(), Some("oracle"));
}

#[test]
fn shv_is_read_only() {
    assert!(!FileType::Shv.can_write());
    assert!(FileType::writable().all(|t| t != FileType::Shv));
    let p = shv::read(&data("oracle.shv")).expect("read");
    assert!(matches!(
        FileType::Shv.write(&p),
        Err(FormatError::UnsupportedFormat(_))
    ));
}

// MARK: PHC

#[test]
fn phc_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_phc_read.json");
    let pattern = phc::read(&data("oracle.phc")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
    assert!(!FileType::Phc.can_write());
    assert!(matches!(
        phc::read(b"#PHX0001\0\0"),
        Err(FormatError::BadSignature(_))
    ));
}

// MARK: PCS

#[test]
fn pcs_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_pcs_read.json");
    let pattern = pcs::read(&data("oracle.pcs")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
}

#[test]
fn pcs_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.pcs");
    assert_eq!(
        pcs::write(&pcs::read(&original).expect("read")).expect("write"),
        original
    );
}

#[test]
fn pcs_coordinates_are_stored_in_sixths_of_a_millimetre() {
    let mut p = with_threads(&[0x00FF00]);
    p.stitch_to(0.0, 0.0);
    p.stitch_to(7.0, -13.0); // 4.2, 7.8 file units
    p.jump_to(-20000.0, 30000.0);
    p.push(Command::End);
    let data = pcs::write(&p).expect("write");
    assert_eq!(data[1], pcs::Hoop::Large as u8);
    let back = pcs::read(&data).expect("read");
    let commands: Vec<Command> = back.stitches.iter().map(|s| s.command).collect();
    assert_eq!(
        commands,
        [
            Command::Stitch,
            Command::Stitch,
            Command::Jump,
            Command::End
        ]
    );
    assert_eq!(back.stitches[1].x, 4.0 * pcs::UNIT_SCALE);
    assert_eq!(back.stitches[1].y, -8.0 * pcs::UNIT_SCALE);
    assert_eq!(back.stitches[2].x, -12000.0 * pcs::UNIT_SCALE);
    assert_eq!(pcs::write(&back).expect("rewrite"), data);
}

#[test]
fn pcs_stop_repeats_the_thread_and_trims_are_dropped() {
    let mut p = with_threads(&[0xFF0000, 0x0000FF]);
    p.stitch_to(0.0, 0.0);
    p.push(Command::Trim);
    p.push(Command::Stop);
    p.stitch_to(30.0, 0.0);
    p.push(Command::ColorChange);
    p.stitch_to(60.0, 0.0);
    p.push(Command::End);
    let back = pcs::read(&pcs::write(&p).expect("write")).expect("read");
    assert_eq!(
        back.threads
            .iter()
            .map(Thread::rgb_value)
            .collect::<Vec<_>>(),
        [0xFF0000, 0xFF0000, 0x0000FF]
    );
    assert_eq!(back.count(Command::ColorChange), 2);
    assert_eq!(back.count(Command::Trim), 0);
}

// MARK: SEW

#[test]
fn sew_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_sew_read.json");
    let pattern = sew::read(&data("oracle.sew")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
}

#[test]
fn sew_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.sew");
    assert_eq!(
        sew::write(&sew::read(&original).expect("read")).expect("write"),
        original
    );
}

#[test]
fn sew_palette_never_uses_the_unknown_entry() {
    assert_eq!(sew::sew_palette(&with_threads(&[0x000000])), [1]);
}

#[test]
fn sew_long_moves_are_split_within_signed_byte_range() {
    let back = sew::read(&sew::write(&long_move_pattern()).expect("write")).expect("read");
    assert_eq!(sewn(&back).last(), Some(&[401.0, -300.0]));
    assert_eq!(back.count(Command::Jump), 4); // ceil(400 / 127)
}

// MARK: XXX

#[test]
fn xxx_reads_oracle_file_like_reference() {
    let expected = stitches("oracle_xxx_read.json");
    let pattern = xxx::read(&data("oracle.xxx")).expect("read");
    assert_eq!(pattern.stitches, expected.stitches);
    assert_eq!(hexes(&pattern.threads), expected.threads);
}

#[test]
fn xxx_rewrite_is_byte_identical_to_reference() {
    let original = data("oracle.xxx");
    assert_eq!(
        xxx::write(&xxx::read(&original).expect("read")).expect("write"),
        original
    );
}

#[test]
fn xxx_end_offset_points_past_the_stitch_records() {
    let data = data("oracle.xxx");
    let end = u32::from_le_bytes(data[0xFC..0x100].try_into().expect("4 bytes")) as usize;
    assert_eq!(&data[end..end + 4], &[0x7F, 0x7F, 0x02, 0x14]);
}

#[test]
fn xxx_long_stitch_uses_sixteen_bit_form() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(124.0, 0.0); // exactly the split limit: one long-form record
    p.push(Command::End);
    let data = xxx::write(&p).expect("write");
    assert_eq!(&data[0x102..0x107], &[0x7D, 124, 0, 0, 0]);
    // The reference reader treats the long form as a move.
    let commands: Vec<Command> = xxx::read(&data)
        .expect("read")
        .stitches
        .iter()
        .map(|s| s.command)
        .collect();
    assert_eq!(commands, [Command::Stitch, Command::Jump, Command::End]);
}

// MARK: FileType

#[test]
fn file_type_lookup_by_extension_and_path() {
    for t in FileType::ALL {
        assert_eq!(FileType::from_extension(t.extension()), Some(t));
        assert_eq!(
            FileType::from_extension(&t.extension().to_uppercase()),
            Some(t)
        );
        assert_eq!(
            FileType::from_path(format!("dir/design.{}", t.extension().to_uppercase())),
            Some(t)
        );
    }
    assert_eq!(FileType::from_extension("svg"), None);
    assert_eq!(FileType::from_path("noext"), None);
}

#[test]
fn every_fixture_round_trips_byte_identically_through_file_type() {
    // oracle.hus holds pyembroidery's uncompressed literal streams; openstitch.hus is the same
    // design as OpenStitch compresses it. SHV is read only.
    for name in common::FIXTURE_FILES
        .into_iter()
        .filter(|&n| FileType::from_path(n).is_some_and(FileType::can_write) && n != "oracle.hus")
    {
        let t = FileType::from_path(name).expect("type");
        let original = data(name);
        let mut pattern = t.read(&original).expect("read");
        pattern.set_name("oracle");
        let written = match t {
            FileType::Jef => jef::write_with(
                &pattern,
                &jef::WriteOptions {
                    date: Some(JEF_DATE.into()),
                    ..Default::default()
                },
            ),
            // pyembroidery's `1t`; FileType writes version 6 with objects.
            FileType::Pes => {
                pes::write_with(&pattern, &pes::WriteOptions::parse("1t").expect("1t"))
            }
            _ => t.write(&pattern),
        }
        .expect("write");
        assert_eq!(written, original, "{name}");
    }
}
