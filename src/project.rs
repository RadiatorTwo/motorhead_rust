//! Project_Point (0x414e54) in Rust: the renderer's hottest small function (0x456d4c calls it
//! for every point of every face, ~4700 a picture at a race's start).
//!
//! A point (edx: x, y, z, floats) through the matrix (ebx: 12 floats, m[k] = [ebx + 4k]) to
//! the screen (eax: x, y, z, 1/w), with clip flags at [ecx]: 1 left, 2 right, 4 top, 8 bottom,
//! 0x10 in front of the near plane but too near ([+8] not computed), 0x20 behind.
//!
//! The same operations in the same order as the game's code. The renderer runs the FPU at 24
//! bits to nearest (fcw 0x07f, set at 0x414ca4): each operation is done on doubles and rounded
//! to a float. That is the FPU's result exactly while it is a float's normal number (or 0):
//! the operands are floats' values, and a double holds products exactly and rounds sums and
//! quotients innocuously (53 >= 2*24+2). For any other result (a float's subnormal or beyond
//! its range: the FPU keeps its wider exponent) and any other control word the game's own
//! code runs. The x87 registers are left as the game's code leaves them, the popped ones
//! included (and in the behind case two slots pushed: it ends with ffree, which pops nothing).
//!
//! `MOTORHEAD_CHECK_PROJECT=1` runs both and compares registers, x87 registers and the memory
//! written ([eax], [ecx]; the stack below esp is the function's scratch and is not compared).

use recomp::rt::{call_original, Cpu, EAX, EBX, ECX, EDX, ESP};
use recomp::ver::Us;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub const PROJECT_POINT: Us = Us(0x414e54);

const NEAR_B4: Us = Us(0x4d17b4);
const NEAR_B8: Us = Us(0x4d17b8);
const SCALE_X: Us = Us(0x4d17c4);
const SCALE_Y: Us = Us(0x4d17c8);
const CLIP_RIGHT: Us = Us(0x521290);
const CLIP_BOTTOM: Us = Us(0x521294);
const CENTER_X: Us = Us(0x521298);
const CENTER_Y: Us = Us(0x52129c);
const SCALE_Z: Us = Us(0x5212b0);

pub fn project_point(c: &mut Cpu) {
    if check() {
        checked(c);
    } else if !fast(c) {
        call_original(c, PROJECT_POINT.at());
    }
}

/// A float's value rounded as the FPU does at 24 bits; `ok` false if that may differ.
#[inline(always)]
fn q(x: f64, ok: &Cell<bool>) -> f64 {
    let f = x as f32;
    ok.set(ok.get() & (x == 0.0 || (x.abs() >= f32::MIN_POSITIVE as f64 && f.is_finite())));
    f as f64
}

/// The sign bit of a float stored and compared as an integer (`cmp dword [..], 0`).
#[inline(always)]
fn negative(x: f64) -> bool {
    (x as f32).is_sign_negative()
}

/// The game's function in Rust. False (nothing changed but [ecx] = 0, which the game's code
/// writes first too) when the game's code must run.
fn fast(c: &mut Cpu) -> bool {
    // precision 24 bits (bits 8-9 = 0), to nearest (bits 10-11 = 0)
    if c.fcw & 0xf00 != 0 {
        return false;
    }
    let m = win32::platform::mem();
    let (out, flags, pt, mx) = (c.r[EAX], c.r[ECX], c.r[EDX], c.r[EBX]);
    m.wr32(flags, 0);
    let f = |a: u32| f32::from_bits(m.rd32(a)) as f64;
    let g = |k: u32| f(mx + 4 * k);
    let (x, y, z) = (f(pt), f(pt + 4), f(pt + 8));
    let ok = Cell::new(true);
    let q = |v: f64| q(v, &ok);

    // w (depth): (x m2 + y m5) + (z m8 + m11)
    let s1 = q(q(x * g(2)) + q(y * g(5)));
    let s2 = q(q(z * g(8)) + g(11));
    let w = q(s1 + s2);
    let ym3 = q(y * g(3));
    let s3 = q(q(x * g(0)) + ym3);
    let d = q(f(NEAR_B8.at()) - w);
    let s4 = q(q(z * g(6)) + g(9));
    let sx0 = q(s3 + s4);
    // the slot k pushes below the entry's top
    let top = c.top;
    let p = |k: u32| (top.wrapping_sub(k) & 7) as usize;

    if (d as f32).to_bits() as i32 <= 0 {
        if !ok.get() {
            return false;
        }
        // behind: [ecx] = 0x20, two slots left pushed
        c.st[p(1)] = w;
        c.st[p(2)] = sx0;
        c.st[p(3)] = s4;
        c.st[p(4)] = d;
        c.st[p(5)] = ym3;
        c.top = p(2) as u32;
        m.wr32(flags, m.rd32(flags) | 0x20);
        c.r[ESP] += 4;
        return true;
    }

    let e = q(w - f(NEAR_B4.at()));
    let ym4 = q(y * g(4));
    let s5 = q(q(x * g(1)) + ym4);
    let s6 = q(q(z * g(7)) + g(10));
    let sy0 = q(s5 + s6);

    if (e as f32).to_bits() as i32 <= 0 {
        if !ok.get() {
            return false;
        }
        // too near: x, y, w stored without the projection, [ecx] = 0x10
        m.wr32(out, (sx0 as f32).to_bits());
        m.wr32(out + 4, (sy0 as f32).to_bits());
        m.wr32(flags, m.rd32(flags) | 0x10);
        m.wr32(out + 8, (w as f32).to_bits());
        c.st[p(1)] = w;
        c.st[p(2)] = sy0;
        c.st[p(3)] = sx0;
        c.st[p(4)] = s6;
        c.st[p(5)] = e;
        c.st[p(6)] = ym4;
        c.r[ESP] += 4;
        return true;
    }

    let r = q(1.0 / w);
    let sx = q(q(q(sx0 * f(SCALE_X.at())) * r) + f(CENTER_X.at()));
    let sy = q(q(q(sy0 * f(SCALE_Y.at())) * r) + f(CENTER_Y.at()));
    let wz = q(w * f(SCALE_Z.at()));
    let u = q(f(CLIP_RIGHT.at()) - sx);
    let v = q(f(CLIP_BOTTOM.at()) - sy);
    if !ok.get() {
        return false;
    }
    m.wr32(out + 8, (wz as f32).to_bits());
    m.wr32(out + 12, (r as f32).to_bits());
    m.wr32(out, (sx as f32).to_bits());
    m.wr32(out + 4, (sy as f32).to_bits());
    let mut fl = 0;
    if negative(sx) {
        fl |= 1;
    }
    if negative(sy) {
        fl |= 4;
    }
    if negative(u) {
        fl |= 2;
    }
    if negative(v) {
        fl |= 8;
    }
    m.wr32(flags, fl);
    c.st[p(1)] = v;
    c.st[p(2)] = u;
    c.st[p(3)] = r;
    c.st[p(4)] = wz;
    c.st[p(5)] = e;
    c.st[p(6)] = ym4;
    c.r[ESP] += 4;
    true
}

fn check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| recomp::config::env_on("CHECK_PROJECT"))
}

/// What the function changes: registers, x87 registers and TOP, status word, [eax..+16],
/// [ecx].
#[derive(PartialEq, Clone, Copy)]
struct State {
    r: [u32; 8],
    st: [u64; 8],
    top: u32,
    fsw: u16,
    out: [u32; 4],
    flags: u32,
}

fn state(c: &Cpu, out: u32, flags: u32) -> State {
    let m = win32::platform::mem();
    State {
        r: c.r,
        st: c.st.map(f64::to_bits),
        top: c.top,
        fsw: c.fsw,
        out: [0, 4, 8, 12].map(|k| m.rd32(out + k)),
        flags: m.rd32(flags),
    }
}

fn put(c: &mut Cpu, s: &State, out: u32, flags: u32) {
    let m = win32::platform::mem();
    c.r = s.r;
    c.st = s.st.map(f64::from_bits);
    c.top = s.top;
    c.fsw = s.fsw;
    for (k, v) in s.out.iter().enumerate() {
        m.wr32(out + 4 * k as u32, *v);
    }
    m.wr32(flags, s.flags);
}

fn checked(c: &mut Cpu) {
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static SLOW: AtomicU64 = AtomicU64::new(0);
    static DIFF: AtomicU64 = AtomicU64::new(0);
    let (out, flags) = (c.r[EAX], c.r[ECX]);
    let before = state(c, out, flags);
    call_original(c, PROJECT_POINT.at());
    let game = state(c, out, flags);
    put(c, &before, out, flags);
    if !fast(c) {
        SLOW.fetch_add(1, Relaxed);
        call_original(c, PROJECT_POINT.at());
    }
    let ours = state(c, out, flags);
    let n = CALLS.fetch_add(1, Relaxed) + 1;
    if ours != game {
        let d = DIFF.fetch_add(1, Relaxed) + 1;
        if d <= 20 {
            let m = win32::platform::mem();
            let fl = |a: u32| f32::from_bits(m.rd32(a));
            let mx: Vec<f32> = (0..12).map(|k| fl(c.r[EBX] + 4 * k)).collect();
            eprintln!(
                "project check: differs (call {n}): point {:?} matrix {mx:?} fcw {:#x}",
                [fl(c.r[EDX]), fl(c.r[EDX] + 4), fl(c.r[EDX] + 8)],
                c.fcw,
            );
            let st = |s: &State| s.st.map(f64::from_bits);
            eprintln!(
                "  game: r {:x?} top {} fsw {:#x} out {:x?} flags {:#x} st {:?}",
                game.r, game.top, game.fsw, game.out, game.flags, st(&game)
            );
            eprintln!(
                "  ours: r {:x?} top {} fsw {:#x} out {:x?} flags {:#x} st {:?}",
                ours.r, ours.top, ours.fsw, ours.out, ours.flags, st(&ours)
            );
        }
    }
    if n.is_power_of_two() || n % 1_000_000 == 0 {
        eprintln!(
            "project check: {n} calls, {} differ, {} by the game's code",
            DIFF.load(Relaxed),
            SLOW.load(Relaxed)
        );
    }
}
