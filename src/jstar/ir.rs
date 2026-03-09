//! Intermediate Representation — Phase 4 of the JStar compiler.
//!
//! Three-address code IR between the typed AST and x86-64 machine code.
//! SSA (Static Single Assignment) form for clean lowering to codegen.
//!
//! IR instructions map 1:1 to semantic operations, not yet to x86 instructions.
//! The codegen phase performs the final IR → x86 lowering.
//!
//! Virtual register IDs are u32 (unlimited supply, allocated by codegen).

use crate::types::MorphResult;
use super::grammar::*;
use super::token_map::JStarInstruction;

// ─── IR Types ───────────────────────────────────────────────────────────────

/// Virtual register ID (SSA — each assignment creates a new register).
pub type VReg = u32;

/// An IR program — a sequence of functions.
#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

/// An IR function — entry point with a body of basic blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub return_type: JStarType,
    pub blocks: Vec<BasicBlock>,
    pub next_vreg: VReg,
}

/// A basic block — straight-line code ending with a terminator.
#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<IrInst>,
    pub terminator: Terminator,
}

/// A three-address IR instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum IrInst {
    /// dest = lhs + rhs (or other binop)
    BinOp {
        dest: VReg,
        op: IrBinOp,
        lhs: IrValue,
        rhs: IrValue,
        ty: JStarType,
    },

    /// dest = op(src)
    UnaryOp {
        dest: VReg,
        op: IrUnaryOp,
        src: IrValue,
        ty: JStarType,
    },

    /// dest = value (copy/move)
    Copy {
        dest: VReg,
        src: IrValue,
        ty: JStarType,
    },

    /// Store value to memory: *addr = value
    Store {
        addr: IrValue,
        value: IrValue,
        ty: JStarType,
    },

    /// dest = *addr (load from memory)
    Load {
        dest: VReg,
        addr: IrValue,
        ty: JStarType,
    },

    /// dest = call name(args...)
    Call {
        dest: VReg,
        name: String,
        args: Vec<IrValue>,
        ty: JStarType,
    },

    /// syscall(number, args...)
    Syscall {
        dest: VReg,
        number: IrValue,
        args: Vec<IrValue>,
    },

    /// Allocate stack space: dest = alloca(size)
    Alloca {
        dest: VReg,
        size: usize,
        ty: JStarType,
    },

    /// Compare two values, set flags
    Compare {
        dest: VReg,
        lhs: IrValue,
        rhs: IrValue,
        ty: JStarType,
    },

    /// No-op (placeholder)
    Nop,
}

/// Binary operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

/// Unary operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnaryOp {
    Neg,
    Not,
}

/// An IR value — either a virtual register or an immediate constant.
#[derive(Debug, Clone, PartialEq)]
pub enum IrValue {
    Reg(VReg),
    Imm(i64),
    /// Named variable (resolved to stack slot in codegen)
    Named(String),
}

/// Basic block terminator — how control leaves the block.
#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    /// Return a value (or void)
    Return(Option<IrValue>),
    /// Unconditional jump
    Jump(String),
    /// Conditional branch: if cond then true_label else false_label
    Branch {
        cond: VReg,
        true_label: String,
        false_label: String,
    },
    /// Process exit (halt)
    Halt(IrValue),
    /// Unreachable (for dead code after return)
    Unreachable,
}

// ─── Lowering: Typed AST → IR ───────────────────────────────────────────────

/// Lower a typed program to IR.
///
/// Currently wraps all top-level statements in a single `_start` function
/// (the ELF entry point). Future: support function definitions.
pub fn lower(program: &TypedProgram) -> MorphResult<IrProgram> {
    let mut lowerer = Lowerer { next_vreg: 0 };
    let main_fn = lowerer.lower_to_function("_start", &program.statements)?;
    Ok(IrProgram {
        functions: vec![main_fn],
    })
}

struct Lowerer {
    next_vreg: VReg,
}

impl Lowerer {
    fn alloc_vreg(&mut self) -> VReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    fn lower_to_function(
        &mut self,
        name: &str,
        statements: &[TypedStatement],
    ) -> MorphResult<IrFunction> {
        let mut instructions = Vec::new();
        let mut terminator = None;

        for stmt in statements {
            match stmt {
                TypedStatement::Execute {
                    op, operands, result_type,
                } => {
                    let ir_insts = self.lower_execute(*op, operands, *result_type)?;
                    instructions.extend(ir_insts);
                }

                TypedStatement::Declare { ty, .. } => {
                    let dest = self.alloc_vreg();
                    instructions.push(IrInst::Alloca {
                        dest,
                        size: ty.size_bytes(),
                        ty: *ty,
                    });
                }

                TypedStatement::Return { value, .. } => {
                    let ir_val = match value {
                        Some(v) => Some(self.lower_operand(v)?),
                        None => None,
                    };
                    terminator = Some(Terminator::Return(ir_val));
                }

                TypedStatement::ControlFlow { body, .. } => {
                    // For now, lower control flow as a flat sequence
                    // (proper basic block splitting comes with optimization passes)
                    for s in body {
                        let lowered = self.lower_statement_to_insts(s)?;
                        instructions.extend(lowered);
                    }
                }

                TypedStatement::Label(_) => {
                    // Labels are handled at the basic block level
                }

                TypedStatement::Nop => {
                    instructions.push(IrInst::Nop);
                }
            }
        }

        // If no explicit terminator, default to exit(0)
        let term = terminator.unwrap_or(Terminator::Halt(IrValue::Imm(0)));

        let block = BasicBlock {
            label: "entry".to_string(),
            instructions,
            terminator: term,
        };

        Ok(IrFunction {
            name: name.to_string(),
            return_type: JStarType::Int,
            blocks: vec![block],
            next_vreg: self.next_vreg,
        })
    }

    fn lower_statement_to_insts(
        &mut self,
        stmt: &TypedStatement,
    ) -> MorphResult<Vec<IrInst>> {
        match stmt {
            TypedStatement::Execute {
                op, operands, result_type,
            } => self.lower_execute(*op, operands, *result_type),

            TypedStatement::Declare { ty, .. } => {
                let dest = self.alloc_vreg();
                Ok(vec![IrInst::Alloca {
                    dest,
                    size: ty.size_bytes(),
                    ty: *ty,
                }])
            }

            TypedStatement::Nop => Ok(vec![IrInst::Nop]),

            _ => Ok(vec![]),
        }
    }

    fn lower_execute(
        &mut self,
        op: JStarInstruction,
        operands: &[TypedOperand],
        result_type: JStarType,
    ) -> MorphResult<Vec<IrInst>> {
        let mut insts = Vec::new();
        let dest = self.alloc_vreg();

        match op {
            // Arithmetic binary ops
            JStarInstruction::Add => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Add,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Sub => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Sub,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Mul => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Mul,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Div => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Div,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Mod => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Mod,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }

            // Unary
            JStarInstruction::Neg => {
                let src = self.get_one_operand(operands)?;
                insts.push(IrInst::UnaryOp {
                    dest,
                    op: IrUnaryOp::Neg,
                    src,
                    ty: result_type,
                });
            }
            JStarInstruction::Not => {
                let src = self.get_one_operand(operands)?;
                insts.push(IrInst::UnaryOp {
                    dest,
                    op: IrUnaryOp::Not,
                    src,
                    ty: result_type,
                });
            }

            // Comparison
            JStarInstruction::Compare
            | JStarInstruction::Equal
            | JStarInstruction::Less
            | JStarInstruction::Greater => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::Compare {
                    dest,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }

            // Memory
            JStarInstruction::Load => {
                let addr = self.get_one_operand(operands)?;
                insts.push(IrInst::Load {
                    dest,
                    addr,
                    ty: result_type,
                });
            }
            JStarInstruction::Store => {
                if operands.len() >= 2 {
                    let value = self.lower_operand(&operands[0])?;
                    let addr = self.lower_operand(&operands[1])?;
                    insts.push(IrInst::Store {
                        addr,
                        value,
                        ty: result_type,
                    });
                }
            }
            JStarInstruction::Move => {
                let src = self.get_one_operand(operands)?;
                insts.push(IrInst::Copy {
                    dest,
                    src,
                    ty: result_type,
                });
            }

            // Control flow (these affect terminators, not instructions)
            JStarInstruction::Call => {
                let name = operands
                    .first()
                    .map(|o| match o {
                        TypedOperand::Variable { name, .. } => name.clone(),
                        _ => "unknown".to_string(),
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                insts.push(IrInst::Call {
                    dest,
                    name,
                    args: vec![],
                    ty: result_type,
                });
            }

            JStarInstruction::Syscall => {
                let number = self.get_one_operand(operands)?;
                insts.push(IrInst::Syscall {
                    dest,
                    number,
                    args: vec![],
                });
            }

            JStarInstruction::Halt => {
                let code = if operands.is_empty() {
                    IrValue::Imm(0)
                } else {
                    self.lower_operand(&operands[0])?
                };
                insts.push(IrInst::Copy {
                    dest,
                    src: code,
                    ty: JStarType::Int,
                });
            }

            // These are handled at statement level, not instruction level
            JStarInstruction::Return
            | JStarInstruction::Jump
            | JStarInstruction::JumpIf => {}

            // Bitwise
            JStarInstruction::And => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::And,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Or => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Or,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Xor => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Xor,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }
            JStarInstruction::Shift => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::BinOp {
                    dest,
                    op: IrBinOp::Shl,
                    lhs,
                    rhs,
                    ty: result_type,
                });
            }

            // Stack ops (push/pop) — lower to store/load on stack pointer
            JStarInstruction::Push | JStarInstruction::Pop => {
                insts.push(IrInst::Nop); // placeholder
            }

            JStarInstruction::Nop => {
                insts.push(IrInst::Nop);
            }
        }

        Ok(insts)
    }

    fn lower_operand(&self, operand: &TypedOperand) -> MorphResult<IrValue> {
        match operand {
            TypedOperand::Immediate(val, _) => Ok(IrValue::Imm(*val)),
            TypedOperand::Variable { name, .. } => Ok(IrValue::Named(name.clone())),
            TypedOperand::Register(_, _) => {
                // Registers are lowered to virtual registers in codegen
                Ok(IrValue::Imm(0)) // placeholder
            }
            TypedOperand::Addressed { target, .. } => self.lower_operand(target),
        }
    }

    fn get_one_operand(&self, operands: &[TypedOperand]) -> MorphResult<IrValue> {
        if let Some(first) = operands.first() {
            self.lower_operand(first)
        } else {
            Ok(IrValue::Imm(0))
        }
    }

    fn get_two_operands(
        &self,
        operands: &[TypedOperand],
    ) -> MorphResult<(IrValue, IrValue)> {
        let lhs = if let Some(first) = operands.first() {
            self.lower_operand(first)?
        } else {
            IrValue::Imm(0)
        };
        let rhs = if let Some(second) = operands.get(1) {
            self.lower_operand(second)?
        } else {
            IrValue::Imm(0)
        };
        Ok((lhs, rhs))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::token_map::ScopeKind;

    #[test]
    fn test_lower_empty_program() {
        let prog = TypedProgram {
            statements: vec![],
        };
        let ir = lower(&prog).unwrap();
        assert_eq!(ir.functions.len(), 1);
        assert_eq!(ir.functions[0].name, "_start");
    }

    #[test]
    fn test_lower_return_immediate() {
        let prog = TypedProgram {
            statements: vec![TypedStatement::Return {
                value: Some(TypedOperand::Immediate(42, JStarType::Int)),
                ty: JStarType::Int,
            }],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.terminator {
            Terminator::Return(Some(IrValue::Imm(42))) => {}
            other => panic!("Expected Return(Imm(42)), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_add() {
        let prog = TypedProgram {
            statements: vec![TypedStatement::Execute {
                op: JStarInstruction::Add,
                operands: vec![
                    TypedOperand::Immediate(1, JStarType::Int),
                    TypedOperand::Immediate(2, JStarType::Int),
                ],
                result_type: JStarType::Int,
            }],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.instructions[0] {
            IrInst::BinOp {
                op: IrBinOp::Add, ..
            } => {}
            other => panic!("Expected BinOp(Add), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_declaration_allocates_stack() {
        let prog = TypedProgram {
            statements: vec![TypedStatement::Declare {
                scope: ScopeKind::Local,
                name: "counter".to_string(),
                ty: JStarType::Int,
            }],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.instructions[0] {
            IrInst::Alloca { size: 4, .. } => {}
            other => panic!("Expected Alloca(4), got {:?}", other),
        }
    }

    #[test]
    fn test_default_halt_terminator() {
        let prog = TypedProgram {
            statements: vec![TypedStatement::Nop],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.terminator {
            Terminator::Halt(IrValue::Imm(0)) => {}
            other => panic!("Expected Halt(0), got {:?}", other),
        }
    }
}
