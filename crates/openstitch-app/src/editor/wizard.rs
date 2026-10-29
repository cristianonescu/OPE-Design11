//! Image to Embroidery wizard model: the conversion chosen in the dialog and its settings,
//! turned into one `digitize.*` command. The conversions are approximations of PE-DESIGN's
//! Auto Punch, Cross Stitch, Photo Stitch 1/2 and Design Center (see `openstitch-digitize`).

use crate::editor::CommandRequest;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    AutoPunch,
    CrossStitch,
    PhotoFill,
    PhotoLines,
    DesignCenter,
}

impl Method {
    pub const ALL: [Method; 5] = [
        Method::AutoPunch,
        Method::CrossStitch,
        Method::PhotoFill,
        Method::PhotoLines,
        Method::DesignCenter,
    ];

    /// Short name for command lines (`--convert`).
    pub fn from_key(key: &str) -> Option<Method> {
        Some(match key {
            "autopunch" => Method::AutoPunch,
            "cross" => Method::CrossStitch,
            "photofill" => Method::PhotoFill,
            "photolines" => Method::PhotoLines,
            "designcenter" => Method::DesignCenter,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Method::AutoPunch => "Auto Punch",
            Method::CrossStitch => "Cross Stitch",
            Method::PhotoFill => "Photo Stitch 1 (fill)",
            Method::PhotoLines => "Photo Stitch 2 (lines)",
            Method::DesignCenter => "Design Center (line drawing)",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Method::AutoPunch => "Colour areas become fill and satin shapes you can edit.",
            Method::CrossStitch => "One cross per grid cell, one stitch pattern per colour.",
            Method::PhotoFill => "Photos: tone becomes stitch coverage in fill rows.",
            Method::PhotoLines => "Photos: zigzag lines that swing wider where the image is dark.",
            Method::DesignCenter => {
                "Line drawings: lines become line shapes, enclosed areas regions."
            }
        }
    }
}

impl Method {
    /// PE-DESIGN 11.50's Max. Number of Colors default for the method's wizard (round-4
    /// captures): Auto Punch and Cross Stitch count the image's colours (0 = automatic), Photo
    /// Stitch 10.
    pub fn default_colors(self) -> u32 {
        match self {
            Method::PhotoFill | Method::PhotoLines => 10,
            _ => 0,
        }
    }
}

pub const LINE_SEWS: [&str; 4] = ["running", "triple", "zigzag", "none"];
pub const REGION_SEWS: [&str; 4] = ["auto", "fill", "satin", "none"];

/// Dialog state. Defaults match the engine commands' defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageWizard {
    pub open: bool,
    pub path: Option<PathBuf>,
    pub method: Method,
    /// `None` fits the image into 80 % of the Design Page.
    pub width_mm: Option<f64>,
    pub colors: u32,
    pub skip_background: bool,
    // Auto Punch
    pub min_area: f64,
    pub satin_max: f64,
    pub curves: bool,
    // Cross Stitch
    pub cell: f64,
    pub half_cross: bool,
    // Photo Stitch
    pub mono: bool,
    pub row_spacing: Option<f64>,
    // Design Center
    pub threshold: u32,
    pub line_sew: &'static str,
    pub region_sew: &'static str,
}

impl Default for ImageWizard {
    fn default() -> Self {
        ImageWizard {
            open: false,
            path: None,
            method: Method::AutoPunch,
            width_mm: None,
            colors: Method::AutoPunch.default_colors(),
            skip_background: true,
            min_area: 1.0,
            satin_max: 6.0,
            curves: true,
            cell: 2.5,
            half_cross: false,
            mono: false,
            row_spacing: None,
            threshold: 128,
            line_sew: "running",
            region_sew: "auto",
        }
    }
}

impl ImageWizard {
    /// The conversion command, or `None` until an image is chosen.
    pub fn request(&self) -> Option<CommandRequest> {
        let path = self.path.as_ref()?;
        let mut p = Map::new();
        p.insert("path".into(), json!(path.display().to_string()));
        if let Some(w) = self.width_mm {
            p.insert("width".into(), json!(w));
        }
        // 0 asks Auto Punch and Cross Stitch for the automatic count.
        let colors = json!(self.colors.min(64));
        let command = match self.method {
            Method::AutoPunch => {
                p.insert("colors".into(), colors);
                p.insert("skipBackground".into(), json!(self.skip_background));
                p.insert("minArea".into(), json!(self.min_area));
                p.insert("satinMaxWidth".into(), json!(self.satin_max));
                p.insert("curves".into(), json!(self.curves));
                "digitize.autoPunch"
            }
            Method::CrossStitch => {
                p.insert("colors".into(), colors);
                p.insert("skipBackground".into(), json!(self.skip_background));
                p.insert("cellSize".into(), json!(self.cell));
                p.insert(
                    "type".into(),
                    json!(if self.half_cross { "half" } else { "full" }),
                );
                "digitize.crossStitch"
            }
            Method::PhotoFill | Method::PhotoLines => {
                let lines = self.method == Method::PhotoLines;
                p.insert("style".into(), json!(if lines { "lines" } else { "fill" }));
                if self.mono {
                    p.insert("mode".into(), json!("mono"));
                } else {
                    p.insert("colors".into(), json!(self.colors.clamp(1, 32)));
                }
                if let Some(r) = self.row_spacing {
                    p.insert("rowSpacing".into(), json!(r));
                }
                "digitize.photoStitch"
            }
            Method::DesignCenter => {
                p.insert("threshold".into(), json!(self.threshold.min(255)));
                p.insert("lineSew".into(), json!(self.line_sew));
                p.insert("regionSew".into(), json!(self.region_sew));
                p.insert("curves".into(), json!(self.curves));
                "digitize.designCenter"
            }
        };
        Some(CommandRequest::new(command, Value::Object(p)))
    }

    /// One line summarising a conversion's result for the status bar.
    pub fn summary(&self, result: &Value) -> String {
        let n = CommandRequest::created_ids(result).len();
        let colors = result
            .get("colors")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        match self.method {
            Method::DesignCenter => format!(
                "{}: {} lines, {} regions → {n} objects",
                self.method.label(),
                result["lines"],
                result["regions"]
            ),
            _ => format!("{}: {n} objects in {colors} colours", self.method.label()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;

    /// A 60×40 PNG: red block, blue bar, black outline square, on white.
    fn sample(dir: &std::path::Path) -> PathBuf {
        let mut rgba = Vec::new();
        for y in 0..40u32 {
            for x in 0..60u32 {
                let inside = |x0, y0, x1, y1| x >= x0 && x < x1 && y >= y0 && y < y1;
                rgba.extend_from_slice(&if inside(5, 5, 25, 35) {
                    [220, 20, 30, 255]
                } else if inside(35, 10, 55, 14) {
                    [20, 40, 200, 255]
                } else if inside(35, 18, 55, 36) && !inside(38, 21, 52, 33) {
                    [0, 0, 0, 255]
                } else {
                    [255, 255, 255, 255]
                });
            }
        }
        let path = dir.join("wizard.png");
        std::fs::write(&path, crate::png::encode_rgba(60, 40, &rgba).expect("png")).expect("write");
        path
    }

    #[test]
    fn every_method_converts_in_one_command() {
        let dir = std::env::temp_dir().join(format!("openstitch-wizard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let mut w = ImageWizard::default();
        assert!(w.request().is_none(), "no image yet");
        w.path = Some(sample(&dir));
        w.width_mm = Some(60.0);
        for m in Method::ALL {
            for mono in [false, true] {
                w.method = m;
                w.mono = mono;
                let r = w.request().expect("request");
                let mut s = Session::new(&dir);
                let result = s
                    .execute(r.command, Some(&r.params))
                    .unwrap_or_else(|e| panic!("{m:?}: {e}"));
                let ids = CommandRequest::created_ids(&result);
                assert!(!ids.is_empty(), "{m:?}");
                assert_eq!(s.design().objects.len(), ids.len(), "{m:?}");
                assert!(w.summary(&result).starts_with(m.label()));
                s.execute("history.undo", None).expect("undo");
                assert!(s.design().objects.is_empty(), "{m:?} is one undo step");
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn requests_carry_the_settings() {
        let w = ImageWizard {
            path: Some(PathBuf::from("a.png")),
            method: Method::CrossStitch,
            colors: 5,
            cell: 3.0,
            half_cross: true,
            skip_background: false,
            ..Default::default()
        };
        let r = w.request().expect("request");
        assert_eq!(r.command, "digitize.crossStitch");
        assert_eq!(
            r.params,
            json!({"path": "a.png", "colors": 5, "cellSize": 3.0, "type": "half", "skipBackground": false})
        );
        let photo = ImageWizard {
            method: Method::PhotoLines,
            mono: true,
            row_spacing: Some(1.0),
            ..w
        };
        let r = photo.request().expect("request");
        assert_eq!(r.params["style"], "lines");
        assert_eq!(r.params["mode"], "mono");
        assert_eq!(r.params["rowSpacing"], 1.0);
        assert!(r.params.get("colors").is_none());
    }
}
