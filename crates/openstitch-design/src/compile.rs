use crate::generate::{self, Run};
use crate::geometry::{Contour, Vec2, v2};
use crate::lines;
use crate::model::{
    Design, DesignObject, LineSewing, LineType, ObjectKind, RegionType, StitchBlock,
};
use crate::patterns::{self, Pattern, PatternKind};
use crate::region;
use crate::route;
use crate::stamp;
use openstitch_core::{Command, Stitch, StitchPattern, Thread};

/// Turns an editable [`Design`] into a machine [`StitchPattern`].
///
/// Order: objects in list order; within a shape, underlay, region, then outline. A thread change
/// between elements is a colour change (after a trim); travel longer than `trim_distance` is a
/// trim + jump, shorter travel a jump, or a straight stitch inside one element below
/// `connect_distance`. All distances 0.1 mm.
#[derive(Clone, Copy, Debug)]
pub struct Compiler {
    pub trim_distance: f64,
    pub connect_distance: f64,
    pub max_stitch: f64,
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler {
            trim_distance: 30.0,
            connect_distance: 40.0,
            max_stitch: 120.0,
        }
    }
}

struct Element {
    thread: Thread,
    runs: Vec<Run>,
}

struct Writer {
    cfg: Compiler,
    out: StitchPattern,
    thread: Option<Thread>,
    position: Option<Vec2>,
    cut: bool,
    /// Long travel is trimmed (off for objects with jump trimming off on multi-needle machines).
    trim_jumps: bool,
}

impl Writer {
    fn push(&mut self, p: Vec2, c: Command) {
        self.out.stitches.push(Stitch::new(p.x, p.y, c));
    }

    fn stitch_line(&mut self, p: Vec2) {
        let from = self.position.unwrap_or(p);
        let n = ((from.distance(p) / self.cfg.max_stitch).ceil() as usize).max(1);
        for k in 1..=n {
            self.push(from.lerp(p, k as f64 / n as f64), Command::Stitch);
        }
        self.position = Some(p);
        self.cut = false;
    }

    fn travel(&mut self, p: Vec2, within_element: bool) {
        let Some(from) = self.position else {
            self.push(p, Command::Jump);
            self.position = Some(p);
            return;
        };
        let d = from.distance(p);
        if d == 0.0 {
            return;
        }
        if within_element && d <= self.cfg.connect_distance && !self.cut {
            self.stitch_line(p);
            return;
        }
        if d > self.cfg.trim_distance && !self.cut && self.trim_jumps {
            self.push(from, Command::Trim);
            self.cut = true;
        }
        self.push(p, Command::Jump);
        self.position = Some(p);
    }

    /// Feed (Manual Punch): the frame moves to `p` with a jump stitch; the thread is not cut.
    fn feed(&mut self, p: Vec2) {
        if self.position != Some(p) {
            self.push(p, Command::Jump);
            self.position = Some(p);
        }
    }

    fn sew(&mut self, element: Element) {
        let runs: Vec<Run> = element.runs.into_iter().filter(|r| !r.is_empty()).collect();
        if runs.is_empty() {
            return;
        }
        match &self.thread {
            Some(t) if *t != element.thread => {
                let p = self.position.unwrap_or_default();
                if !self.cut && self.position.is_some() {
                    self.push(p, Command::Trim);
                    self.cut = true;
                }
                self.push(p, Command::ColorChange);
                self.out.threads.push(element.thread.clone());
            }
            None => self.out.threads.push(element.thread.clone()),
            _ => {}
        }
        self.thread = Some(element.thread);
        for (i, run) in runs.iter().enumerate() {
            self.travel(run[0], i > 0);
            // A jump already ends on the run's first needle point: PE-DESIGN 11 never writes a
            // zero-length stitch there and drops one when it opens a file (compat pes-default-v6).
            let landed = self
                .out
                .stitches
                .last()
                .is_some_and(|s| s.command == Command::Jump && v2(s.x, s.y) == run[0]);
            if landed {
                self.cut = false;
            } else if self.position.is_none() || self.cut {
                self.push(run[0], Command::Stitch);
                self.position = Some(run[0]);
                self.cut = false;
            }
            for &p in &run[1..] {
                self.stitch_line(p);
            }
        }
    }
}

impl Compiler {
    /// The default compiler with the design's jump-stitch trimming setting applied.
    pub fn for_design(design: &Design) -> Self {
        let d = Compiler::default();
        Compiler {
            trim_distance: design.machine.trim_distance(d.trim_distance),
            ..d
        }
    }

    pub fn compile(&self, design: &Design) -> StitchPattern {
        let mut w = self.writer();
        w.out.set_name(design.name.clone());
        for (key, value) in design.properties.fields() {
            if !value.is_empty() {
                w.out.metadata.insert(key.into(), value.into());
            }
        }
        for object in design.objects.iter().filter(|o| o.visible) {
            sew_object(&mut w, object, design);
        }
        w.finish()
    }

    /// The stitches `design.objects[index]` alone generates, as it would be sewn inside the
    /// design (hole sewing still skips its paired inner shape), visible or not. Used to
    /// convert a shape or text into a stitch pattern.
    pub fn compile_object(&self, design: &Design, index: usize) -> StitchPattern {
        let mut w = self.writer();
        if let Some(object) = design.objects.get(index) {
            sew_object(&mut w, object, design);
        }
        w.finish()
    }

    fn writer(&self) -> Writer {
        Writer {
            cfg: *self,
            out: StitchPattern::new(),
            thread: None,
            position: None,
            cut: true,
            trim_jumps: true,
        }
    }
}

impl Writer {
    fn finish(mut self) -> StitchPattern {
        if let Some(p) = self.position {
            if !self.cut {
                self.push(p, Command::Trim);
            }
            self.push(p, Command::End);
        }
        self.out
    }
}

fn sew_object(w: &mut Writer, object: &DesignObject, design: &Design) {
    let multi = design.machine.machine == crate::machine::MachineType::MultiNeedle;
    w.trim_jumps = !(multi && object.trim_jumps == Some(false));
    match object.kind {
        ObjectKind::Shape | ObjectKind::Text => {
            let holes = holes_above(object, design);
            for e in elements(object, &holes, &design.patterns) {
                w.sew(e);
            }
            // Stitch glyphs of user-mapped text sew after the outlined glyphs.
            if let Some(block) = &object.stitch_block
                && object.kind == ObjectKind::Text
            {
                sew_block(w, block);
            }
        }
        ObjectKind::Stitches => {
            if let Some(block) = &object.stitch_block {
                sew_block(w, block);
            }
        }
    }
}

/// Sews fixed needle points: runs split at jumps and trims, one element per colour block.
fn sew_block(w: &mut Writer, block: &StitchBlock) {
    // A feed pattern is jumps only: sewn as the jumps themselves.
    if !block.stitches.is_empty() && block.stitches.iter().all(|s| s.command == Command::Jump) {
        for s in &block.stitches {
            w.feed(v2(s.x, s.y));
        }
        return;
    }
    let mut block_index = 0;
    let mut pending: Vec<Run> = vec![Vec::new()];
    let thread_for = |i: usize| {
        block
            .threads
            .get(i)
            .or(block.threads.last())
            .cloned()
            .unwrap_or(Thread::rgb(0))
    };
    // Where the last jump landed: the first needle point of the run after it (PE-DESIGN files
    // and compiled patterns have no zero-length stitch there).
    let mut landing: Option<Vec2> = None;
    for s in &block.stitches {
        match s.command {
            Command::Stitch => {
                let run = pending.last_mut().expect("non-empty");
                let p = v2(s.x, s.y);
                if let Some(l) = landing.take()
                    && run.is_empty()
                    && l != p
                {
                    run.push(l);
                }
                run.push(p);
            }
            Command::Jump | Command::Trim => {
                if pending.last().is_some_and(|r| !r.is_empty()) {
                    pending.push(Vec::new());
                }
                if s.command == Command::Jump {
                    landing = Some(v2(s.x, s.y));
                }
            }
            Command::ColorChange => {
                landing = None;
                w.sew(Element {
                    thread: thread_for(block_index),
                    runs: std::mem::replace(&mut pending, vec![Vec::new()]),
                });
                block_index += 1;
            }
            Command::Stop | Command::End => {}
        }
    }
    w.sew(Element {
        thread: thread_for(block_index),
        runs: pending,
    });
}

/// Closed outlines of the visible inner shape `object` is sewn around, when it has hole
/// sewing on.
fn holes_above(object: &DesignObject, design: &Design) -> Vec<Contour> {
    crate::holes::paired_inner(object, design)
        .filter(|o| o.visible)
        .and_then(crate::holes::region_contours)
        .unwrap_or_default()
}

fn elements(object: &DesignObject, holes: &[Contour], user: &[Pattern]) -> Vec<Element> {
    let mut out = Vec::new();
    if let (Some(region), true) = (&object.region, object.has_closed_contour()) {
        // A shape is one region; text is one region per glyph, sewn letter by letter.
        for group in object.glyph_groups() {
            let closed: Vec<Contour> = group.iter().filter(|c| c.closed).cloned().collect();
            if closed.is_empty() {
                continue;
            }
            let mut layers = region::layers_with(&closed, region, holes, user);
            if let Some(top) = layers.last_mut() {
                let runs = stamp::apply(std::mem::take(top), &region.stamps);
                *top = route::route_region(runs, &closed, region.entry, region.exit, 25.0);
            }
            // PE-DESIGN's fill under sewing runs straight on into the top stitching.
            if region.underlay
                && region.underlay_style == crate::model::UnderlayStyle::EdgeFill
                && let Some(path) = region.travel_path()
            {
                layers = vec![region::join_layers(layers, &closed, holes, path)];
            }
            if region.kind == RegionType::Satin {
                region::tie_and_lock(&mut layers);
            }
            for runs in layers {
                out.push(Element {
                    thread: region.thread.clone(),
                    runs,
                });
            }
        }
    }
    if let Some(line) = &object.line {
        out.extend(line_elements(object, line, user));
    }
    out
}

fn line_elements(object: &DesignObject, line: &LineSewing, user: &[Pattern]) -> Vec<Element> {
    let pitch = line.pitch * 10.0;
    // The line entry point (L1): closed outlines start there, open ones from the nearer end.
    let contours: Vec<Contour> = object
        .contours
        .iter()
        .map(|c| line.entry.map_or_else(|| c.clone(), |p| c.starting_at(p)))
        .collect();
    if line.kind == LineType::Cutting {
        let mut needles: [Vec<Run>; 4] = Default::default();
        for c in &contours {
            for (n, runs) in lines::cutting(&c.path(), c.closed, line.leave_uncut)
                .into_iter()
                .enumerate()
            {
                needles[n].extend(runs);
            }
        }
        return needles
            .into_iter()
            .enumerate()
            .map(|(n, runs)| Element {
                thread: cutwork_needle(&line.thread, n),
                runs,
            })
            .collect();
    }
    let runs = contours
        .iter()
        .map(|c| {
            let width = line.width * 10.0;
            let spacing = line.spacing_mm() * 10.0;
            match line.kind {
                LineType::Running => generate::running(c, pitch, line.run_times),
                LineType::Triple => generate::triple(c, pitch),
                LineType::Zigzag if line.half_stitch => {
                    lines::zigzag_half(&c.path(), width, 10.0 / line.density.max(0.1))
                }
                LineType::Zigzag => generate::zigzag(c, width, 10.0 / line.density.max(0.1)),
                LineType::Motif => {
                    let pattern =
                        patterns::resolve(user, PatternKind::Motif, line.motif.as_deref());
                    lines::motif(
                        &c.path(),
                        c.closed,
                        &lines::MotifLine {
                            pattern,
                            width,
                            height: line.height.unwrap_or(3.0) * 10.0,
                            gap: spacing,
                            pitch,
                            arrange: line.arrange,
                        },
                    )
                }
                LineType::Stem => lines::stem(
                    &c.path(),
                    width,
                    spacing,
                    // Stored counter-clockwise from the line on screen; the geometry's y points
                    // down.
                    180.0 - line.angle.unwrap_or(45.0),
                    line.single,
                ),
                LineType::Candlewick => lines::candlewick(
                    &c.path(),
                    width,
                    (line.density * 2.0).round().clamp(3.0, 16.0) as usize,
                    spacing,
                    pitch,
                ),
                LineType::EStitch | LineType::VStitch => {
                    let e = lines::StrokeLine {
                        interval: spacing,
                        stroke: width,
                        pitch,
                        run_times: line.run_times,
                        stroke_times: line.stroke_times.unwrap_or(1),
                        arrange: line.arrange,
                    };
                    if line.kind == LineType::EStitch {
                        lines::e_stitch(&c.path(), &e)
                    } else {
                        lines::v_stitch(&c.path(), &e)
                    }
                }
                LineType::Chain => lines::chain(
                    &c.path(),
                    width,
                    line.chain_shape,
                    line.run_times,
                    line.arrange,
                    pitch,
                ),
                LineType::Cutting => Vec::new(),
            }
        })
        .collect();
    vec![Element {
        thread: line.thread.clone(),
        runs,
    }]
}

/// Thread of cutwork needle `n` (0–3, blade at n × 45°): the line's colour, named after the
/// needle so each needle is its own colour block.
pub fn cutwork_needle(thread: &Thread, n: usize) -> Thread {
    let mut t = thread.clone();
    t.name = Some(format!("Cutwork needle {} ({}°)", n + 1, n * 45).into());
    t.catalog_number = None;
    t.brand = None;
    t
}
