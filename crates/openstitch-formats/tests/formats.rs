//! Codec tests: oracle round trips, structural checks and malformed input.

mod common;

use common::{assert_golden, data, hexes, stitches};
use openstitch_core::{Bounds, Command, PEC_CHART, Stitch, StitchPattern, Thread};
use openstitch_formats::{
    FileType, FormatError, dst, exp, hus, jef, lzh, pcs, pec, pes, phc, sew, shv, vip, xxx,
};

use std::path::PathBuf;

const JEF_DATE: &str = "20261018120000";

fn sewn(p: &StitchPattern) -> Vec<[f64; 2]> {
    p.stitches
        .iter()
        .filter(|s| s.command == Command::Stitch)
        .map(|s| [s.x, s.y])
        .collect()
}

/// Needle points relative to the first one: the writers centre a design on (0, 0), as
/// PE-DESIGN does, so a round trip keeps shape and order but not the absolute position.
fn relative(p: &StitchPattern) -> Vec<[f64; 2]> {
    let s = sewn(p);
    let Some(first) = s.first().copied() else {
        return s;
    };
    s.iter()
        .map(|q| [q[0] - first[0], q[1] - first[1]])
        .collect()
}

/// `(jump, dx, dy)` of every DST record after the header up to the end record, y down.
fn dst_records(data: &[u8]) -> Vec<(bool, i64, i64)> {
    data[dst::HEADER_SIZE..]
        .chunks(3)
        .take_while(|r| r[2] & 0xF3 != 0xF3)
        .map(|r| {
            let (dx, dy) = dst::decode(r[0], r[1], r[2]);
            (r[2] & 0x80 != 0, i64::from(dx), i64::from(dy))
        })
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
fn dst_long_stitches_are_written_as_jumps_and_one_stitch() {
    // PE-DESIGN keeps one needle penetration per stitch: the full 12.1 mm pieces are jumps.
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(500.0, -300.0);
    p.push(Command::End);
    let data = dst::write(&p).expect("write");
    let records = dst_records(&data);
    // Three zero-length jumps, the move to the first point (centred), its stitch, then the
    // long stitch: four jumps of 121 along x (y the proportional share of what is left) and
    // the remainder as a stitch.
    assert_eq!(&records[..3], &[(true, 0, 0); 3]);
    let first = records.iter().position(|r| !r.0).expect("a stitch");
    assert_eq!(records[first], (false, 0, 0));
    assert_eq!(
        &records[first + 1..first + 6],
        &[
            (true, 121, -72),
            (true, 121, -72),
            (true, 121, -73),
            (true, 121, -73),
            (false, 16, -10)
        ]
    );
    let back = dst::read(&data).expect("read");
    assert_eq!(relative(&back), [[0.0, 0.0], [500.0, -300.0]]);
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
        let records = dst_records(&data);
        // n zero-length jumps open the file; the trimmed 4 mm move is padded to n jump records
        // with zero-length jumps before its last record; so is the jump home at the end.
        assert_eq!(&records[..n], &vec![(true, 0, 0); n][..]);
        let mut runs = Vec::new();
        let mut run = 0;
        for r in &records {
            if r.0 {
                run += 1;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
        runs.push(run);
        assert_eq!(runs, [n + 1, n, n], "n = {n}");
        let back = dst::read_with(&data, n).expect("read");
        assert_eq!(back.count(Command::Trim), 2, "the move and the jump home");
        assert_eq!(relative(&back), [[0.0, 0.0], [10.0, 0.0], [50.0, 10.0]]);
    }
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
fn pec_writes_no_zero_length_record_after_a_jump() {
    // PE-DESIGN 11 files go straight from a jump record to the first stitch, also a diagonal
    // one (pyembroidery pads that with `00 00`).
    let mut p = StitchPattern::new();
    p.stitches.push(Stitch::new(100.0, 50.0, Command::Jump));
    p.stitch_to(103.0, 54.0);
    p.push(Command::End);
    let bytes = pec::write(&p).expect("write");
    let stitches = &bytes[8 + 512 + 16..];
    assert_eq!(&stitches[..6], [0x90, 0x64, 0x90, 0x32, 0x03, 0x04]);
}

#[test]
fn pes_design_property_is_windows_1252_with_crlf_line_breaks() {
    // PE-DESIGN 11.50 showed OpenStitch's UTF-8 author "Ana Pérez" as "Ana PÃ©rez" and the
    // comment "Line one\nline two" on one line (compat/reference/MANIFEST.json,
    // pes-design-properties): it reads these bytes as a Windows code page and breaks lines on
    // CR LF.
    let mut p = StitchPattern::new();
    p.metadata.insert("author".into(), "Ana Pérez".into());
    p.metadata
        .insert("comments".into(), "Line one\nline two €".into());
    p.metadata.insert("keywords".into(), "rosa, 薔薇".into());
    p.stitch_to(0.0, 0.0);
    p.stitch_to(10.0, 0.0);
    p.push(Command::End);
    let bytes = pes::write(&p).expect("write");
    let header = |s: &[u8]| {
        let mut v = vec![s.len() as u8];
        v.extend_from_slice(s);
        bytes.windows(v.len()).any(|w| w == v)
    };
    assert!(header(b"Ana P\xE9rez"));
    assert!(header(b"Line one\r\nline two \x80"));
    assert!(header(b"rosa, ??"), "outside Windows-1252: ?");

    let back = pes::read(&bytes).expect("read");
    let get = |k: &str| back.metadata.get(k).map(String::as_str);
    assert_eq!(get("author"), Some("Ana Pérez"));
    assert_eq!(get("comments"), Some("Line one\nline two €"));
}

#[test]
fn pes_design_property_matches_a_pe_design_11_authored_file() {
    // Typed into PE-DESIGN 11.50's Design Property on a Windows-1252 system and saved as PES
    // (compat/reference/MANIFEST.json, design-property-native.pes).
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../compat/reference/pes/design-property-native.pes"
    );
    let native = std::fs::read(path).expect("reference file");
    let back = pes::read(&native).expect("read");
    let get = |k: &str| back.metadata.get(k).map(String::as_str);
    assert_eq!(get("author"), Some("Ana Pérez"));
    assert_eq!(get("comments"), Some("Line one\nline two"));
    for s in [
        "Rose",
        "Flowers",
        "Ana Pérez",
        "rose, red, spring",
        "Line one\nline two",
    ] {
        let mut field = pes::encode_header_string(s);
        field.insert(0, field.len() as u8);
        assert!(
            native[..128].windows(field.len()).any(|w| w == field),
            "{s:?}"
        );
    }
}

#[test]
fn pes_header_strings_read_windows_1252_and_utf8() {
    assert_eq!(pes::decode_header_string(b"Ana P\xE9rez"), "Ana Pérez");
    assert_eq!(pes::decode_header_string(b"a\r\nb\x93c\x94"), "a\nb“c”");
    // Earlier OpenStitch versions and pyembroidery wrote UTF-8.
    assert_eq!(
        pes::decode_header_string("Ana Pérez".as_bytes()),
        "Ana Pérez"
    );
    assert_eq!(pes::encode_header_string("a\r\nb\nc"), b"a\r\nb\r\nc");
}

#[test]
fn pes_header_strings_stop_at_255_bytes_without_half_a_line_break() {
    let mut p = StitchPattern::new();
    p.metadata
        .insert("comments".into(), format!("{}\nx", "a".repeat(254)));
    p.stitch_to(0.0, 0.0);
    p.push(Command::End);
    let back = pes::read(&pes::write(&p).expect("write")).expect("read");
    assert_eq!(
        back.metadata.get("comments").map(String::as_str),
        Some("a".repeat(254).as_str())
    );
}

#[test]
fn pes_v6_stitch_group_matches_pe_design_and_keeps_the_trim() {
    // pe_design_v6_trims.pes: PE-DESIGN 11.50's Ver.6.0 re-save of the compat spec
    // pes-csewseg-trims (two Red lines, a trim between them). Its CSewSeg is stitch run, jump
    // (flag 1, CSewSeg has no trim type), stitch run, with no lead-in jump; the trim is the PEC
    // section's 0x20 flag.
    let pe = data("pe_design_v6_trims.pes");
    let pattern = pes::read(&pe).expect("read");
    assert_eq!(pattern.count(Command::Trim), 1);
    // Its thread table strings are MFC Unicode strings (`FF FEFF`, length, UTF-16LE).
    let t = &pattern.threads[0];
    assert_eq!(t.hex(), "#ed171f");
    assert_eq!(
        (
            t.name.as_deref(),
            t.brand.as_deref(),
            t.catalog_number.as_deref()
        ),
        (Some("Red"), Some("Brother"), Some("5"))
    );
    let ours = pes::write(&pattern).expect("write");
    assert_eq!(pes::read(&ours).expect("read").count(Command::Trim), 1);
    // Runs, colours and point offsets within each run (PE stores page coordinates, OpenStitch
    // coordinates relative to the group, so compare relative to the first point).
    let shape = |bytes: &[u8]| {
        let objects = pes::read_objects(bytes).expect("objects");
        assert_eq!(objects.objects.len(), 1);
        let segments = &objects.objects[0].segments;
        let (x0, y0) = segments[0].points[0];
        segments
            .iter()
            .map(|s| {
                let points: Vec<(i16, i16)> =
                    s.points.iter().map(|&(x, y)| (x - x0, y - y0)).collect();
                (s.flag, s.color, points)
            })
            .collect::<Vec<_>>()
    };
    let expected = shape(&pe);
    assert_eq!(expected.iter().map(|s| s.0).collect::<Vec<_>>(), [0, 1, 0]);
    assert_eq!(shape(&ours), expected);
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
fn pes_without_a_pec_section_is_an_error_not_an_empty_design() {
    // PE-DESIGN 11 saves a design that lies off the page with a PEC offset that points at
    // zeros (no `LA:` label); the stitches are then only in the object section.
    let mut data = b"#PES0110".to_vec();
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend(std::iter::repeat_n(0u8, 0x300));
    let err = pes::read(&data).expect_err("no PEC section");
    assert!(
        matches!(&err, FormatError::UnsupportedFormat(m) if m.contains("no PEC section")),
        "{err}"
    );
    // A stand-alone PEC without its label is refused the same way.
    let mut pec_data = b"#PEC0001".to_vec();
    pec_data.extend(std::iter::repeat_n(0u8, 0x300));
    assert!(pec::read(&pec_data).is_err());
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

fn long_move_pattern() -> StitchPattern {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.jump_to(400.0, -300.0);
    p.stitch_to(401.0, -300.0);
    p
}

#[test]
fn exp_long_moves_are_split_within_signed_byte_range() {
    let data = exp::write(&long_move_pattern()).expect("write");
    let back = exp::read(&data).expect("read");
    assert_eq!(relative(&back), [[0.0, 0.0], [401.0, -300.0]]);
    // The move to the first point (2 jumps), the 40 mm move (4 jumps, 127 along x with y
    // proportional) and the jump home (2 jumps), as `80 02` records; trims are inferred from
    // the runs longer than one record.
    assert_eq!(back.count(Command::Jump), 8);
    assert_eq!(back.count(Command::Trim), 2);
    assert_eq!(data.iter().filter(|&&b| b == 0x80).count(), 8);
}

// MARK: JEF

#[test]
fn jef_reads_oracle_file_like_reference() {
    // pyembroidery infers a trim from any jump over 3 mm; OpenStitch only from a jump run
    // longer than one record (PE-DESIGN writes a stitch over 12.7 mm as a jump and a stitch),
    // so the records are compared without either inference.
    let expected = stitches("oracle_jef_read.json");
    let pattern = jef::read_with(&data("oracle.jef"), None).expect("read");
    let without_trims: Vec<Stitch> = expected
        .stitches
        .into_iter()
        .filter(|s| s.command != Command::Trim)
        .collect();
    assert_eq!(pattern.stitches, without_trims);
    assert_eq!(hexes(&pattern.threads), expected.threads);
    assert_eq!(
        jef::read(&data("oracle.jef"))
            .expect("read")
            .count(Command::Trim),
        0
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
    // The opening zero-length jump, the first stitch (folded into the move to it), the two
    // zero-length jumps of the trim, the stitch, the end record and its padding.
    assert_eq!(
        records,
        [
            0x80, 0x02, 0, 0, 0, 0, 0x80, 0x02, 0, 0, 0x80, 0x02, 0, 0, 1, 0, 0x80, 0x10, 0, 0
        ]
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
fn hus_written_by_openstitch_reads_back_as_the_oracle_design() {
    // openstitch.hus is this writer's output (LZ77 + Huffman streams, PE-DESIGN's record
    // layout). An earlier version of it, with the same compressor, was decoded by pyembroidery's
    // EmbCompress to the oracle's stitches (openstitch_hus_read.json); the layout has changed
    // since, the compression (`lzh::tests`) has not.
    let original = hus::read(&data("oracle.hus")).expect("read");
    let written = hus::write(&original).expect("write");
    assert_golden("openstitch.hus", &written);
    let back = hus::read(&written).expect("reread");
    assert_eq!(relative(&back), relative(&original));
    assert_eq!(hexes(&back.threads), hexes(&original.threads));
    assert_eq!(back.count(Command::Trim), original.count(Command::Trim));
    assert_eq!(
        back.count(Command::ColorChange),
        original.count(Command::ColorChange)
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
    assert_eq!(relative(&hus::read(&bytes).expect("read")), relative(&p));
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
    // The trim is PE-DESIGN's zero-length jump after the move; the 50 mm stitch is four
    // stitches of up to 12 mm (HUS alone splits a stitch into stitches).
    assert_eq!(back.count(Command::Trim), 1);
    assert_eq!(back.count(Command::ColorChange), 1);
    assert_eq!(back.count(Command::Stitch), 2 + 4 + 1);
    assert_eq!(relative(&back).last(), Some(&[-480.0, 210.0]));
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
    assert_eq!(relative(&back), relative(&original));
    assert_eq!(back.count(Command::Trim), original.count(Command::Trim));
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
fn pcs_coordinates_are_stored_in_sixths_of_a_millimetre_with_a_fraction_byte() {
    let mut p = with_threads(&[0x00FF00]);
    p.stitch_to(0.0, 0.0);
    p.stitch_to(7.0, -13.0); // 4.2, 7.8 file units
    p.jump_to(-20000.0, 30000.0);
    p.push(Command::End);
    let data = pcs::write(&p).expect("write");
    assert_eq!(data[1], pcs::Hoop::Large as u8);
    let back = pcs::read(&data).expect("read");
    let commands: Vec<Command> = back.stitches.iter().map(|s| s.command).collect();
    // A colour record, the first stitch, the stitch, a jump from there and a jump to the
    // move's end, and the stitch at the move's end.
    assert_eq!(
        commands,
        [
            Command::Stitch,
            Command::Stitch,
            Command::Jump,
            Command::Jump,
            Command::Stitch,
            Command::End
        ]
    );
    // The 1/256 fraction byte keeps the 0.1 mm grid: positions come back within 0.01.
    for (a, b) in relative(&back)
        .iter()
        .zip([[0.0, 0.0], [7.0, -13.0], [-20000.0, 30000.0]])
    {
        assert!(
            (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01,
            "{a:?} vs {b:?}"
        );
    }
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
fn sew_palette_never_uses_the_unknown_entry() {
    assert_eq!(sew::sew_palette(&with_threads(&[0x000000])), [1]);
}

#[test]
fn sew_long_moves_are_split_within_signed_byte_range() {
    let back = sew::read(&sew::write(&long_move_pattern()).expect("write")).expect("read");
    assert_eq!(relative(&back), [[0.0, 0.0], [401.0, -300.0]]);
    // The 40 mm move folded into the stitch after it: three jumps (only the axes over 12.7 mm
    // move) and the stitch. Before it the way from PE-DESIGN's start point (`sew::START_POINT`,
    // 24.7 mm below the first needle point here): a zero-length jump, one jump and a stitch.
    assert_eq!(back.count(Command::Jump), 5);
    assert_eq!(back.stitches[0], Stitch::new(0.0, 0.0, Command::Jump));
    assert_eq!(back.stitches[2], Stitch::new(-41.0, 247.0, Command::Stitch));
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
fn xxx_end_offset_points_past_the_stitch_records() {
    let data = data("oracle.xxx");
    let end = u32::from_le_bytes(data[0xFC..0x100].try_into().expect("4 bytes")) as usize;
    assert_eq!(&data[end..end + 4], &[0x7F, 0x7F, 0x02, 0x14]);
}

#[test]
fn xxx_long_stitches_are_jumps_plus_a_stitch_and_trimmed_moves_use_the_sixteen_bit_form() {
    let mut p = StitchPattern::new();
    p.stitch_to(0.0, 0.0);
    p.stitch_to(124.0, 0.0); // over 12 mm: a 12 mm jump and a 0.4 mm stitch
    p.push(Command::End);
    let data = xxx::write(&p).expect("write");
    // `7F 01 00 00`, then the (trimmed) move to the first point as `7E` with 16-bit deltas
    // and its zero-length stitch, then the long stitch.
    assert_eq!(&data[0x100..0x104], &[0x7F, 0x01, 0, 0]);
    assert_eq!(&data[0x104..0x10B], &[0x7E, 0xC2, 0xFF, 0, 0, 0, 0]);
    assert_eq!(&data[0x10B..0x111], &[0x7F, 0x01, 120, 0, 4, 0]);
    let commands: Vec<Command> = xxx::read(&data)
        .expect("read")
        .stitches
        .iter()
        .map(|s| s.command)
        .collect();
    assert_eq!(
        commands,
        [
            Command::Jump,
            Command::Trim,
            Command::Jump,
            Command::Stitch,
            Command::Jump,
            Command::Stitch,
            Command::Jump,
            Command::End
        ]
    );
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
fn every_fixture_round_trips_through_file_type() {
    // Every writable format reads its oracle fixture, writes it in PE-DESIGN's layout and reads
    // that back to the same needle points, colour changes and threads. SHV is read only.
    for name in common::FIXTURE_FILES
        .into_iter()
        .filter(|&n| FileType::from_path(n).is_some_and(FileType::can_write))
    {
        let t = FileType::from_path(name).expect("type");
        let original = t.read(&data(name)).expect("read");
        let back = t.read(&t.write(&original).expect("write")).expect("reread");
        let mut points = relative(&back);
        if t == FileType::Sew {
            points.dedup(); // PE-DESIGN's zero-length stitch before a colour change
        }
        assert_eq!(points, relative(&original), "{name}");
        assert_eq!(
            back.count(Command::ColorChange),
            original.count(Command::ColorChange),
            "{name}"
        );
        // DST carries no colours; PCS writes PE-DESIGN's fixed 16-colour table.
        if !matches!(t, FileType::Dst | FileType::Pcs) {
            assert_eq!(hexes(&back.threads), hexes(&original.threads), "{name}");
        }
    }
}

// MARK: PE-DESIGN 11.50 layout

/// PE-DESIGN's export of `machine-formats.pes` (compat/reference/machine), or `None` outside
/// the repository checkout.
fn pe_design_export(ext: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../compat/reference/machine")
        .join(format!("machine-formats.{ext}"));
    std::fs::read(path).ok()
}

/// The design PE-DESIGN exported: two Red running lines with a trimmed move between them, a
/// colour change and a Black satin of 15 mm stitches (OpenStitch's own PES, 248 stitches).
fn machine_formats_design() -> StitchPattern {
    let p = pes::read(&data("machine-formats.pes")).expect("read");
    assert_eq!(p.count(Command::Stitch), 248);
    assert_eq!(p.count(Command::Trim), 1);
    assert_eq!(p.count(Command::ColorChange), 1);
    p
}

fn dst_header_lines(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(&data[..dst::HEADER_SIZE])
        .split('\r')
        .filter(|l| l.len() > 3 && !l.starts_with("LA:"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn dst_records_and_header_match_pe_design() {
    let Some(pe) = pe_design_export("dst") else {
        return;
    };
    let ours = dst::write(&machine_formats_design()).expect("write");
    assert_eq!(dst_header_lines(&ours), dst_header_lines(&pe));
    assert_eq!(ours[dst::HEADER_SIZE..], pe[dst::HEADER_SIZE..]);
}

#[test]
fn exp_matches_pe_design_byte_for_byte() {
    let Some(pe) = pe_design_export("exp") else {
        return;
    };
    assert_eq!(exp::write(&machine_formats_design()).expect("write"), pe);
}

#[test]
fn jef_records_and_header_match_pe_design_except_the_date() {
    let Some(pe) = pe_design_export("jef") else {
        return;
    };
    let ours = jef::write(&machine_formats_design()).expect("write");
    assert_eq!(ours.len(), pe.len());
    assert_eq!(ours[..8], pe[..8]);
    // The hoop block for 50 x 50 mm (0x44) is -1 in PE-DESIGN's file and 50 in OpenStitch's.
    assert_eq!(ours[0x16..0x44], pe[0x16..0x44]);
    assert_eq!(ours[0x54..], pe[0x54..]);
}

#[test]
fn sew_records_match_pe_design() {
    let Some(pe) = pe_design_export("sew") else {
        return;
    };
    let ours = sew::write(&machine_formats_design()).expect("write");
    assert_eq!(ours[..6], pe[..6], "colour count and chart indexes");
    // From PE-DESIGN's start point (`sew::START_POINT`) the first needle point is one stitch
    // record away; every record after it is the JEF stream.
    assert_eq!(ours[0x1D78..], pe[0x1D78..]);
}

/// A PE-DESIGN SEW reference beside `machine-formats.*`.
fn pe_design_sew_file(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../compat/reference/machine")
        .join(name);
    std::fs::read(path).ok()
}

#[test]
fn sew_stream_is_design_relative_and_starts_where_pe_design_does() {
    // PE-DESIGN's SEW export of machine-formats' design moved 10 mm right and down on the
    // page is byte-identical to the one in place; OpenStitch's is too, from the moved PES.
    let (Some(asis), Some(moved), Some(moved_pes)) = (
        pe_design_export("sew"),
        pe_design_sew_file("sew-a-moved10.sew"),
        pe_design_sew_file("sew-a-moved10.pes"),
    ) else {
        return;
    };
    assert_eq!(asis, moved, "PE-DESIGN's two exports");
    let p = pes::read(&moved_pes).expect("read");
    assert_eq!(p.count(Command::Stitch), 248);
    let ours = sew::write(&p).expect("write");
    assert_eq!(ours[0x1D78..], asis[0x1D78..]);
    // A 20 x 20 mm square PE-DESIGN drew at the page centre (filled, with a running stitch
    // outline): the same start point, 15.9 mm left of and 9.7 mm above the centre, reached in
    // a jump and a stitch. PE-DESIGN exports its own objects from their first needle point,
    // the end of the PEC's first jump (its tie-in: there, 0.3 mm back, there again), which the
    // PES reader does not take as a needle point, so PE-DESIGN's stream has one record more
    // (the 0.3 mm step back) and arrives 0.3 mm further; every record after that is the same.
    let (Some(pe), Some(square_pes)) = (
        pe_design_sew_file("sew-square-centre.sew"),
        pe_design_sew_file("sew-square-centre.pes"),
    ) else {
        return;
    };
    let p = pes::read(&square_pes).expect("read");
    assert_eq!(p.count(Command::Stitch), 599);
    let ours = sew::write(&p).expect("write");
    assert_eq!(ours[..4], pe[..4], "colour count and chart index");
    let end = |d: &[u8]| {
        d.windows(2)
            .position(|w| w == [0x80, 0x10])
            .expect("end record")
    };
    let (o, r) = (&ours[0x1D78..], &pe[0x1D78..]);
    let (o, r) = (&o[..end(o)], &r[..end(r)]);
    assert_eq!(o[..8], r[..8], "zero-length jump, then the 12.7 mm jump");
    assert_eq!(
        r[8..12],
        [32, (-70_i8) as u8, (-3_i8) as u8, 0],
        "PE-DESIGN: 500, then 497"
    );
    assert_eq!(o[8..10], [29, (-70_i8) as u8], "OpenStitch: 497");
    assert_eq!(o[10..], r[12..]);
}

#[test]
fn xxx_records_and_header_counts_match_pe_design() {
    let Some(pe) = pe_design_export("xxx") else {
        return;
    };
    let ours = xxx::write(&machine_formats_design()).expect("write");
    let end = |d: &[u8]| u32::from_le_bytes(d[0xFC..0x100].try_into().expect("4 bytes")) as usize;
    assert_eq!(end(&ours), end(&pe));
    assert_eq!(
        ours[0x17..0x39],
        pe[0x17..0x39],
        "counts, extents, end position"
    );
    assert_eq!(ours[0x100..end(&ours) + 6], pe[0x100..end(&pe) + 6]);
    // The colour list follows; PE-DESIGN's RGB for its chart colour 800 RED is E3 31 1F where
    // OpenStitch's PEC chart (pyembroidery's table) has ED 17 1F, so only Black is compared.
    assert_eq!(ours[end(&ours) + 10..end(&ours) + 14], [0, 0, 0, 0]);
    assert_eq!(pe[end(&pe) + 10..end(&pe) + 14], [0, 0, 0, 0]);
}

#[test]
fn pcs_matches_pe_design_byte_for_byte() {
    let Some(pe) = pe_design_export("pcs") else {
        return;
    };
    assert_eq!(pcs::write(&machine_formats_design()).expect("write"), pe);
}

/// The header up to the extents and the three decompressed streams of a HUS or VIP file.
fn hus_streams(data: &[u8]) -> (Vec<u8>, [Vec<u8>; 3]) {
    let u32le = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().expect("4 bytes")) as usize;
    let records = u32le(4);
    let offsets = [u32le(0x14), u32le(0x18), u32le(0x1C)];
    let streams = [
        lzh::expand(&data[offsets[0]..offsets[1]], records).expect("commands"),
        lzh::expand(&data[offsets[1]..offsets[2]], records).expect("x"),
        lzh::expand(&data[offsets[2]..], records).expect("y"),
    ];
    (data[..0x14].to_vec(), streams)
}

#[test]
fn hus_streams_and_header_match_pe_design() {
    let Some(pe) = pe_design_export("hus") else {
        return;
    };
    let ours = hus::write(&machine_formats_design()).expect("write");
    let (header_pe, streams_pe) = hus_streams(&pe);
    let (header_ours, streams_ours) = hus_streams(&ours);
    assert_eq!(
        header_ours, header_pe,
        "magic, record and colour counts, extents"
    );
    assert_eq!(streams_ours, streams_pe);
    assert_eq!(ours[0x2A..0x2E], pe[0x2A..0x2E], "HUS chart indexes");
}

#[test]
fn vip_streams_and_header_match_pe_design() {
    let Some(pe) = pe_design_export("vip") else {
        return;
    };
    let ours = vip::write(&machine_formats_design()).expect("write");
    let (header_pe, streams_pe) = hus_streams(&pe);
    let (header_ours, streams_ours) = hus_streams(&ours);
    assert_eq!(
        header_ours, header_pe,
        "magic, record and colour counts, extents"
    );
    assert_eq!(streams_ours, streams_pe);
    assert_eq!(
        ours[0x2A..0x2E],
        pe[0x2A..0x2E],
        "the word libembroidery calls the table length"
    );
    // The obfuscated colour table differs by the RGB of Red (see the XXX test).
    assert_eq!(
        ours[0x36..0x44],
        pe[0x36..0x44],
        "the words after the colour table"
    );
}

#[test]
fn pe_design_exports_read_back_with_one_colour_change_and_their_entry_stitches() {
    // Stitch counts differ by format because of how each marks the needle point a frame
    // move ends at: DST, EXP, JEF and VIP do not (the next stitch is the first penetration),
    // HUS, PCS and XXX add a zero-length stitch there, SEW one before the colour change, and
    // HUS splits the 15 mm satin stitches into two stitches.
    for (ext, stitches, trims) in [
        ("dst", 248, 2),
        ("exp", 248, 1),
        ("jef", 248, 1),
        ("sew", 249, 0),
        ("xxx", 248, 2),
        ("pcs", 251, 0),
        ("hus", 431, 1),
        ("vip", 248, 1),
    ] {
        let Some(pe) = pe_design_export(ext) else {
            return;
        };
        let p = FileType::from_extension(ext)
            .expect("type")
            .read(&pe)
            .expect("read");
        assert_eq!(p.count(Command::Stitch), stitches, "{ext}");
        assert_eq!(p.count(Command::ColorChange), 1, "{ext}");
        assert_eq!(p.count(Command::Trim), trims, "{ext}");
        let b = p.bounds();
        assert_eq!((b.width(), b.height()), (400.0, 400.0), "{ext}");
    }
}
