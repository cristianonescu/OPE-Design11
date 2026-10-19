use crate::{Command, Stitch};

/// Formats without a trim command (DST, JEF) express cuts as runs of jumps. Inserts an
/// explicit trim before a run of jumps once the run reaches `jumps_required` jumps or moves
/// further than `distance_required` on either axis, unless the thread is already cut. With
/// `clipping`, jump runs whose net displacement is zero (pure trim signals) are removed.
///
/// Matches pyembroidery's `interpolate_trims`, the oracle used by the codec tests.
pub fn infer_trims(
    s: &mut Vec<Stitch>,
    jumps_required: Option<usize>,
    distance_required: Option<f64>,
    clipping: bool,
) {
    let mut i: isize = -1;
    let mut last = s.len() as isize - 1;
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let (mut jump_count, mut jump_start) = (0_usize, 0_usize);
    let (mut jump_dx, mut jump_dy) = (0.0_f64, 0.0_f64);
    let (mut jumping, mut trimmed) = (false, true);
    while i < last {
        i += 1;
        let st = s[i as usize];
        let (dx, dy) = (st.x - x, st.y - y);
        x = st.x;
        y = st.y;
        match st.command {
            Command::Stitch => {
                trimmed = false;
                jumping = false;
            }
            Command::ColorChange | Command::Trim => {
                trimmed = true;
                jumping = false;
            }
            _ => {}
        }
        if st.command != Command::Jump {
            continue;
        }
        if !jumping {
            jump_dx = 0.0;
            jump_dy = 0.0;
            jump_count = 0;
            jump_start = i as usize;
            jumping = true;
        }
        jump_count += 1;
        jump_dx += dx;
        jump_dy += dy;
        let far = distance_required.is_some_and(|d| jump_dx.abs() > d || jump_dy.abs() > d);
        if !trimmed && (Some(jump_count) == jumps_required || far) {
            let before = if jump_start > 0 {
                s[jump_start - 1]
            } else {
                Stitch::new(0.0, 0.0, Command::Stitch)
            };
            s.insert(jump_start, Stitch::new(before.x, before.y, Command::Trim));
            jump_start += 1;
            i += 1;
            last += 1;
            trimmed = true;
        }
        if clipping && jump_dx == 0.0 && jump_dy == 0.0 {
            s.drain(jump_start..=i as usize);
            i = jump_start as isize - 1;
            last = s.len() as isize - 1;
        }
    }
}
