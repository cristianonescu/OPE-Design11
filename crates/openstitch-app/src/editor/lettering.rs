//! Lettering dialogs (Lettering menu): the forms behind them, turned into engine commands so
//! every action is one Undo. Fonts come from `fonts.list`, which includes the TrueType fonts
//! loaded into the design.

use crate::editor::CommandRequest;
use serde_json::{Value, json};
use std::path::Path;

/// One row of the font chooser.
#[derive(Clone, Debug, PartialEq)]
pub struct FontChoice {
    pub id: String,
    pub label: String,
}

/// Font choices from a `fonts.list` result: bundled fonts first, then the design's.
pub fn font_choices(list: &Value) -> Vec<FontChoice> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let id = f["id"].as_str()?.to_string();
            let family = f["family"].as_str().unwrap_or(&id);
            let style = f["style"].as_str().unwrap_or_default();
            let mut label = format!("{family} {style}").trim().to_string();
            if f["source"] == "design" {
                label.push_str(&format!(" ({id})"));
            }
            Some(FontChoice { id, label })
        })
        .collect()
}

/// Lettering ▸ Text… : new straight text at the centre of the Design Page.
#[derive(Clone, Debug, PartialEq)]
pub struct TextForm {
    pub open: bool,
    pub text: String,
    pub font: String,
    /// Capital height, mm.
    pub size: f64,
    pub align: &'static str,
}

impl Default for TextForm {
    fn default() -> Self {
        TextForm {
            open: false,
            text: "Text".into(),
            font: "lato-bold".into(),
            size: 10.0,
            align: "center",
        }
    }
}

impl TextForm {
    pub const ALIGNS: [&'static str; 3] = ["left", "center", "right"];

    pub fn request(&self) -> Option<CommandRequest> {
        (!self.text.trim().is_empty()).then(|| {
            CommandRequest::new(
                "text.add",
                json!({
                    "text": self.text, "font": self.font, "size": self.size,
                    "align": self.align,
                }),
            )
        })
    }
}

/// Lettering ▸ Load TrueType Font… : the font file the user picked.
pub fn load_font_request(path: &Path) -> CommandRequest {
    CommandRequest::new(
        "fonts.loadTrueType",
        json!({ "path": path.display().to_string() }),
    )
}

/// A font the design carries, from `fonts.list`.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignFontRow {
    pub id: String,
    pub label: String,
    pub embroidery: bool,
    pub glyphs: u64,
}

/// The design's own fonts in a `fonts.list` result.
pub fn design_fonts(list: &Value) -> Vec<DesignFontRow> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["source"] == "design")
        .filter_map(|f| {
            let id = f["id"].as_str()?.to_string();
            let embroidery = f["kind"] == "embroidery";
            let mut label = format!(
                "{} — {} glyphs",
                f["family"].as_str().unwrap_or(&id),
                f["glyphs"]
            );
            if let Some(h) = f["heightMM"].as_f64() {
                label.push_str(&format!(", {h} mm"));
            }
            Some(DesignFontRow {
                id,
                label,
                embroidery,
                glyphs: f["glyphs"].as_u64().unwrap_or(0),
            })
        })
        .collect()
}

/// Lettering ▸ Fonts… : the design's fonts; embroidery fonts for user-mapped text.
#[derive(Clone, Debug, PartialEq)]
pub struct FontsForm {
    pub open: bool,
    /// New embroidery font.
    pub new_id: String,
    pub new_height: f64,
    /// Embroidery font being edited.
    pub font: String,
    /// Character to register or edit.
    pub character: String,
    /// Font Creator: TrueType font and characters to convert.
    pub convert_from: String,
    pub convert_chars: String,
    /// Font Creator: where a character is drawn on the Design Page (left set line, baseline), mm.
    pub glyph_x: f64,
    pub glyph_y: f64,
}

impl Default for FontsForm {
    fn default() -> Self {
        FontsForm {
            open: false,
            new_id: "my-font".into(),
            new_height: 10.0,
            font: String::new(),
            character: "A".into(),
            convert_from: "lato-bold".into(),
            convert_chars: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .,!?&-"
                .into(),
            glyph_x: 20.0,
            glyph_y: 60.0,
        }
    }
}

impl FontsForm {
    pub fn create(&self) -> CommandRequest {
        CommandRequest::new(
            "fonts.create",
            json!({ "id": self.new_id.trim(), "height": self.new_height }),
        )
    }

    /// Registers design object `object` as the form's character.
    pub fn map_object(&self, object: u64) -> CommandRequest {
        CommandRequest::new(
            "fonts.mapCharacter",
            json!({ "font": self.font, "char": self.character, "object": object }),
        )
    }

    /// Registers stitch files named after their characters (u_A, l_a, …).
    pub fn map_files(&self, paths: &[std::path::PathBuf]) -> CommandRequest {
        let paths: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        CommandRequest::new(
            "fonts.mapFiles",
            json!({ "font": self.font, "paths": paths }),
        )
    }

    /// Font Creator: converts the form's characters of a TrueType font.
    pub fn convert(&self) -> CommandRequest {
        CommandRequest::new(
            "fonts.convertTrueType",
            json!({ "font": self.font, "from": self.convert_from, "chars": self.convert_chars }),
        )
    }

    /// Font Creator: puts the character's outline on the page as a shape to edit.
    pub fn edit_glyph(&self) -> CommandRequest {
        CommandRequest::new(
            "fonts.editGlyph",
            json!({
                "font": self.font, "char": self.character,
                "x": self.glyph_x, "y": self.glyph_y,
            }),
        )
    }

    /// Font Creator: stores the selected outlines as the character, measured from the same
    /// set line and baseline as [`FontsForm::edit_glyph`].
    pub fn set_glyph(&self, ids: &std::collections::BTreeSet<u64>) -> CommandRequest {
        CommandRequest::new(
            "fonts.setGlyph",
            json!({
                "font": self.font, "char": self.character, "ids": CommandRequest::ids(ids),
                "x": self.glyph_x, "y": self.glyph_y,
            }),
        )
    }

    pub fn save(&self, path: &Path) -> CommandRequest {
        CommandRequest::new(
            "fonts.save",
            json!({ "font": self.font, "path": path.display().to_string() }),
        )
    }

    pub fn open_file(path: &Path) -> CommandRequest {
        CommandRequest::new("fonts.open", json!({ "path": path.display().to_string() }))
    }

    pub fn remove(id: &str) -> CommandRequest {
        CommandRequest::new("fonts.remove", json!({ "id": id }))
    }
}

/// Lettering ▸ Monogram… : a monogram, or a decorative frame around the selection.
#[derive(Clone, Debug, PartialEq)]
pub struct MonogramForm {
    pub open: bool,
    pub letters: String,
    pub style: &'static str,
    /// `None` uses the style's font.
    pub font: Option<String>,
    /// Height, mm.
    pub size: f64,
    pub frame: Option<&'static str>,
    pub margin: f64,
    pub keep_aspect: bool,
}

impl Default for MonogramForm {
    fn default() -> Self {
        MonogramForm {
            open: false,
            letters: "ABC".into(),
            style: "block",
            font: None,
            size: 30.0,
            frame: Some("ring"),
            margin: 2.0,
            keep_aspect: true,
        }
    }
}

impl MonogramForm {
    pub const STYLES: [(&'static str, &'static str); 4] = [
        ("block", "Block"),
        ("script", "Script"),
        ("diamond", "Diamond"),
        ("circle", "Circle"),
    ];

    fn params(&self) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        m.insert("letters".into(), json!(self.letters));
        m.insert("style".into(), json!(self.style));
        if let Some(f) = &self.font {
            m.insert("font".into(), json!(f));
        }
        m.insert("size".into(), json!(self.size));
        m.insert("frame".into(), json!(self.frame));
        m.insert("margin".into(), json!(self.margin));
        m.insert("maintainAspect".into(), json!(self.keep_aspect));
        m
    }

    /// A new monogram at the page centre.
    pub fn add(&self) -> CommandRequest {
        CommandRequest::new("monogram.add", Value::Object(self.params()))
    }

    /// Rebuilds the monogram object `id` belongs to with the form's settings.
    pub fn edit(&self, id: u64) -> CommandRequest {
        let mut m = self.params();
        m.insert("id".into(), json!(id));
        CommandRequest::new("monogram.edit", Value::Object(m))
    }

    /// The form's frame around objects `ids` (Decorative Import).
    pub fn decorate(&self, ids: &std::collections::BTreeSet<u64>) -> Option<CommandRequest> {
        self.frame.map(|f| {
            CommandRequest::new(
                "decoration.add",
                json!({
                    "frame": f, "ids": CommandRequest::ids(ids), "margin": self.margin,
                    "maintainAspect": self.keep_aspect,
                }),
            )
        })
    }
}

/// Frame choices from a `monogram.list` result: (id, name).
pub fn frame_choices(list: &Value) -> Vec<(String, String)> {
    list["frames"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f["id"].as_str()?.into(), f["name"].as_str()?.into())))
        .collect()
}

/// Lettering ▸ Name Drop List… : the list (one row per design, one column per Name Drop
/// text) and its output.
#[derive(Clone, Debug, PartialEq)]
pub struct NameDropForm {
    pub open: bool,
    pub rows: Vec<Vec<String>>,
    pub selected: Option<usize>,
    /// Base name of the output files (`<name>001.pes`, …).
    pub name: String,
    pub format: &'static str,
    pub maintain_aspect: bool,
    pub align: &'static str,
    pub valign: &'static str,
}

impl Default for NameDropForm {
    fn default() -> Self {
        NameDropForm {
            open: false,
            rows: Vec::new(),
            selected: None,
            name: "name".into(),
            format: "pes",
            maintain_aspect: true,
            align: "center",
            valign: "middle",
        }
    }
}

impl NameDropForm {
    pub const MAX_ROWS: usize = 500;

    /// Turns the attribute on (with the form's fitting options) or off for objects `ids`.
    pub fn set_attribute(&self, ids: &std::collections::BTreeSet<u64>, on: bool) -> CommandRequest {
        CommandRequest::new(
            "text.setNameDrop",
            json!({
                "ids": CommandRequest::ids(ids), "enabled": on,
                "maintainAspect": self.maintain_aspect, "align": self.align,
                "valign": self.valign,
            }),
        )
    }

    /// Makes every row `columns` wide (new cells empty, extra cells dropped).
    pub fn fit_columns(&mut self, columns: usize) {
        for r in &mut self.rows {
            r.resize(columns, String::new());
        }
    }

    /// Adds a row at the end (the + button), up to 500.
    pub fn add_row(&mut self, columns: usize) {
        if self.rows.len() < Self::MAX_ROWS {
            self.rows.push(vec![String::new(); columns]);
            self.selected = Some(self.rows.len() - 1);
        }
    }

    /// Inserts a row above the selected one (Insert).
    pub fn insert_row(&mut self, columns: usize) {
        if self.rows.len() < Self::MAX_ROWS {
            let at = self.selected.unwrap_or(0).min(self.rows.len());
            self.rows.insert(at, vec![String::new(); columns]);
            self.selected = Some(at);
        }
    }

    /// Removes the selected row (Delete).
    pub fn delete_row(&mut self) {
        if let Some(i) = self.selected.filter(|&i| i < self.rows.len()) {
            self.rows.remove(i);
            self.selected = (!self.rows.is_empty()).then(|| i.min(self.rows.len() - 1));
        }
    }

    /// The selected row put into the design.
    pub fn apply(&self) -> Option<CommandRequest> {
        let row = self.rows.get(self.selected?)?;
        Some(CommandRequest::new(
            "nameDrop.apply",
            json!({ "values": row }),
        ))
    }

    /// To Folder: one file per row in `dir`.
    pub fn export(&self, dir: &Path) -> CommandRequest {
        CommandRequest::new(
            "nameDrop.export",
            json!({
                "rows": self.rows, "dir": dir.display().to_string(),
                "name": self.name, "format": self.format,
            }),
        )
    }

    pub fn read_csv(path: &Path) -> CommandRequest {
        CommandRequest::new(
            "nameDrop.readCsv",
            json!({ "path": path.display().to_string() }),
        )
    }

    pub fn write_csv(&self, path: &Path) -> CommandRequest {
        CommandRequest::new(
            "nameDrop.writeCsv",
            json!({ "path": path.display().to_string(), "rows": self.rows }),
        )
    }

    /// Rows from a `nameDrop.readCsv` result.
    pub fn set_rows(&mut self, result: &Value, columns: usize) {
        self.rows = result["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                r.as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| c.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .take(Self::MAX_ROWS)
            .collect();
        self.fit_columns(columns);
        self.selected = (!self.rows.is_empty()).then_some(0);
    }
}

/// A template the wizard offers (from `template.list`).
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
}

/// One editable text of the open template.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub label: String,
    pub text: String,
}

/// Template Wizard (PE-DESIGN Tutorial 2): 1 choose a template, 2 edit its texts, 3 output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateWizard {
    pub open: bool,
    pub step: u8,
    pub templates: Vec<TemplateRow>,
    pub chosen: Option<String>,
    /// The user's template folder (one subfolder per category).
    pub dir: Option<std::path::PathBuf>,
    pub slots: Vec<Slot>,
}

impl TemplateWizard {
    pub fn list(&self) -> CommandRequest {
        match &self.dir {
            Some(d) => {
                CommandRequest::new("template.list", json!({ "dir": d.display().to_string() }))
            }
            None => CommandRequest::new("template.list", json!({})),
        }
    }

    /// Takes a `template.list` result.
    pub fn set_templates(&mut self, result: &Value) {
        self.templates = result["templates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| {
                Some(TemplateRow {
                    id: t["id"].as_str()?.into(),
                    name: t["name"].as_str()?.into(),
                    category: t["category"].as_str()?.into(),
                    description: t["description"].as_str().unwrap_or_default().into(),
                })
            })
            .collect();
        self.templates
            .sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
    }

    /// Categories in order, each with its templates.
    pub fn categories(&self) -> Vec<(&str, Vec<&TemplateRow>)> {
        let mut out: Vec<(&str, Vec<&TemplateRow>)> = Vec::new();
        for t in &self.templates {
            match out.last_mut() {
                Some((c, v)) if *c == t.category => v.push(t),
                _ => out.push((&t.category, vec![t])),
            }
        }
        out
    }

    /// Step 1 → 2: opens the chosen template as the design.
    pub fn open_chosen(&self) -> Option<CommandRequest> {
        self.chosen
            .as_ref()
            .map(|id| CommandRequest::new("template.open", json!({ "template": id })))
    }

    /// Takes the slots of a `template.open` / `template.fill` result.
    pub fn set_slots(&mut self, result: &Value) {
        self.slots = result["slots"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| Slot {
                label: s["label"].as_str().unwrap_or_default().into(),
                text: s["text"].as_str().unwrap_or_default().into(),
            })
            .collect();
    }

    /// Step 2 → 3: the edited texts into the design.
    pub fn fill(&self) -> CommandRequest {
        let values: Vec<&str> = self.slots.iter().map(|s| s.text.as_str()).collect();
        CommandRequest::new("template.fill", json!({ "values": values }))
    }
}

/// Status line after `fonts.mapFiles`.
pub fn map_summary(result: &Value) -> String {
    let mapped: Vec<&str> = result["mapped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let skipped = result["skipped"].as_array().map_or(0, Vec::len);
    format!(
        "Registered {} character(s): {}; skipped {skipped} file(s)",
        mapped.len(),
        mapped.join(" ")
    )
}

/// Status line after text was added: glyph count, missing characters and shaping warnings.
pub fn text_summary(result: &Value) -> String {
    let mut s = format!("Added text ({} glyphs)", result["glyphs"]);
    let missing: String = result["missing"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !missing.is_empty() {
        s.push_str(&format!("; the font has no glyphs for {missing}"));
    }
    let shaping: Vec<&str> = result["shaping"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !shaping.is_empty() {
        s.push_str(&format!("; not shaped: {}", shaping.join(", ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use openstitch_engine::Session;

    fn run(s: &mut Session, r: &CommandRequest) -> Value {
        s.execute(r.command, Some(&r.params))
            .unwrap_or_else(|e| panic!("{}: {e}", r.command))
    }

    #[test]
    fn fonts_form_builds_a_mapped_font() {
        let mut s = Session::new(std::env::temp_dir());
        let mut form = FontsForm {
            new_id: "deco".into(),
            ..FontsForm::default()
        };
        run(&mut s, &form.create());
        let heart = run(
            &mut s,
            &CommandRequest::new(
                "shape.preset",
                json!({"kind": "heart", "cx": 50, "cy": 50, "width": 10, "fill": "Red"}),
            ),
        )["id"]
            .as_u64()
            .expect("id");
        form.font = "deco".into();
        form.character = "h".into();
        run(&mut s, &form.map_object(heart));
        let rows = design_fonts(&run(&mut s, &CommandRequest::new("fonts.list", json!({}))));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].embroidery && rows[0].glyphs == 1, "{rows:?}");
        assert!(rows[0].label.contains("10 mm"), "{rows:?}");
        let r = run(
            &mut s,
            &TextForm {
                text: "hh".into(),
                font: "deco".into(),
                ..TextForm::default()
            }
            .request()
            .expect("request"),
        );
        assert_eq!(r["glyphs"], 2);
        assert_eq!(
            map_summary(&json!({"mapped": ["A", "b"], "skipped": [{}]})),
            "Registered 2 character(s): A b; skipped 1 file(s)"
        );
    }

    #[test]
    fn font_creator_round_trip_through_shapes() {
        let mut s = Session::new(std::env::temp_dir());
        let mut form = FontsForm {
            new_id: "mine".into(),
            convert_chars: "Hi".into(),
            ..FontsForm::default()
        };
        run(&mut s, &form.create());
        form.font = "mine".into();
        assert_eq!(run(&mut s, &form.convert())["converted"], "Hi");
        form.character = "H".into();
        let shape = run(&mut s, &form.edit_glyph())["id"].as_u64().expect("id");
        let glyph = |s: &Session| match &s.design().fonts[0].data {
            openstitch_design::FontData::Embroidery(e) => e.glyph('H').cloned().expect("H"),
            _ => panic!("embroidery font"),
        };
        let before = glyph(&s);
        run(
            &mut s,
            &form.set_glyph(&std::collections::BTreeSet::from([shape])),
        );
        let after = glyph(&s);
        assert_eq!(after.advance, before.advance, "width kept");
        for (a, b) in after.contours[0]
            .points
            .iter()
            .zip(&before.contours[0].points)
        {
            assert!(
                a.distance(*b) < 1e-9,
                "unchanged shape stores the same outline"
            );
        }
    }

    #[test]
    fn monogram_form_adds_edits_and_decorates() {
        let mut s = Session::new(std::env::temp_dir());
        let list = run(&mut s, &CommandRequest::new("monogram.list", json!({})));
        assert_eq!(frame_choices(&list).len(), 8);
        let mut form = MonogramForm::default();
        let r = run(&mut s, &form.add());
        let first = CommandRequest::created_ids(&r);
        assert_eq!(first.len(), 4);
        form.style = "diamond";
        form.frame = None;
        form.letters = "xy".into();
        let r = run(&mut s, &form.edit(*first.iter().next().expect("id")));
        assert_eq!(r["letters"], "XY");
        let ids = CommandRequest::created_ids(&r);
        assert_eq!(ids.len(), 2);
        assert!(form.decorate(&ids).is_none(), "no frame chosen");
        form.frame = Some("stars");
        let r = run(&mut s, &form.decorate(&ids).expect("request"));
        assert_eq!(CommandRequest::created_ids(&r).len(), 12);
    }

    #[test]
    fn name_drop_list_edits_rows_and_exports() {
        let dir =
            std::env::temp_dir().join(format!("openstitch-app-namedrop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let mut s = Session::new(&dir);
        let id = run(
            &mut s,
            &CommandRequest::new("text.add", json!({"text": "Name"})),
        )["id"]
            .as_u64()
            .expect("id");
        let mut form = NameDropForm::default();
        run(
            &mut s,
            &form.set_attribute(&std::collections::BTreeSet::from([id]), true),
        );
        let cols = run(&mut s, &CommandRequest::new("nameDrop.list", json!({})));
        let n = cols.as_array().expect("cols").len();
        assert_eq!(n, 1);
        form.add_row(n);
        form.rows[0][0] = "Ann".into();
        form.add_row(n);
        form.rows[1][0] = "Bob".into();
        form.selected = Some(1);
        form.insert_row(n);
        form.rows[1][0] = "Ada".into();
        assert_eq!(form.rows, vec![vec!["Ann"], vec!["Ada"], vec!["Bob"]]);
        form.selected = Some(0);
        form.delete_row();
        assert_eq!(form.rows.len(), 2);
        run(&mut s, &form.apply().expect("row"));
        assert_eq!(s.design().objects[0].text.as_ref().expect("t").text, "Ada");
        let csv = dir.join("list.csv");
        run(&mut s, &form.write_csv(&csv));
        let mut back = NameDropForm::default();
        back.set_rows(&run(&mut s, &NameDropForm::read_csv(&csv)), n);
        assert_eq!(back.rows, form.rows);
        let out = dir.join("out");
        std::fs::create_dir_all(&out).expect("out");
        form.name = "kid".into();
        assert_eq!(run(&mut s, &form.export(&out))["count"], 2);
        assert!(out.join("kid002.pes").exists());
    }

    #[test]
    fn template_wizard_steps_through_the_engine() {
        let mut s = Session::new(std::env::temp_dir());
        let mut w = TemplateWizard::default();
        w.set_templates(&run(&mut s, &w.list()));
        let cats: Vec<&str> = w.categories().iter().map(|c| c.0).collect();
        assert_eq!(cats, ["Babies", "Badges", "Labels", "Monograms"]);
        w.chosen = Some("builtin:name-tag".into());
        w.set_slots(&run(&mut s, &w.open_chosen().expect("chosen")));
        assert_eq!(w.slots[0].label, "Name");
        w.slots[0].text = "Sam".into();
        w.set_slots(&run(&mut s, &w.fill()));
        assert_eq!(w.slots[0].text, "Sam");
        assert_eq!(w.slots[1].text, "Head Chef");
    }

    #[test]
    fn text_form_and_font_loading_go_through_the_engine() {
        let dir =
            std::env::temp_dir().join(format!("openstitch-app-lettering-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let font = dir.join("mine.ttf");
        std::fs::write(
            &font,
            include_bytes!("../../../openstitch-design/fonts/Pacifico-Regular.ttf"),
        )
        .expect("font");
        let mut s = Session::new(&dir);
        let loaded = run(&mut s, &load_font_request(&font));
        let choices = font_choices(&run(&mut s, &CommandRequest::new("fonts.list", json!({}))));
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[2].id, loaded["id"].as_str().expect("id"));
        assert!(choices[2].label.starts_with("Pacifico"));

        let mut form = TextForm {
            text: "Hi \u{05D0}".into(),
            font: choices[2].id.clone(),
            ..TextForm::default()
        };
        let r = run(&mut s, &form.request().expect("request"));
        assert_eq!(r["glyphs"], 2);
        let summary = text_summary(&r);
        assert!(summary.contains("no glyphs for \u{05D0}"), "{summary}");
        assert!(summary.contains("Hebrew"), "{summary}");
        form.text = "  ".into();
        assert!(form.request().is_none());
    }
}
