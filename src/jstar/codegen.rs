//! x86-64 Code Generation — Phase 5 of the JStar compiler.
//!
//! Direct x86-64 machine code emission. No LLVM. No Cranelift.
//! The bootstrap compiler must have zero external codegen dependencies
//! so the self-hosted JStar compiler inherits none.
//!
//! Calling convention: System V AMD64 ABI
//!   Args: rdi, rsi, rdx, rcx, r8, r9 (integers/pointers)
//!   Return: rax
//!   Callee-saved: rbx, rbp, r12-r15
//!   Caller-saved: rax, rcx, rdx, rsi, rdi, r8-r11
//!
//! Register allocation: simple linear mapping from virtual registers.
//! SSA form from IR makes this straightforward.

use crate::types::MorphResult;
use super::ir::*;

// ─── x86-64 Register Encoding ───────────────────────────────────────────────

/// x86-64 general-purpose registers (64-bit names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum X86Reg {
    Rax = 0,
    Rcx = 1,
    Rdx = 2,
    Rbx = 3,
    Rsp = 4,
    Rbp = 5,
    Rsi = 6,
    Rdi = 7,
    R8 = 8,
    R9 = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

impl X86Reg {
    /// The 3-bit register encoding for ModR/M and SIB bytes.
    fn encoding(self) -> u8 {
        (self as u8) & 0x07
    }

    /// Whether this register requires a REX.B or REX.R prefix (r8-r15).
    fn needs_rex_ext(self) -> bool {
        (self as u8) >= 8
    }
}

/// Scratch registers available for allocation (caller-saved, minus rsp).
const SCRATCH_REGS: [X86Reg; 7] = [
    X86Reg::Rax,
    X86Reg::Rcx,
    X86Reg::Rdx,
    X86Reg::Rsi,
    X86Reg::Rdi,
    X86Reg::R8,
    X86Reg::R9,
];

// ─── Machine Code Buffer ────────────────────────────────────────────────────

/// The output of code generation: raw x86-64 machine code bytes.
#[derive(Debug, Clone)]
pub struct MachineCode {
    /// The .text section (executable code)
    pub text: Vec<u8>,
    /// The .data section (initialized data)
    pub data: Vec<u8>,
    /// Stack frame size for _start
    pub stack_size: usize,
}

/// Generate x86-64 machine code from IR.
pub fn generate(program: &IrProgram) -> MorphResult<MachineCode> {
    let mut emitter = CodeGen::new();

    for func in &program.functions {
        emitter.emit_function(func)?;
    }

    Ok(MachineCode {
        text: emitter.text,
        data: emitter.data,
        stack_size: emitter.stack_size,
    })
}

struct CodeGen {
    text: Vec<u8>,
    data: Vec<u8>,
    stack_size: usize,
    /// Map virtual register -> stack offset from rbp
    vreg_offsets: std::collections::HashMap<VReg, i32>,
    next_stack_offset: i32,
}

impl CodeGen {
    fn new() -> Self {
        CodeGen {
            text: Vec::new(),
            data: Vec::new(),
            stack_size: 0,
            vreg_offsets: std::collections::HashMap::new(),
            next_stack_offset: -8, // first slot at rbp-8
        }
    }

    /// Allocate a stack slot for a virtual register. Returns offset from rbp.
    fn alloc_stack_slot(&mut self, vreg: VReg, size: usize) -> i32 {
        let aligned_size = ((size + 7) / 8) * 8; // 8-byte align
        self.next_stack_offset -= aligned_size as i32;
        let offset = self.next_stack_offset;
        self.vreg_offsets.insert(vreg, offset);
        offset
    }

    /// Get the stack offset for a virtual register.
    fn vreg_offset(&self, vreg: VReg) -> i32 {
        *self.vreg_offsets.get(&vreg).unwrap_or(&0)
    }

    fn emit_function(&mut self, func: &IrFunction) -> MorphResult<()> {
        // Pre-allocate stack slots for all virtual registers
        for block in &func.blocks {
            for inst in &block.instructions {
                match inst {
                    IrInst::Alloca { dest, size, .. } => {
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
                    | IrInst::Syscall { dest, .. } => {
                        self.alloc_stack_slot(*dest, 8);
                    }
                    IrInst::Store { .. } | IrInst::Nop => {}
                }
            }
        }

        // Calculate total stack frame size (16-byte aligned per ABI)
        self.stack_size = ((-self.next_stack_offset) as usize + 15) & !15;

        // Function prologue: push rbp; mov rbp, rsp; sub rsp, frame_size
        self.emit_push_reg(X86Reg::Rbp);
        self.emit_mov_reg_reg(X86Reg::Rbp, X86Reg::Rsp);
        if self.stack_size > 0 {
            self.emit_sub_reg_imm(X86Reg::Rsp, self.stack_size as i32);
        }

        // Emit each basic block
        for block in &func.blocks {
            self.emit_block(block)?;
        }

        Ok(())
    }

    fn emit_block(&mut self, block: &BasicBlock) -> MorphResult<()> {
        for inst in &block.instructions {
            self.emit_instruction(inst)?;
        }
        self.emit_terminator(&block.terminator)?;
        Ok(())
    }

    fn emit_instruction(&mut self, inst: &IrInst) -> MorphResult<()> {
        match inst {
            IrInst::BinOp {
                dest,
                op,
                lhs,
                rhs,
                ..
            } => {
                // Load lhs into rax
                self.emit_load_value(X86Reg::Rax, lhs);
                // Load rhs into rcx
                self.emit_load_value(X86Reg::Rcx, rhs);

                match op {
                    IrBinOp::Add => self.emit_add_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Sub => self.emit_sub_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Mul => self.emit_imul_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Div => {
                        // idiv: rdx:rax / rcx -> rax (quotient), rdx (remainder)
                        self.emit_cqo(); // sign-extend rax into rdx
                        self.emit_idiv(X86Reg::Rcx);
                    }
                    IrBinOp::Mod => {
                        self.emit_cqo();
                        self.emit_idiv(X86Reg::Rcx);
                        // Remainder is in rdx, move to rax
                        self.emit_mov_reg_reg(X86Reg::Rax, X86Reg::Rdx);
                    }
                    IrBinOp::And => self.emit_and_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Or => self.emit_or_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Xor => self.emit_xor_reg_reg(X86Reg::Rax, X86Reg::Rcx),
                    IrBinOp::Shl => self.emit_shl_reg_cl(X86Reg::Rax),
                    IrBinOp::Shr => self.emit_shr_reg_cl(X86Reg::Rax),
                }

                // Store result from rax to dest's stack slot
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::UnaryOp {
                dest, op, src, ..
            } => {
                self.emit_load_value(X86Reg::Rax, src);
                match op {
                    IrUnaryOp::Neg => self.emit_neg(X86Reg::Rax),
                    IrUnaryOp::Not => self.emit_not(X86Reg::Rax),
                }
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Copy { dest, src, .. } => {
                self.emit_load_value(X86Reg::Rax, src);
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Compare { dest, lhs, rhs, .. } => {
                self.emit_load_value(X86Reg::Rax, lhs);
                self.emit_load_value(X86Reg::Rcx, rhs);
                self.emit_cmp_reg_reg(X86Reg::Rax, X86Reg::Rcx);
                // Set result based on flags (sete for equality)
                self.emit_sete(X86Reg::Rax);
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Load { dest, addr, .. } => {
                self.emit_load_value(X86Reg::Rax, addr);
                // Dereference: mov rax, [rax]
                self.emit_load_indirect(X86Reg::Rax, X86Reg::Rax);
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Store { addr, value, .. } => {
                self.emit_load_value(X86Reg::Rcx, value);
                self.emit_load_value(X86Reg::Rax, addr);
                // Store: mov [rax], rcx
                self.emit_store_indirect(X86Reg::Rax, X86Reg::Rcx);
            }

            IrInst::Syscall {
                dest,
                number,
                args,
            } => {
                // Linux syscall ABI: rax=number, rdi/rsi/rdx/r10/r8/r9=args
                self.emit_load_value(X86Reg::Rax, number);
                let arg_regs = [
                    X86Reg::Rdi,
                    X86Reg::Rsi,
                    X86Reg::Rdx,
                    X86Reg::R10,
                    X86Reg::R8,
                    X86Reg::R9,
                ];
                for (i, arg) in args.iter().enumerate() {
                    if i < arg_regs.len() {
                        self.emit_load_value(arg_regs[i], arg);
                    }
                }
                self.emit_syscall();
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Call { dest, .. } => {
                // Placeholder for function calls — will be resolved in linker
                let offset = self.vreg_offset(*dest);
                self.emit_store_reg_to_rbp_offset(X86Reg::Rax, offset);
            }

            IrInst::Alloca { .. } => {
                // Stack space already allocated in prologue
            }

            IrInst::Nop => {
                self.emit_nop();
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, term: &Terminator) -> MorphResult<()> {
        match term {
            Terminator::Return(value) => {
                // Load return value into rax
                if let Some(val) = value {
                    self.emit_load_value(X86Reg::Rax, val);
                }
                // Exit via syscall (for _start, not a function return)
                // mov rdi, rax (exit code)
                self.emit_mov_reg_reg(X86Reg::Rdi, X86Reg::Rax);
                // mov rax, 60 (sys_exit)
                self.emit_mov_reg_imm64(X86Reg::Rax, 60);
                self.emit_syscall();
            }

            Terminator::Halt(code) => {
                self.emit_load_value(X86Reg::Rdi, code);
                self.emit_mov_reg_imm64(X86Reg::Rax, 60);
                self.emit_syscall();
            }

            Terminator::Jump(_label) => {
                // Placeholder: would need label resolution
                self.emit_nop();
            }

            Terminator::Branch { .. } => {
                // Placeholder: would need label resolution
                self.emit_nop();
            }

            Terminator::Unreachable => {
                // UD2 — undefined instruction trap
                self.text.extend_from_slice(&[0x0F, 0x0B]);
            }
        }
        Ok(())
    }

    /// Load an IrValue into a register.
    fn emit_load_value(&mut self, dest: X86Reg, value: &IrValue) {
        match value {
            IrValue::Imm(imm) => {
                self.emit_mov_reg_imm64(dest, *imm);
            }
            IrValue::Reg(vreg) => {
                let offset = self.vreg_offset(*vreg);
                self.emit_load_from_rbp_offset(dest, offset);
            }
            IrValue::Named(_name) => {
                // Named variables: for now, load 0 (resolved later with symbol table)
                self.emit_mov_reg_imm64(dest, 0);
            }
        }
    }

    // ─── x86-64 Instruction Encoding ────────────────────────────────────────

    /// REX prefix byte for 64-bit operand size.
    fn rex_w(reg: X86Reg, rm: X86Reg) -> u8 {
        let mut rex: u8 = 0x48; // REX.W
        if reg.needs_rex_ext() {
            rex |= 0x04; // REX.R
        }
        if rm.needs_rex_ext() {
            rex |= 0x01; // REX.B
        }
        rex
    }

    /// ModR/M byte for register-register addressing.
    fn modrm_reg(reg: X86Reg, rm: X86Reg) -> u8 {
        0xC0 | (reg.encoding() << 3) | rm.encoding()
    }

    /// ModR/M byte for [rbp+disp32] addressing.
    fn modrm_rbp_disp32(reg: X86Reg) -> u8 {
        0x80 | (reg.encoding() << 3) | X86Reg::Rbp.encoding()
    }

    /// push reg
    fn emit_push_reg(&mut self, reg: X86Reg) {
        if reg.needs_rex_ext() {
            self.text.push(0x41); // REX.B
        }
        self.text.push(0x50 + reg.encoding());
    }

    /// pop reg
    fn emit_pop_reg(&mut self, reg: X86Reg) {
        if reg.needs_rex_ext() {
            self.text.push(0x41);
        }
        self.text.push(0x58 + reg.encoding());
    }

    /// mov reg, reg (64-bit)
    fn emit_mov_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x89); // MOV r/m64, r64
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// mov reg, imm64 (movabs)
    fn emit_mov_reg_imm64(&mut self, dest: X86Reg, imm: i64) {
        // Optimization: use 32-bit mov if value fits
        if imm >= 0 && imm <= u32::MAX as i64 {
            if dest.needs_rex_ext() {
                self.text.push(0x41); // REX.B
            }
            self.text.push(0xB8 + dest.encoding()); // MOV r32, imm32
            self.text.extend_from_slice(&(imm as u32).to_le_bytes());
        } else {
            let mut rex: u8 = 0x48; // REX.W
            if dest.needs_rex_ext() {
                rex |= 0x01; // REX.B
            }
            self.text.push(rex);
            self.text.push(0xB8 + dest.encoding()); // MOV r64, imm64
            self.text.extend_from_slice(&imm.to_le_bytes());
        }
    }

    /// add reg, reg (64-bit)
    fn emit_add_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x01); // ADD r/m64, r64
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// sub reg, reg (64-bit)
    fn emit_sub_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x29); // SUB r/m64, r64
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// sub reg, imm32 (64-bit)
    fn emit_sub_reg_imm(&mut self, reg: X86Reg, imm: i32) {
        let mut rex: u8 = 0x48;
        if reg.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0x81); // SUB r/m64, imm32
        self.text.push(0xC0 | (5 << 3) | reg.encoding()); // /5 = SUB
        self.text.extend_from_slice(&imm.to_le_bytes());
    }

    /// imul reg, reg (64-bit signed multiply)
    fn emit_imul_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(dest, src));
        self.text.push(0x0F);
        self.text.push(0xAF); // IMUL r64, r/m64
        self.text.push(Self::modrm_reg(dest, src));
    }

    /// cqo — sign-extend rax into rdx:rax
    fn emit_cqo(&mut self) {
        self.text.push(0x48); // REX.W
        self.text.push(0x99); // CQO
    }

    /// idiv r/m64 — signed divide rdx:rax by reg
    fn emit_idiv(&mut self, divisor: X86Reg) {
        let mut rex: u8 = 0x48;
        if divisor.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0xF7); // IDIV r/m64
        self.text.push(0xC0 | (7 << 3) | divisor.encoding()); // /7 = IDIV
    }

    /// and reg, reg
    fn emit_and_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x21);
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// or reg, reg
    fn emit_or_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x09);
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// xor reg, reg
    fn emit_xor_reg_reg(&mut self, dest: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, dest));
        self.text.push(0x31);
        self.text.push(Self::modrm_reg(src, dest));
    }

    /// shl reg, cl
    fn emit_shl_reg_cl(&mut self, reg: X86Reg) {
        let mut rex: u8 = 0x48;
        if reg.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0xD3); // SHL r/m64, CL
        self.text.push(0xC0 | (4 << 3) | reg.encoding()); // /4 = SHL
    }

    /// shr reg, cl
    fn emit_shr_reg_cl(&mut self, reg: X86Reg) {
        let mut rex: u8 = 0x48;
        if reg.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0xD3);
        self.text.push(0xC0 | (5 << 3) | reg.encoding()); // /5 = SHR
    }

    /// neg reg (two's complement negate)
    fn emit_neg(&mut self, reg: X86Reg) {
        let mut rex: u8 = 0x48;
        if reg.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0xF7);
        self.text.push(0xC0 | (3 << 3) | reg.encoding()); // /3 = NEG
    }

    /// not reg (bitwise complement)
    fn emit_not(&mut self, reg: X86Reg) {
        let mut rex: u8 = 0x48;
        if reg.needs_rex_ext() {
            rex |= 0x01;
        }
        self.text.push(rex);
        self.text.push(0xF7);
        self.text.push(0xC0 | (2 << 3) | reg.encoding()); // /2 = NOT
    }

    /// cmp reg, reg
    fn emit_cmp_reg_reg(&mut self, lhs: X86Reg, rhs: X86Reg) {
        self.text.push(Self::rex_w(rhs, lhs));
        self.text.push(0x39);
        self.text.push(Self::modrm_reg(rhs, lhs));
    }

    /// sete al; movzx rax, al (set rax to 1 if ZF=1, else 0)
    fn emit_sete(&mut self, _dest: X86Reg) {
        // sete al
        self.text.extend_from_slice(&[0x0F, 0x94, 0xC0]);
        // movzx rax, al
        self.text.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]);
    }

    /// mov reg, [rax] (load indirect)
    fn emit_load_indirect(&mut self, dest: X86Reg, addr: X86Reg) {
        self.text.push(Self::rex_w(dest, addr));
        self.text.push(0x8B); // MOV r64, r/m64
        self.text.push((dest.encoding() << 3) | addr.encoding()); // ModR/M [addr]
    }

    /// mov [addr], src (store indirect)
    fn emit_store_indirect(&mut self, addr: X86Reg, src: X86Reg) {
        self.text.push(Self::rex_w(src, addr));
        self.text.push(0x89);
        self.text.push((src.encoding() << 3) | addr.encoding());
    }

    /// mov reg, [rbp+offset]
    fn emit_load_from_rbp_offset(&mut self, dest: X86Reg, offset: i32) {
        self.text.push(Self::rex_w(dest, X86Reg::Rbp));
        self.text.push(0x8B); // MOV r64, r/m64
        self.text.push(Self::modrm_rbp_disp32(dest));
        self.text.extend_from_slice(&offset.to_le_bytes());
    }

    /// mov [rbp+offset], reg
    fn emit_store_reg_to_rbp_offset(&mut self, src: X86Reg, offset: i32) {
        self.text.push(Self::rex_w(src, X86Reg::Rbp));
        self.text.push(0x89); // MOV r/m64, r64
        self.text.push(Self::modrm_rbp_disp32(src));
        self.text.extend_from_slice(&offset.to_le_bytes());
    }

    /// syscall
    fn emit_syscall(&mut self) {
        self.text.extend_from_slice(&[0x0F, 0x05]);
    }

    /// nop
    fn emit_nop(&mut self) {
        self.text.push(0x90);
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::grammar::JStarType;

    #[test]
    fn test_generate_empty_program() {
        let ir = IrProgram {
            functions: vec![IrFunction {
                name: "_start".to_string(),
                return_type: JStarType::Int,
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: Terminator::Halt(IrValue::Imm(0)),
                }],
                next_vreg: 0,
            }],
        };
        let mc = generate(&ir).unwrap();
        assert!(!mc.text.is_empty(), "Should generate prologue + halt code");
    }

    #[test]
    fn test_generate_return_42() {
        let ir = IrProgram {
            functions: vec![IrFunction {
                name: "_start".to_string(),
                return_type: JStarType::Int,
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: Terminator::Return(Some(IrValue::Imm(42))),
                }],
                next_vreg: 0,
            }],
        };
        let mc = generate(&ir).unwrap();
        // The code should contain:
        // mov rdi, 42 (exit code)
        // mov rax, 60 (sys_exit)
        // syscall
        assert!(mc.text.len() > 10, "Should generate meaningful code");
        // Check syscall instruction is present (0x0F 0x05)
        let has_syscall = mc
            .text
            .windows(2)
            .any(|w| w == [0x0F, 0x05]);
        assert!(has_syscall, "Should contain syscall instruction");
    }

    #[test]
    fn test_codegen_determinism() {
        let ir = IrProgram {
            functions: vec![IrFunction {
                name: "_start".to_string(),
                return_type: JStarType::Int,
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![IrInst::BinOp {
                        dest: 0,
                        op: IrBinOp::Add,
                        lhs: IrValue::Imm(1),
                        rhs: IrValue::Imm(2),
                        ty: JStarType::Int,
                    }],
                    terminator: Terminator::Halt(IrValue::Imm(0)),
                }],
                next_vreg: 1,
            }],
        };
        let a = generate(&ir).unwrap();
        let b = generate(&ir).unwrap();
        assert_eq!(a.text, b.text, "Codegen must be deterministic");
    }

    #[test]
    fn test_rex_prefix() {
        // r8-r15 need REX extension
        assert!(X86Reg::R8.needs_rex_ext());
        assert!(X86Reg::R15.needs_rex_ext());
        assert!(!X86Reg::Rax.needs_rex_ext());
        assert!(!X86Reg::Rdi.needs_rex_ext());
    }

    #[test]
    fn test_register_encoding() {
        assert_eq!(X86Reg::Rax.encoding(), 0);
        assert_eq!(X86Reg::Rcx.encoding(), 1);
        assert_eq!(X86Reg::R8.encoding(), 0); // lower 3 bits
        assert_eq!(X86Reg::R15.encoding(), 7);
    }
}
