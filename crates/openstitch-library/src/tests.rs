#![allow(clippy::unwrap_used)]

use crate::catalog::{self, ColorList, Setup, Style, Units};
use crate::convert::{self, ConvertOptions, Converted, Output};
use crate::info::{self, Kind};
use crate::ops::{self, Conflict, FolderTrash, Outcome, Transfer};
use crate::search::{self, Query, Range, name_matches};
use crate::{Index, LibraryError, Limits, scan, thumb};
use openstitch_core::{Command, StitchPattern, Thread};
use openstitch_formats::FileType;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh folder under the system temp directory, removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new() -> Temp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "openstitch-library-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A square of `size` mm in two colours, `rows` rows of stitches each.
fn square(size: f64, rows: usize) -> StitchPattern {
    let mut p = StitchPattern::new();
    p.threads = vec![
        Thread::rgb(0xd0_20_20).with_name("Red"),
        Thread::rgb(0x20_40_d0).with_name("Blue"),
    ];
    let s = size * 10.0;
    for colour in 0..2 {
        for r in 0..rows {
            let y = s * r as f64 / (rows.max(2) - 1) as f64;
            p.stitch_to(0.0, y);
            p.stitch_to(s, y);
        }
        if colour == 0 {
            p.push(Command::ColorChange);
        }
    }
    p.push(Command::End);
    p
}

fn write_design(path: &Path, pattern: &StitchPattern) {
    let ty = FileType::from_path(path).unwrap();
    std::fs::write(path, ty.write(pattern).unwrap()).unwrap();
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

// MARK: reading

#[test]
fn inspect_reads_size_stitches_and_colours() {
    let t = Temp::new();
    let p = t.path("rose.pes");
    write_design(&p, &square(40.0, 10));
    let (info, pattern) = info::inspect(&p, &Limits::default()).unwrap();
    assert_eq!(info.kind, Kind::Machine(FileType::Pes));
    assert!(pattern.is_some());
    let s = info.summary.unwrap();
    assert!((s.width_mm - 40.0).abs() < 0.2, "{}", s.width_mm);
    assert!((s.height_mm - 40.0).abs() < 0.2);
    assert_eq!(s.stitches, 40);
    assert_eq!(s.color_changes(), 2);
    assert_eq!(s.total_colors(), 2);
    assert!(s.sewing_minutes > 0.0);
    assert_eq!(info.bytes, std::fs::metadata(&p).unwrap().len());
}

#[test]
fn repeated_thread_counts_once_in_totals() {
    let mut p = square(10.0, 2);
    p.threads[1] = p.threads[0].clone();
    let s = info::Summary::of(&p);
    assert_eq!((s.color_changes(), s.total_colors()), (2, 1));
}

#[test]
fn projects_are_compiled() {
    let t = Temp::new();
    let mut session = openstitch_engine::Session::new(t.0.clone());
    session
        .execute(
            "shape.rectangle",
            Some(&serde_json::json!({"x": 10, "y": 10, "width": 30, "height": 20, "fill": "#ff0000"})),
        )
        .unwrap();
    let design = session.design().clone();
    let path = t.path("proj.osd");
    std::fs::write(&path, openstitch_design::project::encode(&design).unwrap()).unwrap();
    let (info, _) = info::inspect(&path, &Limits::default()).unwrap();
    assert_eq!(info.kind, Kind::Project);
    let s = info.summary.unwrap();
    assert!(s.stitches > 10);
    assert!((s.width_mm - 30.0).abs() < 2.0, "{}", s.width_mm);
}

#[test]
fn hostile_files_are_listed_with_an_error_and_never_panic() {
    let t = Temp::new();
    let limits = Limits::default();
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    for ty in FileType::ALL {
        for len in [0usize, 1, 7, 64, 513, 4096] {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect();
            let p = t.path(&format!("junk{len}.{}", ty.extension()));
            std::fs::write(&p, &bytes).unwrap();
            let (info, _) = info::inspect(&p, &limits).unwrap();
            assert_eq!(info.kind, Kind::Machine(ty));
            // Random bytes may decode to something; they must never panic or hang.
            let _ = info.summary;
        }
        // A real file cut short.
        let good = ty.can_write().then(|| ty.write(&square(20.0, 4)).unwrap());
        if let Some(good) = good {
            let p = t.path(&format!("cut.{}", ty.extension()));
            std::fs::write(&p, &good[..good.len() / 3]).unwrap();
            let _ = info::inspect(&p, &limits).unwrap();
        }
    }
    let p = t.path("bad.osd");
    std::fs::write(
        &p,
        b"{\"format\":\"openstitch.design\",\"version\":1,\"design\":7}",
    )
    .unwrap();
    assert!(info::inspect(&p, &limits).unwrap().0.summary.is_err());
}

#[test]
fn oversized_files_are_not_decoded() {
    let t = Temp::new();
    let p = t.path("big.dst");
    std::fs::write(&p, vec![0u8; 4096]).unwrap();
    let limits = Limits {
        max_file_bytes: 1024,
        ..Limits::default()
    };
    assert!(matches!(
        info::load_pattern(&p, &limits),
        Err(LibraryError::TooLarge { bytes: 4096, .. })
    ));
    let (info, _) = info::inspect(&p, &limits).unwrap();
    assert!(info.summary.unwrap_err().contains("limit"));
}

#[cfg(unix)]
#[test]
fn pipes_are_never_opened() {
    let t = Temp::new();
    let fifo = t.path("pipe.pes");
    let made = std::process::Command::new("mkfifo").arg(&fifo).status();
    if !made.is_ok_and(|s| s.success()) {
        return; // no mkfifo on this system
    }
    // Opening a FIFO for reading would block forever; inspect must refuse it at once.
    assert!(matches!(
        info::inspect(&fifo, &Limits::default()),
        Err(LibraryError::NotAFile(_))
    ));
    let listing = scan::list(&t.0, &Limits::default()).unwrap();
    assert!(listing.designs.is_empty());
}

#[test]
fn unsupported_extensions_are_refused() {
    let t = Temp::new();
    let p = t.path("notes.txt");
    std::fs::write(&p, b"hello").unwrap();
    assert!(matches!(
        info::inspect(&p, &Limits::default()),
        Err(LibraryError::Unsupported(_))
    ));
}

// MARK: scanning

#[test]
fn list_sorts_and_filters() {
    let t = Temp::new();
    for n in ["b.PES", "a.dst", "c.osd", "notes.txt", ".hidden.pes"] {
        std::fs::write(t.path(n), b"x").unwrap();
    }
    std::fs::create_dir(t.path("Zeta")).unwrap();
    std::fs::create_dir(t.path("alpha")).unwrap();
    std::fs::create_dir(t.path(".git")).unwrap();
    let l = scan::list(&t.0, &Limits::default()).unwrap();
    assert_eq!(names(&l.designs), ["a.dst", "b.PES", "c.osd"]);
    assert_eq!(names(&l.folders), ["alpha", "Zeta"]);
    assert!(!l.truncated);
}

#[cfg(unix)]
#[test]
fn list_follows_links_to_files_and_drops_broken_ones() {
    let t = Temp::new();
    std::fs::write(t.path("real.pes"), b"x").unwrap();
    symlink(&t.path("real.pes"), &t.path("link.pes"));
    symlink(&t.path("missing.pes"), &t.path("broken.pes"));
    let l = scan::list(&t.0, &Limits::default()).unwrap();
    assert_eq!(names(&l.designs), ["link.pes", "real.pes"]);
}

#[test]
fn huge_folders_are_cut_at_the_limit() {
    let t = Temp::new();
    for i in 0..1500 {
        std::fs::write(t.path(&format!("d{i:04}.dst")), b"").unwrap();
    }
    let limits = Limits {
        max_folder_entries: 1000,
        max_walk_files: 700,
        ..Limits::default()
    };
    let l = scan::list(&t.0, &limits).unwrap();
    assert!(l.truncated);
    assert_eq!(l.designs.len(), 1000);
    let w = scan::walk(&t.0, &limits);
    assert!(w.truncated);
    assert_eq!(w.designs.len(), 700);
    // Without limits everything is listed.
    assert_eq!(
        scan::list(&t.0, &Limits::default()).unwrap().designs.len(),
        1500
    );
}

#[cfg(unix)]
#[test]
fn walks_end_on_symlink_loops() {
    let t = Temp::new();
    std::fs::create_dir_all(t.path("a/b")).unwrap();
    std::fs::write(t.path("a/b/x.pes"), b"").unwrap();
    symlink(&t.0, &t.path("a/b/loop"));
    symlink(&t.path("a"), &t.path("a/b/up"));
    let w = scan::walk(&t.0, &Limits::default());
    assert_eq!(names(&w.designs), ["x.pes"]);
    assert_eq!(w.links_skipped, 2);
    assert!(!w.truncated);
    // A walk started inside the loop also ends.
    let w = scan::walk(&t.path("a/b/loop"), &Limits::default());
    assert_eq!(w.designs.len(), 1);
}

#[test]
fn walks_respect_depth_and_folder_limits() {
    let t = Temp::new();
    let mut dir = t.0.clone();
    for d in 0..6 {
        dir = dir.join(format!("level{d}"));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join(format!("f{d}.dst")), b"").unwrap();
    }
    let shallow = Limits {
        max_depth: 2,
        ..Limits::default()
    };
    let w = scan::walk(&t.0, &shallow);
    assert_eq!(names(&w.designs), ["f0.dst", "f1.dst"]);
    assert_eq!(w.too_deep, 1);
    let few = Limits {
        max_walk_folders: 3,
        ..Limits::default()
    };
    let w = scan::walk(&t.0, &few);
    assert!(w.truncated);
    assert_eq!(w.folders_visited, 3);
}

// MARK: index and search

#[test]
fn index_decodes_once_and_refreshes_changed_files() {
    let t = Temp::new();
    let p = t.path("a.dst");
    write_design(&p, &square(10.0, 2));
    let mut index = Index::default();
    let limits = Limits::default();
    let first = index.info(&p, &limits).unwrap();
    index.info(&p, &limits).unwrap();
    assert_eq!(index.decoded, 1);
    write_design(&p, &square(10.0, 8));
    let f = std::fs::File::options().write(true).open(&p).unwrap();
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap();
    let second = index.info(&p, &limits).unwrap();
    assert_eq!(index.decoded, 2);
    assert_ne!(
        first.summary.unwrap().stitches,
        second.summary.unwrap().stitches
    );
    index.forget(&t.0);
    assert!(index.is_empty());
}

#[test]
fn wildcards_match_whole_names() {
    assert!(name_matches("ro*", "Rose.pes"));
    assert!(name_matches("*.PES", "rose.pes"));
    assert!(name_matches("r?se.pes", "rose.pes"));
    assert!(!name_matches("r?se", "rose.pes"));
    assert!(name_matches("os", "Rose.pes"));
    assert!(!name_matches("tulip", "Rose.pes"));
    assert!(name_matches("*a*b*c*", "xxaxxbxxcxx.dst"));
    // Many stars on a long name stays fast.
    let long = "a".repeat(5000);
    assert!(!name_matches(&format!("{}b", "*a".repeat(50)), &long));
}

#[test]
fn search_combines_criteria_and_subfolders() {
    let t = Temp::new();
    std::fs::create_dir(t.path("sub")).unwrap();
    write_design(&t.path("small.pes"), &square(20.0, 4));
    write_design(&t.path("large.dst"), &square(90.0, 4));
    write_design(&t.path("sub/medium.pes"), &square(50.0, 60));
    let mut mono = square(30.0, 4);
    mono.threads[1] = mono.threads[0].clone();
    write_design(&t.path("sub/mono.jef"), &mono);
    let limits = Limits::default();
    let mut index = Index::default();
    let mut run = |q: &Query, sub: bool| {
        let mut n = names(
            &search::search(&t.0, q, sub, &limits, &mut index)
                .designs
                .iter()
                .map(|d| d.path.clone())
                .collect::<Vec<_>>(),
        );
        n.sort();
        n
    };
    assert_eq!(run(&Query::default(), false), ["large.dst", "small.pes"]);
    assert_eq!(run(&Query::default(), true).len(), 4);
    let q = Query {
        kinds: vec![Kind::Machine(FileType::Pes)],
        ..Query::default()
    };
    assert_eq!(run(&q, true), ["medium.pes", "small.pes"]);
    let q = Query {
        width_mm: Range::new(Some(25.0), Some(60.0)),
        ..Query::default()
    };
    assert_eq!(run(&q, true), ["medium.pes", "mono.jef"]);
    let q = Query {
        stitches: Range::new(Some(200), None),
        ..Query::default()
    };
    assert_eq!(run(&q, true), ["medium.pes"]);
    // DST stores no thread colours, so its blocks count as one colour.
    let q = Query {
        total_colors: Range::new(None, Some(1)),
        ..Query::default()
    };
    assert_eq!(run(&q, true), ["large.dst", "mono.jef"]);
    let q = Query {
        name: Some("m*".into()),
        height_mm: Range::new(Some(40.0), None),
        ..Query::default()
    };
    assert_eq!(run(&q, true), ["medium.pes"]);
}

#[test]
fn property_search_needs_every_word() {
    let mut p = square(10.0, 2);
    p.metadata
        .insert("keywords".into(), "Flower Spring red".into());
    let t = Temp::new();
    write_design(&t.path("f.pes"), &p);
    let info = Index::default()
        .info(&t.path("f.pes"), &Limits::default())
        .unwrap();
    let q = |words: &str| Query {
        properties: [("keywords".to_string(), words.to_string())].into(),
        ..Query::default()
    };
    if info
        .summary
        .as_ref()
        .unwrap()
        .properties
        .contains_key("keywords")
    {
        assert!(q("spring flower").matches(&info));
        assert!(!q("spring winter").matches(&info));
    }
    assert!(q("").matches(&info));
}

#[test]
fn unreadable_files_fail_summary_criteria_but_match_names() {
    let t = Temp::new();
    std::fs::write(t.path("junk.pes"), b"not a pes").unwrap();
    let info = Index::default()
        .info(&t.path("junk.pes"), &Limits::default())
        .unwrap();
    assert!(
        Query {
            name: Some("junk".into()),
            ..Query::default()
        }
        .matches(&info)
    );
    assert!(
        !Query {
            stitches: Range::new(Some(0), None),
            ..Query::default()
        }
        .matches(&info)
    );
}

// MARK: thumbnails

#[test]
fn thumbnails_draw_thread_colours_on_the_background() {
    let mut two = square(30.0, 12);
    // Shift the blue rows to the right so both colours show.
    let mut block = 0;
    for s in &mut two.stitches {
        if s.command == Command::ColorChange {
            block += 1;
        }
        if block == 1 {
            s.x += 400.0;
        }
    }
    let img = thumb::render(&two, 64, [255, 255, 255]);
    assert_eq!(
        (img.width, img.height, img.rgba.len()),
        (64, 64, 64 * 64 * 4)
    );
    let px: Vec<&[u8; 4]> = img.rgba.as_chunks::<4>().0.iter().collect();
    assert!(
        px.iter().any(|p| p[2] > 150 && p[0] < 100),
        "blue thread drawn"
    );
    assert!(
        px.iter().any(|p| p[0] > 150 && p[2] < 100),
        "red thread drawn"
    );
    assert_eq!(*px[0], [255, 255, 255, 255], "corner stays background");
    let empty = thumb::render(&StitchPattern::new(), 16, [10, 20, 30]);
    assert!(
        empty
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [10, 20, 30, 255])
    );
    let png = img.png();
    assert_eq!(&png[1..4], b"PNG");
    // Absurd sizes are clamped.
    assert_eq!(
        thumb::render(&square(1.0, 2), 1 << 20, [0; 3]).width,
        thumb::MAX_SIDE
    );
}

// MARK: organising

#[test]
fn names_are_validated() {
    for bad in [
        "", " ", ".", "..", "a/b", "a\\b", "a:b", "x*", "con", "LPT1.pes", "trail.", "nul\0",
    ] {
        assert!(ops::validate_name(bad).is_err(), "{bad:?}");
    }
    for good in ["Roses", "rose 2.pes", "Ünïcødé ✿", "a.b.c"] {
        assert!(ops::validate_name(good).is_ok(), "{good:?}");
    }
}

#[test]
fn folders_are_created_once() {
    let t = Temp::new();
    let p = ops::create_folder(&t.0, "Flowers").unwrap();
    assert!(p.is_dir());
    assert!(matches!(
        ops::create_folder(&t.0, "Flowers"),
        Err(LibraryError::Exists(_))
    ));
    assert!(ops::create_folder(&t.0, "../escape").is_err());
    assert!(!t.0.parent().unwrap().join("escape").exists());
}

#[test]
fn rename_keeps_the_extension_and_never_overwrites() {
    let t = Temp::new();
    std::fs::write(t.path("rose.pes"), b"1").unwrap();
    std::fs::write(t.path("tulip.pes"), b"2").unwrap();
    let p = ops::rename(&t.path("rose.pes"), "red rose").unwrap();
    assert_eq!(p, t.path("red rose.pes"));
    let p = ops::rename(&p, "red rose 2.pes").unwrap();
    assert_eq!(p, t.path("red rose 2.pes"));
    // A different extension is part of the name; the real one stays.
    let p = ops::rename(&p, "rose.dst").unwrap();
    assert_eq!(p, t.path("rose.dst.pes"));
    assert!(matches!(
        ops::rename(&p, "tulip"),
        Err(LibraryError::Exists(_))
    ));
    assert_eq!(std::fs::read(t.path("tulip.pes")).unwrap(), b"2");
    // Case-only change.
    let p = ops::rename(&t.path("tulip.pes"), "Tulip").unwrap();
    assert_eq!(
        names(&scan::list(&t.0, &Limits::default()).unwrap().designs),
        ["rose.dst.pes", "Tulip.pes"]
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"2");
    // Folders rename by their whole name.
    std::fs::create_dir(t.path("old")).unwrap();
    assert_eq!(
        ops::rename(&t.path("old"), "new.d").unwrap(),
        t.path("new.d")
    );
    assert!(ops::rename(&t.path("new.d"), "a/b").is_err());
}

#[test]
fn copy_and_move_never_overwrite() {
    let t = Temp::new();
    std::fs::create_dir(t.path("src")).unwrap();
    std::fs::create_dir(t.path("dst")).unwrap();
    std::fs::write(t.path("src/a.pes"), b"new").unwrap();
    std::fs::write(t.path("dst/a.pes"), b"old").unwrap();
    std::fs::write(t.path("src/b.pes"), b"b").unwrap();
    let limits = Limits::default();
    let files = [t.path("src/a.pes"), t.path("src/b.pes")];
    let r = ops::transfer(
        &files,
        &t.path("dst"),
        Transfer::Copy,
        Conflict::Skip,
        &limits,
    );
    assert!(matches!(r[0], Outcome::Skipped { .. }));
    assert!(r[1].is_done());
    assert_eq!(std::fs::read(t.path("dst/a.pes")).unwrap(), b"old");
    let r = ops::transfer(
        &files[..1],
        &t.path("dst"),
        Transfer::Copy,
        Conflict::KeepBoth,
        &limits,
    );
    assert!(r[0].is_done());
    assert_eq!(std::fs::read(t.path("dst/a (2).pes")).unwrap(), b"new");
    assert_eq!(std::fs::read(t.path("dst/a.pes")).unwrap(), b"old");
    // Copy into the same folder makes a numbered copy.
    let r = ops::transfer(
        &files[1..],
        &t.path("src"),
        Transfer::Copy,
        Conflict::KeepBoth,
        &limits,
    );
    assert!(r[0].is_done());
    assert!(t.path("src/b (2).pes").exists());
    // Move.
    let r = ops::transfer(
        &files[1..],
        &t.path("dst"),
        Transfer::Move,
        Conflict::KeepBoth,
        &limits,
    );
    assert!(r[0].is_done());
    assert!(!t.path("src/b.pes").exists());
    assert_eq!(std::fs::read(t.path("dst/b (2).pes")).unwrap(), b"b");
    // Moving into the folder it is in does nothing.
    let r = ops::transfer(
        &[t.path("dst/a.pes")],
        &t.path("dst"),
        Transfer::Move,
        Conflict::KeepBoth,
        &limits,
    );
    assert!(matches!(r[0], Outcome::Skipped { .. }));
    // Missing source and a file as destination fail cleanly.
    let r = ops::transfer(
        &[t.path("nope.pes")],
        &t.path("dst"),
        Transfer::Copy,
        Conflict::Skip,
        &limits,
    );
    assert!(matches!(r[0], Outcome::Failed { .. }));
    let r = ops::transfer(
        &[t.path("dst/a.pes")],
        &t.path("dst/a (2).pes"),
        Transfer::Copy,
        Conflict::Skip,
        &limits,
    );
    assert!(matches!(r[0], Outcome::Failed { .. }));
}

#[test]
fn folders_copy_recursively_but_not_into_themselves() {
    let t = Temp::new();
    std::fs::create_dir_all(t.path("lib/inner")).unwrap();
    std::fs::write(t.path("lib/inner/x.dst"), b"x").unwrap();
    std::fs::create_dir(t.path("out")).unwrap();
    let limits = Limits::default();
    let r = ops::transfer(
        &[t.path("lib")],
        &t.path("out"),
        Transfer::Copy,
        Conflict::Skip,
        &limits,
    );
    assert!(r[0].is_done());
    assert_eq!(std::fs::read(t.path("out/lib/inner/x.dst")).unwrap(), b"x");
    for mode in [Transfer::Copy, Transfer::Move] {
        let r = ops::transfer(
            &[t.path("lib")],
            &t.path("lib/inner"),
            mode,
            Conflict::KeepBoth,
            &limits,
        );
        assert!(
            matches!(
                &r[0],
                Outcome::Failed {
                    error: LibraryError::Refused(_),
                    ..
                }
            ),
            "{r:?}"
        );
    }
    // Copy limits.
    let tight = Limits {
        max_walk_files: 1,
        ..Limits::default()
    };
    let r = ops::transfer(
        &[t.path("lib")],
        &t.path("out"),
        Transfer::Copy,
        Conflict::KeepBoth,
        &tight,
    );
    assert!(matches!(r[0], Outcome::Failed { .. }));
}

#[cfg(unix)]
#[test]
fn folder_copies_skip_links() {
    let t = Temp::new();
    std::fs::create_dir(t.path("lib")).unwrap();
    std::fs::write(t.path("lib/a.pes"), b"a").unwrap();
    symlink(&t.path("lib"), &t.path("lib/loop"));
    symlink(Path::new("/etc/hosts"), &t.path("lib/hosts.pes"));
    std::fs::create_dir(t.path("out")).unwrap();
    let r = ops::transfer(
        &[t.path("lib")],
        &t.path("out"),
        Transfer::Copy,
        Conflict::Skip,
        &Limits::default(),
    );
    assert!(r[0].is_done());
    let l = scan::list(&t.path("out/lib"), &Limits::default()).unwrap();
    assert_eq!(names(&l.designs), ["a.pes"]);
    assert!(l.folders.is_empty());
}

#[test]
fn delete_goes_to_the_trash() {
    let t = Temp::new();
    std::fs::create_dir(t.path("trash")).unwrap();
    std::fs::write(t.path("a.pes"), b"a").unwrap();
    std::fs::create_dir(t.path("folder")).unwrap();
    let trash = FolderTrash(t.path("trash"));
    assert_eq!(
        ops::delete(&[t.path("a.pes"), t.path("folder")], &trash).unwrap(),
        2
    );
    assert!(!t.path("a.pes").exists());
    assert_eq!(std::fs::read(t.path("trash/a.pes")).unwrap(), b"a");
    assert!(t.path("trash/folder").is_dir());
    for bad in [
        PathBuf::from("/"),
        t.path("x/../a.pes"),
        PathBuf::from("relative.pes"),
    ] {
        assert!(
            ops::delete(std::slice::from_ref(&bad), &trash).is_err(),
            "{bad:?}"
        );
    }
    assert!(ops::delete(&[t.path("missing.pes")], &trash).is_err());
}

// MARK: conversion

#[test]
fn batch_conversion_writes_new_files_and_leaves_sources_alone() {
    let t = Temp::new();
    std::fs::create_dir(t.path("out")).unwrap();
    let sources = [
        t.path("a.pes"),
        t.path("b.jef"),
        t.path("c.dst"),
        t.path("junk.pes"),
    ];
    write_design(&sources[0], &square(20.0, 4));
    write_design(&sources[1], &square(30.0, 4));
    write_design(&sources[2], &square(10.0, 4));
    std::fs::write(&sources[3], [0x80u8; 3]).unwrap();
    let before: Vec<_> = sources.iter().map(|s| std::fs::read(s).unwrap()).collect();
    let limits = Limits::default();

    let same = convert::convert(
        &sources,
        &ConvertOptions::new(FileType::Dst, Output::SameFolder),
        &limits,
    );
    assert!(matches!(same[0], Converted::Written { .. }));
    assert!(matches!(same[1], Converted::Written { .. }));
    assert!(
        matches!(same[2], Converted::Skipped { .. }),
        "same format, same folder"
    );
    let a = info::load_pattern(&t.path("a.dst"), &limits).unwrap();
    // DST splits stitches longer than its 12.1 mm maximum.
    assert!(a.count(Command::Stitch) >= 16);

    let other = convert::convert(
        &sources,
        &ConvertOptions::new(FileType::Dst, Output::Folder(t.path("out"))),
        &limits,
    );
    assert!(
        matches!(other[2], Converted::Copied { .. }),
        "same format, other folder"
    );
    assert_eq!(std::fs::read(t.path("out/c.dst")).unwrap(), before[2]);
    // Converting again skips existing outputs, or numbers them when asked.
    let again = convert::convert(
        &sources[..1],
        &ConvertOptions::new(FileType::Dst, Output::Folder(t.path("out"))),
        &limits,
    );
    assert!(matches!(again[0], Converted::Skipped { .. }));
    let mut keep = ConvertOptions::new(FileType::Dst, Output::Folder(t.path("out")));
    keep.conflict = Conflict::KeepBoth;
    let again = convert::convert(&sources[..1], &keep, &limits);
    assert!(matches!(&again[0], Converted::Written { to, .. } if to.ends_with("a (2).dst")));
    // Hostile source fails; nothing half-written remains.
    assert!(matches!(other[3], Converted::Failed { .. }));
    assert!(!t.path("out/junk.dst").exists());
    // Sources unchanged.
    for (s, b) in sources.iter().zip(&before) {
        assert_eq!(&std::fs::read(s).unwrap(), b);
    }
}

#[test]
fn conversion_refuses_read_only_targets_and_missing_folders() {
    let t = Temp::new();
    write_design(&t.path("a.pes"), &square(20.0, 4));
    let limits = Limits::default();
    let r = convert::convert(
        &[t.path("a.pes")],
        &ConvertOptions::new(FileType::Csd, Output::SameFolder),
        &limits,
    );
    assert!(matches!(
        r[0],
        Converted::Failed {
            error: LibraryError::Refused(_),
            ..
        }
    ));
    let r = convert::convert(
        &[t.path("a.pes")],
        &ConvertOptions::new(FileType::Dst, Output::Folder(t.path("nope"))),
        &limits,
    );
    assert!(matches!(r[0], Converted::Failed { .. }));
}

#[test]
fn dst_trim_setting_is_applied() {
    let mut p = square(20.0, 4);
    p.stitches.pop(); // the End record: the trim goes before it
    p.push(Command::Trim);
    p.stitch_to(0.0, 0.0);
    p.push(Command::End);
    let sizes: Vec<usize> = [1usize, 5]
        .iter()
        .map(|&n| {
            let mut o = ConvertOptions::new(FileType::Dst, Output::SameFolder);
            o.dst_trim_jumps = n;
            convert::encode(&p, &o).unwrap().len()
        })
        .collect();
    // PE-DESIGN's layout: the opening zero-length jumps, the trimmed move and the jump home
    // are each padded to the setting's count, so four more 3-byte records in each of the three.
    assert_eq!(
        sizes[1] - sizes[0],
        3 * 4 * 3,
        "four more jump records in three runs"
    );
}

#[test]
fn write_file_overwrites_only_when_asked() {
    let t = Temp::new();
    let p = t.path("out.csv");
    convert::write_file(&p, b"one", false).unwrap();
    assert!(matches!(
        convert::write_file(&p, b"two", false),
        Err(LibraryError::Exists(_))
    ));
    assert_eq!(std::fs::read(&p).unwrap(), b"one");
    convert::write_file(&p, b"two", true).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"two");
    assert_eq!(
        std::fs::read_dir(&t.0).unwrap().count(),
        1,
        "no temporary file left"
    );
}

// MARK: catalogues

fn items(t: &Temp, n: usize) -> Vec<catalog::Item> {
    let mut paths = Vec::new();
    for i in 0..n {
        let p = t.path(&format!("design{i}.pes"));
        write_design(&p, &square(10.0 + i as f64, 4));
        paths.push(p);
    }
    let junk = t.path("broken.pes");
    std::fs::write(&junk, b"\x80").unwrap();
    paths.push(junk);
    catalog::load_items(&paths, &Limits::default())
}

#[test]
fn pdf_catalogue_is_well_formed_and_deterministic() {
    let t = Temp::new();
    let items = items(&t, 5);
    for (style, pages) in [
        (Style::One, 6),
        (Style::Four, 2),
        (Style::Twelve, 1),
        (Style::Instructions, 6),
    ] {
        let setup = Setup {
            style,
            title: "Spring (2026) \\ ✿ catalogue".into(),
            ..Setup::default()
        };
        let pdf = catalog::pdf(&items, &setup);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(&pdf);
        assert_eq!(text.matches("/Type /Page ").count(), pages, "{style:?}");
        assert_eq!(catalog::page_count(items.len(), &setup), pages);
        // Every xref offset points at its object.
        let xref = text.rfind("xref\n").unwrap();
        let mut lines = text[xref..].lines().skip(3);
        for id in 1.. {
            let Some(line) = lines.next().filter(|l| l.ends_with(" n ")) else {
                break;
            };
            let off: usize = line[..10].parse().unwrap();
            assert!(
                pdf[off..].starts_with(format!("{id} 0 obj").as_bytes()),
                "object {id}"
            );
        }
        assert!(text.contains("(Spring \\(2026\\) \\\\ ? catalogue)"));
        assert_eq!(pdf, catalog::pdf(&items, &setup));
    }
}

#[test]
fn png_pages_render_at_the_requested_resolution() {
    let t = Temp::new();
    let items = items(&t, 3);
    let setup = Setup {
        title: "Catalogue".into(),
        landscape: true,
        ..Setup::default()
    };
    let pages = catalog::png_pages(&items, &setup, 72.0);
    assert_eq!(pages.len(), 1);
    let decoder = png::Decoder::new(std::io::Cursor::new(&pages[0]));
    let reader = decoder.read_info().unwrap();
    let info = reader.info();
    assert_eq!((info.width, info.height), (842, 595));
}

#[test]
fn html_catalogue_writes_page_and_image_folder() {
    let t = Temp::new();
    let mut items = items(&t, 2);
    items[0].info.path = t.path("<img src=x onerror=alert(1)>.pes");
    let setup = Setup {
        title: "My <designs>".into(),
        color_list: ColorList::Totals,
        ..Setup::default()
    };
    let html = t.path("out/My catalogue.html");
    std::fs::create_dir(t.path("out")).unwrap();
    let written = catalog::write_html(&items, &setup, &html, false).unwrap();
    assert_eq!(written.len(), 3, "two images and the page");
    assert!(t.path("out/My catalogue/0001.png").is_file());
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(page.contains("<title>My &lt;designs&gt;</title>"));
    assert!(page.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!page.contains("<img src=x"));
    assert!(page.contains("src=\"My%20catalogue/0001.png\""));
    assert!(matches!(
        catalog::write_html(&items, &setup, &html, false),
        Err(LibraryError::Exists(_))
    ));
    catalog::write_html(&items, &setup, &html, true).unwrap();
}

#[test]
fn csv_quotes_cells_and_defuses_formulas() {
    let t = Temp::new();
    let mut items = items(&t, 1);
    items[0].info.path = t.path("=HYPERLINK(\"x\"),1.pes");
    let setup = Setup {
        units: Units::Inch,
        brand_numbers: true,
        ..Setup::default()
    };
    let csv = catalog::csv(&items, &setup);
    let mut lines = csv.split("\r\n");
    let header = lines.next().unwrap();
    assert!(
        header.starts_with("File name,Width (in),Height (in),Stitches,Color changes,Total colors")
    );
    let row = lines.next().unwrap();
    assert!(
        row.starts_with("\"'=HYPERLINK(\"\"x\"\"),1.pes\",0.39,0.39,16,2,2,"),
        "{row}"
    );
    let broken = lines.next().unwrap();
    assert!(broken.starts_with("broken.pes,,,,,,,PES,1,"), "{broken}");
    assert_eq!(lines.next(), Some(""));
}
