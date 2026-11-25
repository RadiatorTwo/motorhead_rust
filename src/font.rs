//! The menu's text. Motorhead's fonts (data\FONT*.FNT) are machine code: each glyph is a run of
//! `add eax, edi|esi` (next row, two rows), `mov [eax+d], bl|bx|ebx` and `fst qword [eax+d]`
//! (pixels in the text colour, which the game keeps in ebx and in st0), ending in a jump the
//! game points back at its loop (0x4ba0ec patches it to 0x415918 when the font is loaded).
//! That code is not compiled (`loaded.rs`): the string drawer 0x415878 is replaced by this.
//!
//! The loaded font: per character k an entry of 8 bytes at 8k: +4 the glyph's code (offset
//! in the font), +8 the advance in bytes.

use crate::loaded::{self, Regs};
use recomp::rt::{Cpu, ESP};
use recomp::ver::Us;
use win32::platform::mem;

/// The string drawer: draws [STRING] at [DEST] with the font [FONT], rows [PITCH] bytes apart.
pub const FONT_DRAW: Us = Us(0x415878);

/// 0: 8-bit pixels (colour [COLOUR8]), 1: 16-bit (colour [COLOUR16])
const DEPTH: Us = Us(0x4d37b4);
const COLOUR16: Us = Us(0x4d37b8);
const COLOUR8: Us = Us(0x4d37c0);
const STRING: Us = Us(0x4d37c8);
const FONT: Us = Us(0x4d37cc);
const DEST: Us = Us(0x4d37d0);
const PITCH: Us = Us(0x4d37d4);

pub fn draw(c: &mut Cpu) {
    let m = mem();
    // the colour as the game builds it in ebx: one 8-bit pixel four times, or one 16-bit
    // pixel twice; st0 holds ebx twice
    let colour = if m.rd32(DEPTH.at()) == 0 {
        u32::from_ne_bytes([m.rd8(COLOUR8.at()); 4])
    } else {
        let c16 = m.rd16(COLOUR16.at()) as u32;
        c16 | c16 << 16
    };
    let pitch = m.rd32(PITCH.at());
    let font = m.rd32(FONT.at());
    let mut s = m.rd32(STRING.at());
    let mut dest = m.rd32(DEST.at());
    loop {
        let ch = m.rd8(s) as u32;
        if ch == 0 {
            break;
        }
        let mut regs = Regs {
            r: [dest, s, 0, colour, 0, font, 2 * pitch, pitch],
            st0: (colour as u64) << 32 | colour as u64,
        };
        loaded::run(font + m.rd32(font + ch * 8 + 4), &mut regs);
        dest = dest.wrapping_add(m.rd32(font + ch * 8 + 8));
        s += 1;
    }
    // registers and the FPU are as the game's leaves them (all saved, st0 pushed and popped)
    c.r[ESP] += 4;
}
