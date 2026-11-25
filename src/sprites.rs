//! The 2D sprites (data\*.SPR: the menus, the race's overlays). Each sprite is a run of
//! machine code per row (`mov [eax+d], constant`, `ret`), which the game's three sprite
//! drawers call with eax at the row in the picture. That code is not compiled
//! (`loaded.rs`): the drawers are replaced by these, each the game's loop instruction for
//! instruction, the registers carried through the rows as in the original.
//!
//! The loaded sprite set: [0] the number of sprites, [8] the rows of each, [0xc + 4k] sprite
//! k's offset; a sprite: per row the offset of its code (from the sprite).

use crate::loaded::{self, Regs};
use recomp::rt::{Cpu, ESP};
use recomp::ver::Us;
use win32::platform::mem;

/// Draws sprite [INDEX] at ([X], [Y]), rows above the picture's top cut off, rows past
/// [BOTTOM] too.
pub const DRAW_CLIPPED: Us = Us(0x410010);
/// Draws sprite [INDEX] at ([X], [Y]), a row every [STEP] bytes; rows outside the picture
/// ([BASE] .. [END]) are left out.
pub const DRAW_STEP: Us = Us(0x41009a);
/// Draws [STEP] rows from the list at [LIST] (pairs: offset from the sprite's place, row of
/// sprite [INDEX]); [INDEX] is overwritten with the place.
pub const DRAW_LIST: Us = Us(0x410113);

/// the picture: first byte, bytes a row, rows, one past its last byte
const BASE: Us = Us(0x513ed8);
const PITCH: Us = Us(0x513edc);
const BOTTOM: Us = Us(0x513ee0);
const END: Us = Us(0x513f00);
/// the loaded sprite set
const SET: Us = Us(0x513ee4);
const X: Us = Us(0x513ee8);
const Y: Us = Us(0x513eec);
const INDEX: Us = Us(0x513ef0);
const LIST: Us = Us(0x513ef8);
const STEP: Us = Us(0x513efc);

const EAX: usize = 0;
const ECX: usize = 1;
const EDX: usize = 2;
const EBX: usize = 3;
const EBP: usize = 5;
const ESI: usize = 6;
const EDI: usize = 7;

fn rd(a: Us) -> u32 {
    mem().rd32(a.at())
}

/// The sprite set and sprite [INDEX]'s address, if there is such a sprite.
fn sprite() -> Option<(u32, u32)> {
    let set = rd(SET);
    if set == 0 || rd(INDEX) as i32 >= mem().rd32(set) as i32 {
        return None;
    }
    Some((set, set.wrapping_add(mem().rd32(set + rd(INDEX) * 4 + 0xc))))
}

/// Call the row code at `code` with the registers `r` (esp unchanged, the code returns).
fn row(code: u32, r: &mut [u32; 8]) {
    let mut regs = Regs { r: *r, st0: 0 };
    loaded::run(code, &mut regs);
    *r = regs.r;
}

fn ret(c: &mut Cpu) {
    // every register is saved and restored by the game's drawers
    c.r[ESP] += 4;
}

pub fn draw_clipped(c: &mut Cpu) {
    let m = mem();
    let mut r = c.r;
    let mut y = rd(Y) as i32;
    r[EDI] = 0;
    if y < 0 {
        r[EDI] = y.wrapping_neg() as u32;
        y = 0;
    }
    r[EBX] = (rd(X) as i32)
        .wrapping_add(y.wrapping_mul(rd(PITCH) as i32))
        .wrapping_add(rd(BASE) as i32) as u32;
    let Some((set, spr)) = sprite() else {
        return ret(c);
    };
    r[EBP] = spr;
    let rows = (m.rd32(set + 8) as i32).wrapping_sub(r[EDI] as i32);
    if rows < 0 {
        return ret(c);
    }
    r[ECX] = rows as u32;
    r[EDX] = spr.wrapping_add(r[EDI] * 4);
    let below = (rd(BOTTOM) as i32).wrapping_sub(rows).wrapping_sub(rd(Y) as i32);
    if below < 0 {
        let rows = rows.wrapping_add(below);
        if rows < 0 {
            return ret(c);
        }
        r[ECX] = rows as u32;
    }
    // the game's loop runs at least once (and 2^32 times for 0 rows): none here
    while r[ECX] != 0 {
        r[EDI] = rd(PITCH);
        r[ESI] = m.rd32(r[EDX]).wrapping_add(r[EBP]);
        r[EAX] = r[EBX];
        r[EDX] = r[EDX].wrapping_add(4);
        row(r[ESI], &mut r);
        r[EBX] = r[EBX].wrapping_add(r[EDI]);
        r[ECX] = r[ECX].wrapping_sub(1);
    }
    ret(c)
}

pub fn draw_step(c: &mut Cpu) {
    let m = mem();
    let mut r = c.r;
    r[EBX] = rd(Y)
        .wrapping_mul(rd(PITCH))
        .wrapping_add(rd(X))
        .wrapping_add(rd(BASE));
    let Some((set, spr)) = sprite() else {
        return ret(c);
    };
    r[EBP] = spr;
    r[EDX] = spr;
    r[ECX] = m.rd32(set + 8);
    r[EDI] = rd(PITCH);
    loop {
        r[ESI] = m.rd32(r[EDX]);
        r[EAX] = r[EBX];
        if r[EAX] as i32 >= rd(END) as i32 {
            break;
        }
        r[EDX] = r[EDX].wrapping_add(4);
        if r[EAX] as i32 >= rd(BASE) as i32 {
            r[ESI] = r[ESI].wrapping_add(r[EBP]);
            row(r[ESI], &mut r);
        }
        r[EBX] = r[EBX].wrapping_add(rd(STEP));
        r[ECX] = r[ECX].wrapping_sub(1);
        if r[ECX] == 0 {
            break;
        }
    }
    ret(c)
}

pub fn draw_list(c: &mut Cpu) {
    let m = mem();
    let mut r = c.r;
    r[EDI] = rd(Y)
        .wrapping_mul(rd(PITCH))
        .wrapping_add(rd(X))
        .wrapping_add(rd(BASE));
    let Some((_, spr)) = sprite() else {
        return ret(c);
    };
    r[EBP] = spr;
    r[EDX] = spr;
    r[ECX] = rd(STEP);
    m.wr32(INDEX.at(), r[EDI]);
    r[EDX] = rd(LIST);
    r[EDI] = rd(PITCH);
    loop {
        r[EAX] = m.rd32(r[EDX]).wrapping_add(rd(INDEX));
        if r[EAX] as i32 >= rd(END) as i32 {
            break;
        }
        if r[EAX] as i32 >= rd(BASE) as i32 {
            r[EBX] = m.rd32(r[EDX] + 4);
            r[ESI] = m.rd32(r[EBP].wrapping_add(r[EBX] * 4)).wrapping_add(r[EBP]);
            r[EBX] = r[EAX];
            row(r[ESI], &mut r);
        }
        r[EDX] = r[EDX].wrapping_add(8);
        r[ECX] = r[ECX].wrapping_sub(1);
        if r[ECX] == 0 {
            break;
        }
    }
    ret(c)
}
