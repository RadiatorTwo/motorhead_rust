//! Render_Polygons (0x456d4c) in Rust, piece by piece: a face of an object to a polygon for
//! the renderer (see PLAN.md and data/symbols.txt for the data). What is written in Rust
//! runs here; for the ways not written yet the game's function runs, from its start: all it
//! did before the polygon goes into its list it does again the same way (the caches are
//! filled, the polygon's record is written over), so a way may be left before that.
//!
//! `MOTORHEAD_CHECK_POLYGONS=n`: every n-th call runs the game's function and this one on
//! the same memory and compares memory (but the stack below esp) and registers.

use crate::fpu::{Fpu, from_f32};
use recomp::rt::{Cpu, EAX, EBX, ECX, EDX, ESP, call_original};
use recomp::ver::Us;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub const RENDER_POLYGONS: Us = Us(0x456d4c);

/// the polygon being made (its record, 0xc0 bytes)
const POLYGON: Us = Us(0x521220);
/// the projected points (16 bytes each: x, y, z, 1/w; x = -1: not yet)
const PROJECTED: Us = Us(0x5e12b4);
/// their clip flags (see 0x414e54)
const CLIP: Us = Us(0x6e12b4);
/// the detail level (4: low)
const DETAIL: Us = Us(0x520fe8);
/// the list the polygon goes into
const LIST: Us = Us(0x521224);

pub fn render_polygons(c: &mut Cpu) {
    if let Some(n) = check_every()
        && CALLS.fetch_add(1, Relaxed) % n == 0
    {
        checked(c);
    } else {
        ours(c);
    }
}

static CALLS: AtomicU64 = AtomicU64::new(0);
static KEYS: std::sync::Mutex<std::collections::BTreeMap<u32, u64>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn check_every() -> Option<u64> {
    static N: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        recomp::config::env("CHECK_POLYGONS").map(|v| v.parse().unwrap_or(1).max(1))
    })
}

/// The function's ways, counted (`MOTORHEAD_CHECK_POLYGONS` prints them)
#[derive(Clone, Copy, Debug)]
enum Way {
    /// a point behind the camera
    Behind,
    /// all points beyond one edge of the screen
    Outside,
    /// not written yet: the game's function
    Game,
    /// colour mode 1, close, not cut
    Mode1,
    /// no point close
    Far,
    /// colour mode 4, close, not cut
    Mode4,
    /// colour mode 4 with a reflection ([face+1] flag 1), close, not cut
    Reflect,
}
static WAYS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

fn ours(c: &mut Cpu) {
    let way = run(c);
    WAYS[way as usize].fetch_add(1, Relaxed);
}

/// Registers at the end of the game's function (the ones it leaves changed).
fn ret(c: &mut Cpu, eax: u32, ebx: u32, ecx: u32, edx: u32) {
    c.r[EAX] = eax;
    c.r[EBX] = ebx;
    c.r[ECX] = ecx;
    c.r[EDX] = edx;
    c.r[ESP] += 4;
}

fn run(c: &mut Cpu) -> Way {
    let m = win32::platform::mem();
    let entry = c.r;
    let (face, obj) = (c.r[EAX], c.r[EDX]);
    let fpu = Fpu::new(c.fcw);
    let rec = m.rd32(POLYGON.at());
    let (projected, clip) = (PROJECTED.at(), CLIP.at());

    // 0x456d4c: what the face's flags say
    // [esp+68], [esp+94], [esp+6c]
    let (near, z_far, z_fade) = if m.rd8(DETAIL.at()) == 4 {
        (0.5f32, 0x3f19_999au32, 0x4060_0000u32)
    } else {
        (f32::from_bits(0x3f33_3333), 0x3f4c_cccd, 0x40e0_0000)
    };
    m.wr32(rec, 0);
    let f0 = m.rd8(face);
    let n: u32 = if f0 & 1 != 0 {
        m.wr8(rec, m.rd8(rec) | 1);
        4
    } else {
        3
    };
    if f0 & 8 != 0 || m.rd8(face + 1) & 1 != 0 {
        m.wr8(rec, m.rd8(rec) | 8);
    }
    if f0 & 4 != 0 {
        m.wr8(rec, m.rd8(rec) | 8);
    }

    // 0x456e92: the points, projected if not yet (0x414e54), into the polygon
    let matrix = obj + 0x34;
    let points = m.rd32(face + 8);
    let (mut all, mut near_count, mut close) = (0x3fu32, 0u32, 0u32);
    // ecx as the game's code leaves it (0x456dc1, 0x456e16)
    let mut ecx = entry[ECX];
    if m.rd8(DETAIL.at()) != 4 {
        ecx = 0x3f33_3333;
    }
    if f0 & 1 == 0 {
        ecx = 3;
    }
    let mut ebx = entry[EBX];
    let mut flags = [0u8; 4];
    for k in 0..n {
        let v = m.rd16(points + 2 * k) as u32;
        let p = projected + 16 * v;
        if m.rd32(p) == 0xffff_ffff {
            ecx = clip + 4 * v;
            c.r[EAX] = p;
            c.r[EDX] = m.rd32(obj + 0x8c) + 12 * v;
            c.r[EBX] = matrix;
            c.r[ECX] = ecx;
            // (the return address: the game's caller's, as good as any)
            c.r[ESP] -= 4;
            m.wr32(c.r[ESP], m.rd32(entry[ESP]));
            crate::project::project_point(c);
            c.r = entry;
        }
        let at = rec + 0x20 * k;
        m.wr32(at + 4, m.rd32(p));
        m.wr32(at + 8, m.rd32(p + 4));
        m.wr32(at + 0xc, m.rd32(p + 8));
        ebx = m.rd32(p + 12);
        m.wr32(at + 0x10, ebx);
        let fl = m.rd32(clip + 4 * v);
        flags[k as usize] = fl as u8;
        if fl & 0x20 != 0 {
            ret(c, fl, ebx, ecx, k);
            return Way::Behind;
        }
        if fl & 0x10 != 0 {
            near_count += 1;
        }
        all &= fl;
        // (fcomp; ja: not above, or unordered)
        if !(from_f32(m.rd32(at + 0xc)) > near as f64) {
            close += 1;
        }
    }
    if all != 0 {
        ret(c, n, ebx, ecx, n);
        return Way::Outside;
    }
    let _ = flags;
    let f1 = m.rd8(face + 1);
    let mode = entry[ECX];
    // 0x4574d2: no point close (and the detail not low): no colours, alpha by depth
    if m.rd32(obj + 0xa8) == 0 && close == 0 && m.rd8(DETAIL.at()) != 4 {
        let textured = f1 & 4 != 0;
        if textured {
            texture(rec, face, n);
        }
        far(&fpu, rec, n, z_far, z_fade);
        let list = Us(if textured { 0x520f28 } else { 0x520f24 }).at();
        m.wr32(LIST.at(), list);
        m.wr32(rec + 0xbc, m.rd32(list));
        m.wr32(list, rec);
        m.wr32(POLYGON.at(), rec + 0xc0);
        ret(c, list, rec, n << 5, rec + 0xc0);
        return Way::Far;
    }
    // 0x457049: the way of the most faces (colour mode 1, a point close, not cut at the near
    // plane): 0x457f30 -> 0x457617 -> 0x457832 -> 0x458081 -> 0x4585ae -> 0x458909
    if m.rd32(obj + 0xa8) == 0
        && close != 0
        && mode == 1
        && near_count == 0
        && f1 & 1 == 0
        && !(f0 & 2 != 0 && f1 & 4 != 0)
    {
        colours_mode1(&fpu, face, rec, n);
        fog(&fpu, rec, n);
        let list = if f0 & 2 != 0 {
            texture(rec, face, n);
            m.rd32(Us(0x520f04).at()) + 4 * m.rd32(face + 0x14)
        } else {
            Us(0x520f20).at()
        };
        let head = finish(&fpu, rec, n, list);
        ret(c, face, rec, n, head);
        return Way::Mode1;
    }
    // 0x457b1b: colour mode 4 (lit), close, not cut, the face without flags 0x10 and 1 in
    // [+1]: 0x457bc4 -> 0x457cac -> 0x457832 -> 0x458081 -> 0x4585ae -> 0x458909
    if m.rd32(obj + 0xa8) == 0
        && close != 0
        && mode == 4
        && near_count == 0
        && f1 & 0x11 == 0
        && !(f0 & 2 != 0 && f1 & 4 != 0)
    {
        let lit = Lit { face, obj, rec, ebx: entry[EBX] };
        colours_mode4(c, &fpu, &lit, n);
        c.r = entry;
        fog(&fpu, rec, n);
        let list = if f0 & 2 != 0 {
            texture(rec, face, n);
            m.rd32(Us(0x520f04).at()) + 4 * m.rd32(face + 0x14)
        } else {
            Us(0x520f20).at()
        };
        let head = finish(&fpu, rec, n, list);
        ret(c, face, rec, n, head);
        return Way::Mode4;
    }
    // 0x457b1b with [face+1] flag 1 and [0x739f2c] != 1: the same colours, the reflection's
    // texture and uvs instead of the face's: 0x457bc4 -> 0x457832 -> 0x458081 -> 0x45839e ->
    // 0x45875e -> 0x458909 (-> 0x45895a: no second polygon)
    if m.rd32(obj + 0xa8) == 0
        && close != 0
        && mode == 4
        && near_count == 0
        && f1 & 0x11 == 1
        && m.rd32(REFLECT_TWICE.at()) != 1
    {
        let lit = Lit { face, obj, rec, ebx: entry[EBX] };
        colours_mode4(c, &fpu, &lit, n);
        c.r = entry;
        fog(&fpu, rec, n);
        reflection(&fpu, face, obj, rec, n);
        let head = finish(&fpu, rec, n, Us(0x520f0c).at());
        ret(c, face, rec, n, head);
        return Way::Reflect;
    }
    if check_every().is_some() {
        // which way the game's function goes on (for choosing what to write next)
        let key = (m.rd32(obj + 0xa8) != 0) as u32
            | ((close != 0) as u32) << 1
            | ((m.rd8(DETAIL.at()) == 4) as u32) << 2
            | (entry[ECX] & 7) << 3
            | (m.rd8(face + 1) as u32 & 0x15) << 6
            | ((f0 as u32 & 2) << 11)
            | ((m.rd32(Us(0x739f2c).at()) == 1) as u32) << 14
            | ((near_count != 0) as u32) << 15;
        *KEYS.lock().unwrap().entry(key).or_default() += 1;
    }
    c.r = entry;
    call_original(c, RENDER_POLYGONS.at());
    Way::Game
}

/// A double in memory.
fn q64(a: u32) -> f64 {
    let m = win32::platform::mem();
    f64::from_bits(m.rd32(a) as u64 | (m.rd32(a + 4) as u64) << 32)
}

/// fistp qword of an integer's value (rounding does not matter): out of range or NaN gives
/// the "integer indefinite".
fn fistp64(x: f64) -> i64 {
    if (-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&x) {
        x as i64
    } else {
        i64::MIN
    }
}

/// 0x441d0c: the three floats at `a` kept between `lo` and `hi` (a NaN becomes `lo`).
fn clamp(a: u32, lo: u32, hi: u32) {
    let m = win32::platform::mem();
    for k in 0..3 {
        let v = from_f32(m.rd32(a + 4 * k));
        if v > from_f32(hi) {
            m.wr32(a + 4 * k, hi);
        } else if !(v >= from_f32(lo)) {
            m.wr32(a + 4 * k, lo);
        }
    }
}

const COLOUR_SCALE: Us = Us(0x4dc1e8);

/// 0x457617: colours from the face's colour bytes times `COLOUR_SCALE` (red, green, blue),
/// kept in 0..255: the first point's, every point's for a face with flag 4.
fn colours_mode1(fpu: &Fpu, face: u32, rec: u32, n: u32) {
    let m = win32::platform::mem();
    let scale = COLOUR_SCALE.at();
    let points = if m.rd8(face) & 4 != 0 { n } else { 1 };
    for k in 0..points {
        let bytes = m.rd32(face + 0x10) + 4 * k;
        let at = rec + 0x84 + 12 * k;
        for j in 0..3 {
            let b = m.rd8(bytes + j) as f64;
            let v = fpu.mul(b, from_f32(m.rd32(scale + 4 * j)));
            m.wr32(at + 4 * j, fpu.f32_bits(v));
        }
        clamp(at, 0, 0x437f_0000);
    }
}

struct Lit {
    face: u32,
    obj: u32,
    rec: u32,
    /// the function's ebx (0x455e40's)
    ebx: u32,
}

const LIGHT: Us = Us(0x5212b4);
const LIGHT_NORMAL: Us = Us(0x455e40);

/// 0x457bc4: colour mode 4: a base colour (the face's colour bytes, or for a textured face
/// without them the constant one times [0x4fbc7e]) plus the normals' light (0x455e40 if not
/// yet worked out, 12 bytes at `LIGHT`) times 50 or 1, times the light again; the first
/// normal's, every normal's for a face with flag 8.
fn colours_mode4(c: &mut Cpu, fpu: &Fpu, l: &Lit, n: u32) {
    let m = win32::platform::mem();
    let (f0, f1) = (m.rd8(l.face), m.rd8(l.face + 1));
    let k = Us(0x4dc208).at();
    let (base, times) = if f1 & 1 == 0 && f0 & 2 != 0 {
        // 0x457c5d
        let q = q64(Us(0x4fbc7e).at());
        let c3 = [0, 1, 2].map(|j| fpu.f32_bits(fpu.mul(from_f32(m.rd32(k + 4 * j)), q)));
        (c3, 0x4248_0000u32)
    } else {
        // 0x457bd2
        let bytes = m.rd32(l.face + 0x10);
        let c3 = [0, 1, 2].map(|j| {
            fpu.f32_bits(fpu.mul(m.rd8(bytes + j) as f64, from_f32(m.rd32(k + 4 * j))))
        });
        (c3, if f1 & 1 != 0 { 0x4248_0000 } else { 0x3f80_0000 })
    };
    let each = (f0 & 8 != 0 || f1 & 1 != 0) && n > 1;
    let normals = m.rd32(l.face + 0xc);
    for j in 0..if each { n } else { 1 } {
        let v = m.rd16(normals + 2 * j) as u32;
        let a = LIGHT.at() + 12 * v;
        if m.rd32(a) == 0xffff_ffff {
            c.r[EAX] = a;
            c.r[EDX] = m.rd32(l.obj + 0x94) + 12 * v;
            c.r[EBX] = l.ebx;
            win32::platform::guest_call(c, LIGHT_NORMAL.at(), &[]);
        }
        let f = from_f32(times);
        let lx = from_f32(m.rd32(a));
        let ly = from_f32(m.rd32(a + 4));
        let lz = from_f32(m.rd32(a + 8));
        let x = fpu.add(fpu.mul(lx, f), from_f32(base[0]));
        let y = fpu.add(fpu.mul(ly, f), from_f32(base[1]));
        let z = fpu.add(fpu.mul(f, lz), from_f32(base[2]));
        let at = l.rec + 0x84 + 12 * j;
        m.wr32(at, fpu.f32_bits(fpu.mul(x, lx)));
        let y = from_f32(fpu.f32_bits(y));
        let z = from_f32(fpu.f32_bits(z));
        m.wr32(at + 4, fpu.f32_bits(fpu.mul(y, ly)));
        m.wr32(at + 8, fpu.f32_bits(fpu.mul(z, lz)));
        clamp(at, 0, 0x437f_0000);
    }
}

/// 0x457832: each point's alpha ([+0x18], top byte) by its depth: opaque near, fading out
/// from `FOG_FROM` on.
fn fog(fpu: &Fpu, rec: u32, n: u32) {
    let m = win32::platform::mem();
    let (from, add, minus, scale, to_byte) = (
        q64(Us(0x4fbcf2).at()),
        q64(Us(0x4fbd06).at()),
        q64(Us(0x4fbcfa).at()),
        q64(Us(0x4fbc96).at()),
        q64(Us(0x4fbc9e).at()),
    );
    for k in 0..n {
        let at = rec + 0x20 * k;
        let z = from_f32(m.rd32(at + 0xc));
        if !(z >= from) {
            m.wr32(at + 0x18, 0xff00_0000);
            continue;
        }
        let t = fpu.sub(minus, fpu.add(z, add));
        let s = if 0.0 > t {
            0
        } else {
            let u = fpu.mul(t, scale);
            if 1.0 >= u { fpu.f32_bits(u) } else { 0x3f80_0000 }
        };
        // 0x4c17d6: frndint, truncating
        let v = fpu.mul(from_f32(s), to_byte).trunc();
        m.wr32(at + 0x18, (fistp64(v) as u32) << 24);
    }
}

/// 0x457547: each point's colour [+0x14] = [0x521228] with an alpha by its depth (opaque
/// from `z_far` + 1 / `z_fade` on), its [+0xc] = [0x521274] times its [+0x10].
fn far(fpu: &Fpu, rec: u32, n: u32, z_far: u32, z_fade: u32) {
    let m = win32::platform::mem();
    let (to_byte, k0) = (
        q64(Us(0x4fbc9e).at()),
        from_f32(m.rd32(Us(0x521274).at())),
    );
    for k in 0..n {
        let at = rec + 0x20 * k;
        m.wr32(at + 0x14, m.rd32(Us(0x521228).at()));
        let t = fpu.sub(from_f32(m.rd32(at + 0xc)), from_f32(z_far));
        let s = if 0.0 > t { 0 } else { fpu.f32_bits(t) };
        let a = fpu.mul(from_f32(s), from_f32(z_fade));
        let s = if 1.0 >= a {
            if 0.0 > a { 0 } else { fpu.f32_bits(a) }
        } else {
            0x3f80_0000
        };
        let v = fpu.mul(from_f32(s), to_byte);
        let w = fpu.mul(k0, from_f32(m.rd32(at + 0x10)));
        let alpha = (fistp64(v.trunc()) as u32) << 24;
        m.wr32(at + 0xc, fpu.f32_bits(w));
        m.wr32(at + 0x14, m.rd32(at + 0x14) | alpha);
    }
}

/// 0x4585b7: a textured face: its texture ([0x520d48] by [face+0x14]) and its points' uvs.
fn texture(rec: u32, face: u32, n: u32) {
    let m = win32::platform::mem();
    m.wr8(rec, m.rd8(rec) | 2);
    m.wr32(rec + 0xb4, m.rd32(Us(0x520d48).at() + 4 * m.rd32(face + 0x14)));
    let uv = m.rd32(face + 0x18);
    for k in 0..n {
        m.wr32(rec + 0x1c + 0x20 * k, m.rd32(uv + 8 * k));
        m.wr32(rec + 0x20 + 0x20 * k, m.rd32(uv + 8 * k + 4));
    }
}

/// 1: a reflecting face is drawn twice (its colours, then the reflection: 0x458092,
/// 0x45896b)
const REFLECT_TWICE: Us = Us(0x739f2c);

/// 0x45839e: a reflecting face (no point cut at the near plane): the reflection's texture
/// ([0x520f08]) and each point's uv from its place on the screen plus its normal turned by
/// the object's matrix (0x441dc8), kept in 0..1.
fn reflection(fpu: &Fpu, face: u32, obj: u32, rec: u32, n: u32) {
    let m = win32::platform::mem();
    let s = fpu.f32_bits(fpu.mul(from_f32(m.rd32(Us(0x52128c).at())), q64(Us(0x4fbca6).at())));
    let s = from_f32(s);
    m.wr8(rec, m.rd8(rec) | 2);
    m.wr32(rec + 0xb4, m.rd32(Us(0x520f08).at()));
    let (cx, cy) = (from_f32(m.rd32(Us(0x521298).at())), from_f32(m.rd32(Us(0x52129c).at())));
    let (h, g) = (from_f32(m.rd32(Us(0x4fbcbe).at())), from_f32(m.rd32(Us(0x4fbcb6).at())));
    let matrix = obj + 0x34;
    let (points, normals) = (m.rd32(face + 8), m.rd32(face + 0xc));
    for k in 0..n {
        let p = PROJECTED.at() + 16 * m.rd16(points + 2 * k) as u32;
        let at = rec + 0x20 * k;
        for j in 0..4 {
            m.wr32(at + 4 + 4 * j, m.rd32(p + 4 * j));
        }
        // 0x458472
        let a = fpu.sub(from_f32(m.rd32(at + 4)), cx);
        let b = fpu.sub(cy, from_f32(m.rd32(at + 8)));
        let u0 = from_f32(fpu.f32_bits(fpu.mul(s, a)));
        let v0 = from_f32(fpu.f32_bits(fpu.mul(b, s)));
        // 0x441dc8: the normal times the matrix's first two columns
        let nv = m.rd32(obj + 0x94) + 12 * m.rd16(normals + 2 * k) as u32;
        let r = [0, 1].map(|i| {
            let e = |a: u32| from_f32(m.rd32(a));
            let x = fpu.mul(e(nv + 4), e(matrix + 0xc + 4 * i));
            let y = fpu.mul(e(nv), e(matrix + 4 * i));
            let z = fpu.mul(e(nv + 8), e(matrix + 0x18 + 4 * i));
            from_f32(fpu.f32_bits(fpu.add(fpu.add(x, y), z)))
        });
        // 0x4584d7
        let u = fpu.add(fpu.add(u0, h), fpu.mul(r[0], g));
        let v = fpu.add(fpu.add(h, v0), fpu.mul(g, r[1]));
        for (off, t) in [(0x1c, u), (0x20, v)] {
            let bits = fpu.f32_bits(t);
            let t = from_f32(bits);
            let bits = if 0.0 > t {
                0
            } else if 1.0 >= t {
                bits
            } else {
                0x3f80_0000
            };
            m.wr32(at + off, bits);
        }
    }
}

/// 0x458909: each point's [+0xc] = [0x521274] times its [+0x10]; the polygon put first in
/// `list` and the next one's record begun. The list's old head (edx at the end).
fn finish(fpu: &Fpu, rec: u32, n: u32, list: u32) -> u32 {
    let m = win32::platform::mem();
    let k0 = from_f32(m.rd32(Us(0x521274).at()));
    for k in 0..n {
        let at = rec + 0x20 * k;
        let v = fpu.mul(k0, from_f32(m.rd32(at + 0x10)));
        m.wr32(at + 0xc, fpu.f32_bits(v));
    }
    m.wr32(LIST.at(), list);
    let head = m.rd32(list);
    m.wr32(rec + 0xbc, head);
    m.wr32(list, rec);
    m.wr32(POLYGON.at(), rec + 0xc0);
    head
}

#[derive(PartialEq, Clone, Copy, Debug)]
struct Regs {
    r: [u32; 8],
    top: u32,
}

fn regs(c: &Cpu) -> Regs {
    Regs { r: c.r, top: c.top }
}

fn checked(c: &mut Cpu) {
    use win32::platform::MemoryCopy;
    static CHECKS: AtomicU64 = AtomicU64::new(0);
    static DIFF: AtomicU64 = AtomicU64::new(0);
    let before = MemoryCopy::take();
    let (r0, st0) = (regs(c), c.st);
    call_original(c, RENDER_POLYGONS.at());
    let game = MemoryCopy::take();
    let rg = regs(c);
    before.restore();
    (c.r, c.top, c.st) = (r0.r, r0.top, st0);
    let face = c.r[EAX];
    let way = run(c);
    WAYS[way as usize].fetch_add(1, Relaxed);
    let ours = MemoryCopy::take();
    let ro = regs(c);
    // the stack below esp: the functions' own (not compared)
    let esp = r0.r[ESP];
    let at = game.differ(&ours, esp.saturating_sub(0x10000)..esp, 8);
    let n = CHECKS.fetch_add(1, Relaxed) + 1;
    if !at.is_empty() || rg != ro {
        let d = DIFF.fetch_add(1, Relaxed) + 1;
        if d <= 20 {
            eprintln!(
                "polygons check: differs (check {n}, way {way:?}, face {face:#x}): memory at {:x?}",
                at
            );
            for &a in at.iter().take(4) {
                eprintln!(
                    "  {a:#x}: game {:#010x} ours {:#010x}",
                    game.rd32(a & !3),
                    ours.rd32(a & !3)
                );
            }
            if rg != ro {
                eprintln!("  registers: game {rg:x?}\n             ours {ro:x?}");
            }
        }
    }
    if n.is_power_of_two() || n % 1000 == 0 {
        let w: Vec<u64> = WAYS.iter().map(|w| w.load(Relaxed)).collect();
        eprintln!(
            "polygons check: {n} checks, {} differ; ways (behind, outside, game, mode 1, far, mode 4, reflect) {w:?}",
            DIFF.load(Relaxed)
        );
        let mut k: Vec<(u32, u64)> = KEYS.lock().unwrap().iter().map(|(&a, &b)| (a, b)).collect();
        k.sort_by(|a, b| b.1.cmp(&a.1));
        for (key, count) in k.iter().take(8) {
            eprintln!(
                "  {count}: a8 {} close {} detail4 {} mode {} face1 {:#x} face0&2 {} 739f2c {} near {}",
                key & 1,
                key >> 1 & 1,
                key >> 2 & 1,
                key >> 3 & 7,
                key >> 6 & 0x15,
                key >> 12 & 1,
                key >> 14 & 1,
                key >> 15 & 1
            );
        }
    }
}
