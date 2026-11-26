//! The gamepad as a modern racing game has it, whatever control layout is chosen in the game.
//!
//! - In a race the pad presses the keys of the game's Keyboard layout (`Game\Keyboard.clo`),
//!   once per logic frame (so replays press them at the same frames): RT gas, LT brake, the
//!   left stick or the D-pad steers, RB/LB gear up/down, A handbrake, B horn, Y rear view,
//!   X the camera. With the Joystick layout the pad's own buttons work as well (RT is its
//!   JoyButton1, LT JoyButton2).
//! - In the menus the game takes the pad's first button (ours: RT) to confirm: there A is
//!   reported as the first button and B as the second, the triggers as nothing.
//!
//! Start is Escape (the kit's), the game's menu in a race.

use std::sync::Mutex;
use win32::platform::dinput;

/// Our pad buttons (`dinput::CONTROLS`).
const RT: u32 = 0;
const LT: u32 = 1;
const B: u32 = 2;
const A: u32 = 3;
const X: u32 = 4;
const Y: u32 = 5;
const LB: u32 = 6;
const RB: u32 = 7;
const LEFT: u32 = 13;
const RIGHT: u32 = 14;

/// DirectInput key codes of the Keyboard layout's keys.
const UP_ARROW: u8 = 0xc8;
const DOWN_ARROW: u8 = 0xd0;
const LEFT_ARROW: u8 = 0xcb;
const RIGHT_ARROW: u8 = 0xcd;
const KEY_S: u8 = 0x1f;
const KEY_A: u8 = 0x1e;
const SHIFT: u8 = 0x2a;
const SPACE: u8 = 0x39;
const CTRL: u8 = 0x1d;
const F3: u8 = 0x3d;

/// Each key and the buttons that press it.
const KEYS: [(u8, u32); 9] = [
    (UP_ARROW, 1 << RT),
    (DOWN_ARROW, 1 << LT),
    (LEFT_ARROW, 1 << LEFT),
    (RIGHT_ARROW, 1 << RIGHT),
    (KEY_S, 1 << RB),
    (KEY_A, 1 << LB),
    (SHIFT, 1 << A),
    (SPACE, 1 << B),
    (CTRL, 1 << Y),
];

/// The camera key: pressed once per press of X (held, the game would switch every frame).
const CAMERA: (u8, u32) = (F3, 1 << X);

/// The stick steers past this far.
const STEER: f32 = 0.5;

/// The keys the pad holds down now.
static HELD: Mutex<Vec<u8>> = Mutex::new(Vec::new());
/// The buttons held at the last logic frame (for X's press).
static LAST: Mutex<u32> = Mutex::new(0);

/// A race's logic frame, before the game's: the keys as the pad has them.
pub fn race_frame() {
    let p = dinput::current_pad(0);
    let mut b = p.buttons();
    if p.stick[0] < -STEER {
        b |= 1 << LEFT;
    }
    if p.stick[0] > STEER {
        b |= 1 << RIGHT;
    }
    let mut last = LAST.lock().unwrap();
    let pressed = b & !*last;
    *last = b;
    let mut want: Vec<u8> = KEYS.iter().filter(|k| b & k.1 != 0).map(|k| k.0).collect();
    if pressed & CAMERA.1 != 0 {
        want.push(CAMERA.0);
    }
    set(want);
}

/// Out of a race: the pad holds no key.
pub fn release() {
    *LAST.lock().unwrap() = 0;
    set(Vec::new());
}

fn set(want: Vec<u8>) {
    let mut held = HELD.lock().unwrap();
    for &k in held.iter().filter(|k| !want.contains(k)) {
        dinput::key(k, false);
    }
    for &k in want.iter().filter(|k| !held.contains(k)) {
        dinput::key(k, true);
    }
    *held = want;
}

/// The buttons the game sees: in the menus A first and B second.
pub fn buttons(buttons: u32) -> u32 {
    if crate::frames::in_race() {
        return buttons;
    }
    release();
    let bit = |k: u32| buttons >> k & 1;
    buttons & !(1 << RT | 1 << LT | 1 << B | 1 << A) | bit(A) << RT | bit(B) << LT
}
