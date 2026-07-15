//! AArch64 Code Generation — low-level instruction encoder.
//!
//! Provides the AArch64 register enum (`Aarch64Reg`), condition-code enum,
//! a minimal system-register wrapper, and `Aarch64Emitter`: a byte-buffer
//! emitter for the instruction categories used by the JStar ARM64 backend.
//!
//! This file intentionally implements only instruction encoding.  IR-to-
//! machine-code translation lives in the caller (Task 4).

use super::MachineCode;
use crate::jstar::ir::IrProgram;
use crate::types::{MorphResult, MorphlexError};

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

    // ── Procedure return ───────────────────────────────────────────────────

    /// `ret`.
    pub fn emit_ret(&mut self) {
        self.emit_u32(0xD65F03C0);
    }
}

/// Generate AArch64 machine code from IR.
///
/// This is intentionally a placeholder: IR lowering is Task 4.  The function
/// remains so that the architecture-neutral `codegen::generate` dispatcher
/// compiles.
pub fn generate(_program: &IrProgram) -> MorphResult<MachineCode> {
    Err(MorphlexError::CodegenError(
        "AArch64 backend not yet implemented".to_string(),
    ))
}

// ─── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
}
