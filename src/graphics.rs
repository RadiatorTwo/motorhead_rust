//! The game's graphics options: only the renderer the port has (Glide) offered, and the
//! port's resolutions (the F10 menu's Resolution row) in the game's resolution list.
//!
//! The game lists its renderers at the start (`RENDERERS`, 0x4a9c74): a chain of 0x284-byte
//! records (+0x54 the name, +0x278 the number of resolutions, +0x27c their 0x34-byte records,
//! +0x280 the next). Software ("cliff"), Direct3D and PowerVR ("psgl") are not ported: left out
//! of the chain. Glide's resolutions come from `GLIDE_MODES` (0x4bb480, five of them up to
//! 1024x768): ours instead. The menu keeps the renderer's list and the chosen entry in `MODES`
//! (number, records, index; first the one matching the config's ScreenDimension) and writes
//! the chosen one into the config (MotorHead.CFG) when the game ends.
//!
//! Glide only gives the game its coordinates; the picture is drawn at the port's resolution
//! (`display::render_size`). So the game keeps drawing in one 4:3 mode (`GAME_MODE`, the
//! largest it knows), and the menu's resolution is the port's: `sync` keeps the two the same
//! in the menus (a change in the game's menu sets the port's, an F10 change the menu's).

use recomp::rt::{Cpu, EAX, EDX, ESP, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use win32::platform::{display, guest_call, mem, options, window};

/// Lists the renderers (eax: where the chain's first record goes).
pub const RENDERERS: Us = Us(0x4a9c74);
/// Glide's resolutions: eax where their number goes, edx where the records' pointer goes.
pub const GLIDE_MODES: Us = Us(0x4bb480);
/// Opens Glide in a resolution (edx: its record), one of the Glide table's (0x4f1cb4).
pub const GLIDE_OPEN: Us = Us(0x4bb5f4);
/// The game's allocation (eax the size, edx 0).
const MALLOC: Us = Us(0x418320);
/// The graphics menu's resolutions: their number, their records, the chosen one's index.
const MODES: Us = Us(0x7d92d0);

/// The renderer the port has.
const GLIDE_NAME: &[u8] = b"voodoo\0";
/// The mode the game draws in (its coordinates).
const GAME_MODE: (u32, u32) = (1024, 768);

/// The renderer chain without those not ported (unless that leaves none).
pub fn renderers(c: &mut Cpu) {
    let out = c.r[EAX];
    call_original(c, RENDERERS.at());
    let m = mem();
    let mut all = Vec::new();
    let mut r = m.rd32(out);
    while r != 0 && all.len() < 64 {
        all.push(r);
        r = m.rd32(r + 0x280);
    }
    let kept: Vec<u32> = all
        .iter()
        .copied()
        .filter(|&r| m.slice(r + 0x54, GLIDE_NAME.len()) == GLIDE_NAME)
        .collect();
    if kept.is_empty() {
        return;
    }
    for (k, &r) in kept.iter().enumerate() {
        m.wr32(r + 0x280, kept.get(k + 1).copied().unwrap_or(0));
    }
    m.wr32(out, kept[0]);
}

/// Where Glide's resolutions go (their number, their pointer) while the display is not known
/// yet (the game lists them before it opens its window): made again by `sync`.
static LATER: Mutex<Option<(u32, u32)>> = Mutex::new(None);

/// Glide's resolutions: the port's.
pub fn glide_modes(c: &mut Cpu) {
    let (count, list) = (c.r[EAX], c.r[EDX]);
    if window::display_size().is_none() {
        *LATER.lock().unwrap() = Some((count, list));
    }
    write_modes(c, count, list);
    c.r[ESP] += 4;
}

/// The fullscreen entry: the display's size (a window that large is no use), shown as
/// "Fullscreen" (`label`). None while the display is not known.
static FULL: Mutex<Option<(u32, u32)>> = Mutex::new(None);

/// The port's resolutions (all of them while the display is not known) and the fullscreen
/// entry, as records like the game's (16 bits, 565): their number into `count`, the records'
/// pointer into `list`.
fn write_modes(c: &mut Cpu, count: u32, list: u32) {
    let full = window::display_size();
    *FULL.lock().unwrap() = full;
    let mut sizes = options::resolutions(full.unwrap_or((u32::MAX, u32::MAX)));
    let now = display::render_size();
    if !sizes.contains(&now) {
        sizes.push(now);
    }
    if let Some(full) = full {
        sizes.retain(|&s| s != full);
        sizes.push(full);
    }
    let saved = c.r;
    c.r[EAX] = sizes.len() as u32 * 0x34;
    c.r[EDX] = 0;
    let p = guest_call(c, MALLOC.at(), &[]);
    c.r = saved;
    let m = mem();
    m.wr32(count, if p != 0 { sizes.len() as u32 } else { 0 });
    if p != 0 {
        m.wr32(list, p);
        for (k, &(w, h)) in sizes.iter().enumerate() {
            let r = p + k as u32 * 0x34;
            let fields = [0, w, h, w * 2, 16, 5, 6, 5, 0, 0, 5, 11, 0];
            for (i, v) in fields.into_iter().enumerate() {
                m.wr32(r + 4 * i as u32, v);
            }
        }
    }
}

/// Glide opened in `GAME_MODE`, whatever resolution the record has (it stays as it was).
pub fn glide_open(c: &mut Cpu) {
    let m = mem();
    let r = c.r[EDX];
    let was = [m.rd32(r + 4), m.rd32(r + 8), m.rd32(r + 0xc)];
    let (w, h) = GAME_MODE;
    for (i, v) in [w, h, w * 2].into_iter().enumerate() {
        m.wr32(r + 4 + 4 * i as u32, v);
    }
    call_original(c, GLIDE_OPEN.at());
    for (i, v) in was.into_iter().enumerate() {
        m.wr32(r + 4 + 4 * i as u32, v);
    }
}

/// The menu's resolutions and chosen index when `sync` saw them last.
static SEEN: Mutex<Option<(u32, u32)>> = Mutex::new(None);

/// The menu's resolutions (number, records) and the chosen index.
fn menu() -> Option<(u32, u32, u32)> {
    let m = mem();
    let a = MODES.at();
    let (n, list, k) = (m.rd32(a), m.rd32(a + 4), m.rd32(a + 8));
    (list != 0 && k < n).then_some((n, list, k))
}

fn size(list: u32, k: u32) -> (u32, u32) {
    let m = mem();
    (m.rd32(list + k * 0x34 + 4), m.rd32(list + k * 0x34 + 8))
}

/// The entry the port's state is: fullscreen, else the resolution drawn at.
fn port_size() -> (u32, u32) {
    match *FULL.lock().unwrap() {
        Some(full) if window::is_fullscreen() => full,
        _ => display::render_size(),
    }
}

/// Once a frame in the menus: Glide's resolutions made again once the display is known; the
/// game's menu chose another entry, the port takes it (fullscreen, or a window that size);
/// else the menu gets the port's.
pub fn sync(c: &mut Cpu) {
    let later = *LATER.lock().unwrap();
    if let Some((count, list)) = later.filter(|_| window::display_size().is_some()) {
        *LATER.lock().unwrap() = None;
        write_modes(c, count, list);
    }
    let Some((n, list, k)) = menu() else { return };
    let mut seen = SEEN.lock().unwrap();
    if seen.is_some_and(|s| s.0 == list && s.1 != k) {
        let chosen = size(list, k);
        if Some(chosen) == *FULL.lock().unwrap() {
            display::remember_chosen(None);
            display::set_fullscreen(true);
        } else {
            display::set_fullscreen(false);
            display::set_chosen(Some(chosen));
        }
    } else {
        // only when the port's changed (F10, Alt+Enter, the start): a change from the menu
        // takes the window a few frames
        let port = port_size();
        let mut last = PORT.lock().unwrap();
        if *last != Some(port) || seen.is_none_or(|s| s.0 != list) {
            *last = Some(port);
            if let Some(i) = (0..n).find(|&i| size(list, i) == port) {
                mem().wr32(MODES.at() + 8, i);
            }
        }
    }
    *seen = Some((list, mem().rd32(MODES.at() + 8)));
}

/// The port's entry when `sync` looked last.
static PORT: Mutex<Option<(u32, u32)>> = Mutex::new(None);

/// The Render device row changed: the game takes the new renderer's resolution "nearest" the
/// chosen one (by |dw * dh|: any with the same width or height). The same one kept instead.
pub const RENDERER_CHANGED: Us = Us(0x4840c8);

pub fn renderer_changed(c: &mut Cpu) {
    let was = menu().map(|(_, list, k)| size(list, k));
    call_original(c, RENDERER_CHANGED.at());
    if let (Some(was), Some((n, list, _))) = (was, menu()) {
        if let Some(i) = (0..n).find(|&i| size(list, i) == was) {
            mem().wr32(MODES.at() + 8, i);
            *SEEN.lock().unwrap() = Some((list, i));
        }
    }
}

/// The game's sprintf (cdecl: destination, format, arguments).
pub const SPRINTF: Us = Us(0x4c21bc);
/// The graphics menu's resolution label, "%4ldx%-4ld".
const RES_FORMAT: Us = Us(0x4fe7a9);

/// The fullscreen entry's label "Fullscreen" instead of its size.
pub fn label(c: &mut Cpu) {
    let m = mem();
    let sp = c.r[ESP];
    let arg = |k: u32| m.rd32(sp + 4 + 4 * k);
    let full = *FULL.lock().unwrap();
    if arg(1) == RES_FORMAT.at() && full == Some((arg(2), arg(3))) {
        let text = b"Fullscreen\0";
        m.slice(arg(0), text.len()).copy_from_slice(text);
        c.r[EAX] = text.len() as u32 - 1;
        c.r[ESP] += 4;
        return;
    }
    call_original(c, SPRINTF.at());
}