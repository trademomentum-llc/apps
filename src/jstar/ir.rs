//! Intermediate Representation — Phase 4 of the JStar compiler.
//!
//! Three-address code IR between the typed AST and x86-64 machine code.
//! SSA (Static Single Assignment) form for clean lowering to codegen.
//!
//! IR instructions map 1:1 to semantic operations, not yet to x86 instructions.
//! The codegen phase performs the final IR → x86 lowering.
//!
//! Virtual register IDs are u32 (unlimited supply, allocated by codegen).

use std::collections::HashMap;
use crate::types::MorphResult;
use super::grammar::*;
use super::token_map::{JStarInstruction, FlowKind, AddrMode};

// ─── IR Types ───────────────────────────────────────────────────────────────

/// Virtual register ID (SSA — each assignment creates a new register).
pub type VReg = u32;

/// An IR program — a sequence of functions.
#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
    /// Accumulated string literal data for the .data section.
    pub string_data: Vec<u8>,
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

    /// dest = &vreg (address of stack slot)
    AddressOf {
        dest: VReg,
        src: VReg,
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
        kind: CmpKind,
        ty: JStarType,
    },

    /// Print a value to stdout as decimal ASCII + newline.
    /// High-level IR instruction — codegen expands to itoa + sys_write.
    Print {
        value: IrValue,
    },

    /// Print a string literal to stdout (no newline appended).
    /// data_offset = byte offset in .data section, len = string length.
    PrintStr {
        data_offset: usize,
        len: usize,
    },

    /// Store value to array at base + index: base[index] = value
    StoreIndexed {
        base: VReg,
        index: IrValue,
        value: IrValue,
        ty: JStarType,
    },

    /// Load from array at base + index: dest = base[index]
    LoadIndexed {
        dest: VReg,
        base: VReg,
        index: IrValue,
        ty: JStarType,
    },

    /// No-op (placeholder)
    Nop,
}

/// Comparison kind — determines the condition code in codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpKind {
    /// Equal (sete / ZF=1)
    Eq,
    /// Not equal (setne / ZF=0)
    Ne,
    /// Less than (setl / SF!=OF)
    Lt,
    /// Less or equal (setle / ZF=1 or SF!=OF)
    Le,
    /// Greater than (setg / ZF=0 and SF=OF)
    Gt,
    /// Greater or equal (setge / SF=OF)
    Ge,
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
/// Function definitions are lowered to separate IrFunctions.
/// Top-level statements go into `_start` (the ELF entry point).
pub fn lower(program: &TypedProgram) -> MorphResult<IrProgram> {
    let mut lowerer = Lowerer {
        next_vreg: 0,
        last_result: None,
        variables: HashMap::new(),
        blocks: Vec::new(),
        current_label: String::new(),
        current_insts: Vec::new(),
        block_counter: 0,
        string_data: Vec::new(),
    };

    // Separate function definitions from top-level statements
    let mut functions = Vec::new();
    let mut top_level = Vec::new();

    for stmt in &program.statements {
        match stmt {
            TypedStatement::FunctionDef { name, params, body, return_type } => {
                // Lower each function definition
                lowerer.reset();
                // Declare parameters as variables
                for (pname, pty) in params {
                    let dest = lowerer.alloc_vreg();
                    lowerer.current_insts.push(IrInst::Alloca {
                        dest,
                        size: pty.size_bytes(),
                        ty: *pty,
                    });
                    lowerer.variables.insert(pname.clone(), dest);
                }
                let func = lowerer.lower_to_function(name, body)?;
                functions.push(IrFunction {
                    return_type: *return_type,
                    ..func
                });
            }
            other => top_level.push(other.clone()),
        }
    }

    // Lower top-level statements into _start
    lowerer.reset();
    let main_fn = lowerer.lower_to_function("_start", &top_level)?;
    functions.insert(0, main_fn);

    Ok(IrProgram {
        functions,
        string_data: lowerer.string_data,
    })
}

struct Lowerer {
    next_vreg: VReg,
    /// The vreg holding the result of the last Execute statement.
    /// "it" (Accumulator) and "that" (LastResult) resolve to this.
    last_result: Option<VReg>,
    /// Variable name -> alloca vreg. Populated by Declare statements.
    /// When a Variable operand is lowered, it resolves to Reg(alloca_vreg)
    /// instead of Named(name), enabling direct stack-slot access in codegen.
    variables: HashMap<String, VReg>,

    // ─── Block-builder state (multi-block CFG support) ──────────────────
    /// Completed basic blocks
    blocks: Vec<BasicBlock>,
    /// Label of the block currently being built
    current_label: String,
    /// Instructions accumulated for the current block
    current_insts: Vec<IrInst>,
    /// Counter for generating unique block labels
    block_counter: u32,

    /// Accumulated string literal data for the .data section.
    /// Each entry is the raw bytes. Strings are appended with a newline.
    string_data: Vec<u8>,
}

impl Lowerer {
    fn alloc_vreg(&mut self) -> VReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    /// Reset per-function state for lowering a new function.
    fn reset(&mut self) {
        self.next_vreg = 0;
        self.last_result = None;
        self.variables.clear();
        self.blocks.clear();
        self.current_label.clear();
        self.current_insts.clear();
        self.block_counter = 0;
        // Note: string_data is NOT cleared — it accumulates across functions
    }

    /// Generate a unique block label with a prefix.
    fn make_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }

    /// Finish the current block with the given terminator and start a new one.
    fn finish_block(&mut self, terminator: Terminator, new_label: &str) {
        let block = BasicBlock {
            label: self.current_label.clone(),
            instructions: std::mem::take(&mut self.current_insts),
            terminator,
        };
        self.blocks.push(block);
        self.current_label = new_label.to_string();
    }

    fn lower_to_function(
        &mut self,
        name: &str,
        statements: &[TypedStatement],
    ) -> MorphResult<IrFunction> {
        self.current_label = "entry".to_string();
        // Preserve any instructions already in current_insts (e.g. param Allocas)
        self.blocks.clear();

        let mut final_terminator: Option<Terminator> = None;

        for stmt in statements {
            self.lower_statement(stmt, &mut final_terminator)?;
        }

        // Finalize the last open block
        let term = final_terminator.unwrap_or(Terminator::Halt(IrValue::Imm(0)));
        let last_block = BasicBlock {
            label: self.current_label.clone(),
            instructions: std::mem::take(&mut self.current_insts),
            terminator: term,
        };
        self.blocks.push(last_block);

        let blocks = std::mem::take(&mut self.blocks);

        Ok(IrFunction {
            name: name.to_string(),
            return_type: JStarType::Int,
            blocks,
            next_vreg: self.next_vreg,
        })
    }

    /// Lower a single statement, appending instructions to current_insts.
    /// May create new basic blocks for control flow.
    fn lower_statement(
        &mut self,
        stmt: &TypedStatement,
        final_terminator: &mut Option<Terminator>,
    ) -> MorphResult<()> {
        match stmt {
            TypedStatement::Execute {
                op, operands, result_type,
            } => {
                let (ir_insts, dest) = self.lower_execute(*op, operands, *result_type)?;
                self.current_insts.extend(ir_insts);
                self.last_result = Some(dest);
            }

            TypedStatement::Declare { name, ty, size, .. } => {
                let dest = self.alloc_vreg();
                // size is element count; multiply by element size for byte allocation
                let alloc_size = match size {
                    Some(n) => *n * ty.size_bytes(),
                    None => ty.size_bytes(),
                };
                self.current_insts.push(IrInst::Alloca {
                    dest,
                    size: alloc_size,
                    ty: *ty,
                });
                self.variables.insert(name.clone(), dest);
            }

            TypedStatement::Return { value, .. } => {
                let ir_val = match value {
                    Some(v) => Some(self.lower_operand(v)?),
                    None => None,
                };
                *final_terminator = Some(Terminator::Return(ir_val));
            }

            TypedStatement::ControlFlow { kind, condition, body, else_body } => {
                match kind {
                    FlowKind::Conditional => {
                        self.lower_if(condition, body, else_body, final_terminator)?;
                    }
                    FlowKind::Loop => {
                        self.lower_while(condition, body, final_terminator)?;
                    }
                    _ => {
                        // Sequence/Branch: flat inline (no condition)
                        for s in body {
                            self.lower_statement(s, final_terminator)?;
                        }
                    }
                }
            }

            TypedStatement::FunctionDef { .. } => {
                // Function definitions are handled at the top level in lower()
            }

            TypedStatement::Label(_) => {
                // Labels are handled at the basic block level
            }

            TypedStatement::Nop => {
                self.current_insts.push(IrInst::Nop);
            }
        }
        Ok(())
    }

    /// Lower an `if` or `if/else` block into CFG.
    ///
    /// Without else (3 blocks):
    ///   current: Branch(cond, if_body, if_end)
    ///   if_body: ...body..., Jump(if_end)
    ///   if_end:  ...continues...
    ///
    /// With else (4 blocks):
    ///   current:   Branch(cond, if_body, else_body)
    ///   if_body:   ...body..., Jump(if_end) or Return
    ///   else_body: ...else..., Jump(if_end) or Return
    ///   if_end:    ...continues...
    ///
    /// If a branch contains a `return`, that return becomes the block
    /// terminator directly instead of Jump(if_end).
    fn lower_if(
        &mut self,
        condition: &Option<Box<TypedStatement>>,
        body: &[TypedStatement],
        else_body: &[TypedStatement],
        final_terminator: &mut Option<Terminator>,
    ) -> MorphResult<()> {
        let body_label = self.make_label("if_body");
        let else_label = if else_body.is_empty() {
            None
        } else {
            Some(self.make_label("if_else"))
        };
        let end_label = self.make_label("if_end");

        // Lower the condition into the current block
        let cond_vreg = self.lower_condition(condition)?;

        // Finish current block with Branch
        let false_target = else_label.as_deref().unwrap_or(&end_label).to_string();
        self.finish_block(
            Terminator::Branch {
                cond: cond_vreg,
                true_label: body_label.clone(),
                false_label: false_target,
            },
            &body_label,
        );

        // Emit true-branch body
        // Save final_terminator — if the branch has a return, it should be
        // the block terminator, not the function-level final terminator.
        let saved_term = final_terminator.take();
        for s in body {
            self.lower_statement(s, final_terminator)?;
        }
        let true_term = final_terminator
            .take()
            .unwrap_or_else(|| Terminator::Jump(end_label.clone()));
        self.finish_block(
            true_term,
            else_label.as_deref().unwrap_or(&end_label),
        );

        // Emit else-branch body (if present)
        if !else_body.is_empty() {
            for s in else_body {
                self.lower_statement(s, final_terminator)?;
            }
            let else_term = final_terminator
                .take()
                .unwrap_or_else(|| Terminator::Jump(end_label.clone()));
            self.finish_block(else_term, &end_label);
        }

        // Restore saved terminator (function-level return from before the if)
        *final_terminator = saved_term;

        // Now current block is if_end — subsequent statements go here
        Ok(())
    }

    /// Lower a `while` block into a 4-block CFG:
    ///
    ///   current_block:
    ///     ...pre-while instructions...
    ///     Jump(while_cond)
    ///
    ///   while_cond:
    ///     v_cond = condition
    ///     Branch(v_cond, while_body, while_end)
    ///
    ///   while_body:
    ///     ...body statements...
    ///     Jump(while_cond)              <-- back-edge
    ///
    ///   while_end:
    ///     ...post-while instructions continue here...
    fn lower_while(
        &mut self,
        condition: &Option<Box<TypedStatement>>,
        body: &[TypedStatement],
        final_terminator: &mut Option<Terminator>,
    ) -> MorphResult<()> {
        let cond_label = self.make_label("while_cond");
        let body_label = self.make_label("while_body");
        let end_label = self.make_label("while_end");

        // Finish current block with Jump to condition
        self.finish_block(
            Terminator::Jump(cond_label.clone()),
            &cond_label,
        );

        // Emit condition into the cond block
        let cond_vreg = self.lower_condition(condition)?;

        // Finish cond block with Branch
        self.finish_block(
            Terminator::Branch {
                cond: cond_vreg,
                true_label: body_label.clone(),
                false_label: end_label.clone(),
            },
            &body_label,
        );

        // Emit body statements
        for s in body {
            self.lower_statement(s, final_terminator)?;
        }

        // Finish body block with Jump back to cond (back-edge)
        self.finish_block(
            Terminator::Jump(cond_label.clone()),
            &end_label,
        );

        // Now current block is while_end
        Ok(())
    }

    /// Lower a condition statement and return the vreg holding the result.
    /// If no condition is provided, defaults to Imm(1) (always true).
    fn lower_condition(
        &mut self,
        condition: &Option<Box<TypedStatement>>,
    ) -> MorphResult<VReg> {
        match condition {
            Some(cond_stmt) => {
                match cond_stmt.as_ref() {
                    TypedStatement::Execute { op, operands, result_type } => {
                        let (ir_insts, dest) = self.lower_execute(*op, operands, *result_type)?;
                        self.current_insts.extend(ir_insts);
                        self.last_result = Some(dest);
                        Ok(dest)
                    }
                    _ => {
                        // Non-execute condition: emit as constant true
                        let dest = self.alloc_vreg();
                        self.current_insts.push(IrInst::Copy {
                            dest,
                            src: IrValue::Imm(1),
                            ty: JStarType::Boolean,
                        });
                        Ok(dest)
                    }
                }
            }
            None => {
                // No condition: always true
                let dest = self.alloc_vreg();
                self.current_insts.push(IrInst::Copy {
                    dest,
                    src: IrValue::Imm(1),
                    ty: JStarType::Boolean,
                });
                Ok(dest)
            }
        }
    }

    fn lower_statement_to_insts(
        &mut self,
        stmt: &TypedStatement,
    ) -> MorphResult<Vec<IrInst>> {
        match stmt {
            TypedStatement::Execute {
                op, operands, result_type,
            } => {
                let (insts, dest) = self.lower_execute(*op, operands, *result_type)?;
                self.last_result = Some(dest);
                Ok(insts)
            }

            TypedStatement::Declare { name, ty, size, .. } => {
                let dest = self.alloc_vreg();
                let alloc_size = match size {
                    Some(n) => *n * ty.size_bytes(),
                    None => ty.size_bytes(),
                };
                self.variables.insert(name.clone(), dest);
                Ok(vec![IrInst::Alloca {
                    dest,
                    size: alloc_size,
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
    ) -> MorphResult<(Vec<IrInst>, VReg)> {
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

            // Comparison — each variant maps to a CmpKind
            JStarInstruction::Compare => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                // "compare X Y" in control flow context means "X != Y" (nonzero = true)
                insts.push(IrInst::Compare {
                    dest,
                    lhs,
                    rhs,
                    kind: CmpKind::Ne,
                    ty: result_type,
                });
            }
            JStarInstruction::Equal => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::Compare {
                    dest,
                    lhs,
                    rhs,
                    kind: CmpKind::Eq,
                    ty: result_type,
                });
            }
            JStarInstruction::Less => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::Compare {
                    dest,
                    lhs,
                    rhs,
                    kind: CmpKind::Lt,
                    ty: result_type,
                });
            }
            JStarInstruction::Greater => {
                let (lhs, rhs) = self.get_two_operands(operands)?;
                insts.push(IrInst::Compare {
                    dest,
                    lhs,
                    rhs,
                    kind: CmpKind::Gt,
                    ty: result_type,
                });
            }

            // Memory
            JStarInstruction::Load => {
                let addr = self.get_one_operand(operands)?;

                // Get the array element type from the source variable's declared type.
                // For "load from buffer at INDEX", the type comes from the "from buffer" operand.
                let array_ty = match &operands[0] {
                    TypedOperand::Addressed { target, .. } => target.ty(),
                    other => other.ty(),
                };

                // Check for indexed addressing: "load from buffer at INDEX"
                let index_operand = operands.get(1).and_then(|op| {
                    if let TypedOperand::Addressed { mode: AddrMode::At, target, .. } = op {
                        Some(target.as_ref())
                    } else {
                        None
                    }
                });

                if let Some(idx_op) = index_operand {
                    let idx = self.lower_operand(idx_op)?;
                    if let IrValue::Reg(base_vreg) = addr {
                        insts.push(IrInst::LoadIndexed {
                            dest,
                            base: base_vreg,
                            index: idx,
                            ty: array_ty,
                        });
                    } else {
                        insts.push(IrInst::Load { dest, addr, ty: result_type });
                    }
                } else {
                    insts.push(IrInst::Load { dest, addr, ty: result_type });
                }
            }
            JStarInstruction::Store => {
                if operands.len() >= 2 {
                    let value = self.lower_operand(&operands[0])?;
                    let addr = self.lower_operand(&operands[1])?;

                    // Get the array element type from the destination variable's declared type.
                    // For "store X into buffer at INDEX", the type comes from the "into buffer" operand.
                    let array_ty = match &operands[1] {
                        TypedOperand::Addressed { target, .. } => target.ty(),
                        other => other.ty(),
                    };

                    // Check for indexed addressing: "store X into buffer at INDEX"
                    let index_operand = operands.get(2).and_then(|op| {
                        if let TypedOperand::Addressed { mode: AddrMode::At, target, .. } = op {
                            Some(target.as_ref())
                        } else {
                            None
                        }
                    });

                    if let Some(idx_op) = index_operand {
                        let idx = self.lower_operand(idx_op)?;
                        if let IrValue::Reg(base_vreg) = addr {
                            insts.push(IrInst::StoreIndexed {
                                base: base_vreg,
                                index: idx,
                                value,
                                ty: array_ty,
                            });
                        }
                    } else {
                        insts.push(IrInst::Store {
                            addr,
                            value,
                            ty: result_type,
                        });
                    }
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
                // Remaining operands are arguments
                let mut args = Vec::new();
                for op in operands.iter().skip(1) {
                    args.push(self.lower_operand(op)?);
                }
                insts.push(IrInst::Call {
                    dest,
                    name,
                    args,
                    ty: result_type,
                });
            }

            JStarInstruction::Syscall => {
                // syscall NUMBER arg1 arg2 ... arg6
                let number = self.lower_operand(&operands[0])?;
                let mut args = Vec::new();
                for op in operands.iter().skip(1) {
                    args.push(self.lower_operand(op)?);
                }
                insts.push(IrInst::Syscall {
                    dest,
                    number,
                    args,
                });
            }

            JStarInstruction::AddressOf => {
                // addressof X — get the stack address of variable X
                let src = self.get_one_operand(operands)?;
                match src {
                    IrValue::Reg(src_vreg) => {
                        insts.push(IrInst::AddressOf {
                            dest,
                            src: src_vreg,
                        });
                    }
                    _ => {
                        // For non-register operands, treat as no-op
                        insts.push(IrInst::Copy {
                            dest,
                            src,
                            ty: result_type,
                        });
                    }
                }
            }

            JStarInstruction::Allocate => {
                // allocate N — reserve N bytes on stack, dest = pointer to buffer
                let size_val = self.get_one_operand(operands)?;
                let size = match size_val {
                    IrValue::Imm(n) => n as usize,
                    _ => 256,
                };
                let buf_vreg = self.alloc_vreg();
                insts.push(IrInst::Alloca {
                    dest: buf_vreg,
                    size,
                    ty: result_type,
                });
                insts.push(IrInst::AddressOf {
                    dest,
                    src: buf_vreg,
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

            // I/O
            JStarInstruction::Print => {
                // Check if the first operand is a string literal
                if let Some(TypedOperand::StringLiteral(s)) = operands.first() {
                    let offset = self.string_data.len();
                    let bytes = s.as_bytes();
                    self.string_data.extend_from_slice(bytes);
                    self.string_data.push(b'\n'); // append newline
                    insts.push(IrInst::PrintStr {
                        data_offset: offset,
                        len: bytes.len() + 1, // include newline
                    });
                    insts.push(IrInst::Copy {
                        dest,
                        src: IrValue::Imm(0),
                        ty: result_type,
                    });
                } else {
                    let value = self.get_one_operand(operands)?;
                    insts.push(IrInst::Print { value: value.clone() });
                    // Also copy value to dest so "it" tracks the printed value
                    insts.push(IrInst::Copy {
                        dest,
                        src: value,
                        ty: result_type,
                    });
                }
            }

            // Stack ops (push/pop) — lower to store/load on stack pointer
            JStarInstruction::Push | JStarInstruction::Pop => {
                insts.push(IrInst::Nop); // placeholder
            }

            JStarInstruction::Nop => {
                insts.push(IrInst::Nop);
            }
        }

        Ok((insts, dest))
    }

    fn lower_operand(&self, operand: &TypedOperand) -> MorphResult<IrValue> {
        match operand {
            TypedOperand::Immediate(val, _) => Ok(IrValue::Imm(*val)),
            TypedOperand::Variable { name, .. } => {
                // Resolve declared variables to their alloca vreg.
                // This enables direct stack-slot access in codegen.
                match self.variables.get(name) {
                    Some(&vreg) => Ok(IrValue::Reg(vreg)),
                    None => Ok(IrValue::Named(name.clone())),
                }
            }
            TypedOperand::Register(_, _) => {
                // "it" / "that" = result of the last Execute
                match self.last_result {
                    Some(vreg) => Ok(IrValue::Reg(vreg)),
                    None => Ok(IrValue::Imm(0)), // no prior result
                }
            }
            TypedOperand::StringLiteral(_) => {
                // String literals are handled at the instruction level (PrintStr),
                // not as operand values. If one reaches here, treat as 0.
                Ok(IrValue::Imm(0))
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
                size: None,
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

    #[test]
    fn test_lower_variable_store_and_return() {
        // Declare counter, store 42, return counter.
        // Variable operands should resolve to Reg(alloca_vreg), not Named.
        let prog = TypedProgram {
            statements: vec![
                TypedStatement::Declare {
                    scope: ScopeKind::Local,
                    name: "counter".to_string(),
                    ty: JStarType::Int,
                    size: None,
                },
                TypedStatement::Execute {
                    op: JStarInstruction::Store,
                    operands: vec![
                        TypedOperand::Immediate(42, JStarType::Byte),
                        TypedOperand::Variable {
                            name: "counter".to_string(),
                            scope: ScopeKind::Local,
                            ty: JStarType::Int,
                        },
                    ],
                    result_type: JStarType::Void,
                },
                TypedStatement::Return {
                    value: Some(TypedOperand::Variable {
                        name: "counter".to_string(),
                        scope: ScopeKind::Local,
                        ty: JStarType::Int,
                    }),
                    ty: JStarType::Int,
                },
            ],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];

        // First instruction: Alloca for counter
        match &block.instructions[0] {
            IrInst::Alloca { dest: 0, size: 4, .. } => {}
            other => panic!("Expected Alloca(v0, 4), got {:?}", other),
        }

        // Second instruction: Store { addr: Reg(0), value: Imm(42) }
        // The variable "counter" resolves to Reg(0) — the alloca vreg.
        match &block.instructions[1] {
            IrInst::Store {
                addr: IrValue::Reg(0),
                value: IrValue::Imm(42),
                ..
            } => {}
            other => panic!("Expected Store(Reg(0), Imm(42)), got {:?}", other),
        }

        // Terminator: Return(Reg(0)) — counter's alloca vreg
        match &block.terminator {
            Terminator::Return(Some(IrValue::Reg(0))) => {}
            other => panic!("Expected Return(Reg(0)), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_if_creates_3_blocks() {
        // "if compare X 0 ... end" should produce 3 basic blocks:
        //   entry: ...pre-if, Branch(cond, if_body, if_end)
        //   if_body: ...body, Jump(if_end)
        //   if_end: ...post-if, terminator
        let prog = TypedProgram {
            statements: vec![
                TypedStatement::ControlFlow {
                    kind: FlowKind::Conditional,
                    condition: Some(Box::new(TypedStatement::Execute {
                        op: JStarInstruction::Compare,
                        operands: vec![
                            TypedOperand::Immediate(1, JStarType::Int),
                            TypedOperand::Immediate(0, JStarType::Int),
                        ],
                        result_type: JStarType::Boolean,
                    })),
                    body: vec![TypedStatement::Nop],
                    else_body: vec![],
                },
            ],
        };
        let ir = lower(&prog).unwrap();
        let func = &ir.functions[0];
        assert_eq!(func.blocks.len(), 3, "if should produce 3 basic blocks");

        // Block 0 (entry): terminates with Branch
        match &func.blocks[0].terminator {
            Terminator::Branch { true_label, false_label, .. } => {
                assert!(true_label.starts_with("if_body"));
                assert!(false_label.starts_with("if_end"));
            }
            other => panic!("Expected Branch, got {:?}", other),
        }

        // Block 1 (if_body): terminates with Jump to if_end
        match &func.blocks[1].terminator {
            Terminator::Jump(label) => {
                assert!(label.starts_with("if_end"));
            }
            other => panic!("Expected Jump, got {:?}", other),
        }
    }

    #[test]
    fn test_lower_while_creates_4_blocks() {
        // "while compare X 0 ... end" should produce 4 basic blocks:
        //   entry: Jump(while_cond)
        //   while_cond: Compare, Branch(cond, while_body, while_end)
        //   while_body: ...body, Jump(while_cond)  <-- back-edge
        //   while_end: terminator
        let prog = TypedProgram {
            statements: vec![
                TypedStatement::ControlFlow {
                    kind: FlowKind::Loop,
                    condition: Some(Box::new(TypedStatement::Execute {
                        op: JStarInstruction::Compare,
                        operands: vec![
                            TypedOperand::Immediate(5, JStarType::Int),
                            TypedOperand::Immediate(0, JStarType::Int),
                        ],
                        result_type: JStarType::Boolean,
                    })),
                    body: vec![TypedStatement::Nop],
                    else_body: vec![],
                },
            ],
        };
        let ir = lower(&prog).unwrap();
        let func = &ir.functions[0];
        assert_eq!(func.blocks.len(), 4, "while should produce 4 basic blocks");

        // Block 0 (entry): Jump to while_cond
        match &func.blocks[0].terminator {
            Terminator::Jump(label) => {
                assert!(label.starts_with("while_cond"));
            }
            other => panic!("Expected Jump, got {:?}", other),
        }

        // Block 1 (while_cond): Branch to while_body or while_end
        match &func.blocks[1].terminator {
            Terminator::Branch { true_label, false_label, .. } => {
                assert!(true_label.starts_with("while_body"));
                assert!(false_label.starts_with("while_end"));
            }
            other => panic!("Expected Branch, got {:?}", other),
        }

        // Block 2 (while_body): Jump back to while_cond (back-edge)
        match &func.blocks[2].terminator {
            Terminator::Jump(label) => {
                assert!(label.starts_with("while_cond"));
            }
            other => panic!("Expected Jump (back-edge), got {:?}", other),
        }
    }

    #[test]
    fn test_compare_uses_cmpkind_ne() {
        // JStarInstruction::Compare should lower to CmpKind::Ne
        let prog = TypedProgram {
            statements: vec![TypedStatement::Execute {
                op: JStarInstruction::Compare,
                operands: vec![
                    TypedOperand::Immediate(1, JStarType::Int),
                    TypedOperand::Immediate(0, JStarType::Int),
                ],
                result_type: JStarType::Boolean,
            }],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.instructions[0] {
            IrInst::Compare { kind: CmpKind::Ne, .. } => {}
            other => panic!("Expected Compare(Ne), got {:?}", other),
        }
    }

    #[test]
    fn test_lower_undeclared_variable_uses_named() {
        // An undeclared variable should fall back to IrValue::Named
        let prog = TypedProgram {
            statements: vec![TypedStatement::Return {
                value: Some(TypedOperand::Variable {
                    name: "unknown".to_string(),
                    scope: ScopeKind::Local,
                    ty: JStarType::Int,
                }),
                ty: JStarType::Int,
            }],
        };
        let ir = lower(&prog).unwrap();
        let block = &ir.functions[0].blocks[0];
        match &block.terminator {
            Terminator::Return(Some(IrValue::Named(name))) if name == "unknown" => {}
            other => panic!("Expected Return(Named(unknown)), got {:?}", other),
        }
    }
}
