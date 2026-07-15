//! AArch64 Code Generation — Phase 5 of the JStar compiler (ARM64 backend).
//!
//! Direct AArch64 machine code emission. This file is a placeholder for the
//! AArch64 backend; the actual emitter will be implemented in later tasks.

use super::MachineCode;
use crate::jstar::ir::IrProgram;
use crate::types::{MorphResult, MorphlexError};

/// Generate AArch64 machine code from IR.
pub fn generate(_program: &IrProgram) -> MorphResult<MachineCode> {
    Err(MorphlexError::CodegenError(
        "AArch64 backend not yet implemented".to_string(),
    ))
}
