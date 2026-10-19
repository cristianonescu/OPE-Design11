//! Hostile-input tests: every reader must return `Ok` or `Err` — never panic, never hang —
//! for truncated, corrupted and random bytes.

mod common;

use common::{FIXTURE_FILES, data};
use openstitch_formats::{FileType, FormatError, jef, pes, sew, xxx};

/// Small deterministic xorshift PRNG, so failures reproduce.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }
}

fn read_all(bytes: &[u8]) {
    for t in FileType::ALL {
        // The result is irrelevant; reaching the next line without a panic is the test.
        let _ = t.read(bytes);
    }
}

#[test]
fn every_reader_survives_every_truncation_of_every_fixture() {
    for name in FIXTURE_FILES {
        let bytes = data(name);
        let own = FileType::from_path(name).expect("type");
        for len in 0..bytes.len() {
            read_all(&bytes[..len]);
            let _ = own.read(&bytes[..len]);
        }
    }
}

#[test]
fn readers_report_truncation_of_their_own_headers() {
    let cases: [(&str, usize); 10] = [
        ("oracle.pes", 0x16 + 0x200),
        ("oracle.pec", 0x200),
        ("oracle.jef", 0x74),
        ("oracle.pcs", 4),
        ("oracle.sew", 0x1D78),
        ("oracle.xxx", 0x100),
        ("openstitch.hus", 0x2A),
        ("openstitch.vip", 0x2E),
        ("oracle.phc", 0x4F),
        ("oracle.shv", 0x56 + 1 + 6 + 2 + 12 + 1 + 18 + 2 * 14 - 2),
    ];
    for (name, header) in cases {
        let bytes = data(name);
        let t = FileType::from_path(name).expect("type");
        for len in 0..header.min(bytes.len()) {
            assert!(
                t.read(&bytes[..len]).is_err(),
                "{name} cut to {len} bytes should fail"
            );
        }
    }
}

#[test]
fn every_reader_survives_random_bytes() {
    let mut rng = Rng(0x5EED_CAFE_F00D_D00D);
    for _ in 0..3000 {
        let len = rng.below(1200);
        read_all(&rng.bytes(len));
    }
    // Large inputs past the fixed-offset headers (SEW 0x1D78, PES/PEC 0x200).
    for _ in 0..40 {
        let len = 0x1D78 + rng.below(4000);
        read_all(&rng.bytes(len));
    }
}

#[test]
fn every_reader_survives_corrupted_fixtures() {
    let mut rng = Rng(0xBAD_C0DE);
    for name in FIXTURE_FILES {
        let original = data(name);
        let t = FileType::from_path(name).expect("type");
        for _ in 0..1500 {
            let mut bytes = original.clone();
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(bytes.len());
                bytes[i] = match rng.below(3) {
                    0 => bytes[i] ^ (1 << rng.below(8)),
                    1 => 0xFF,
                    _ => rng.next() as u8,
                };
            }
            let _ = t.read(&bytes);
        }
    }
}

#[test]
fn pes_rejects_pec_offsets_outside_the_file() {
    let mut bytes = data("oracle.pes");
    for offset in [0_u32, bytes.len() as u32, u32::MAX] {
        bytes[8..12].copy_from_slice(&offset.to_le_bytes());
        assert!(
            matches!(pes::read(&bytes), Err(FormatError::Truncated(_))),
            "offset {offset}"
        );
    }
}

#[test]
fn signatures_are_checked() {
    assert!(matches!(
        FileType::Pec.read(b"#PEC0002xxxxxxxx"),
        Err(FormatError::BadSignature(_))
    ));
    assert!(matches!(
        FileType::Pes.read(b"#PEX0001xxxxxxxx"),
        Err(FormatError::BadSignature(_))
    ));
}

#[test]
fn jef_rejects_oversized_colour_count_without_allocating() {
    let mut bytes = data("oracle.jef");
    bytes[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(jef::read(&bytes), Err(FormatError::Truncated(_))));
}

#[test]
fn jef_stitch_offset_past_the_end_reads_no_stitches() {
    let mut bytes = data("oracle.jef");
    bytes[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    let pattern = jef::read(&bytes).expect("read");
    assert_eq!(pattern.stitches.len(), 1); // just the end marker
}

#[test]
fn colour_counts_beyond_the_data_fail_cleanly() {
    let mut sew_bytes = data("oracle.sew");
    sew_bytes[0..2].copy_from_slice(&0xFFFF_u16.to_le_bytes());
    assert!(matches!(
        sew::read(&sew_bytes),
        Err(FormatError::Truncated(_))
    ));

    let mut xxx_bytes = data("oracle.xxx");
    xxx_bytes[0x27..0x29].copy_from_slice(&0xFFFF_u16.to_le_bytes());
    let pattern = xxx::read(&xxx_bytes).expect("read");
    assert!(pattern.threads.len() < 100);

    let mut pcs_bytes = data("oracle.pcs");
    pcs_bytes[2..4].copy_from_slice(&0xFFFF_u16.to_le_bytes());
    assert!(matches!(
        FileType::Pcs.read(&pcs_bytes),
        Err(FormatError::Truncated(_))
    ));
}

#[test]
fn pec_maximum_colour_count_is_bounded_by_the_header() {
    let mut bytes = data("oracle.pec");
    bytes[8 + 0x30] = 0xFF; // 256 colours: still inside the 512-byte header
    let _ = FileType::Pec.read(&bytes);
}

#[test]
fn hus_and_vip_reject_oversized_counts_and_bad_offsets() {
    for name in ["openstitch.hus", "openstitch.vip"] {
        let t = FileType::from_path(name).expect("type");
        let original = data(name);
        let mut bytes = original.clone();
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes()); // records
        assert!(matches!(
            t.read(&bytes),
            Err(FormatError::ValueOutOfRange(_))
        ));
        let mut bytes = original.clone();
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes()); // colours
        assert!(matches!(
            t.read(&bytes),
            Err(FormatError::ValueOutOfRange(_))
        ));
        let mut bytes = original.clone();
        bytes[0x1C..0x20].copy_from_slice(&u32::MAX.to_le_bytes()); // x stream offset
        assert!(matches!(t.read(&bytes), Err(FormatError::Truncated(_))));
        let mut bytes = original;
        bytes[0] ^= 1;
        assert!(matches!(t.read(&bytes), Err(FormatError::BadSignature(_))));
    }
}

#[test]
fn pes_object_sections_survive_truncation_and_corruption() {
    let mut rng = Rng(0x000B_1EC7);
    for name in ["oracle_v1.pes", "oracle_v6.pes", "oracle_v6t.pes"] {
        let original = data(name);
        for len in 0..original.len() {
            let _ = pes::read_objects(&original[..len]);
            let _ = pes::read(&original[..len]);
        }
        for _ in 0..2000 {
            let mut bytes = original.clone();
            for _ in 0..1 + rng.below(6) {
                let i = rng.below(bytes.len().min(0x200));
                bytes[i] = rng.next() as u8;
            }
            let _ = pes::read_objects(&bytes);
            let _ = pes::read(&bytes);
        }
    }
}
