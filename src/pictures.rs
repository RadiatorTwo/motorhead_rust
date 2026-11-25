//! What moves in a race, put between its last two logic frames while a picture is drawn
//! (recomp_kit GUIDE 8b, step 3). Every picture, the logic frame's own too, shows the race a
//! little more than a frame late (`LogicClock::alpha`): the cars and the camera are set to
//! their places at that time for the drawing and put back after it, so the logic only ever sees
//! its own values.
//!
//! - A car (`[CARS + i * PLAYER]`, 8 players): its rotation (3x3 floats) at +4, its place at
//!   +0x28 (0x49dad0 builds a matrix from them).
//! - The camera (`CAMERA`, built by the logic from the followed car): rotation at +0x10, place
//!   at +0x40; the drawing turns it into the view (0x419750, into 0x73a118).

use std::collections::HashMap;
use std::sync::Mutex;
use win32::platform::mem;
use win32::platform::tween::{Hist, from_quat, is_rotation, quat};

/// The players' records: 8, 0x2c00 bytes apart; their car's object at +0 of each here.
const CARS: u32 = 0x7499a9;
const PLAYER: u32 = 0x2c00;
const PLAYERS: u32 = 8;
/// The camera.
const CAMERA: u32 = 0x739f80;

/// Something that moves: where its rotation and its place are.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Mover {
    rot: u32,
    at: u32,
}

#[derive(Clone, Copy)]
struct Pose {
    rot: [f32; 9],
    at: [f32; 3],
}

/// The most anything moves in a logic frame to be interpolated: more is a jump (a car put back
/// on the track, the camera switched).
const MAX_STEP: f32 = 30.0;

/// The most a rotation turns in a frame to be interpolated (cos of half the turn: 60°).
const MAX_TURN_COS: f64 = 0.866;

static HIST: Mutex<Option<HashMap<Mover, Hist<Pose>>>> = Mutex::new(None);

fn movers() -> Vec<Mover> {
    let m = mem();
    let mut v = vec![Mover {
        rot: CAMERA + 0x10,
        at: CAMERA + 0x40,
    }];
    for i in 0..PLAYERS {
        let car = m.rd32(CARS + i * PLAYER);
        if car != 0 {
            v.push(Mover {
                rot: car + 4,
                at: car + 0x28,
            });
        }
    }
    v
}

fn read(o: Mover) -> Pose {
    let m = mem();
    let f = |a: u32| f32::from_bits(m.rd32(a));
    Pose {
        rot: std::array::from_fn(|k| f(o.rot + 4 * k as u32)),
        at: std::array::from_fn(|k| f(o.at + 4 * k as u32)),
    }
}

fn write(o: Mover, p: &Pose) {
    let m = mem();
    for (k, v) in p.rot.iter().enumerate() {
        m.wr32(o.rot + 4 * k as u32, v.to_bits());
    }
    for (k, v) in p.at.iter().enumerate() {
        m.wr32(o.at + 4 * k as u32, v.to_bits());
    }
}

/// Pictures drawn, and things put between frames in them (`MOTORHEAD_RACE_LOG`).
static DRAWN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static MIXED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// After logic frame `frame` (at its own drawing): where everything is.
pub fn noted(frame: u64) {
    if crate::frames::race_log() && frame > 0 && frame % 600 == 0 {
        use std::sync::atomic::Ordering::Relaxed;
        let (d, x) = (DRAWN.swap(0, Relaxed), MIXED.swap(0, Relaxed));
        let pic = crate::frames::PICTURE_NS.swap(0, Relaxed) as f64 / 1e6;
        let fr = crate::frames::FRAME_NS.swap(0, Relaxed) as f64 / 1e6;
        // the frames' own drawings count as pictures too: 600 of them
        let extra = d.saturating_sub(600).max(1) as f64;
        eprintln!(
            "pictures {frame}: {:.2} a logic frame, {:.2} things mixed in each; \
             a picture {:.2} ms, a logic frame {:.2} ms",
            d as f64 / 600.0,
            x as f64 / d.max(1) as f64,
            pic / extra,
            fr / 600.0
        );
    }
    let mut h = HIST.lock().unwrap();
    let old = h.take().unwrap_or_default();
    let new = movers()
        .into_iter()
        .map(|o| (o, Hist::note(old.get(&o), frame, read(o))))
        .collect();
    *h = Some(new);
}

/// Out of the race: nothing to interpolate from.
pub fn forget() {
    *HIST.lock().unwrap() = None;
}

/// `a` to `b` by `t`; None for a jump.
fn mix(a: &Pose, b: &Pose, t: f32) -> Option<Pose> {
    let d: f32 = (0..3).map(|k| (b.at[k] - a.at[k]).powi(2)).sum::<f32>().sqrt();
    if !(d <= MAX_STEP) {
        return None;
    }
    let at = std::array::from_fn(|k| a.at[k] + (b.at[k] - a.at[k]) * t);
    let rot = mix_rotation(&a.rot, &b.rot, t as f64)?;
    Some(Pose { rot, at })
}

/// Rotations the short way (quaternions); anything else (a scaled matrix) element by element.
fn mix_rotation(a: &[f32; 9], b: &[f32; 9], t: f64) -> Option<[f32; 9]> {
    if a == b {
        return Some(*b);
    }
    let (ma, mb) = (a.map(|v| v as f64), b.map(|v| v as f64));
    if !(is_rotation(&ma, 0.02) && is_rotation(&mb, 0.02)) {
        return Some(std::array::from_fn(|k| {
            (ma[k] + (mb[k] - ma[k]) * t) as f32
        }));
    }
    let (qa, mut qb) = (quat(&ma), quat(&mb));
    let mut dot: f64 = (0..4).map(|i| qa[i] * qb[i]).sum();
    if dot < 0.0 {
        qb = qb.map(|v| -v);
        dot = -dot;
    }
    if dot < MAX_TURN_COS {
        return None;
    }
    let q: [f64; 4] = std::array::from_fn(|i| qa[i] + (qb[i] - qa[i]) * t);
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    Some(from_quat(q.map(|v| v / n)).map(|v| v as f32))
}

/// Draw (`f`) with everything at `alpha` / 256 of the way from the last logic frame to the
/// newest, then everything put back.
pub fn draw(alpha: Option<u32>, f: impl FnOnce()) {
    let mut put = Vec::new();
    if let Some(alpha) = alpha {
        let t = alpha as f32 / 256.0;
        let h = HIST.lock().unwrap();
        for (&o, hist) in h.iter().flatten() {
            let (Some(prev), now) = (hist.prev, read(o)) else {
                continue;
            };
            // only what is still as the logic left it (a picture after a new logic frame
            // whose drawing has not yet been noted draws it as it is)
            if now.rot != hist.cur.rot || now.at != hist.cur.at {
                continue;
            }
            if let Some(p) = mix(&prev, &hist.cur, t) {
                write(o, &p);
                put.push((o, now));
            }
        }
    }
    DRAWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    MIXED.fetch_add(put.len() as u64, std::sync::atomic::Ordering::Relaxed);
    f();
    for (o, p) in put {
        write(o, &p);
    }
}
