//! Machine type, hoops and output settings (PE-DESIGN's Design Settings dialog), plus thread
//! length and sewing-time estimates.
//!
//! Hoop sizes come only from Brother's public pages; each entry names its source. Which hoop a
//! given machine model accepts is not published in one place, so every hoop is offered for both
//! machine types (PE-DESIGN warns the user not to pick a hoop the machine cannot use).

use openstitch_core::{Command, StitchPattern};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MachineType {
    /// Single-needle home embroidery machine.
    #[default]
    SingleNeedle,
    /// Multi-needle machine (PE-DESIGN's multi-needle icon); enables jump-stitch trimming
    /// settings.
    MultiNeedle,
}

impl MachineType {
    pub fn as_str(self) -> &'static str {
        match self {
            MachineType::SingleNeedle => "single",
            MachineType::MultiNeedle => "multi",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "single" => Some(MachineType::SingleNeedle),
            "multi" => Some(MachineType::MultiNeedle),
            _ => None,
        }
    }

    /// Default speed for estimates, stitches per minute: a typical value, not a calibration.
    pub fn default_speed(self) -> f64 {
        match self {
            MachineType::SingleNeedle => 800.0,
            MachineType::MultiNeedle => 1000.0,
        }
    }

    /// Seconds a colour change adds to the estimate. A single-needle machine waits for the user
    /// to rethread, which is not sewing time, so it adds nothing.
    pub fn color_change_seconds(self) -> f64 {
        match self {
            MachineType::SingleNeedle => 0.0,
            MachineType::MultiNeedle => 5.0,
        }
    }
}

/// Where the needle starts and how large the exported design is (Design Settings ▸ Output).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SewingArea {
    /// The needle starts at the centre of the Design Page; the design is as large as the page.
    #[default]
    DesignPage,
    /// "Use existing design area": the needle starts at the centre of the stitches, which keep
    /// their real extent.
    DesignArea,
}

impl SewingArea {
    pub fn as_str(self) -> &'static str {
        match self {
            SewingArea::DesignPage => "page",
            SewingArea::DesignArea => "design",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "page" => Some(SewingArea::DesignPage),
            "design" => Some(SewingArea::DesignArea),
            _ => None,
        }
    }
}

/// A hoop from Brother's published sizes. Dimensions are the sewing field in millimetres,
/// width × height as PE-DESIGN lists them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hoop {
    pub id: &'static str,
    pub width: f64,
    pub height: f64,
    pub label: &'static str,
    /// Multi-position hoop (PE-DESIGN marks these with "*"); designs are split over three
    /// installation positions.
    pub multi_position: bool,
    pub source: &'static str,
}

const GUIDE: &str = "PE-DESIGN 11 guide: Specifying the Design Page Settings";
const UPDATES: &str = "PE-DESIGN 11 update history (support.brother.com ped11update_info)";

const fn hoop(
    id: &'static str,
    width: f64,
    height: f64,
    label: &'static str,
    source: &'static str,
) -> Hoop {
    Hoop {
        id,
        width,
        height,
        label,
        multi_position: false,
        source,
    }
}

/// Brother hoop sizes, smallest first. See `docs/research/hoops.md` for the sources.
pub const BROTHER_HOOPS: &[Hoop] = &[
    hoop(
        "100x100",
        100.0,
        100.0,
        "100 × 100 mm (4\" × 4\")",
        "PE-DESIGN 8 manual: Creating Design for Multi-Position Hoops",
    ),
    hoop(
        "130x180",
        130.0,
        180.0,
        "130 × 180 mm (5\" × 7\")",
        "PE-DESIGN 8 manual: Creating Design for Multi-Position Hoops",
    ),
    hoop("100x180", 100.0, 180.0, "100 × 180 mm (4\" × 7\")", UPDATES),
    hoop(
        "160x260",
        160.0,
        260.0,
        "160 × 260 mm (6\" × 10\")",
        "Brother SA441 6\" × 10\" embroidery hoop",
    ),
    hoop(
        "200x300",
        200.0,
        300.0,
        "200 × 300 mm (8\" × 12\")",
        "Brother SA447 / PRH300 8\" × 12\" frames",
    ),
    hoop(
        "254x254",
        254.0,
        254.0,
        "254 × 254 mm (10\" × 10\")",
        UPDATES,
    ),
    hoop(
        "272x272",
        272.0,
        272.0,
        "272 × 272 mm (10-5/8\" × 10-5/8\")",
        UPDATES,
    ),
    hoop(
        "180x360",
        180.0,
        360.0,
        "180 × 360 mm (7\" × 14\")",
        UPDATES,
    ),
    hoop(
        "200x360",
        200.0,
        360.0,
        "200 × 360 mm (8\" × 14\")",
        UPDATES,
    ),
    hoop(
        "240x360",
        240.0,
        360.0,
        "240 × 360 mm (9-1/2\" × 14\")",
        "Brother USA embroidery area listing (9-1/2\" × 14\")",
    ),
    hoop(
        "360x360",
        360.0,
        360.0,
        "360 × 360 mm (14\" × 14\", jumbo frame)",
        UPDATES,
    ),
    hoop(
        "297x465",
        297.0,
        465.0,
        "297 × 465 mm (11-5/8\" × 18-1/4\")",
        UPDATES,
    ),
    Hoop {
        id: "100x172-multi",
        width: 100.0,
        height: 172.0,
        label: "100 × 172 mm * (multi-position)",
        multi_position: true,
        source: GUIDE,
    },
    Hoop {
        id: "130x300-multi",
        width: 130.0,
        height: 300.0,
        label: "130 × 300 mm * (multi-position)",
        multi_position: true,
        source: GUIDE,
    },
];

pub fn brother_hoop(id: &str) -> Option<&'static Hoop> {
    BROTHER_HOOPS.iter().find(|h| h.id == id)
}

/// A hoop size the user added (Edit User Hoop). Millimetres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserHoop {
    pub id: u32,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

/// The selected hoop: a Brother hoop id, `user-<n>` for a user hoop, or `None` for a custom
/// Design Page size.
pub fn user_hoop_key(id: u32) -> String {
    format!("user-{id}")
}

/// Design Settings stored with a design. Everything defaults to what designs had before these
/// settings existed, and the field is left out of `.osd` files while it is at the default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineSettings {
    pub machine: MachineType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoop: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rotate90: bool,
    pub sewing_area: SewingArea,
    /// Multi-needle only: jumps at least this long (mm) are trimmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_jump_trim: Option<f64>,
    /// Stitches per minute for the sewing-time estimate; the machine type's default when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub user_hoops: Vec<UserHoop>,
    /// Split design: hoop sections of a custom Design Page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<crate::sections::SectionSettings>,
    /// Multi-position hoops: group patterns by installation position.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optimize_hoop_change: bool,
    /// Jumbo frame: overlap of Side 1 and Side 2 around the middle, mm.
    #[serde(skip_serializing_if = "is_zero")]
    pub jumbo_overlap: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl MachineSettings {
    pub fn is_default(&self) -> bool {
        *self == MachineSettings::default()
    }

    pub fn user_hoop(&self, key: &str) -> Option<&UserHoop> {
        let id: u32 = key.strip_prefix("user-")?.parse().ok()?;
        self.user_hoops.iter().find(|h| h.id == id)
    }

    pub fn next_user_hoop_id(&self) -> u32 {
        self.user_hoops.iter().map(|h| h.id + 1).max().unwrap_or(1)
    }

    /// Design Page width × height in mm for the selected hoop with rotation applied, if a hoop is selected
    /// and still exists.
    pub fn hoop_size(&self) -> Option<(f64, f64)> {
        let key = self.hoop.as_deref()?;
        let (w, h) = match (brother_hoop(key), self.user_hoop(key)) {
            // The jumbo frame's Design Page is smaller than the frame (guide: 350 × 350).
            (Some(b), _) if b.id == crate::sections::JUMBO => (
                crate::sections::JUMBO_PAGE_MM,
                crate::sections::JUMBO_PAGE_MM,
            ),
            (Some(b), _) => (b.width, b.height),
            (None, Some(u)) => (u.width, u.height),
            _ => return None,
        };
        Some(if self.rotate90 { (h, w) } else { (w, h) })
    }

    /// Multi-needle "minimum jump stitch length for trimming" (mm) when the design sets none.
    /// PE-DESIGN's own default is not observed yet; 3 mm is what OpenStitch sewed before.
    pub const DEFAULT_MIN_JUMP_TRIM: f64 = 3.0;

    /// Travel longer than this (0.1 mm) is trimmed by the compiler: on multi-needle machines
    /// the design's minimum jump length, else `fallback` (the compiler's single-needle rule).
    pub fn trim_distance(&self, fallback: f64) -> f64 {
        match (self.machine, self.min_jump_trim) {
            (MachineType::MultiNeedle, Some(mm)) => mm * 10.0,
            (MachineType::MultiNeedle, None) => Self::DEFAULT_MIN_JUMP_TRIM * 10.0,
            _ => fallback,
        }
    }

    pub fn speed(&self) -> f64 {
        self.speed.unwrap_or(self.machine.default_speed())
    }
}

/// Thread allowance per needle penetration (mm): the needle thread goes into the fabric and
/// back. A modelling constant, not calibrated against PE-DESIGN.
pub const PENETRATION_ALLOWANCE_MM: f64 = 1.0;
/// Needle thread left as tails at each trim (mm).
pub const TRIM_TAIL_MM: f64 = 10.0;
/// Bobbin thread as a fraction of the needle thread (the usual 1:3 rule of thumb).
pub const BOBBIN_RATIO: f64 = 1.0 / 3.0;
/// Seconds a thread trim takes.
pub const TRIM_SECONDS: f64 = 3.0;

/// Thread consumption and sewing time for a compiled design.
#[derive(Clone, Debug, PartialEq)]
pub struct Estimate {
    /// Needle thread per colour block, mm.
    pub needle_thread: Vec<f64>,
    pub bobbin_thread: f64,
    pub seconds: f64,
}

impl Estimate {
    pub fn needle_total(&self) -> f64 {
        self.needle_thread.iter().sum()
    }
}

/// Estimates thread length and sewing time. Needle thread is the sewn path plus
/// [`PENETRATION_ALLOWANCE_MM`] per stitch and [`TRIM_TAIL_MM`] per trim; time is every stitch and
/// jump at `settings.speed()`, plus [`TRIM_SECONDS`] per trim and the machine type's colour-change
/// time.
pub fn estimate(pattern: &StitchPattern, settings: &MachineSettings) -> Estimate {
    let mut needle = vec![0.0];
    let mut last: Option<(f64, f64)> = None;
    let (mut stitches, mut jumps, mut trims, mut changes) = (0usize, 0usize, 0usize, 0usize);
    for s in &pattern.stitches {
        let here = (s.x, s.y);
        match s.command {
            Command::Stitch => {
                stitches += 1;
                let run = last.map_or(0.0, |(x, y)| (s.x - x).hypot(s.y - y) / 10.0);
                *needle.last_mut().expect("non-empty") += run + PENETRATION_ALLOWANCE_MM;
            }
            Command::Jump => jumps += 1,
            Command::Trim => {
                trims += 1;
                *needle.last_mut().expect("non-empty") += TRIM_TAIL_MM;
            }
            Command::ColorChange => {
                changes += 1;
                needle.push(0.0);
            }
            Command::Stop | Command::End => {}
        }
        last = Some(here);
    }
    let total: f64 = needle.iter().sum();
    let seconds = (stitches + jumps) as f64 * 60.0 / settings.speed()
        + trims as f64 * TRIM_SECONDS
        + changes as f64 * settings.machine.color_change_seconds();
    Estimate {
        needle_thread: needle,
        bobbin_thread: total * BOBBIN_RATIO,
        seconds,
    }
}
