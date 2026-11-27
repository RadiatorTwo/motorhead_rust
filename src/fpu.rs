//! x87 arithmetic as the lifter does it (crates/recomp/src/lift/x87.rs), for game functions
//! written again in Rust: the same result as the game's own code recompiled, bit for bit.
//! Values are doubles holding what the FPU's registers hold; the control word decides the
//! rounding of each operation.

use recomp::rt::{FpuOp, rt_fpu_round};

#[derive(Clone, Copy)]
pub struct Fpu {
    cw: u32,
    /// 53 or 64 bits to nearest: doubles as they are
    usual: bool,
    /// 24 bits to nearest
    near24: bool,
}

/// The bits of a double beyond a float's 24
const LOW: u64 = (1 << 29) - 1;

impl Fpu {
    pub fn new(cw: u16) -> Fpu {
        let cw = cw as u32;
        Fpu {
            cw,
            usual: cw & 0xe00 == 0x200,
            near24: cw & 0xf00 == 0,
        }
    }

    #[inline(always)]
    fn op(&self, op: FpuOp, x: f64, y: f64) -> f64 {
        let fast = match op {
            FpuOp::Add => x + y,
            FpuOp::Sub => x - y,
            FpuOp::Mul => x * y,
            FpuOp::Div => x / y,
            FpuOp::Sqrt => x.sqrt(),
            FpuOp::Round => x as f32 as f64,
        };
        if self.usual {
            return fast;
        }
        if matches!(op, FpuOp::Round) {
            // a float's value is stored as it is (`is_float`)
            let b = x.to_bits();
            let mag = b & !(1 << 63);
            if b & LOW == 0
                && (mag == 0 || (0x3810_0000_0000_0000..0x47f0_0000_0000_0000).contains(&mag))
            {
                return x;
            }
        } else if self.near24 {
            // floats' values: the double rounded to a float (`floats`)
            let low = match op {
                FpuOp::Sqrt => x.to_bits() & LOW,
                _ => (x.to_bits() | y.to_bits()) & LOW,
            };
            let mag = fast.to_bits() & !(1 << 63);
            let r = fast as f32;
            if low == 0 && mag >= 0x3810_0000_0000_0000 && r.is_finite() {
                return r as f64;
            }
        }
        rt_fpu_round(op as u32, x, y, self.cw)
    }

    #[inline(always)]
    pub fn add(&self, x: f64, y: f64) -> f64 {
        self.op(FpuOp::Add, x, y)
    }
    #[inline(always)]
    pub fn sub(&self, x: f64, y: f64) -> f64 {
        self.op(FpuOp::Sub, x, y)
    }
    #[inline(always)]
    pub fn mul(&self, x: f64, y: f64) -> f64 {
        self.op(FpuOp::Mul, x, y)
    }
    #[allow(dead_code)]
    #[inline(always)]
    pub fn div(&self, x: f64, y: f64) -> f64 {
        self.op(FpuOp::Div, x, y)
    }
    #[allow(dead_code)]
    #[inline(always)]
    pub fn sqrt(&self, x: f64) -> f64 {
        self.op(FpuOp::Sqrt, x, x)
    }
    /// fst/fstp dword: the float's bits
    #[inline(always)]
    pub fn f32_bits(&self, x: f64) -> u32 {
        (self.op(FpuOp::Round, x, x) as f32).to_bits()
    }
}

/// A float in memory as the FPU loads it.
#[inline(always)]
pub fn from_f32(bits: u32) -> f64 {
    f32::from_bits(bits) as f64
}
