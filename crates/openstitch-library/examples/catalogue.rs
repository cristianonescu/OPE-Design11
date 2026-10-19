//! Writes a catalogue of every design under a folder:
//!
//! ```text
//! cargo run -p openstitch-library --example catalogue -- FOLDER OUT_DIR [1|4|12|guide]
//! ```
//! OUT_DIR gets `catalogue.pdf`, `catalogue-N.png` pages, `catalogue.html` (with its image
//! folder) and `catalogue.csv`; existing files there are replaced.

use openstitch_library::catalog::{self, Setup, Style};
use openstitch_library::{Limits, scan};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(folder), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: catalogue FOLDER OUT_DIR [1|4|12|guide]");
        std::process::exit(2);
    };
    let style = match args.next().as_deref() {
        Some("1") => Style::One,
        Some("12") => Style::Twelve,
        Some("guide") => Style::Instructions,
        _ => Style::Four,
    };
    let limits = Limits::default();
    let walk = scan::walk(&PathBuf::from(&folder), &limits);
    let items = catalog::load_items(&walk.designs, &limits);
    let setup = Setup {
        style,
        title: format!("Designs in {folder}"),
        ..Setup::default()
    };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("catalogue.pdf"), catalog::pdf(&items, &setup))?;
    for (i, page) in catalog::png_pages(&items, &setup, 110.0).iter().enumerate() {
        std::fs::write(out.join(format!("catalogue-{}.png", i + 1)), page)?;
    }
    catalog::write_html(&items, &setup, &out.join("catalogue.html"), true)?;
    std::fs::write(out.join("catalogue.csv"), catalog::csv(&items, &setup))?;
    println!(
        "{} designs, {} pages",
        items.len(),
        catalog::page_count(items.len(), &setup)
    );
    Ok(())
}
