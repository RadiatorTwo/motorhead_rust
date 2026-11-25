//! Code the game loads from its data files and jumps into: its fonts (data\FONT*.FNT, a run of
//! code per glyph, `font.rs`) and sprites (data\*.SPR, a run of code per row). None of it is
//! in the EXE, so none of it is compiled: the game functions that jump into it are replaced
//! (`font.rs`, `sprites.rs`), and the loaded code runs here, an instruction at a time. It is
//! simple: moves of constants or of the colour register into the picture, `add` of the pitch
//! to the row pointer, `fst qword` of a colour pattern, and `ret` or `jmp` at the end.

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind, Register};
use std::collections::HashSet;
use std::sync::Mutex;
use win32::platform::mem;

/// The registers loaded code sees: eax ecx edx ebx esp ebp esi edi, and st0's 64 bits.
pub struct Regs {
    pub r: [u32; 8],
    pub st0: u64,
}

/// How a run of loaded code ended.
pub enum End {
    Ret,
    /// a jump (the fonts' glyphs end in one back to the game's loop)
    Jmp,
    /// an instruction this does not do (logged once per address)
    Unknown,
}

/// Longest run followed (instructions), against damaged data.
const MAX_STEPS: u32 = 100_000;

fn reg32(r: Register) -> usize {
    r.full_register32() as usize - Register::EAX as usize
}

fn get(regs: &Regs, r: Register) -> u32 {
    let v = regs.r[reg32(r)];
    match r.size() {
        1 if matches!(r, Register::AH | Register::CH | Register::DH | Register::BH) => {
            (v >> 8) & 0xff
        }
        1 => v & 0xff,
        2 => v & 0xffff,
        _ => v,
    }
}

fn set(regs: &mut Regs, r: Register, v: u32) {
    let k = reg32(r);
    let old = regs.r[k];
    regs.r[k] = match r.size() {
        1 if matches!(r, Register::AH | Register::CH | Register::DH | Register::BH) => {
            old & !0xff00 | (v & 0xff) << 8
        }
        1 => old & !0xff | v & 0xff,
        2 => old & !0xffff | v & 0xffff,
        _ => v,
    };
}

fn address(regs: &Regs, i: &Instruction) -> u32 {
    let mut a = i.memory_displacement32();
    if i.memory_base() != Register::None {
        a = a.wrapping_add(get(regs, i.memory_base()));
    }
    if i.memory_index() != Register::None {
        a = a.wrapping_add(get(regs, i.memory_index()).wrapping_mul(i.memory_index_scale()));
    }
    a
}

/// Operand `k`'s value (`size` bytes).
fn read(regs: &Regs, i: &Instruction, k: u32, size: usize) -> Option<u32> {
    let m = mem();
    Some(match i.op_kind(k) {
        OpKind::Register => get(regs, i.op_register(k)),
        OpKind::Memory => {
            let a = address(regs, i);
            match size {
                1 => m.rd8(a) as u32,
                2 => m.rd16(a) as u32,
                _ => m.rd32(a),
            }
        }
        OpKind::Immediate8 | OpKind::Immediate16 | OpKind::Immediate32 => i.immediate(k) as u32,
        OpKind::Immediate8to32 | OpKind::Immediate8to16 => i.immediate(k) as u32,
        _ => return None,
    })
}

fn write(regs: &mut Regs, i: &Instruction, size: usize, v: u32) -> bool {
    let m = mem();
    match i.op0_kind() {
        OpKind::Register => set(regs, i.op0_register(), v),
        OpKind::Memory => {
            let a = address(regs, i);
            match size {
                1 => m.wr8(a, v as u8),
                2 => m.wr16(a, v as u16),
                _ => m.wr32(a, v),
            }
        }
        _ => return false,
    }
    true
}

/// Run the loaded code at `code` until its `ret` or `jmp`.
pub fn run(code: u32, regs: &mut Regs) -> End {
    let m = mem();
    let mut pc = code;
    for _ in 0..MAX_STEPS {
        let bytes = m.slice(pc, 16).to_vec();
        let i = Decoder::with_ip(32, &bytes, pc as u64, DecoderOptions::NONE).decode();
        pc = i.next_ip32();
        let size = match i.op0_kind() {
            OpKind::Register => i.op0_register().size(),
            OpKind::Memory => i.memory_size().size(),
            _ => 4,
        };
        let ok = match i.mnemonic() {
            Mnemonic::Ret => {
                regs.r[4] = regs.r[4].wrapping_add(4 + i.immediate16() as u32);
                return End::Ret;
            }
            Mnemonic::Jmp => return End::Jmp,
            Mnemonic::Mov => read(regs, &i, 1, size).is_some_and(|v| write(regs, &i, size, v)),
            Mnemonic::Lea if i.op0_kind() == OpKind::Register => {
                let a = address(regs, &i);
                set(regs, i.op0_register(), a);
                true
            }
            Mnemonic::Add | Mnemonic::Sub | Mnemonic::And | Mnemonic::Or | Mnemonic::Xor => {
                match (read(regs, &i, 0, size), read(regs, &i, 1, size)) {
                    (Some(a), Some(b)) => {
                        let v = match i.mnemonic() {
                            Mnemonic::Add => a.wrapping_add(b),
                            Mnemonic::Sub => a.wrapping_sub(b),
                            Mnemonic::And => a & b,
                            Mnemonic::Or => a | b,
                            _ => a ^ b,
                        };
                        write(regs, &i, size, v)
                    }
                    _ => false,
                }
            }
            Mnemonic::Inc | Mnemonic::Dec => read(regs, &i, 0, size).is_some_and(|a| {
                let v = if i.mnemonic() == Mnemonic::Inc {
                    a.wrapping_add(1)
                } else {
                    a.wrapping_sub(1)
                };
                write(regs, &i, size, v)
            }),
            Mnemonic::Fst | Mnemonic::Fstp
                if i.op0_kind() == OpKind::Memory && i.memory_size().size() == 8 =>
            {
                let a = address(regs, &i);
                m.wr32(a, regs.st0 as u32);
                m.wr32(a.wrapping_add(4), (regs.st0 >> 32) as u32);
                true
            }
            Mnemonic::Nop => true,
            _ => false,
        };
        if !ok {
            static SEEN: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
            if SEEN.lock().unwrap().get_or_insert_default().insert(i.ip32()) {
                eprintln!("loaded code at {:#x}: {i} not done (run from {code:#x})", i.ip32());
            }
            return End::Unknown;
        }
    }
    eprintln!("loaded code from {code:#x}: no end after {MAX_STEPS} instructions");
    End::Unknown
}
