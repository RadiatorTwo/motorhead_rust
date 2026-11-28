//! The game's own input in its menus and its control options: the pad's buttons all
//! assignable there (with their names), the pad works the menus, the mouse does not.
//!
//! Each frame the game's input mapper (`INPUT_MAP`, 0x46522c) turns the input into a "key"
//! array of 256 codes (`KEYS`, 0x80 held): the keyboard's DirectInput codes, the joystick's
//! three axes (0xf3..0xf8), its first eight buttons (0xf2 down to 0xeb: the game reads no
//! more) and the mouse (0xe9, 0xea, 0xf9..0xff); then which codes are newly pressed (with
//! repeat). A race task bound to a code asks for it (`pad::CONTROL_VALUE`), the control
//! options' assignment (0x486a44) takes the first newly pressed code. With `MENU_PAD` set (the
//! menus) the mapper also makes the stick the arrow keys, button 1 Enter and button 2 Escape.
//!
//! Here, after the joystick's poll (`JOY_POLL`, the last read before the mapper; the keyboard's
//! read before it fills the whole array): our pad buttons beyond the eighth (Back, R3, L3, the D-pad) are
//! the game's unused codes `EXTRA` (named "UndefinedE2".. in its own table, so its layout
//! files keep them), Start the code `START` ("UndefinedE1"); in the menus the D-pad is the arrow keys, Back Escape (Start is Enter, the
//! kit's `pad_start`; sliders: `menu_step`). The assignment shows the pad's own names (`key_name`).

use recomp::rt::{Cpu, EAX, ESP, call_original};
use recomp::ver::Us;
use win32::platform::mem;

/// The game's joystick poll (DIJOYSTATE into [0x51c58c..]), at the input mapper's (0x46522c)
/// start after the mouse's and the keyboard's.
pub const JOY_POLL: Us = Us(0x43433c);
/// The game's key array: 256 codes, 0x80 held.
const KEYS: Us = Us(0x51c3d4);
/// 1: the joystick works the menus (stick arrows, buttons 1 and 2 Enter and Escape); 0 in the
/// control options' assignment.
const MENU_PAD: Us = Us(0x737e74);

/// A code's name (al): the system's key name, else the game's table (0x4dc710, 20 bytes a
/// code); the name is the code's in the layout files too.
pub const KEY_NAME: Us = Us(0x468150);
/// The control options' screen (0x4863ec..0x486a44): its calls of `KEY_NAME` show a name.
const OPTIONS_SCREEN: (Us, Us) = (Us(0x4863ec), Us(0x486a44));
/// Where the game builds a key's name (32 bytes).
const NAME_BUFFER: Us = Us(0x73807c);

/// The game's codes for our pad buttons 8..14 (Back, R3, L3, D-pad up, down, left, right):
/// unused in the game ("UndefinedE2".."UndefinedE8").
const EXTRA: [u8; 7] = [0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8];
/// The game's code for the pad's Start (unused in the game: "UndefinedE1").
pub const START: u8 = 0xe1;
/// The game's joystick button codes: our button n is 0xf2 - n (n < 8).
const JOY_BUTTON_1: u8 = 0xf2;

/// Keyboard codes the pad gives in the menus.
const ESCAPE: u8 = 0x01;
const ARROWS: [u8; 4] = [0xc8, 0xd0, 0xcb, 0xcd];
/// Our D-pad buttons (up, down, left, right) and Back.
const DPAD: u32 = 11;
const BACK: u32 = 8;

/// Our pad buttons' names (`dinput::CONTROLS` order).
const PAD_NAMES: [&str; 15] = [
    "Pad RT", "Pad LT", "Pad B", "Pad A", "Pad X", "Pad Y", "Pad LB", "Pad RB", "Pad Back",
    "Pad R3", "Pad L3", "Pad Up", "Pad Down", "Pad Left", "Pad Right",
];

/// The joystick's poll, then the pad's extra buttons put in the key array.
pub fn joy_poll(c: &mut Cpu) {
    call_original(c, JOY_POLL.at());
    let m = mem();
    let pad = win32::platform::dinput::current_pad(0);
    let buttons = pad.buttons();
    let held = |k: u32| buttons >> k & 1 != 0;
    let press = |code: u8| {
        let a = KEYS.at() + code as u32;
        m.wr8(a, m.rd8(a) | 0x80);
    };
    for (k, &code) in (BACK..).zip(&EXTRA) {
        if held(k) {
            press(code);
        }
    }
    if pad.start() {
        press(START);
    }
    if m.rd32(MENU_PAD.at()) == 1 && !crate::frames::in_race() {
        for (k, &code) in (DPAD..).zip(&ARROWS) {
            if held(k) {
                press(code);
            }
        }
        if held(BACK) {
            press(ESCAPE);
        }
    }
}

/// The pad's name for a game code (None: not the pad's).
fn pad_name(code: u8) -> Option<&'static str> {
    match code {
        0xeb..=JOY_BUTTON_1 => Some(PAD_NAMES[(JOY_BUTTON_1 - code) as usize]),
        0xe2..=0xe8 => Some(PAD_NAMES[8 + (code - 0xe2) as usize]),
        START => Some("Pad Start"),
        0xf3 => Some("Stick Left"),
        0xf4 => Some("Stick Right"),
        0xf5 => Some("Stick Up"),
        0xf6 => Some("Stick Down"),
        _ => None,
    }
}

/// A code's name: on the control options' screen the pad's own (the layout files keep the
/// game's).
pub fn key_name(c: &mut Cpu) {
    let m = mem();
    let from = m.rd32(c.r[ESP]);
    let code = c.r[EAX] as u8;
    let shown = (OPTIONS_SCREEN.0.at()..OPTIONS_SCREEN.1.at()).contains(&from);
    match pad_name(code).filter(|_| shown) {
        Some(name) => {
            let b = NAME_BUFFER.at();
            for (k, &ch) in name.as_bytes().iter().chain(&[0]).enumerate() {
                m.wr8(b + k as u32, ch);
            }
            c.r[EAX] = b;
            // `ret` as the game's function
            c.r[ESP] += 4;
        }
        None => call_original(c, KEY_NAME.at()),
    }
}

/// The pad's Start as a key: Escape in a race (its pause; nothing when the layout gives Start a
/// task) and videos, Enter in the menus, nothing while the control options assign a control
/// (there it is `START`, "Pad Start").
pub fn pad_start() -> win32::PadStart {
    let m = mem();
    if crate::frames::in_race() {
        if crate::pad::layout_uses(START) {
            win32::PadStart::Nothing
        } else {
            win32::PadStart::Escape
        }
    } else if m.rd32(ASSIGNING.at()) != 0 {
        win32::PadStart::Nothing
    } else if m.rd32(MENU_PAD.at()) == 1 {
        win32::PadStart::Enter
    } else {
        // a video (skipped by Escape)
        win32::PadStart::Escape
    }
}

/// The control options' assignment: its state, 0 while none runs.
const ASSIGNING: Us = Us(0x7ddbfc);

/// The options screens' step for a held arrow (graphics, sound, controls, personal: their
/// sliders move by it each frame while Left or Right is held): the frame's ms times 0.07
/// (x2 with Shift, x4 Ctrl, x0.5 Alt), in st0. The game's mouse speed moved ~3.5% a frame,
/// over 20% in a short tap: here the first frame of a press is `FIRST_STEP` (the mouse speed's
/// 0.03 a step: 1% exactly), then nothing until `HOLD_PAUSE`, then 20% of the game's speed
/// growing to all of it over `HOLD_RAMP`.
pub const MENU_STEP: Us = Us(0x47bb24);
const ARROW_LEFT: u8 = 0xcb;
const ARROW_RIGHT: u8 = 0xcd;
/// A press's first step.
const FIRST_STEP: f64 = 1.0 / 3.0;
/// Calls this soon after the press are still its first frame's.
const FIRST_FRAME: f64 = 0.004;
const HOLD_PAUSE: f64 = 0.35;
const HOLD_RAMP: f64 = 1.0;

/// Since when Left or Right is held (None: neither).
static HELD_SINCE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub fn menu_step(c: &mut Cpu) {
    call_original(c, MENU_STEP.at());
    let m = mem();
    let held = [ARROW_LEFT, ARROW_RIGHT]
        .iter()
        .any(|&k| m.rd8(KEYS.at() + k as u32) & 0x80 != 0);
    let mut since = HELD_SINCE.lock().unwrap();
    let step = c.st(0);
    let step = match (*since, held) {
        (_, false) => {
            *since = None;
            return;
        }
        (None, true) => {
            *since = Some(std::time::Instant::now());
            FIRST_STEP
        }
        (Some(t), true) => {
            let t = t.elapsed().as_secs_f64();
            if t < FIRST_FRAME {
                // called more than once a frame
                FIRST_STEP
            } else if t < HOLD_PAUSE {
                0.0
            } else {
                step * ((t - HOLD_PAUSE) / HOLD_RAMP).clamp(0.2, 1.0)
            }
        }
    };
    c.set_st(0, step);
}

/// The mouse's movement ([0x51c56c..]) and buttons ([0x51c4dc..]) as the mouse's poll left
/// them.
const MOUSE_MOVE: Us = Us(0x51c56c);
const MOUSE_BUTTONS: Us = Us(0x51c4dc);

/// After the mouse's poll: the menus see no mouse (a move alone changed settings there; they
/// need none).
pub fn mouse_polled() {
    if crate::frames::in_race() {
        return;
    }
    let m = mem();
    for k in 0..3 {
        m.wr32(MOUSE_MOVE.at() + 4 * k, 0);
        m.wr32(MOUSE_BUTTONS.at() + 4 * k, 0);
    }
}
