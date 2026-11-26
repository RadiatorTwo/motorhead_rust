//! The gamepad as a modern racing game has it, whatever control layout is chosen in the game.
//!
//! The game asks for each of its tasks (steer, accelerate...) through `CONTROL_VALUE`
//! (0x43bd60: eax the task's control code from the layout, edx where its strength goes, 0..1,
//! or 0; ebx 1: only a new press counts; returns whether it is active). The layout's codes for
//! the race's tasks are in `TASKS` (0x73ad18, one dword each, in the order of the .clo files).
//! A joystick axis gives a strength between 0 and 1 (steering and gas are analog), a key 1.
//!
//! - In a race the pad's controls answer for the tasks besides the layout's own: the left
//!   stick (and the D-pad) steers (squared: the user found it twitchy linear), RT accelerates, LT brakes, all analog; RB/LB change gear,
//!   A handbrake, B horn, Y rear view, X the camera.
//! - In the menus the game takes the pad's first button (ours: RT) to confirm: there A is
//!   reported as the first button and B as the second, the triggers as nothing.
//!
//! Start is Escape (the kit's), the game's menu in a race.

use recomp::rt::{Cpu, EAX, EBX, EDX, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use win32::platform::{dinput, mem};

/// The game's question for one task (see the module's head).
pub const CONTROL_VALUE: Us = Us(0x43bd60);
/// The control codes of the tasks, in the layout's order.
const TASKS: Us = Us(0x73ad18);

/// Our pad buttons (`dinput::CONTROLS`).
const B: u32 = 2;
const A: u32 = 3;
const X: u32 = 4;
const Y: u32 = 5;
const LB: u32 = 6;
const RB: u32 = 7;
const DPAD_LEFT: u32 = 13;
const DPAD_RIGHT: u32 = 14;

/// The tasks (their index in `TASKS`).
const TURN_LEFT: u32 = 0;
const TURN_RIGHT: u32 = 1;
const ACCELERATE: u32 = 2;
const BRAKE: u32 = 3;
const GEAR_UP: u32 = 4;
const GEAR_DOWN: u32 = 5;
const HORN: u32 = 6;
const HAND_BRAKE: u32 = 7;
const REAR_VIEW: u32 = 8;
const CAMERA: u32 = 14;
/// The tasks looked at.
const COUNT: u32 = 24;

/// The stick's dead zone: inside it nothing, beyond it 0..1.
const DEAD_ZONE: f32 = 0.12;

/// How strongly the pad asks for task `t` now (0: not at all).
fn strength(t: u32, p: &dinput::Pad) -> f32 {
    let b = |k: u32| (p.buttons() >> k & 1) as f32;
    // squared: fine near the middle, full at the edge
    let stick = |s: f32| ((s - DEAD_ZONE) / (1.0 - DEAD_ZONE)).clamp(0.0, 1.0).powi(2);
    match t {
        TURN_LEFT => stick(-p.stick[0]).max(b(DPAD_LEFT)),
        TURN_RIGHT => stick(p.stick[0]).max(b(DPAD_RIGHT)),
        ACCELERATE => p.triggers[0],
        BRAKE => p.triggers[1],
        GEAR_UP => b(RB),
        GEAR_DOWN => b(LB),
        HORN => b(B),
        HAND_BRAKE => b(A),
        REAR_VIEW => b(Y),
        CAMERA => b(X),
        _ => 0.0,
    }
}

/// Each task's pad state at its last question (for "only a new press").
static HELD: Mutex<[bool; COUNT as usize]> = Mutex::new([false; COUNT as usize]);

/// The game's `CONTROL_VALUE`, with the pad's answer in a race.
pub fn control_value(c: &mut Cpu) {
    let (code, out, new_only) = (c.r[EAX], c.r[EDX], c.r[EBX] != 0);
    call_original(c, CONTROL_VALUE.at());
    if !crate::frames::in_race() {
        return;
    }
    let m = mem();
    let Some(t) = (0..COUNT).find(|&t| m.rd32(TASKS.at() + 4 * t) == code) else {
        return;
    };
    let v = strength(t, &dinput::current_pad(0));
    let mut held = HELD.lock().unwrap();
    let was = std::mem::replace(&mut held[t as usize], v > 0.0);
    if v <= 0.0 || (new_only && was) {
        return;
    }
    if out != 0 {
        // the stronger of the layout's own and the pad's
        let game = if c.r[EAX] != 0 { f32::from_bits(m.rd32(out)) } else { 0.0 };
        m.wr32(out, v.max(game).to_bits());
    }
    c.r[EAX] = 1;
    c.r[EDX] = 1;
}

/// The buttons the game sees: in the menus A first and B second.
pub fn buttons(buttons: u32) -> u32 {
    if crate::frames::in_race() {
        return buttons;
    }
    let bit = |k: u32| buttons >> k & 1;
    buttons & !(1 | 2 | 1 << B | 1 << A) | bit(A) | bit(B) << 1
}
