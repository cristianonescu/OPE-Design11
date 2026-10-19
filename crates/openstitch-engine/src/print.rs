//! `design.print`: printable PDF output (Actual Size, Reduced Size, positioning sheets, Print
//! and Stitch). Layout and the PDF writer are `openstitch-print`.

use crate::commands::{CommandSpec, Effect, ParamDoc};
use crate::params::{CommandResult, Params};
use crate::sandbox::Access;
use crate::session::EngineState;
use openstitch_print::{Options, PAPERS};
use serde_json::json;

const fn opt(name: &'static str, kind: &'static str, summary: &'static str) -> ParamDoc {
    ParamDoc {
        name,
        kind,
        required: false,
        summary,
    }
}

pub fn commands() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "design.print",
        summary: "Write a printable PDF: actual size (1:1, tiled over several sheets if needed, with a sewing information page), reduced (one page), positioning sheets (one per hooping) or Print and Stitch (background image and alignment sheet)",
        effect: Effect::File,
        params: vec![
            ParamDoc {
                name: "path",
                kind: "string",
                required: true,
                summary: "Output .pdf file",
            },
            opt(
                "mode",
                "actual|reduced|positioning|image",
                "What to print (default actual)",
            ),
            opt(
                "paper",
                "a4|letter|a3|legal",
                "Paper size (default a4); orientation is chosen to fit",
            ),
            opt(
                "sewingAreaBox",
                "bool",
                "Sewing area box and centre axes (default true)",
            ),
            opt(
                "centerMark",
                "bool",
                "Embroidery positioning mark at the centre (default true)",
            ),
            opt(
                "templateGrid",
                "bool",
                "10 mm template grid in green (default false)",
            ),
            opt(
                "mirror",
                "bool",
                "Print and Stitch: mirror the image for iron-on transfer sheets",
            ),
        ],
        run,
    }]
}

fn run(s: &mut EngineState, p: &Params) -> CommandResult {
    let path = s.resolve(&p.string("path", None)?, Access::Write, &p.command)?;
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(p.invalid("path", "must end in .pdf"));
    }
    let paper_name = p.string("paper", Some("a4"))?;
    let (_, pw, ph) = PAPERS
        .iter()
        .find(|(n, _, _)| *n == paper_name)
        .copied()
        .ok_or_else(|| p.invalid("paper", "use a4, letter, a3 or legal"))?;
    let opts = Options {
        paper: (pw, ph),
        sewing_area_box: p.bool("sewingAreaBox", Some(true))?,
        center_mark: p.bool("centerMark", Some(true))?,
        template_grid: p.bool("templateGrid", Some(false))?,
    };
    let mode = p.string("mode", Some("actual"))?;
    let d = &s.design;
    let pages = match mode.as_str() {
        "actual" => openstitch_print::actual_size(d, &opts),
        "reduced" => openstitch_print::reduced(d, &opts),
        "positioning" => {
            let plan = openstitch_design::sections::plan(d);
            if !plan.errors.is_empty() {
                return Err(p.failed(plan.errors.join("; ")));
            }
            openstitch_print::positioning_sheets(d, &opts)
        }
        "image" => {
            let mirror = p.bool("mirror", Some(false))?;
            let image = background_image(s, p)?;
            openstitch_print::print_and_stitch(d, image, mirror, &opts).map_err(|e| p.failed(e))?
        }
        _ => return Err(p.invalid("mode", "use actual, reduced, positioning or image")),
    };
    let bytes = openstitch_print::write(&pages, &d.name);
    std::fs::write(&path, &bytes).map_err(|e| p.failed(e.to_string()))?;
    Ok(json!({ "path": path.display().to_string(), "pages": pages.len(), "bytes": bytes.len() }))
}

#[cfg(feature = "digitize")]
fn background_image(
    s: &EngineState,
    p: &Params,
) -> Result<openstitch_print::Image, crate::CommandError> {
    let bg = s
        .design
        .background
        .as_ref()
        .ok_or_else(|| p.failed("the design has no background image (background.set)"))?;
    let path = s.resolve(&bg.path, Access::Read, &p.command)?;
    let meta = std::fs::metadata(&path)
        .map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    if meta.len() > 64 << 20 {
        return Err(p.failed("background image too large"));
    }
    let data =
        std::fs::read(&path).map_err(|_| p.failed(format!("cannot read {}", path.display())))?;
    let r = openstitch_digitize::Raster::decode(&data)
        .map_err(|e| p.failed(e.to_string()))?
        .with_adjust(&bg.adjust);
    let (w, h) = (r.width(), r.height());
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            // Transparent pixels print as paper white.
            let [cr, cg, cb, a] = r.pixel(x, y);
            let mix =
                |c: u8| ((u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a))) / 255) as u8;
            rgb.extend([mix(cr), mix(cg), mix(cb)]);
        }
    }
    Ok(openstitch_print::Image {
        width: w,
        height: h,
        rgb,
    })
}

#[cfg(not(feature = "digitize"))]
fn background_image(
    _: &EngineState,
    p: &Params,
) -> Result<openstitch_print::Image, crate::CommandError> {
    Err(p.failed("this build cannot decode images (digitize feature off)"))
}
