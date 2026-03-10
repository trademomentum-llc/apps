//! JStar Grammar & AST — Phase 1 of the JStar compiler.
//!
//! Defines the JStar abstract syntax tree as algebraic data types.
//! The grammar is POS-driven structured English:
//!
//!   program    := statement*
//!   statement  := verb_phrase (noun_phrase)? (prep_phrase)* | control_flow | declaration
//!   verb_phrase := exec_modifier? operation
//!   noun_phrase := scope? type_modifier* data_ref
//!   prep_phrase := addressing noun_phrase
//!   control_flow := flow_kind statement+ "end"
//!   declaration := scope type_modifier* data_ref
//!
//! Example: "store the unsigned integer into buffer"
//!   → Statement::Execute { op: Store, operand: integer(u32, global), dest: buffer }

use super::token_map::*;

// ─── JStar AST Node Types ───────────────────────────────────────────────────

/// A JStar program — a sequence of top-level statements.
#[derive(Debug, Clone, PartialEq)]
pub struct JStarProgram {
    pub statements: Vec<JStarStatement>,
}

/// A statement in the JStar language.
#[derive(Debug, Clone, PartialEq)]
pub enum JStarStatement {
    /// An operation: verb + optional operands
    /// e.g., "add the integer to counter"
    Execute {
        op: JStarInstruction,
        operands: Vec<JStarOperand>,
    },

    /// Variable/data declaration
    /// e.g., "a mutable integer counter"
    /// e.g., "a buffer 256" (array of 256 bytes)
    Declare {
        scope: ScopeKind,
        name: String,
        ty: JStarType,
        size: Option<usize>,
    },

    /// Control flow block
    /// e.g., "if compare counter 0 ... end"
    /// condition: the first statement (e.g. Compare) that produces a truth value.
    /// body: the statements inside the block.
    ControlFlow {
        kind: FlowKind,
        condition: Option<Box<JStarStatement>>,
        body: Vec<JStarStatement>,
        else_body: Vec<JStarStatement>,
    },

    /// Return a value
    /// e.g., "return the result"
    Return {
        value: Option<JStarOperand>,
    },

    /// Label for jump targets
    Label(String),

    /// Function definition
    /// e.g., "define greet ... end"
    FunctionDef {
        name: String,
        params: Vec<(String, JStarType)>,
        body: Vec<JStarStatement>,
        return_type: JStarType,
    },

    /// No-op (unrecognized or ignored tokens)
    Nop,
}

/// An operand in a JStar instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum JStarOperand {
    /// A named variable reference
    /// e.g., "the counter", "a buffer"
    Variable {
        name: String,
        scope: ScopeKind,
        modifiers: Vec<TypeMod>,
    },

    /// An immediate integer literal
    /// e.g., "42"
    Immediate(i64),

    /// A register alias
    /// e.g., "it" (accumulator), "that" (last result)
    Register(RegAlias),

    /// A string literal
    /// e.g., "hello world"
    StringLiteral(String),

    /// An addressed operand (preposition + operand)
    /// e.g., "into buffer", "from counter"
    Addressed {
        mode: AddrMode,
        target: Box<JStarOperand>,
    },
}

/// The JStar type system — mapped to Java's 8 primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JStarType {
    /// boolean → i8 (0 or 1)
    Boolean,
    /// byte → i8
    Byte,
    /// short → i16
    Short,
    /// int → i32 (default)
    Int,
    /// long → i64
    Long,
    /// float → f32
    Float,
    /// double → f64
    Double,
    /// char → u16 (UTF-16 code unit)
    Char,
    /// Void (no value — for return type of procedures)
    Void,
}

impl JStarType {
    /// Size in bytes on the target platform.
    pub fn size_bytes(self) -> usize {
        match self {
            JStarType::Boolean => 1,
            JStarType::Byte => 1,
            JStarType::Short => 2,
            JStarType::Int => 4,
            JStarType::Long => 8,
            JStarType::Float => 4,
            JStarType::Double => 8,
            JStarType::Char => 2,
            JStarType::Void => 0,
        }
    }

    /// Infer a JStar type from a noun lemma.
    pub fn from_noun(lemma: &str) -> Self {
        match lemma {
            "boolean" | "bool" | "flag" => JStarType::Boolean,
            "byte" => JStarType::Byte,
            "short" => JStarType::Short,
            "integer" | "int" | "number" | "count" | "counter" | "result" | "value" => {
                JStarType::Int
            }
            "long" => JStarType::Long,
            "float" => JStarType::Float,
            "double" => JStarType::Double,
            "char" | "character" | "letter" => JStarType::Char,
            _ => JStarType::Int, // default to int
        }
    }

    /// Apply a type modifier to refine the type.
    pub fn apply_modifier(self, modifier: TypeMod) -> Self {
        match modifier {
            TypeMod::Long => match self {
                JStarType::Int => JStarType::Long,
                JStarType::Float => JStarType::Double,
                other => other,
            },
            TypeMod::Short => match self {
                JStarType::Int => JStarType::Short,
                other => other,
            },
            TypeMod::Unsigned => self, // tracked in codegen, not type change
            _ => self,
        }
    }
}

// ─── Typed AST (post type-check) ────────────────────────────────────────────

/// A type-checked JStar program.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    pub statements: Vec<TypedStatement>,
}

/// A type-checked statement.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedStatement {
    Execute {
        op: JStarInstruction,
        operands: Vec<TypedOperand>,
        result_type: JStarType,
    },
    Declare {
        scope: ScopeKind,
        name: String,
        ty: JStarType,
        size: Option<usize>,
    },
    ControlFlow {
        kind: FlowKind,
        condition: Option<Box<TypedStatement>>,
        body: Vec<TypedStatement>,
        else_body: Vec<TypedStatement>,
    },
    Return {
        value: Option<TypedOperand>,
        ty: JStarType,
    },
    FunctionDef {
        name: String,
        params: Vec<(String, JStarType)>,
        body: Vec<TypedStatement>,
        return_type: JStarType,
    },
    Label(String),
    Nop,
}

/// A type-annotated operand.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedOperand {
    Variable {
        name: String,
        scope: ScopeKind,
        ty: JStarType,
    },
    Immediate(i64, JStarType),
    Register(RegAlias, JStarType),
    StringLiteral(String),
    Addressed {
        mode: AddrMode,
        target: Box<TypedOperand>,
        ty: JStarType,
    },
}

impl TypedOperand {
    /// Get the type of this operand.
    pub fn ty(&self) -> JStarType {
        match self {
            TypedOperand::Variable { ty, .. } => *ty,
            TypedOperand::Immediate(_, ty) => *ty,
            TypedOperand::Register(_, ty) => *ty,
            TypedOperand::StringLiteral(_) => JStarType::Long, // pointer-sized
            TypedOperand::Addressed { ty, .. } => *ty,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_sizes() {
        assert_eq!(JStarType::Boolean.size_bytes(), 1);
        assert_eq!(JStarType::Byte.size_bytes(), 1);
        assert_eq!(JStarType::Short.size_bytes(), 2);
        assert_eq!(JStarType::Int.size_bytes(), 4);
        assert_eq!(JStarType::Long.size_bytes(), 8);
        assert_eq!(JStarType::Float.size_bytes(), 4);
        assert_eq!(JStarType::Double.size_bytes(), 8);
        assert_eq!(JStarType::Char.size_bytes(), 2);
        assert_eq!(JStarType::Void.size_bytes(), 0);
    }

    #[test]
    fn test_type_from_noun() {
        assert_eq!(JStarType::from_noun("integer"), JStarType::Int);
        assert_eq!(JStarType::from_noun("boolean"), JStarType::Boolean);
        assert_eq!(JStarType::from_noun("character"), JStarType::Char);
        assert_eq!(JStarType::from_noun("unknown_thing"), JStarType::Int);
    }

    #[test]
    fn test_type_modifier_long() {
        let ty = JStarType::Int.apply_modifier(TypeMod::Long);
        assert_eq!(ty, JStarType::Long);
    }

    #[test]
    fn test_type_modifier_short() {
        let ty = JStarType::Int.apply_modifier(TypeMod::Short);
        assert_eq!(ty, JStarType::Short);
    }

    #[test]
    fn test_program_construction() {
        let prog = JStarProgram {
            statements: vec![
                JStarStatement::Execute {
                    op: JStarInstruction::Return,
                    operands: vec![JStarOperand::Immediate(42)],
                },
            ],
        };
        assert_eq!(prog.statements.len(), 1);
    }
}
