use crate::generate::{self, Run};
use crate::geometry::{Contour, Vec2, v2};
use crate::lines;
use crate::model::{
    ChainShape, Design, DesignObject, LineSewing, LineType, ObjectKind, RegionType, StartEnd,
    StitchBlock,
};
use crate::patterns::{self, Pattern, PatternKind};
use crate::region;
use crate::regions;
use crate::route;
use crate::stamp;
use openstitch_core::{Command, Stitch, StitchPattern, Thread, round_coordinate};

/// Turns an editable [`Design`] into a machine [`StitchPattern`].
///
/// Order: objects in list order; within a shape, underlay, region, then outline. A thread change
/// between elements is a colour change (after a trim); travel longer than `trim_distance` is a
/// trim + jump, shorter travel a jump, or a straight stitch inside one element below
/// `connect_distance`. All distances 0.1 mm.
///
/// `trim_distance` is 0 by default: every frame move inside the design is trimmed, as in
/// PE-DESIGN 11.50's files. In 177 of its PES files (compat/reference and the owner's round-2
/// folders, 2026-10-27) every interior move carries the PEC trim flag and a plain jump appears only
/// as the first record and right after a colour change. A move between objects no longer than
/// `sew_across_distance` is not a jump at all but a stitch across the gap: PE-DESIGN sews a
/// 2.24 mm needle-to-needle move across and trims a 2.33 mm one (compat machine-trim-fine,
/// machine-trim-bound, machine-jumps, 2026-10-28), so the bound is 2.3 mm. Multi-needle
/// designs use PE-DESIGN's "minimum jump stitch length for trimming" setting for the trims
/// instead ([`Compiler::for_design`]).
#[derive(Clone, Copy, Debug)]
pub struct Compiler {
    pub trim_distance: f64,
    pub connect_distance: f64,
    pub max_stitch: f64,
    /// A move between objects up to this long (0.1 mm) is sewn as a stitch.
    pub sew_across_distance: f64,
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler {
            trim_distance: 0.0,
            connect_distance: 40.0,
            max_stitch: 120.0,
            sew_across_distance: 23.0,
        }
    }
}

struct Element {
    thread: Thread,
    runs: Vec<Run>,
    /// Stitches longer than `max_stitch` stay whole: satin, whose crossings PE-DESIGN 11
    /// writes as one stitch however wide the column (30 mm in region-travel-u-satin).
    whole: bool,
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
        // A stitch on the needle point just sewn would be a zero-length record: a second
        // penetration in the same hole. PE-DESIGN 11.50 writes none (no PEC record of length 0 in
        // any of its 177 files measured on 2026-10-27), so generators that end one run where the
        // next begins, or produce two points within 0.05 mm, sew one needle point. A zero-length
        // stitch right after a jump is kept: the jump's landing is not a penetration, so that
        // stitch is the first needle point of the block (PE-DESIGN ties in there instead).
        if c == Command::Stitch
            && let Some(last) = self.out.stitches.last()
            && last.command == Command::Stitch
            && round_coordinate(last.x) == round_coordinate(p.x)
            && round_coordinate(last.y) == round_coordinate(p.y)
        {
            return;
        }
        self.out.stitches.push(Stitch::new(p.x, p.y, c));
    }

    fn stitch_line(&mut self, p: Vec2) {
        self.stitch_split(p, self.cfg.max_stitch);
    }

    /// A stitch to `p`, split into equal stitches no longer than `max`.
    fn stitch_split(&mut self, p: Vec2, max: f64) {
        let from = self.position.unwrap_or(p);
        let n = ((from.distance(p) / max).ceil() as usize).max(1);
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
        // PE-DESIGN sews a short move between objects across as a stitch rather than jumping.
        if !within_element && d <= self.cfg.sew_across_distance && !self.cut {
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
        let max = if element.whole {
            f64::INFINITY
        } else {
            self.cfg.max_stitch
        };
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
                self.stitch_split(p, max);
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

/// Sews fixed needle points: runs split at jumps and trims, one element per colour block. The
/// points are the machine's, so a stitch longer than `max_stitch` stays one penetration, as
/// PE-DESIGN 11.50 exports an opened stitch file (compat machine-formats: its 15 mm satin
/// stitches are one jump and one stitch in every format, never two stitches).
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
                    whole: true,
                });
                block_index += 1;
            }
            Command::Stop | Command::End => {}
        }
    }
    w.sew(Element {
        thread: thread_for(block_index),
        runs: pending,
        whole: true,
    });
}

/// Closed outlines of the visible inner shape `object` is sewn around, when it has hole
/// sewing on. The inner shape stays a hole when its own region is Not Sewn, as in PE-DESIGN
/// 11.50 (references `concentric-ring-hole15`, `spiral-ring-hole15`: a 40 mm circle with a
/// 15 mm hole whose inner circle sews nothing).
fn holes_above(object: &DesignObject, design: &Design) -> Vec<Contour> {
    crate::holes::paired_inner(object, design)
        .filter(|o| o.visible && o.is_outline())
        .map(|o| o.contours.iter().filter(|c| c.closed).cloned().collect())
        .unwrap_or_default()
}

fn elements(object: &DesignObject, holes: &[Contour], user: &[Pattern]) -> Vec<Element> {
    let mut out = Vec::new();
    if let (Some(stored), true) = (&object.region, object.has_closed_contour()) {
        // The guide's exclusions: no gradation or feathered edge on text, no feathered edge
        // with hole sewing. Values a file holds anyway are kept, not sewn.
        let text = object.kind == crate::model::ObjectKind::Text;
        let mut sewn = stored.clone();
        if text {
            sewn.gradation = None;
        }
        if text || !holes.is_empty() {
            sewn.feather = None;
        }
        let region = &sewn;
        // A shape is one region; text is one region per glyph, sewn letter by letter.
        for group in object.glyph_groups() {
            let closed: Vec<Contour> = group.iter().filter(|c| c.closed).cloned().collect();
            if closed.is_empty() {
                continue;
            }
            let mut layers = region::layers_with(&closed, region, holes, user);
            if let Some(top) = layers.last_mut() {
                let runs = stamp::apply(std::mem::take(top), &region.stamps);
                // PE-DESIGN 11.50 sews Concentric Circle and Spiral in the order the Start
                // Point button gives, jumping in and ending where the pattern ends (references
                // concentric-*, spiral-*): no travel from R1 or to R2, no reversal.
                *top = if matches!(
                    region.kind,
                    RegionType::ConcentricCircle | RegionType::Spiral
                ) {
                    runs
                } else {
                    // Travel from the start point and to the end point: PE-DESIGN's motif fill
                    // walks it in steps of at most 2 mm (motif-fill-2pt).
                    let pitch = if region.kind == RegionType::MotifFill {
                        regions::MOTIF_FILL_TRAVEL_PITCH
                    } else {
                        25.0
                    };
                    route::route_region(runs, &closed, region.entry, region.exit, pitch)
                };
            }
            // PE-DESIGN's fill under sewing runs straight on into the top stitching.
            let parts = region.underlay_parts();
            if region.underlay
                && region.direction_lines.len() < 2
                && !region.legacy_underlay()
                && (parts.lattice
                    || (matches!(region.kind, RegionType::Fill | RegionType::ProgrammableFill)
                        && parts.any()))
                && let Some(path) = region.travel_path()
            {
                layers = vec![region::join_layers(layers, &closed, holes, path)];
            }
            // Satin, fill and motif fill tie in and lock as PE-DESIGN 11 sews them
            // (motif-fill-2pt: one 0.3 mm step out and back at the start and at the end).
            if matches!(
                region.kind,
                RegionType::Satin | RegionType::Fill | RegionType::MotifFill
            ) {
                region::tie_and_lock(&mut layers);
            }
            for runs in layers {
                out.push(Element {
                    thread: region.thread.clone(),
                    runs,
                    whole: region.kind == RegionType::Satin,
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
    // Closed stem, candlewick and triangle chain lines keep their elements in place and start
    // at the entry point as PE-DESIGN's Start/End Type says (see `start_loop` below).
    let in_place = |c: &Contour| {
        c.closed
            && match line.kind {
                LineType::Stem | LineType::Candlewick => true,
                LineType::Chain => line.chain_shape == ChainShape::Triangle,
                _ => false,
            }
    };
    // The line entry point (L1): closed outlines start there, open ones from the nearer end.
    let contours: Vec<Contour> = object
        .contours
        .iter()
        .map(|c| match line.entry {
            Some(p) if !in_place(c) => c.starting_at(p),
            _ => c.clone(),
        })
        .collect();
    // Whether the first line starts with running stitch to the pattern (a shorter tie-in).
    let mut lead_in = false;
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
                whole: false,
            })
            .collect();
    }
    let mut runs_so_far = 0;
    let mut runs: Vec<Run> = contours
        .iter()
        .map(|c| {
            let width = line.width * 10.0;
            let spacing = line.spacing_mm() * 10.0;
            let mut run = if line.kind == LineType::Zigzag && line.underlay {
                lines::zigzag_underlay(&c.path(), width / 4.0, pitch)
            } else {
                Vec::new()
            };
            let loop_line = match line.kind {
                LineType::Stem => Some(lines::LoopLine::Stem {
                    width,
                    spacing,
                    angle: line.angle.unwrap_or(45.0),
                    single: line.single,
                }),
                LineType::Candlewick => Some(lines::LoopLine::Candlewick {
                    size: width,
                    points: (line.density * 2.0).round().clamp(4.0, 48.0) as usize,
                    spacing,
                    pitch,
                }),
                LineType::Chain => Some(lines::LoopLine::Chain {
                    size: width,
                    times: line.run_times,
                    arrange: line.arrange,
                }),
                _ => None,
            };
            if let (Some(entry), Some(l)) = (line.entry, loop_line)
                && in_place(c)
            {
                let path = c.path();
                // The pattern starts and ends at the exit point (L2); Run leads in from L1.
                let exit = lines::arc_of(&path, line.exit.unwrap_or(entry));
                let run_from = match line.start_end {
                    StartEnd::Feed => None,
                    StartEnd::Run => Some(lines::arc_of(&path, entry)),
                };
                if let Some((top, lead)) = lines::start_loop(&path, l, exit, run_from) {
                    lead_in |= lead && runs_so_far == 0;
                    run.extend(top);
                    run.dedup_by(|a, b| a.distance(*b) < 1e-9);
                    runs_so_far += 1;
                    return run;
                }
            }
            runs_so_far += 1;
            let top = match line.kind {
                LineType::Running => generate::running(c, pitch, line.run_times),
                LineType::Triple => generate::triple(c, pitch),
                // PE-DESIGN's zigzag (rails matched along each leg) on straight-line paths;
                // curves keep the evenly spaced zigzag, with half stitch where asked.
                LineType::Zigzag if !c.has_curves() => {
                    lines::zigzag_rails(&c.path(), width, 10.0 / line.density.max(0.1))
                }
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
                    // Counter-clockwise from the line on screen, as stored.
                    line.angle.unwrap_or(45.0),
                    line.single,
                ),
                // PE-DESIGN's Medium knot is a star of 24 points (density 12).
                LineType::Candlewick => lines::candlewick(
                    &c.path(),
                    width,
                    (line.density * 2.0).round().clamp(4.0, 48.0) as usize,
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
                ),
                LineType::Cutting => Vec::new(),
            };
            run.extend(top);
            run.dedup_by(|a, b| a.distance(*b) < 1e-9);
            run
        })
        .collect();
    // PE-DESIGN ties in and locks these lines like a satin (line-*-default.pes); V, stem,
    // candlewicking and chain tie in with three steps out instead of one, but not where a
    // moved entry point starts a stem or candlewick with running stitch
    // (line-*-startend-*.pes; a chain's lead keeps the long one).
    if !matches!(line.kind, LineType::Motif | LineType::Cutting) {
        region::tie_and_lock(std::slice::from_mut(&mut runs));
        if matches!(
            line.kind,
            LineType::VStitch | LineType::Stem | LineType::Candlewick | LineType::Chain
        ) && !(lead_in && line.kind != LineType::Chain)
            && let Some(run) = runs.iter_mut().find(|r| r.len() >= 4)
        {
            long_tie_in(run);
        }
    }
    vec![Element {
        thread: line.thread.clone(),
        runs,
        whole: false,
    }]
}

/// Turns the tie-in that [`region::tie_and_lock`] put at the start of `run` (start, 0.3 mm
/// out, start) into PE-DESIGN's longer one for V, stem, candlewicking and chain: 0.3, 0.6 and
/// 0.9 mm out along the first stitch (at most its length), back to 0.5 mm, then the start.
fn long_tie_in(run: &mut Run) {
    let (a, b) = (run[0], run[3]);
    let len = a.distance(b);
    if len <= 1e-9 || run[2].distance(a) > 1e-9 {
        return;
    }
    let at = |d: f64| a + (b - a) * (d.min(len) / len);
    run.splice(1..3, [at(3.0), at(6.0), at(9.0), at(5.0), a]);
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
