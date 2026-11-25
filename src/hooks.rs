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
