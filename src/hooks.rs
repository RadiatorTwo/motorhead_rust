//! Game functions replaced by Rust functions. A hook gets the guest's registers (`Cpu`); its
//! arguments are on the guest stack above the return address (`win32::platform::arg(c, k)`).
//! `call_original(c, address)` runs the game's own function (it pops the return address
//! itself); a hook that does not call it must pop the return address (and, for stdcall, the
//! arguments) and set eax itself.
//!
//! Addresses are the reference version's (`Us(0x...)`), `.at()` gives their place in the
//! version that runs. Write what you learn about each function into data/symbols.txt.

#[allow(unused_imports)]
use recomp::rt::call_original;
use recomp::rt::{Cpu, Hook};
#[allow(unused_imports)]
use recomp::ver::Us;

/// Every hook: (address in the version that runs, function).
pub fn all() -> Vec<(u32, Hook)> {
    let table: &[(Us, Hook)] = &[
        (POW10_TABLE, pow10_table),
        (PATH_NEAREST, path_nearest),
        (LIGHTS_VISIBLE, lights_visible),
        (crate::project::PROJECT_POINT, crate::project::project_point),
        (crate::polygons::RENDER_POLYGONS, crate::polygons::render_polygons),
        (crate::frames::RACE_FRAME, crate::frames::race_frame),
        (crate::frames::CLOCK_ADVANCE, crate::frames::clock_advance),
        (crate::frames::RACE_DRAW, crate::frames::race_draw),
        (crate::wide::SCENE, crate::wide::scene),
        (crate::pad::CONTROL_VALUE, crate::pad::control_value),
        (crate::font::FONT_DRAW, crate::font::draw),
        (crate::sprites::DRAW_CLIPPED, crate::sprites::draw_clipped),
        (crate::sprites::DRAW_STEP, crate::sprites::draw_step),
        (crate::sprites::DRAW_LIST, crate::sprites::draw_list),
    ];
    table.iter().map(|&(a, h)| (a.at(), h)).collect()
}

/// Watcom's runtime (number formatting): `call +x; <table>`, where x is `pop edi; ret`: edi =
/// the table of powers of ten right after the call. The bytes after the call are data, so the
/// function is not lifted; this is what it does.
const POW10_TABLE: Us = Us(0x4cde6f);

fn pow10_table(c: &mut Cpu) {
    c.r[recomp::rt::EDI] = POW10_TABLE.at() + 5;
    c.r[recomp::rt::ESP] += 4;
}

/// The node of a path (a track's line) nearest a point: eax the path ([+4] its nodes, [+0x18]
/// their number), edx the point, ebx where the last found node is kept. Crashed twice in
/// races with a path whose nodes were 0 or garbage: logged here (diagnosis).
const PATH_NEAREST: Us = Us(0x46fc88);

fn path_nearest(c: &mut Cpu) {
    use recomp::rt::{EAX, EBX};
    let m = win32::platform::mem();
    let p = c.r[EAX];
    let (nodes, count) = (m.rd32(p + 4), m.rd32(p + 0x18));
    if nodes == 0 || count == 0 || count > 100_000 {
        eprintln!(
            "path_nearest: path {p:#x} nodes {nodes:#x} count {count} stride {} last {:#x}              from {:#x}",
            m.rd32(p + 0x1c),
            if c.r[EBX] != 0 { m.rd32(c.r[EBX]) } else { 0 },
            m.rd32(c.r[recomp::rt::ESP]),
        );
    }
    if crate::frames::race_log() {
        let f = crate::frames::FRAME.load(std::sync::atomic::Ordering::Relaxed);
        if f % crate::frames::race_log_every() == 0 {
            let pt = c.r[recomp::rt::EDX];
            let v = |k: u32| f32::from_bits(m.rd32(pt + 4 * k));
            eprintln!("race {f}: path {p:#x} at {:.3} {:.3} {:.3}", v(0), v(1), v(2));
        }
    }
    call_original(c, PATH_NEAREST.at());
}

/// Glide_LightsVisible: for each light of the list (first at `[LIGHTS]`, next at [+0x50]) at
/// screen point ([+0x44], [+0x48], floats, truncated), visible ([+0x40] = 1) if the depth
/// buffer's z-buffer value there is below its z [+0x4c] times `[DEPTH_SCALE]`. The game
/// locks the whole depth buffer for this (grLfbLock AUXBUFFER, read) and reads one value a
/// light: converting the whole screen to Glide's format each picture was ~10% of a race.
/// Here the values are read at the lights' points only, from the same picture-late depth
/// (`glide::depth_at`). A light beside the screen is hidden (the game read memory beside
/// the buffer). Keeps ebx, ecx, edx, esi as the game's; eax 1 (grLfbUnlock's), 0 without
/// lights.
const LIGHTS_VISIBLE: Us = Us(0x4be458);
const LIGHTS: Us = Us(0x52107c);
const DEPTH_SCALE: Us = Us(0x504dd0);

fn lights_visible(c: &mut Cpu) {
    use recomp::rt::{EAX, ESP};
    let m = win32::platform::mem();
    let f = |a: u32| f32::from_bits(m.rd32(a));
    let mut lights = Vec::new();
    let mut l = m.rd32(LIGHTS.at());
    // (a list gone round in a circle stops)
    while l != 0 && lights.len() < 4096 {
        lights.push(l);
        l = m.rd32(l + 0x50);
    }
    let points: Vec<(i64, i64)> = lights
        .iter()
        .map(|&l| (f(l + 0x44).trunc() as i64, f(l + 0x48).trunc() as i64))
        .collect();
    let depth = win32::platform::glide::depth_at(&points);
    let scale = f(DEPTH_SCALE.at()) as f64;
    for (&l, d) in lights.iter().zip(depth) {
        let z = f(l + 0x4c) as f64 * scale;
        let visible = d.is_some_and(|d| (d as f64) < z);
        m.wr32(l + 0x40, visible as u32);
    }
    c.r[EAX] = !lights.is_empty() as u32;
    c.r[ESP] += 4;
}

// Example: the game's frame wait, where debug commands and savestates can run.
//
// const FRAME_WAIT: Us = Us(0x406a30);
//
// fn frame_wait(c: &mut Cpu) {
//     win32::platform::savestate::frame();
//     win32::platform::cheats::run_pending(c);
//     call_original(c, FRAME_WAIT.at());
// }

#[allow(dead_code)]
fn _example(_c: &mut Cpu) {}

