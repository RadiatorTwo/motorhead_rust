//! The game's motion blur (config `MotionBlur On`), as at the game's own frame rate.
//!
//! With it on, Glide's scene (0x4bc2f8, and 0x4bebcc) clears no picture: it draws the scene
//! with a constant alpha of 0x80 (grAlphaCombine local constant, grAlphaBlendFunction
//! 1 - src alpha, src alpha) over the last one, which keeps half of it. That is meant for a
//! picture per frame (60 a second here). The port shows up to 240 pictures a second: each
//! halved the last again, and the halos and lens flares (added, not blended) summed up to
//! about six times their light instead of twice. So each picture starts from the picture of
//! the last logic frame (`glide::restore_picture`), kept after the frame's own drawing
//! (`frame_drawn`): every picture is half the scene now and half the last frame, as in the
//! original.

use recomp::ver::Us;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use win32::platform::glide;

/// Where the scenes set the blur's constant colour (the return addresses of their
/// grConstantColorValue calls), before they draw.
const BLUR_SET: [Us; 2] = [Us(0x4bc497), Us(0x4becc4)];

/// A kept picture older than this is another race's (or from before a pause): not put back.
const STALE: Duration = Duration::from_millis(500);

/// The picture being drawn is blurred.
static BLURRED: AtomicBool = AtomicBool::new(false);
/// When the last logic frame's picture was kept.
static KEPT: Mutex<Option<Instant>> = Mutex::new(None);

/// grConstantColorValue: at the blur's, the last logic frame's picture is put back to draw
/// over. The value stays the game's.
pub fn constant_color(value: u32, from: u32) -> u32 {
    if BLUR_SET.iter().any(|a| a.at() == from) {
        BLURRED.store(true, Ordering::Relaxed);
        if KEPT.lock().unwrap().is_some_and(|t| t.elapsed() < STALE) {
            glide::restore_picture();
        }
    }
    value
}

/// A logic frame's own drawing is done: its picture is kept for the pictures after it.
pub fn frame_drawn() {
    if BLURRED.swap(false, Ordering::Relaxed) {
        glide::keep_picture();
        *KEPT.lock().unwrap() = Some(Instant::now());
    }
}

/// A picture between logic frames is done (its blur is not kept).
pub fn picture_drawn() {
    BLURRED.store(false, Ordering::Relaxed);
}
