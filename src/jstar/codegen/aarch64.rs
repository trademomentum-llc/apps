//! AArch64 Code Generation — low-level instruction encoder and IR emitter.
//!
//! Provides the AArch64 register enum (`Aarch64Reg`), condition-code enum,
//! a minimal system-register wrapper, and `Aarch64Emitter`: a byte-buffer
//! emitter for the instruction categories used by the JStar ARM64 backend.
//!
//! The IR-to-machine-code translation implemented here is an MVP that covers
//! the JStar integer subset used by the bootstrap tests: arithmetic, calls,
//! control flow, and function prologue/epilogue using the AAPCS64 convention.

use super::MachineCode;
use crate::jstar::grammar::JStarType;
use crate::jstar::ir::*;
use crate::types::{MorphResult, MorphlexError};
use std::collections::{HashMap, HashSet};

// ─── AArch64 Register Encoding ──────────────────────────────────────────────

/// 64-bit general-purpose registers and the zero/stack pseudo-register.
///
/// `Sp` and `Xzr` both encode as register number 31.  Which one is intended
/// is determined by context (e.g. `Xzr` as a source operand, `Sp` as a base
/// register).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Aarch64Reg {
    X0 = 0,
    X1 = 1,
    X2 = 2,
    X3 = 3,
    X4 = 4,
    X5 = 5,
    X6 = 6,
    X7 = 7,
    X8 = 8,
    X9 = 9,
    X10 = 10,
    X11 = 11,
    X12 = 12,
    X13 = 13,
    X14 = 14,
    X15 = 15,
    X16 = 16,
    X17 = 17,
    X18 = 18,
    X19 = 19,
    X20 = 20,
    X21 = 21,
    X22 = 22,
    X23 = 23,
    X24 = 24,
    X25 = 25,
    X26 = 26,
    X27 = 27,
    X28 = 28,
    X29 = 29,
    X30 = 30,
    Sp = 31,
}

impl Aarch64Reg {
    /// The 5-bit AArch64 register encoding used by most instructions.
    pub fn encoding(self) -> u8 {
        self as u8
    }

    /// The zero register alias.  It encodes identically to [`Aarch64Reg::Sp`].
    #[allow(non_upper_case_globals)]
    pub const Xzr: Aarch64Reg = Aarch64Reg::Sp;
}

/// AArch64 condition codes for `b.cond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Aarch64Cond {
    Eq = 0,
    Ne = 1,
    Cs = 2,
    Cc = 3,
    Mi = 4,
    Pl = 5,
    Vs = 6,
    Vc = 7,
    Hi = 8,
    Ls = 9,
    Ge = 10,
    Lt = 11,
    Gt = 12,
    Le = 13,
    Al = 14,
    Nv = 15,
}

/// Compact encoding of a system register as used by `mrs`/`msr`.
///
/// The 16-bit value is the concatenation of
/// `op0[1:0] | op1[2:0] | CRn[3:0] | CRm[3:0] | op2[2:0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64SysReg(pub u16);

impl Aarch64SysReg {
    /// Build a system-register encoding from its architectural fields.
    pub const fn new(op0: u8, op1: u8, crn: u8, crm: u8, op2: u8) -> Self {
        let enc = ((op0 as u16 & 0x3) << 14)
            | ((op1 as u16 & 0x7) << 11)
            | ((crn as u16 & 0xf) << 7)
            | ((crm as u16 & 0xf) << 3)
            | (op2 as u16 & 0x7);
        Self(enc)
    }

    pub const fn encoding(self) -> u16 {
        self.0
    }

    pub const TPIDR_EL0: Self = Self::new(0b11, 0b011, 0b1101, 0b0000, 0b010);
}

// ─── Emitter ────────────────────────────────────────────────────────────────

/// A simple AArch64 machine-code emitter.
#[derive(Debug, Clone, Default)]
pub struct Aarch64Emitter {
    /// The `.text` section under construction.
    pub text: Vec<u8>,
    /// The `.data` section (initialized data — string literals, etc.).
    pub data: Vec<u8>,
    /// Size of the uninitialized `.bss` section.
    pub bss_size: usize,
    /// Byte offsets in `.text` that need patching with `.data` addresses.
    pub data_fixups: Vec<usize>,
}

impl Aarch64Emitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of bytes emitted into `.text` so far.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Emit one 32-bit little-endian instruction word.
    pub fn emit_u32(&mut self, word: u32) {
        self.text.extend_from_slice(&word.to_le_bytes());
    }

    /// Convert this emitter into the shared `MachineCode` output type.
    pub fn into_machine_code(self) -> MachineCode {
        MachineCode {
            text: self.text,
            data: self.data,
            bss_size: self.bss_size,
            stack_size: 0,
            data_vaddr: 0,
            data_fixups: self.data_fixups,
        }
    }

    // ── Moves and immediates ───────────────────────────────────────────────

    /// `movz  xd, #imm16, lsl #shift`  (shift must be 0, 16, 32 or 48).
    pub fn emit_movz(&mut self, rd: Aarch64Reg, imm16: u16, shift: u8) {
        assert!(
            shift % 16 == 0 && shift <= 48,
            "movz shift must be 0, 16, 32 or 48"
        );
        let hw = ((shift / 16) & 0x3) as u32;
        let imm = (imm16 as u32) & 0xFFFF;
        let insn = 0xD2800000 | (hw << 21) | (imm << 5) | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `movn  xd, #imm16, lsl #shift`  (shift must be 0, 16, 32 or 48).
    pub fn emit_movn(&mut self, rd: Aarch64Reg, imm16: u16, shift: u8) {
        assert!(
            shift % 16 == 0 && shift <= 48,
            "movn shift must be 0, 16, 32 or 48"
        );
        let hw = ((shift / 16) & 0x3) as u32;
        let imm = (imm16 as u32) & 0xFFFF;
        let insn = 0x92800000 | (hw << 21) | (imm << 5) | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `movk  xd, #imm16, lsl #shift`  (shift must be 0, 16, 32 or 48).
    pub fn emit_movk(&mut self, rd: Aarch64Reg, imm16: u16, shift: u8) {
        assert!(
            shift % 16 == 0 && shift <= 48,
            "movk shift must be 0, 16, 32 or 48"
        );
        let hw = ((shift / 16) & 0x3) as u32;
        let imm = (imm16 as u32) & 0xFFFF;
        let insn = 0xF2800000 | (hw << 21) | (imm << 5) | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `mov  xd, xm` — alias for `orr xd, xzr, xm`.
    pub fn emit_mov(&mut self, rd: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xAA000000
            | ((rm.encoding() as u32) << 16)
            | (Aarch64Reg::Xzr.encoding() as u32) << 5
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// Convenience: load a 16-bit immediate with `movz`.
    pub fn emit_mov_imm(&mut self, rd: Aarch64Reg, imm16: u16) {
        self.emit_movz(rd, imm16, 0);
    }

    /// Load an arbitrary 64-bit immediate into `rd` using `movz`/`movk`.
    pub fn emit_load_imm64(&mut self, rd: Aarch64Reg, imm: i64) {
        let bits = imm as u64;
        let chunks = [
            (bits & 0xFFFF) as u16,
            ((bits >> 16) & 0xFFFF) as u16,
            ((bits >> 32) & 0xFFFF) as u16,
            ((bits >> 48) & 0xFFFF) as u16,
        ];
        let mut emitted = false;
        for (i, chunk) in chunks.iter().enumerate() {
            if !emitted {
                if *chunk != 0 || (imm == 0 && i == 0) {
                    self.emit_movz(rd, *chunk, (i * 16) as u8);
                    emitted = true;
                }
            } else if *chunk != 0 {
                self.emit_movk(rd, *chunk, (i * 16) as u8);
            }
        }
        if !emitted {
            self.emit_movz(rd, 0, 0);
        }
    }

    // ── Integer arithmetic (register) ──────────────────────────────────────

    /// `add  xd, xn, xm` (shifted register, LSL #0).
    pub fn emit_add(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0x8B000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `sub  xd, xn, xm` (shifted register, LSL #0).
    pub fn emit_sub(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xCB000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `add  xd, xn, #imm12` with optional `lsl #12`.
    pub fn emit_add_imm(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, imm12: u16, shift12: bool) {
        assert!(imm12 <= 4095, "add immediate out of range");
        let sh = if shift12 { 1 << 22 } else { 0 };
        let insn = 0x91000000
            | sh
            | ((imm12 as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `sub  xd, xn, #imm12` with optional `lsl #12`.
    pub fn emit_sub_imm(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, imm12: u16, shift12: bool) {
        assert!(imm12 <= 4095, "sub immediate out of range");
        let sh = if shift12 { 1 << 22 } else { 0 };
        let insn = 0xD1000000
            | sh
            | ((imm12 as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `madd  xd, xn, xm, xa`.
    pub fn emit_madd(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg, ra: Aarch64Reg) {
        let insn = 0x9B000000
            | ((rm.encoding() as u32) << 16)
            | ((ra.encoding() as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `msub  xd, xn, xm, xa`.
    pub fn emit_msub(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg, ra: Aarch64Reg) {
        let insn = 0x9B008000
            | ((rm.encoding() as u32) << 16)
            | ((ra.encoding() as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `sdiv  xd, xn, xm`.
    pub fn emit_sdiv(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0x9AC00000
            | ((rm.encoding() as u32) << 16)
            | (3 << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `udiv  xd, xn, xm`.
    pub fn emit_udiv(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0x9AC00000
            | ((rm.encoding() as u32) << 16)
            | (2 << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `cmp  xn, xm` — alias for `subs xzr, xn, xm`.
    pub fn emit_cmp(&mut self, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xEB000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | Aarch64Reg::Xzr.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `cset  xd, <cond>` — alias for `csinc xd, xzr, xzr, invert(cond)`.
    pub fn emit_cset(&mut self, rd: Aarch64Reg, cond: Aarch64Cond) {
        let inv = ((cond as u8) ^ 1) as u32;
        let insn = 0x9A800400
            | (Aarch64Reg::Xzr.encoding() as u32) << 16
            | (inv << 12)
            | (Aarch64Reg::Xzr.encoding() as u32) << 5
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    // ── Bitwise and shifts ─────────────────────────────────────────────────

    /// `and  xd, xn, xm` (shifted register, LSL #0).
    pub fn emit_and(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0x8A000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `orr  xd, xn, xm` (shifted register, LSL #0).
    pub fn emit_orr(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xAA000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `eor  xd, xn, xm` (shifted register, LSL #0).
    pub fn emit_eor(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xCA000000
            | ((rm.encoding() as u32) << 16)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `mvn  xd, xm` — alias for `orn xd, xzr, xm`.
    pub fn emit_mvn(&mut self, rd: Aarch64Reg, rm: Aarch64Reg) {
        let insn = 0xAA200000
            | ((rm.encoding() as u32) << 16)
            | (Aarch64Reg::Xzr.encoding() as u32) << 5
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `lsl  xd, xn, xm` — alias for `lslv`.
    pub fn emit_lsl(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        self.emit_shift_var(rd, rn, rm, 0b001000);
    }

    /// `lsr  xd, xn, xm` — alias for `lsrv`.
    pub fn emit_lsr(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        self.emit_shift_var(rd, rn, rm, 0b001001);
    }

    /// `asr  xd, xn, xm` — alias for `asrv`.
    pub fn emit_asr(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg) {
        self.emit_shift_var(rd, rn, rm, 0b001010);
    }

    fn emit_shift_var(&mut self, rd: Aarch64Reg, rn: Aarch64Reg, rm: Aarch64Reg, op2: u32) {
        let insn = 0x9AC00000
            | ((rm.encoding() as u32) << 16)
            | (op2 << 10)
            | ((rn.encoding() as u32) << 5)
            | rd.encoding() as u32;
        self.emit_u32(insn);
    }

    // ── Loads and stores (unsigned immediate offset) ───────────────────────

    fn emit_load_store_unsigned(
        &mut self,
        size: u8,
        load: bool,
        rt: Aarch64Reg,
        rn: Aarch64Reg,
        offset: u32,
    ) {
        let scale = 1u32 << size;
        assert!(
            offset % scale == 0,
            "load/store offset must be aligned to access size"
        );
        assert!(
            (offset / scale) <= 0xFFF,
            "load/store offset out of range"
        );
        let imm12 = (offset / scale) & 0xFFF;
        let mut insn = ((size as u32 & 0x3) << 30) | 0x39000000;
        if load {
            insn |= 1 << 22;
        }
        insn |= (imm12 << 10) | ((rn.encoding() as u32) << 5) | rt.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `ldr  xt, [xn, #offset]` (64-bit, unsigned offset).
    pub fn emit_ldr(&mut self, rt: Aarch64Reg, rn: Aarch64Reg, offset: u32) {
        self.emit_load_store_unsigned(3, true, rt, rn, offset);
    }

    /// `str  xt, [xn, #offset]` (64-bit, unsigned offset).
    pub fn emit_str(&mut self, rt: Aarch64Reg, rn: Aarch64Reg, offset: u32) {
        self.emit_load_store_unsigned(3, false, rt, rn, offset);
    }

    /// `ldrb  wt, [xn, #offset]` (byte, unsigned offset).
    pub fn emit_ldrb(&mut self, rt: Aarch64Reg, rn: Aarch64Reg, offset: u32) {
        self.emit_load_store_unsigned(0, true, rt, rn, offset);
    }

    /// `strb  wt, [xn, #offset]` (byte, unsigned offset).
    pub fn emit_strb(&mut self, rt: Aarch64Reg, rn: Aarch64Reg, offset: u32) {
        self.emit_load_store_unsigned(0, false, rt, rn, offset);
    }

    /// `ldp  xt1, xt2, [xn, #offset]` (signed offset, multiple of 8).
    pub fn emit_ldp(&mut self, rt1: Aarch64Reg, rt2: Aarch64Reg, rn: Aarch64Reg, offset: i32) {
        assert!(offset % 8 == 0, "ldp offset must be a multiple of 8");
        assert!(-512 <= offset && offset <= 504, "ldp offset out of range");
        let imm7 = ((offset / 8) as u32) & 0x7F;
        let insn = 0xA9400000
            | (imm7 << 15)
            | ((rt2.encoding() as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rt1.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `stp  xt1, xt2, [xn, #offset]` (signed offset, multiple of 8).
    pub fn emit_stp(&mut self, rt1: Aarch64Reg, rt2: Aarch64Reg, rn: Aarch64Reg, offset: i32) {
        assert!(offset % 8 == 0, "stp offset must be a multiple of 8");
        assert!(-512 <= offset && offset <= 504, "stp offset out of range");
        let imm7 = ((offset / 8) as u32) & 0x7F;
        let insn = 0xA9000000
            | (imm7 << 15)
            | ((rt2.encoding() as u32) << 10)
            | ((rn.encoding() as u32) << 5)
            | rt1.encoding() as u32;
        self.emit_u32(insn);
    }

    // ── Branches ───────────────────────────────────────────────────────────

    fn encode_imm19(offset: i32) -> u32 {
        assert!(offset % 4 == 0, "branch offset must be a multiple of 4");
        assert!(
            -(1 << 18) * 4 <= offset && offset <= ((1 << 18) - 1) * 4,
            "branch offset out of range for imm19"
        );
        ((offset / 4) as u32) & 0x7FFFF
    }

    fn encode_imm26(offset: i32) -> u32 {
        assert!(offset % 4 == 0, "branch offset must be a multiple of 4");
        assert!(
            -(1 << 25) * 4 <= offset && offset <= ((1 << 25) - 1) * 4,
            "branch offset out of range for imm26"
        );
        ((offset / 4) as u32) & 0x3FFFFFF
    }

    /// `b  label` (unconditional branch).
    ///
    /// `offset` is the signed byte distance from the branch instruction to
    /// the target, and must be a multiple of 4.
    pub fn emit_b(&mut self, offset: i32) {
        self.emit_u32(0x14000000 | Self::encode_imm26(offset));
    }

    /// `bl  label`.
    pub fn emit_bl(&mut self, offset: i32) {
        self.emit_u32(0x94000000 | Self::encode_imm26(offset));
    }

    /// `cbz  xt, label`.
    pub fn emit_cbz(&mut self, rt: Aarch64Reg, offset: i32) {
        self.emit_u32(0xB4000000 | (Self::encode_imm19(offset) << 5) | rt.encoding() as u32);
    }

    /// `cbnz  xt, label`.
    pub fn emit_cbnz(&mut self, rt: Aarch64Reg, offset: i32) {
        self.emit_u32(0xB5000000 | (Self::encode_imm19(offset) << 5) | rt.encoding() as u32);
    }

    /// `b.<cond>  label`.
    pub fn emit_b_cond(&mut self, cond: Aarch64Cond, offset: i32) {
        self.emit_u32(0x54000000 | (Self::encode_imm19(offset) << 5) | ((cond as u32) << 1));
    }

    // ── System registers ───────────────────────────────────────────────────

    /// `mrs  xt, <systemreg>`.
    pub fn emit_mrs(&mut self, rt: Aarch64Reg, sysreg: Aarch64SysReg) {
        let insn = 0xD5200000 | ((sysreg.encoding() as u32) << 5) | rt.encoding() as u32;
        self.emit_u32(insn);
    }

    /// `msr  <systemreg>, xt`.
    pub fn emit_msr(&mut self, sysreg: Aarch64SysReg, rt: Aarch64Reg) {
        let insn = 0xD5000000 | ((sysreg.encoding() as u32) << 5) | rt.encoding() as u32;
        self.emit_u32(insn);
    }

    // ── System call ────────────────────────────────────────────────────────

    /// `svc  #imm`.
    pub fn emit_svc(&mut self, imm: u16) {
        let insn = 0xD4000001 | ((imm as u32 & 0xFFFF) << 5);
        self.emit_u32(insn);
    }

    // ── Procedure return ───────────────────────────────────────────────────

    /// `ret`.
    pub fn emit_ret(&mut self) {
        self.emit_u32(0xD65F03C0);
    }
}

// ─── IR-to-machine-code translation ─────────────────────────────────────────

/// Generate AArch64 machine code from IR.
pub fn generate(program: &IrProgram) -> MorphResult<MachineCode> {
    let mut cg = CodeGen::new();

    // .data section: string literals (initialized) + global variable data (BSS).
    cg.emitter.data = program.string_data.clone();
    let global_base = cg.emitter.data.len();
    cg.emitter.bss_size = program.global_data.len();

    // Build the global_vregs map with absolute offsets into .data.
    for (&vreg, &offset) in &program.global_vregs {
        let abs_offset = global_base + offset;
        cg.global_vregs.insert(vreg, abs_offset);
        let is_direct_storage = program
            .global_vars
            .values()
            .find(|(var_offset, _, _)| *var_offset == offset)
            .map(|(_, alloc_size, ty)| *alloc_size > ty.size_bytes())
            .unwrap_or(false);
        if is_direct_storage {
            cg.direct_storage_vregs.insert(vreg);
        }
    }

    for func in &program.functions {
        cg.emit_function(func)?;
    }

    cg.apply_call_fixups()?;

    Ok(MachineCode {
        text: cg.emitter.text,
        data: cg.emitter.data,
        bss_size: cg.emitter.bss_size,
        stack_size: cg.stack_size,
        data_vaddr: 0,
        data_fixups: cg.emitter.data_fixups,
    })
}

/// AArch64 argument registers (AAPCS64).
const ARG_REGS: [Aarch64Reg; 8] = [
    Aarch64Reg::X0,
    Aarch64Reg::X1,
    Aarch64Reg::X2,
    Aarch64Reg::X3,
    Aarch64Reg::X4,
    Aarch64Reg::X5,
    Aarch64Reg::X6,
    Aarch64Reg::X7,
];

/// Scratch registers used internally by the emitter.
const SCRATCH: Aarch64Reg = Aarch64Reg::X9;
const SCRATCH2: Aarch64Reg = Aarch64Reg::X10;
const SCRATCH3: Aarch64Reg = Aarch64Reg::X11;
const ADDR_REG: Aarch64Reg = Aarch64Reg::X16;
const ADDR_REG2: Aarch64Reg = Aarch64Reg::X17;

#[derive(Debug, Clone, Copy)]
enum BranchKind {
    B,
    Cbnz,
}

struct CodeGen {
    emitter: Aarch64Emitter,
    /// Map virtual register -> stack offset from `sp`.
    vreg_offsets: HashMap<VReg, u32>,
    next_stack_offset: u32,
    /// Label name -> byte offset in .text (within the current function).
    label_offsets: HashMap<String, usize>,
    /// Branch fixups to resolve after a function is emitted.
    fixups: Vec<(usize, String, BranchKind)>,
    /// Function name -> byte offset in .text.
    function_offsets: HashMap<String, usize>,
    /// Call fixups to resolve after all functions are emitted.
    call_fixups: Vec<(usize, String)>,
    /// Whether the current function is `_start`.
    is_entry_point: bool,
    /// Global vregs: vreg -> absolute offset in .data section.
    global_vregs: HashMap<VReg, usize>,
    /// Vregs whose identity is the storage object itself.
    direct_storage_vregs: HashSet<VReg>,
    /// Frame size of the current/last function.
    frame_size: u32,
    /// Stack size reported in the output (frame size of the last function).
    stack_size: usize,
}

impl CodeGen {
    fn new() -> Self {
        Self {
            emitter: Aarch64Emitter::new(),
            vreg_offsets: HashMap::new(),
            next_stack_offset: 0,
            label_offsets: HashMap::new(),
            fixups: Vec::new(),
            function_offsets: HashMap::new(),
            call_fixups: Vec::new(),
            is_entry_point: false,
            global_vregs: HashMap::new(),
            direct_storage_vregs: HashSet::new(),
            frame_size: 0,
            stack_size: 0,
        }
    }

    fn alloc_stack_slot(&mut self, vreg: VReg, size: usize) -> u32 {
        let aligned_size = ((size + 7) / 8 * 8).max(8) as u32;
        let offset = self.next_stack_offset;
        self.next_stack_offset += aligned_size;
        self.vreg_offsets.insert(vreg, offset);
        offset
    }

    fn vreg_offset(&self, vreg: VReg) -> u32 {
        *self.vreg_offsets.get(&vreg).unwrap_or_else(|| {
            panic!(
                "codegen: vreg {} has no stack slot (allocated vregs: {:?})",
                vreg,
                self.vreg_offsets.keys().collect::<Vec<_>>()
            )
        })
    }

    fn is_global_vreg(&self, vreg: VReg) -> bool {
        self.global_vregs.contains_key(&vreg)
    }

    #[allow(dead_code)]
    fn is_direct_storage_vreg(&self, vreg: VReg) -> bool {
        self.direct_storage_vregs.contains(&vreg)
    }

    fn instruction_allocates_direct_storage(&self, inst: &IrInst) -> bool {
        match inst {
            IrInst::Alloca { size, ty, .. } => *size > ty.size_bytes(),
            IrInst::ArrayAlloc { .. } => true,
            _ => false,
        }
    }

    fn is_byte_type(ty: &JStarType) -> bool {
        matches!(ty, JStarType::Boolean | JStarType::Byte)
    }

    fn emit_function(&mut self, func: &IrFunction) -> MorphResult<()> {
        // Reset per-function state.
        self.vreg_offsets.clear();
        self.next_stack_offset = 0;
        self.label_offsets.clear();
        self.fixups.clear();
        self.is_entry_point = func.name == "_start";

        // Record function offset for call resolution.
        self.function_offsets
            .insert(func.name.clone(), self.emitter.len());

        // Pre-allocate stack slots for all virtual registers.
        for block in &func.blocks {
            for inst in &block.instructions {
                match inst {
                    IrInst::Alloca { dest, size, .. } => {
                        if self.instruction_allocates_direct_storage(inst) {
                            self.direct_storage_vregs.insert(*dest);
                        }
                        self.alloc_stack_slot(*dest, *size);
                    }
                    IrInst::BinOp { dest, ty, .. }
                    | IrInst::UnaryOp { dest, ty, .. }
                    | IrInst::Copy { dest, ty, .. }
                    | IrInst::Load { dest, ty, .. } => {
                        self.alloc_stack_slot(*dest, ty.size_bytes().max(8));
                    }
                    IrInst::Compare { dest, .. }
                    | IrInst::Call { dest, .. }
                    | IrInst::Syscall { dest, .. }
                    | IrInst::AddressOf { dest, .. } => {
                        self.alloc_stack_slot(*dest, 8);
                    }
                    IrInst::LoadIndexed { dest, .. } => {
                        self.alloc_stack_slot(*dest, 8);
                    }
                    IrInst::ArrayAlloc { dest, count } => {
                        self.alloc_stack_slot(*dest, *count * 8);
                    }
                    IrInst::ArrayLoad { dest, .. }
                    | IrInst::ArrayLength { dest, .. }
                    | IrInst::HashOp { dest, .. }
                    | IrInst::FileOpen { dest, .. }
                    | IrInst::FileRead { dest, .. }
                    | IrInst::StrCmp { dest, .. }
                    | IrInst::StrLen { dest, .. } => {
                        self.alloc_stack_slot(*dest, 8);
                    }
                    IrInst::Store { .. }
                    | IrInst::StoreIndexed { .. }
                    | IrInst::Print { .. }
                    | IrInst::PrintStr { .. }
                    | IrInst::Nop
                    | IrInst::ArrayStore { .. }
                    | IrInst::FileClose { .. }
                    | IrInst::StrCopy { .. } => {}
                }
            }
        }

        // Frame size must be 16-byte aligned per AAPCS64.
        self.frame_size = ((self.next_stack_offset + 15) / 16) * 16;
        self.stack_size = self.frame_size as usize;

        // Function prologue: save x29/x30 and allocate the frame.
        self.emitter
            .emit_stp(Aarch64Reg::X29, Aarch64Reg::X30, Aarch64Reg::Sp, -16);
        self.emitter
            .emit_mov(Aarch64Reg::X29, Aarch64Reg::Sp);
        self.emit_frame_adjust(true);

        // Store incoming arguments into their parameter stack slots.
        if !self.is_entry_point && func.param_count > 0 {
            let mut param_vregs: Vec<VReg> = Vec::new();
            if let Some(entry_block) = func.blocks.first() {
                for inst in &entry_block.instructions {
                    if let IrInst::Alloca { dest, .. } = inst
                        && param_vregs.len() < func.param_count
                    {
                        param_vregs.push(*dest);
                    }
                }
            }
            if param_vregs.len() != func.param_count {
                return Err(MorphlexError::CodegenError(format!(
                    "function '{}': expected {} parameter allocas in entry block, found {}",
                    func.name,
                    func.param_count,
                    param_vregs.len()
                )));
            }
            for (i, vreg) in param_vregs.iter().enumerate() {
                if i >= ARG_REGS.len() {
                    break;
                }
                self.emit_store_vreg(ARG_REGS[i], *vreg);
            }
        }

        // Emit each basic block, recording label offsets.
        for block in &func.blocks {
            self.label_offsets
                .insert(block.label.clone(), self.emitter.len());
            self.emit_block(block)?;
        }

        // Resolve all local branch fixups.
        self.apply_local_fixups()?;

        Ok(())
    }

    fn emit_block(&mut self, block: &BasicBlock) -> MorphResult<()> {
        for inst in &block.instructions {
            self.emit_instruction(inst)?;
        }
        self.emit_terminator(&block.terminator)?;
        Ok(())
    }

    fn emit_frame_adjust(&mut self, subtract: bool) {
        if self.frame_size == 0 {
            return;
        }
        if self.frame_size <= 4095 {
            if subtract {
                self.emitter
                    .emit_sub_imm(Aarch64Reg::Sp, Aarch64Reg::Sp, self.frame_size as u16, false);
            } else {
                self.emitter
                    .emit_add_imm(Aarch64Reg::Sp, Aarch64Reg::Sp, self.frame_size as u16, false);
            }
        } else {
            self.emitter
                .emit_load_imm64(Aarch64Reg::X16, self.frame_size as i64);
            if subtract {
                self.emitter
                    .emit_sub(Aarch64Reg::Sp, Aarch64Reg::Sp, Aarch64Reg::X16);
            } else {
                self.emitter
                    .emit_add(Aarch64Reg::Sp, Aarch64Reg::Sp, Aarch64Reg::X16);
            }
        }
    }

    fn emit_epilogue(&mut self) {
        self.emit_frame_adjust(false);
        self.emitter
            .emit_ldp(Aarch64Reg::X29, Aarch64Reg::X30, Aarch64Reg::Sp, 16);
        self.emitter.emit_ret();
    }

    fn emit_instruction(&mut self, inst: &IrInst) -> MorphResult<()> {
        match inst {
            IrInst::BinOp {
                dest, op, lhs, rhs, ..
            } => {
                self.emit_load_value(SCRATCH, lhs)?;
                self.emit_load_value(SCRATCH2, rhs)?;
                match op {
                    IrBinOp::Add => self.emitter.emit_add(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Sub => self.emitter.emit_sub(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Mul => self
                        .emitter
                        .emit_madd(SCRATCH, SCRATCH, SCRATCH2, Aarch64Reg::Xzr),
                    IrBinOp::Div => self.emitter.emit_sdiv(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Mod => {
                        self.emitter.emit_sdiv(SCRATCH3, SCRATCH, SCRATCH2);
                        self.emitter
                            .emit_msub(SCRATCH, SCRATCH3, SCRATCH2, SCRATCH);
                    }
                    IrBinOp::And => self.emitter.emit_and(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Or => self.emitter.emit_orr(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Xor => self.emitter.emit_eor(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Shl => self.emitter.emit_lsl(SCRATCH, SCRATCH, SCRATCH2),
                    IrBinOp::Shr => self.emitter.emit_lsr(SCRATCH, SCRATCH, SCRATCH2),
                }
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::UnaryOp { dest, op, src, .. } => {
                self.emit_load_value(SCRATCH, src)?;
                match op {
                    IrUnaryOp::Neg => {
                        self.emitter
                            .emit_sub(SCRATCH, Aarch64Reg::Xzr, SCRATCH);
                    }
                    IrUnaryOp::Not => self.emitter.emit_mvn(SCRATCH, SCRATCH),
                }
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Copy { dest, src, .. } => {
                self.emit_load_value(SCRATCH, src)?;
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Compare {
                dest, lhs, rhs, kind, ..
            } => {
                self.emit_load_value(SCRATCH, lhs)?;
                self.emit_load_value(SCRATCH2, rhs)?;
                self.emitter.emit_cmp(SCRATCH, SCRATCH2);
                let cond = match kind {
                    CmpKind::Eq => Aarch64Cond::Eq,
                    CmpKind::Ne => Aarch64Cond::Ne,
                    CmpKind::Lt => Aarch64Cond::Lt,
                    CmpKind::Le => Aarch64Cond::Le,
                    CmpKind::Gt => Aarch64Cond::Gt,
                    CmpKind::Ge => Aarch64Cond::Ge,
                };
                self.emitter.emit_cset(SCRATCH, cond);
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Load { dest, addr, ty } => {
                let byte = Self::is_byte_type(ty);
                match addr {
                    IrValue::Reg(vreg) if self.is_global_vreg(*vreg) => {
                        let data_offset = self.global_vregs[vreg];
                        self.emit_load_global_value(SCRATCH, data_offset);
                    }
                    IrValue::Reg(vreg) => {
                        if byte {
                            self.emit_loadb_vreg(SCRATCH, *vreg);
                        } else {
                            self.emit_load_vreg(SCRATCH, *vreg);
                        }
                    }
                    _ => {
                        self.emit_load_value(ADDR_REG, addr)?;
                        if byte {
                            self.emitter.emit_ldrb(SCRATCH, ADDR_REG, 0);
                        } else {
                            self.emitter.emit_ldr(SCRATCH, ADDR_REG, 0);
                        }
                    }
                }
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Store { addr, value, ty } => {
                self.emit_load_value(SCRATCH, value)?;
                let byte = Self::is_byte_type(ty);
                match addr {
                    IrValue::Reg(vreg) if self.is_global_vreg(*vreg) => {
                        let data_offset = self.global_vregs[vreg];
                        self.emit_store_global_value(SCRATCH, data_offset);
                    }
                    IrValue::Reg(vreg) => {
                        if byte {
                            self.emit_storeb_vreg(SCRATCH, *vreg);
                        } else {
                            self.emit_store_vreg(SCRATCH, *vreg);
                        }
                    }
                    _ => {
                        self.emit_load_value(ADDR_REG, addr)?;
                        if byte {
                            self.emitter.emit_strb(SCRATCH, ADDR_REG, 0);
                        } else {
                            self.emitter.emit_str(SCRATCH, ADDR_REG, 0);
                        }
                    }
                }
            }

            IrInst::AddressOf { dest, src } => {
                if self.is_global_vreg(*src) {
                    let data_offset = self.global_vregs[src];
                    self.emit_literal_address(SCRATCH, data_offset);
                } else {
                    let src_offset = self.vreg_offset(*src);
                    self.emit_local_address(SCRATCH, src_offset);
                }
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Call {
                dest, name, args, ..
            } => {
                if args.len() > ARG_REGS.len() {
                    return Err(MorphlexError::CodegenError(format!(
                        "AArch64 backend: function call to '{}' has {} integer arguments; \
                         AAPCS64 supports at most {} via registers (stack passing not implemented)",
                        name,
                        args.len(),
                        ARG_REGS.len()
                    )));
                }
                for (i, arg) in args.iter().enumerate() {
                    self.emit_load_value(ARG_REGS[i], arg)?;
                }
                let pos = self.emitter.len();
                self.emitter.emit_bl(0);
                self.call_fixups.push((pos, name.clone()));
                self.emit_store_vreg(Aarch64Reg::X0, *dest);
            }

            IrInst::Syscall { dest, number, args } => {
                const MAX_SYSCALL_ARGS: usize = 6;
                if args.len() > MAX_SYSCALL_ARGS {
                    return Err(MorphlexError::CodegenError(format!(
                        "AArch64 backend: syscall has {} arguments; \
                         AArch64 supports at most {}",
                        args.len(),
                        MAX_SYSCALL_ARGS
                    )));
                }
                self.emit_load_value(Aarch64Reg::X8, number)?;
                for (i, arg) in args.iter().enumerate() {
                    self.emit_load_value(ARG_REGS[i], arg)?;
                }
                self.emitter.emit_svc(0);
                self.emit_store_vreg(Aarch64Reg::X0, *dest);
            }

            IrInst::ArrayLength { dest, count } => {
                self.emitter.emit_load_imm64(SCRATCH, *count as i64);
                self.emit_store_vreg(SCRATCH, *dest);
            }

            IrInst::Alloca { .. } | IrInst::ArrayAlloc { .. } | IrInst::Nop => {
                // Stack space already allocated; nothing to emit.
            }

            unsupported => {
                return Err(MorphlexError::CodegenError(format!(
                    "AArch64 backend does not yet support IR instruction: {:?}",
                    unsupported
                )));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, term: &Terminator) -> MorphResult<()> {
        match term {
            Terminator::Return(value) => {
                if let Some(val) = value {
                    self.emit_load_value(Aarch64Reg::X0, val)?;
                }
                if self.is_entry_point {
                    // _start: exit via syscall (x8 = 93, x0 = exit code).
                    self.emitter.emit_load_imm64(Aarch64Reg::X8, 93);
                    self.emitter.emit_svc(0);
                } else {
                    self.emit_epilogue();
                }
            }

            Terminator::Halt(code) => {
                if self.is_entry_point {
                    self.emit_load_value(Aarch64Reg::X0, code)?;
                    self.emitter.emit_load_imm64(Aarch64Reg::X8, 93);
                    self.emitter.emit_svc(0);
                } else {
                    self.emitter.emit_load_imm64(Aarch64Reg::X0, 0);
                    self.emit_epilogue();
                }
            }

            Terminator::Jump(label) => {
                self.emit_branch_b(label);
            }

            Terminator::Branch {
                cond,
                true_label,
                false_label,
            } => {
                self.emit_load_vreg(SCRATCH, *cond);
                self.emit_branch_cbnz(SCRATCH, true_label);
                self.emit_branch_b(false_label);
            }

            Terminator::Unreachable => {
                self.emitter.emit_u32(0xD4200000); // brk #0
            }
        }
        Ok(())
    }

    fn emit_load_value(&mut self, dest: Aarch64Reg, value: &IrValue) -> MorphResult<()> {
        match value {
            IrValue::Imm(imm) => self.emitter.emit_load_imm64(dest, *imm),
            IrValue::Reg(vreg) => {
                if let Some(&data_offset) = self.global_vregs.get(vreg) {
                    self.emit_load_global_value(dest, data_offset);
                } else {
                    self.emit_load_vreg(dest, *vreg);
                }
            }
            IrValue::Named(_) => self.emitter.emit_load_imm64(dest, 0),
        }
        Ok(())
    }

    fn emit_local_address(&mut self, rd: Aarch64Reg, offset: u32) {
        let tmp = if rd == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
        if offset <= 4095 {
            self.emitter.emit_add_imm(rd, Aarch64Reg::Sp, offset as u16, false);
        } else {
            self.emitter.emit_load_imm64(tmp, offset as i64);
            self.emitter.emit_add(rd, Aarch64Reg::Sp, tmp);
        }
    }

    /// Maximum unsigned immediate offset for 64-bit load/store (scaled by 8).
    const MAX_UIMM12_64: u32 = 0xFFF * 8; // 32760
    /// Maximum unsigned immediate offset for byte load/store (scaled by 1).
    const MAX_UIMM12_8: u32 = 0xFFF; // 4095

    /// Load a 64-bit value from `vreg`'s stack slot, materializing the address
    /// in a scratch register if the offset exceeds the unsigned immediate range.
    fn emit_load_vreg(&mut self, dest: Aarch64Reg, vreg: VReg) {
        let offset = self.vreg_offset(vreg);
        if offset <= Self::MAX_UIMM12_64 {
            self.emitter.emit_ldr(dest, Aarch64Reg::Sp, offset);
        } else {
            let addr = if dest == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
            self.emit_local_address(addr, offset);
            self.emitter.emit_ldr(dest, addr, 0);
        }
    }

    /// Store a 64-bit value to `vreg`'s stack slot, materializing the address
    /// in a scratch register if the offset exceeds the unsigned immediate range.
    fn emit_store_vreg(&mut self, src: Aarch64Reg, vreg: VReg) {
        let offset = self.vreg_offset(vreg);
        if offset <= Self::MAX_UIMM12_64 {
            self.emitter.emit_str(src, Aarch64Reg::Sp, offset);
        } else {
            let addr = if src == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
            self.emit_local_address(addr, offset);
            self.emitter.emit_str(src, addr, 0);
        }
    }

    /// Load a byte from `vreg`'s stack slot, materializing the address if needed.
    fn emit_loadb_vreg(&mut self, dest: Aarch64Reg, vreg: VReg) {
        let offset = self.vreg_offset(vreg);
        if offset <= Self::MAX_UIMM12_8 {
            self.emitter.emit_ldrb(dest, Aarch64Reg::Sp, offset);
        } else {
            let addr = if dest == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
            self.emit_local_address(addr, offset);
            self.emitter.emit_ldrb(dest, addr, 0);
        }
    }

    /// Store a byte to `vreg`'s stack slot, materializing the address if needed.
    fn emit_storeb_vreg(&mut self, src: Aarch64Reg, vreg: VReg) {
        let offset = self.vreg_offset(vreg);
        if offset <= Self::MAX_UIMM12_8 {
            self.emitter.emit_strb(src, Aarch64Reg::Sp, offset);
        } else {
            let addr = if src == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
            self.emit_local_address(addr, offset);
            self.emitter.emit_strb(src, addr, 0);
        }
    }

    fn emit_literal_address(&mut self, rd: Aarch64Reg, data_offset: usize) {
        // ldr rd, [pc, #4] followed by the 64-bit address placeholder.
        let insn = 0x58000000 | (1u32 << 5) | rd.encoding() as u32;
        self.emitter.emit_u32(insn);
        let fixup_pos = self.emitter.len();
        self.emitter
            .text
            .extend_from_slice(&(data_offset as u64).to_le_bytes());
        self.emitter.data_fixups.push(fixup_pos);
    }

    fn emit_load_global_value(&mut self, dest: Aarch64Reg, data_offset: usize) {
        let addr_reg = if dest == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
        self.emit_literal_address(addr_reg, data_offset);
        self.emitter.emit_ldr(dest, addr_reg, 0);
    }

    fn emit_store_global_value(&mut self, src: Aarch64Reg, data_offset: usize) {
        let addr_reg = if src == ADDR_REG { ADDR_REG2 } else { ADDR_REG };
        self.emit_literal_address(addr_reg, data_offset);
        self.emitter.emit_str(src, addr_reg, 0);
    }

    fn emit_branch_b(&mut self, label: &str) {
        let pos = self.emitter.len();
        self.emitter.emit_b(0);
        self.fixups.push((pos, label.to_string(), BranchKind::B));
    }

    fn emit_branch_cbnz(&mut self, rt: Aarch64Reg, label: &str) {
        let pos = self.emitter.len();
        self.emitter.emit_cbnz(rt, 0);
        self.fixups.push((pos, label.to_string(), BranchKind::Cbnz));
    }

    fn apply_local_fixups(&mut self) -> MorphResult<()> {
        for (pos, label, kind) in self.fixups.drain(..) {
            let target = self.label_offsets.get(label.as_str()).ok_or_else(|| {
                MorphlexError::CodegenError(format!(
                    "AArch64 backend: unresolved local label '{}'",
                    label
                ))
            })?;
            let offset = (*target as i64 - pos as i64) as i32;
            match kind {
                BranchKind::B => Self::patch_imm26(&mut self.emitter.text, pos, offset),
                BranchKind::Cbnz => Self::patch_imm19(&mut self.emitter.text, pos, offset),
            }
        }
        Ok(())
    }

    fn apply_call_fixups(&mut self) -> MorphResult<()> {
        for (pos, name) in self.call_fixups.drain(..) {
            let target = self.function_offsets.get(&name).ok_or_else(|| {
                MorphlexError::CodegenError(format!(
                    "AArch64 backend: unresolved function '{}'",
                    name
                ))
            })?;
            let offset = (*target as i64 - pos as i64) as i32;
            Self::patch_imm26(&mut self.emitter.text, pos, offset);
        }
        Ok(())
    }

    fn patch_imm19(text: &mut [u8], pos: usize, offset: i32) {
        let word = u32::from_le_bytes(text[pos..pos + 4].try_into().unwrap());
        let imm19 = ((offset as u32) >> 2) & 0x7FFFF;
        let patched = (word & !(0x7FFFF << 5)) | (imm19 << 5);
        text[pos..pos + 4].copy_from_slice(&patched.to_le_bytes());
    }

    fn patch_imm26(text: &mut [u8], pos: usize, offset: i32) {
        let word = u32::from_le_bytes(text[pos..pos + 4].try_into().unwrap());
        let imm26 = ((offset as u32) >> 2) & 0x3FFFFFF;
        let patched = (word & !0x3FFFFFF) | imm26;
        text[pos..pos + 4].copy_from_slice(&patched.to_le_bytes());
    }
}

// ─── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jstar::grammar::JStarType;
    use crate::jstar::ir::{
        BasicBlock, IrFunction, IrInst, IrProgram, IrValue, Terminator,
    };
    use std::collections::HashMap;

    fn empty_program(functions: Vec<IrFunction>) -> IrProgram {
        IrProgram {
            functions,
            string_data: Vec::new(),
            global_data: Vec::new(),
            global_vars: HashMap::new(),
            global_vregs: HashMap::new(),
        }
    }

    #[test]
    fn test_movz_x0_42() {
        let mut e = Aarch64Emitter::new();
        e.emit_movz(Aarch64Reg::X0, 42, 0);
        assert_eq!(e.text, vec![0x40, 0x05, 0x80, 0xD2]);
    }

    #[test]
    fn test_ret() {
        let mut e = Aarch64Emitter::new();
        e.emit_ret();
        assert_eq!(e.text, vec![0xC0, 0x03, 0x5F, 0xD6]);
    }

    #[test]
    fn test_register_arithmetic_and_logic() {
        let mut e = Aarch64Emitter::new();
        e.emit_add(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_sub(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_madd(
            Aarch64Reg::X0,
            Aarch64Reg::X1,
            Aarch64Reg::X2,
            Aarch64Reg::X3,
        );
        e.emit_msub(
            Aarch64Reg::X0,
            Aarch64Reg::X1,
            Aarch64Reg::X2,
            Aarch64Reg::X3,
        );
        e.emit_sdiv(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_udiv(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_and(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_orr(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_eor(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_lsl(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_lsr(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);
        e.emit_asr(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2);

        #[rustfmt::skip]
        let expected: Vec<u8> = vec![
            0x20, 0x00, 0x02, 0x8B, // add  x0, x1, x2
            0x20, 0x00, 0x02, 0xCB, // sub  x0, x1, x2
            0x20, 0x0C, 0x02, 0x9B, // madd x0, x1, x2, x3
            0x20, 0x8C, 0x02, 0x9B, // msub x0, x1, x2, x3
            0x20, 0x0C, 0xC2, 0x9A, // sdiv x0, x1, x2
            0x20, 0x08, 0xC2, 0x9A, // udiv x0, x1, x2
            0x20, 0x00, 0x02, 0x8A, // and  x0, x1, x2
            0x20, 0x00, 0x02, 0xAA, // orr  x0, x1, x2
            0x20, 0x00, 0x02, 0xCA, // eor  x0, x1, x2
            0x20, 0x20, 0xC2, 0x9A, // lsl  x0, x1, x2
            0x20, 0x24, 0xC2, 0x9A, // lsr  x0, x1, x2
            0x20, 0x28, 0xC2, 0x9A, // asr  x0, x1, x2
        ];
        assert_eq!(e.text, expected);
    }

    #[test]
    fn test_immediates_and_moves() {
        let mut e = Aarch64Emitter::new();
        e.emit_add_imm(Aarch64Reg::X0, Aarch64Reg::X1, 1234, false);
        e.emit_sub_imm(Aarch64Reg::X0, Aarch64Reg::X1, 1234, false);
        e.emit_movz(Aarch64Reg::X0, 42, 16);
        e.emit_movn(Aarch64Reg::X0, 42, 0);
        e.emit_movk(Aarch64Reg::X0, 42, 0);
        e.emit_mov(Aarch64Reg::X0, Aarch64Reg::X1);
        e.emit_mov_imm(Aarch64Reg::X0, 42);

        let expected: Vec<u8> = vec![
            0x20, 0x48, 0x13, 0x91, // add x0, x1, #1234
            0x20, 0x48, 0x13, 0xD1, // sub x0, x1, #1234
            0x40, 0x05, 0xA0, 0xD2, // movz x0, #42, lsl #16
            0x40, 0x05, 0x80, 0x92, // movn x0, #42
            0x40, 0x05, 0x80, 0xF2, // movk x0, #42
            0xE0, 0x03, 0x01, 0xAA, // mov  x0, x1
            0x40, 0x05, 0x80, 0xD2, // mov  x0, #42 (movz)
        ];
        assert_eq!(e.text, expected);
    }

    #[test]
    fn test_loads_and_stores() {
        let mut e = Aarch64Emitter::new();
        e.emit_ldr(Aarch64Reg::X0, Aarch64Reg::X1, 16);
        e.emit_str(Aarch64Reg::X0, Aarch64Reg::X1, 16);
        e.emit_ldrb(Aarch64Reg::X0, Aarch64Reg::X1, 16);
        e.emit_strb(Aarch64Reg::X0, Aarch64Reg::X1, 16);
        e.emit_ldp(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2, 16);
        e.emit_stp(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2, 16);

        let expected: Vec<u8> = vec![
            0x20, 0x08, 0x40, 0xF9, // ldr  x0, [x1, #16]
            0x20, 0x08, 0x00, 0xF9, // str  x0, [x1, #16]
            0x20, 0x40, 0x40, 0x39, // ldrb w0, [x1, #16]
            0x20, 0x40, 0x00, 0x39, // strb w0, [x1, #16]
            0x40, 0x04, 0x41, 0xA9, // ldp  x0, x1, [x2, #16]
            0x40, 0x04, 0x01, 0xA9, // stp  x0, x1, [x2, #16]
        ];
        assert_eq!(e.text, expected);
    }

    #[test]
    fn test_branches() {
        let mut e = Aarch64Emitter::new();
        // Branch back 80 bytes from the current instruction.
        e.emit_b(-80);
        e.emit_bl(-84);
        e.emit_cbz(Aarch64Reg::X0, -88);
        e.emit_cbnz(Aarch64Reg::X0, -92);
        e.emit_b_cond(Aarch64Cond::Eq, -96);

        let expected: Vec<u8> = vec![
            0xEC, 0xFF, 0xFF, 0x17, // b    -80
            0xEB, 0xFF, 0xFF, 0x97, // bl   -84
            0x40, 0xFD, 0xFF, 0xB4, // cbz  x0, -88
            0x20, 0xFD, 0xFF, 0xB5, // cbnz x0, -92
            0x00, 0xFD, 0xFF, 0x54, // b.eq -96
        ];
        assert_eq!(e.text, expected);
    }

    #[test]
    fn test_system_registers() {
        let mut e = Aarch64Emitter::new();
        e.emit_mrs(Aarch64Reg::X0, Aarch64SysReg::TPIDR_EL0);
        e.emit_msr(Aarch64SysReg::TPIDR_EL0, Aarch64Reg::X0);

        let expected: Vec<u8> = vec![
            0x40, 0xD0, 0x3B, 0xD5, // mrs x0, TPIDR_EL0
            0x40, 0xD0, 0x1B, 0xD5, // msr TPIDR_EL0, x0
        ];
        assert_eq!(e.text, expected);
    }

    #[test]
    #[should_panic(expected = "movz shift must be 0, 16, 32 or 48")]
    fn test_movz_rejects_bad_shift() {
        let mut e = Aarch64Emitter::new();
        e.emit_movz(Aarch64Reg::X0, 42, 8);
    }

    #[test]
    #[should_panic(expected = "load/store offset out of range")]
    fn test_ldr_rejects_out_of_range_offset() {
        let mut e = Aarch64Emitter::new();
        e.emit_ldr(Aarch64Reg::X0, Aarch64Reg::X1, 8 * (0xFFF + 1));
    }

    #[test]
    #[should_panic(expected = "ldp offset out of range")]
    fn test_ldp_rejects_out_of_range_offset() {
        let mut e = Aarch64Emitter::new();
        e.emit_ldp(Aarch64Reg::X0, Aarch64Reg::X1, Aarch64Reg::X2, 512);
    }

    #[test]
    #[should_panic(expected = "branch offset out of range for imm26")]
    fn test_b_rejects_out_of_range_offset() {
        let mut e = Aarch64Emitter::new();
        e.emit_b(((1 << 25) - 1) * 4 + 4);
    }

    /// A stack slot beyond the 64-bit unsigned immediate range (32760 bytes)
    /// must not panic; the backend should materialize the address in a scratch
    /// register and use an offset-0 load/store.
    #[test]
    fn test_large_stack_frame_no_panic() {
        // ArrayAlloc of 4097 64-bit elements consumes 32768 bytes, so the next
        // vreg lives at offset 32768 (> 32760).
        let func = IrFunction {
            name: "large_frame".to_string(),
            return_type: JStarType::Int,
            param_vregs: vec![],
            param_count: 0,
            next_vreg: 2,
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    IrInst::ArrayAlloc { dest: 0, count: 4097 },
                    IrInst::Copy {
                        dest: 1,
                        src: IrValue::Imm(42),
                        ty: JStarType::Int,
                    },
                ],
                terminator: Terminator::Return(Some(IrValue::Reg(1))),
            }],
        };
        let program = empty_program(vec![func]);
        let code = generate(&program).expect("large stack frame should not panic");
        assert!(!code.text.is_empty(), "backend produced no machine code");
    }

    /// A function call with more than 8 integer arguments is unsupported.
    #[test]
    fn test_call_too_many_args_errors() {
        let func = IrFunction {
            name: "caller".to_string(),
            return_type: JStarType::Int,
            param_vregs: vec![],
            param_count: 0,
            next_vreg: 2,
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![IrInst::Call {
                    dest: 0,
                    name: "callee".to_string(),
                    args: (1..=9).map(IrValue::Imm).collect(),
                    ty: JStarType::Int,
                }],
                terminator: Terminator::Return(Some(IrValue::Reg(0))),
            }],
        };
        let program = empty_program(vec![func]);
        let err = generate(&program).expect_err("call with >8 args should error");
        let msg = format!("{err}");
        assert!(
            msg.contains("9 integer arguments"),
            "unexpected error message: {msg}"
        );
    }

    /// A syscall with more than 6 arguments is unsupported.
    #[test]
    fn test_syscall_too_many_args_errors() {
        let func = IrFunction {
            name: "_start".to_string(),
            return_type: JStarType::Void,
            param_vregs: vec![],
            param_count: 0,
            next_vreg: 1,
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![IrInst::Syscall {
                    dest: 0,
                    number: IrValue::Imm(93),
                    args: (0..7).map(IrValue::Imm).collect(),
                }],
                terminator: Terminator::Halt(IrValue::Imm(0)),
            }],
        };
        let program = empty_program(vec![func]);
        let err = generate(&program).expect_err("syscall with >6 args should error");
        let msg = format!("{err}");
        assert!(
            msg.contains("7 arguments"),
            "unexpected error message: {msg}"
        );
    }
}
