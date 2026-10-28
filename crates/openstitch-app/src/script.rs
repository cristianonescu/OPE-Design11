//! `--script`: scripted pointer and key input fed through egui's raw input, so a run exercises
//! the real canvas, tool and shortcut code paths (used for the documentation screenshots and
//! for checking a build on a platform without clicking).
//!
//! Steps are separated by `;`, coordinates are model millimetres on the Design Page:
//! `drag:x1,y1,x2,y2` · `click:x,y` · `sclick:x,y` (window points, for panels and menus) · `dclick:x,y` · `move:x,y` · `type:r` (text, e.g. tool
//! shortcuts) · `key:Enter` · `wait:N` (frames) · `close:0` (the window's close request, as the
//! X button sends it). Prefix `shift+` / `cmd+` to hold modifiers, e.g. `shift+drag:…`,
//! `cmd+key:Z`.

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2};
use openstitch_app::editor::Viewport;
use openstitch_design::{Vec2, v2};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Drag(Vec2, Vec2),
    Click(Vec2),
    DoubleClick(Vec2),
    Move(Vec2),
    /// Click at a window position in points (panels, menus, popups).
    ScreenClick(f32, f32),
    Type(String),
    Key(Key),
    Wait(usize),
    /// The window's close request (X button, Alt+F4); the argument is ignored.
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub action: Action,
    pub modifiers: Modifiers,
}

fn point(s: &str, n: usize) -> Result<Vec<Vec2>, String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| {
            x.trim()
                .parse::<f64>()
                .map_err(|_| format!("bad number '{x}'"))
        })
        .collect::<Result<_, _>>()?;
    if v.len() != 2 * n {
        return Err(format!("'{s}' needs {} numbers", 2 * n));
    }
    // mm → model units (0.1 mm).
    Ok(v.chunks(2).map(|c| v2(c[0] * 10.0, c[1] * 10.0)).collect())
}

pub fn parse(script: &str) -> Result<Vec<Step>, String> {
    script
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (head, arg) = s
                .split_once(':')
                .ok_or_else(|| format!("step '{s}' needs ':'"))?;
            let mut modifiers = Modifiers::NONE;
            let mut parts: Vec<&str> = head.split('+').collect();
            let verb = parts.pop().unwrap_or_default();
            for m in parts {
                match m {
                    "shift" => modifiers |= Modifiers::SHIFT,
                    "cmd" => modifiers |= Modifiers::COMMAND,
                    "alt" => modifiers |= Modifiers::ALT,
                    _ => return Err(format!("unknown modifier '{m}'")),
                }
            }
            let action = match verb {
                "drag" => {
                    let p = point(arg, 2)?;
                    Action::Drag(p[0], p[1])
                }
                "click" => Action::Click(point(arg, 1)?[0]),
                "dclick" => Action::DoubleClick(point(arg, 1)?[0]),
                "move" => Action::Move(point(arg, 1)?[0]),
                "sclick" => {
                    let p = point(arg, 1)?[0];
                    Action::ScreenClick((p.x / 10.0) as f32, (p.y / 10.0) as f32)
                }
                "type" => Action::Type(arg.to_string()),
                "key" => {
                    Action::Key(Key::from_name(arg).ok_or_else(|| format!("unknown key '{arg}'"))?)
                }
                "wait" => Action::Wait(
                    arg.parse()
                        .map_err(|_| format!("bad frame count '{arg}'"))?,
                ),
                "close" => Action::Close,
                _ => return Err(format!("unknown step '{verb}'")),
            };
            Ok(Step { action, modifiers })
        })
        .collect()
}

/// Turns one step into per-frame event batches, mapping model points through `vp`.
pub fn frames(step: &Step, vp: &Viewport) -> Vec<Vec<Event>> {
    let m = step.modifiers;
    let at = |p: Vec2| {
        let s = vp.to_screen(p);
        Pos2::new(s.x as f32, s.y as f32)
    };
    let button = |p: Vec2, pressed| Event::PointerButton {
        pos: at(p),
        button: PointerButton::Primary,
        pressed,
        modifiers: m,
    };
    let key = |k, pressed| Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: m,
    };
    match &step.action {
        Action::Move(p) => vec![vec![Event::PointerMoved(at(*p))]],
        Action::Click(p) => vec![
            vec![Event::PointerMoved(at(*p))],
            vec![button(*p, true)],
            vec![button(*p, false)],
        ],
        Action::DoubleClick(p) => vec![
            vec![Event::PointerMoved(at(*p))],
            vec![button(*p, true)],
            vec![button(*p, false)],
            vec![button(*p, true)],
            vec![button(*p, false)],
        ],
        Action::Drag(a, b) => {
            let mut f = vec![vec![Event::PointerMoved(at(*a))], vec![button(*a, true)]];
            for k in 1..=8 {
                f.push(vec![Event::PointerMoved(
                    at(a.lerp(*b, f64::from(k) / 8.0)),
                )]);
            }
            f.push(vec![button(*b, false)]);
            f
        }
        Action::ScreenClick(x, y) => {
            let p = Pos2::new(*x, *y);
            let b = |pressed| Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: m,
            };
            vec![
                vec![Event::PointerMoved(p)],
                vec![b(true)],
                vec![b(false)],
                Vec::new(),
            ]
        }
        Action::Type(t) => vec![vec![Event::Text(t.clone())]],
        Action::Key(k) => vec![vec![key(*k, true)], vec![key(*k, false)]],
        Action::Wait(n) => vec![Vec::new(); *n],
        Action::Close => vec![Vec::new()],
    }
}

/// Pending steps plus the frames of the step in progress.
#[derive(Default)]
pub struct Player {
    steps: VecDeque<Step>,
    current: VecDeque<(Vec<Event>, Modifiers)>,
    /// A `close` step started: the next frame carries the viewport's close request.
    close: bool,
}

impl Player {
    pub fn new(steps: Vec<Step>) -> Self {
        Player {
            steps: steps.into(),
            current: VecDeque::new(),
            close: false,
        }
    }

    pub fn is_done(&self) -> bool {
        self.steps.is_empty() && self.current.is_empty()
    }

    /// Events and held modifiers for the next frame.
    pub fn next_frame(&mut self, vp: &Viewport) -> Option<(Vec<Event>, Modifiers)> {
        if self.current.is_empty() {
            let step = self.steps.pop_front()?;
            self.close = step.action == Action::Close;
            self.current = frames(&step, vp)
                .into_iter()
                .map(|f| (f, step.modifiers))
                .collect();
        }
        self.current.pop_front()
    }

    /// Whether the frame just returned by [`Self::next_frame`] is a `close` step.
    pub fn take_close_request(&mut self) -> bool {
        std::mem::take(&mut self.close)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_steps_and_modifiers() {
        let s = parse("drag:10,10,30,20; shift+click:5,5 ;type:r;cmd+key:Z;wait:3;dclick:1,2")
            .expect("ok");
        assert_eq!(s.len(), 6);
        assert_eq!(
            s[0].action,
            Action::Drag(v2(100.0, 100.0), v2(300.0, 200.0))
        );
        assert_eq!(s[1].modifiers, Modifiers::SHIFT);
        assert_eq!(
            s[3],
            Step {
                action: Action::Key(Key::Z),
                modifiers: Modifiers::COMMAND
            }
        );
        assert_eq!(
            parse("sclick:100,20").expect("ok")[0].action,
            Action::ScreenClick(100.0, 20.0)
        );
        assert!(parse("drag:1,2,3").is_err());
        assert!(parse("hover:1,2").is_err());
        assert!(parse("ctrl+click:1,2").is_err());
        assert!(parse("key:NoSuchKey").is_err());
        assert_eq!(parse("close:0").expect("ok")[0].action, Action::Close);
    }

    #[test]
    fn drag_expands_to_press_moves_release_in_screen_space() {
        let vp = Viewport {
            scale: 2.0,
            offset: v2(10.0, 0.0),
        };
        let step = Step {
            action: Action::Drag(v2(0.0, 0.0), v2(80.0, 0.0)),
            modifiers: Modifiers::NONE,
        };
        let f = frames(&step, &vp);
        assert_eq!(f.len(), 11);
        assert!(
            matches!(f[1][0], Event::PointerButton { pressed: true, pos, .. } if pos == Pos2::new(10.0, 0.0))
        );
        assert!(
            matches!(f[10][0], Event::PointerButton { pressed: false, pos, .. } if pos == Pos2::new(170.0, 0.0))
        );
        let mut p = Player::new(vec![
            step.clone(),
            Step {
                action: Action::Wait(2),
                modifiers: Modifiers::NONE,
            },
        ]);
        let mut n = 0;
        while p.next_frame(&vp).is_some() {
            n += 1;
        }
        assert_eq!(n, 13);
        assert!(p.is_done());
    }
}
