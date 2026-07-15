//! Architecture-neutral code generation entry point.

pub mod aarch64;
pub mod x86_64;

use super::ir::IrProgram;
use crate::types::MorphResult;

pub use x86_64::MachineCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

pub fn generate(arch: Arch, program: &IrProgram) -> MorphResult<MachineCode> {
    match arch {
        Arch::X86_64 => x86_64::generate(program),
        Arch::Aarch64 => aarch64::generate(program),
    }
}
