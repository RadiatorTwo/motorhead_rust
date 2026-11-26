//! Widescreen (Hor+, recomp_kit GUIDE 8c): a race takes the window's aspect and shows more at
//! the left and right; menus, videos and the loading screens stay 4:3.
//!
//! The camera (0x739f80) keeps its projection in a few values: +0 the focal length
//! 1 / tan(half the horizontal field of view) (0x41a3a4 from the angle), +0x18 the height of
//! the picture over its width (0x41a5f0); 0x41a608 makes the rest from them (+0xc the tan,
//! the view's corners and planes from +0x4c on, which cut away what is outside). In a wide
//! race picture the focal length and the height over width are made smaller by
//! k = (4/3) / aspect (the field of view wider, its height the same) and the camera made
//! again: the game still draws into its 512 columns, stretched over the whole frame
//! (`View::Full`), so the proportions are right and the sides show more of the track.
//!
//! The race's drawing (0x4775d8) copies the camera into the view (0x73a118, Camera_View) and
//! hands that to the renderer (Glide's +0x30 copies it to 0x80b9c8). The 3D is Glide's +0x34
//! (`SCENE`); everything after it (+0x68 switches to 2D) is the HUD, drawn unstretched in the
//! frame's 4:3 middle (`View::Hud(0.5)`) with the camera's own values back (the names over the
//! cars are projected with them and land where the stretched 3D shows the cars).
//!
//! On by default; the settings menu (F10, page Display) switches it.

use recomp::rt::{Cpu, EAX, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use win32::platform::glide::{self, View};
use win32::platform::setting::Setting;
use win32::platform::{guest_call, mem};

static WIDE: Setting = Setting::new("Widescreen", "WIDE", true);

/// The settings menu's row.
pub fn option() -> win32::platform::options::Row {
    win32::platform::options::Row::on_off(
        "Widescreen",
        "Races as wide as the window, the HUD in the middle",
        || WIDE.get(),
        |on| WIDE.set(on),
    )
}

/// The camera, the view the race's drawing copies it to and Glide's copy of that.
const CAMERA: Us = Us(0x739f80);
const VIEW: Us = Us(0x73a118);
const GLIDE_VIEW: Us = Us(0x80b9c8);
/// The camera's (and view's) size.
const CAMERA_SIZE: u32 = 0x198;
/// The focal length and the picture's height over its width.
const FOCAL: u32 = 0;
const HEIGHT_OVER_WIDTH: u32 = 0x18;
/// Makes the camera's derived values (eax the camera).
const CAMERA_MAKE: Us = Us(0x41a608);
/// The parts of the camera made from the projection (all but its rotation at +0x1c and its
/// place at +0x40), put back after the 3D.
const MADE: [(u32, u32); 2] = [(0, 0x1c), (0x4c, CAMERA_SIZE)];

/// The renderer: `[[RENDERER] + 0x298]` its function table, Glide's at `GLIDE_TABLE`.
const RENDERER: Us = Us(0x77d95c);
const GLIDE_TABLE: Us = Us(0x505eb0);
/// Glide's 3D scene (the renderer's +0x34).
pub const SCENE: Us = Us(0x4bc2f8);

/// How long after the last race picture the frame stays wide.
const LINGER: Duration = Duration::from_millis(250);

/// When the last race picture was drawn.
static LAST_RACE: Mutex<Option<Instant>> = Mutex::new(None);

/// The camera's parts as the game made them, while the 3D is drawn with wider ones.
static SAVED: Mutex<Option<Vec<u32>>> = Mutex::new(None);

/// The aspect the next frame is drawn with, for a window of aspect `window`.
pub fn aspect(window: Option<f32>) -> f32 {
    let race = LAST_RACE
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < LINGER);
    match window {
        Some(a) if WIDE.get() && race => a.max(4.0 / 3.0),
        _ => 4.0 / 3.0,
    }
}

/// The frame being drawn is wide.
fn wide_now() -> bool {
    glide::frame_aspect() > 4.0 / 3.0 + 0.01
}

/// The renderer is Glide (the others are not ported).
fn glide_renderer() -> bool {
    let m = mem();
    let r = m.rd32(RENDERER.at());
    r != 0 && m.rd32(r + 0x298) == GLIDE_TABLE.at()
}

/// The race's drawing (`draw` runs the game's): the camera widened for it, put back after.
pub fn race_draw(c: &mut Cpu, draw: impl FnOnce(&mut Cpu)) {
    if !glide_renderer() {
        return draw(c);
    }
    *LAST_RACE.lock().unwrap() = Some(Instant::now());
    let wide = wide_now();
    if wide {
        widen(c, (4.0 / 3.0) / glide::frame_aspect());
        // the clear covers the whole frame
        glide::set_view(View::Full);
    }
    draw(c);
    // (the 3D's end put it back already; this is for a drawing that left out the 3D)
    put_back();
    if wide {
        glide::set_view(View::Hud(0.5));
    }
}

/// Glide's 3D scene: over the whole frame; the HUD after it in the 4:3 middle, with the
/// camera's own values.
pub fn scene(c: &mut Cpu) {
    let wide = wide_now() && SAVED.lock().unwrap().is_some();
    if wide {
        glide::set_view(View::Full);
    }
    call_original(c, SCENE.at());
    if wide {
        put_back();
        glide::set_view(View::Hud(0.5));
    }
}

/// The camera's field of view wider by 1 / k, its height the same.
fn widen(c: &mut Cpu, k: f32) {
    let m = mem();
    let cam = CAMERA.at();
    let mut saved = SAVED.lock().unwrap();
    if saved.is_some() {
        return;
    }
    *saved = Some(
        MADE.iter()
            .flat_map(|&(a, b)| (a..b).step_by(4))
            .map(|o| m.rd32(cam + o))
            .collect(),
    );
    for o in [FOCAL, HEIGHT_OVER_WIDTH] {
        let v = f32::from_bits(m.rd32(cam + o));
        m.wr32(cam + o, (v * k).to_bits());
    }
    let regs = c.r;
    c.r[EAX] = cam;
    guest_call(c, CAMERA_MAKE.at(), &[]);
    c.r = regs;
}

/// The camera's own values back, in the camera and both copies of it.
fn put_back() {
    let Some(old) = SAVED.lock().unwrap().take() else {
        return;
    };
    let m = mem();
    let cam = CAMERA.at();
    for (o, v) in MADE.iter().flat_map(|&(a, b)| (a..b).step_by(4)).zip(old) {
        m.wr32(cam + o, v);
    }
    for copy in [VIEW.at(), GLIDE_VIEW.at()] {
        for o in (0..CAMERA_SIZE).step_by(4) {
            m.wr32(copy + o, m.rd32(cam + o));
        }
    }
}
