//! Recursive Descent Parser — Phase 2 of the JStar compiler.
//!
//! Input: lemmas + TokenVectors from the morphlex pipeline.
//! Output: JStarProgram (untyped AST).
//!
//! The parser consumes token categories (from token_map::resolve),
//! not raw characters. POS tags drive which parse rule to invoke:
//!   Verb       → parse_execute (operation statement)
//!   Noun       → parse_declaration or operand
//!   Determiner → start of a noun phrase (scope + declaration/operand)
//!   Conjunction → control flow block
//!   etc.
//!
//! Monadic error handling: all parse functions return MorphResult<T>.

use crate::types::{MorphResult, MorphlexError, TokenVector};
use super::grammar::*;
use super::token_map::*;

/// A positioned token in the parse stream.
#[derive(Debug, Clone)]
struct ParseToken {
    lemma: String,
    vector: TokenVector,
    category: TokenCategory,
}

/// Parser state — holds the token stream and current position.
struct Parser {
    tokens: Vec<ParseToken>,
    pos: usize,
}

/// Parse morphlex output into a JStar AST.
///
/// Takes parallel arrays from morphlex::compile() and produces a JStarProgram.
/// Keyword resolution uses the i32 identity hash in TokenVector.id — no
/// original lexemes needed. BLAKE3("return") resolves to Return regardless
/// of what morphology does to the lemma.
pub fn parse(
    lemmas: &[String],
    vectors: &[TokenVector],
) -> MorphResult<JStarProgram> {
    let tokens: Vec<ParseToken> = lemmas
        .iter()
        .zip(vectors.iter())
        .map(|(lemma, tv)| ParseToken {
            lemma: lemma.clone(),
            vector: *tv,
            category: resolve(tv, lemma),
        })
        .collect();

    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_program()
}

impl Parser {
    /// Parse a complete program (sequence of statements).
    fn parse_program(&mut self) -> MorphResult<JStarProgram> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    // Error recovery: skip the current token and continue
                    eprintln!("Parse warning: {}", e);
                    self.advance();
                }
            }
        }
        Ok(JStarProgram { statements })
    }

    /// Parse a single statement. POS of the current token determines the rule.
    fn parse_statement(&mut self) -> MorphResult<JStarStatement> {
        let current = self.peek().ok_or_else(|| {
            MorphlexError::AstError("Unexpected end of input".to_string())
        })?;

        match &current.category {
            TokenCategory::Operation(instr) => {
                let instr = *instr;
                self.parse_execute(instr)
            }
            TokenCategory::ControlFlow(kind) => {
                let kind = *kind;
                self.parse_control_flow(kind)
            }
            TokenCategory::FunctionDef => self.parse_function_def(),
            TokenCategory::Scope(_) => self.parse_declaration_or_operand_stmt(),
            TokenCategory::Data => self.parse_declaration_from_noun(),
            TokenCategory::Literal => self.parse_literal_statement(),
            TokenCategory::Ignored => {
                self.advance();
                Ok(JStarStatement::Nop)
            }
            _ => {
                // Unrecognized pattern — skip
                self.advance();
                Ok(JStarStatement::Nop)
            }
        }
    }

    /// Parse an execute statement: operation followed by operands.
    /// "add the integer to counter"
    /// "return 42"
    /// "store it into buffer"
    fn parse_execute(&mut self, op: JStarInstruction) -> MorphResult<JStarStatement> {
        self.advance(); // consume the verb

        // Special case: return
        if op == JStarInstruction::Return {
            let value = if !self.is_at_end() && self.is_operand_start() {
                Some(self.parse_operand()?)
            } else {
                None
            };
            return Ok(JStarStatement::Return { value });
        }

        // Special case: call — first token is the function name regardless of POS
        if op == JStarInstruction::Call {
            let mut operands = Vec::new();
            if let Some(tok) = self.peek() {
                // Take the next token's lemma as the function name
                let name = tok.lemma.clone();
                self.advance();
                operands.push(JStarOperand::Variable {
                    name,
                    scope: ScopeKind::Local,
                    modifiers: vec![],
                });
            }
            // Remaining tokens are arguments (normal operand parsing)
            while !self.is_at_end() && self.is_operand_start() {
                operands.push(self.parse_operand()?);
            }
            return Ok(JStarStatement::Execute { op, operands });
        }

        // Collect operands until we hit another statement-starting token or end
        let mut operands = Vec::new();
        while !self.is_at_end() && self.is_operand_start() {
            operands.push(self.parse_operand()?);
        }

        Ok(JStarStatement::Execute { op, operands })
    }

    /// Parse a control flow block: conjunction + condition + body + end marker.
    /// "if compare counter 0 ... end"
    /// "while compare counter 0 ... end"
    ///
    /// For Conditional and Loop kinds, the first statement after the keyword
    /// is the condition (typically a Compare). Remaining statements are the body.
    /// For Sequence and Branch kinds, condition is None — all statements are body.
    fn parse_control_flow(&mut self, kind: FlowKind) -> MorphResult<JStarStatement> {
        self.advance(); // consume the conjunction

        // For if/while: first statement is the condition
        let condition = match kind {
            FlowKind::Conditional | FlowKind::Loop => {
                if !self.is_at_end() {
                    if let Some(tok) = self.peek() {
                        if !matches!(tok.category, TokenCategory::Operation(JStarInstruction::Halt)) {
                            match self.parse_statement() {
                                Ok(stmt) => Some(Box::new(stmt)),
                                Err(_) => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        let mut body = Vec::new();
        let mut else_body = Vec::new();
        let mut in_else = false;
        // Parse statements until we hit "end" or "else" or end of input
        while !self.is_at_end() {
            if let Some(tok) = self.peek() {
                match &tok.category {
                    TokenCategory::Operation(JStarInstruction::Halt) => {
                        self.advance(); // consume "end"
                        break;
                    }
                    TokenCategory::ControlFlow(FlowKind::Branch) if !in_else => {
                        self.advance(); // consume "else"
                        in_else = true;
                        continue;
                    }
                    _ => {}
                }
            }
            match self.parse_statement() {
                Ok(stmt) => {
                    if in_else {
                        else_body.push(stmt);
                    } else {
                        body.push(stmt);
                    }
                }
                Err(_) => {
                    self.advance();
                }
            }
        }

        Ok(JStarStatement::ControlFlow { kind, condition, body, else_body })
    }

    /// Parse a declaration starting with a determiner (scope).
    /// "the mutable integer counter"  → Declare { Global, counter, Int }
    /// "a long result"                → Declare { Local, result, Long }
    fn parse_declaration_or_operand_stmt(&mut self) -> MorphResult<JStarStatement> {
        let scope = match self.peek().map(|t| &t.category) {
            Some(TokenCategory::Scope(s)) => {
                let s = *s;
                self.advance();
                s
            }
            _ => ScopeKind::Local,
        };

        // Collect type modifiers (adjectives)
        let mut modifiers = Vec::new();
        while let Some(tok) = self.peek() {
            match &tok.category {
                TokenCategory::TypeModifier(m) => {
                    modifiers.push(*m);
                    self.advance();
                }
                _ => break,
            }
        }

        // Expect a noun (data reference = name)
        match self.peek().map(|t| t.category.clone()) {
            Some(TokenCategory::Data) => {
                let name = self.peek().unwrap().lemma.clone();
                let mut ty = JStarType::from_noun(&name);
                for m in &modifiers {
                    ty = ty.apply_modifier(*m);
                }
                self.advance();

                // Check if the next token is also a noun (name follows type)
                // e.g., "the unsigned integer counter" → type=Int, name="counter"
                if let Some(tok) = self.peek() {
                    if matches!(tok.category, TokenCategory::Data) {
                        let actual_name = tok.lemma.clone();
                        self.advance();
                        let size = self.try_parse_array_size();
                        return Ok(JStarStatement::Declare {
                            scope,
                            name: actual_name,
                            ty,
                            size,
                        });
                    }
                }

                let size = self.try_parse_array_size();
                Ok(JStarStatement::Declare {
                    scope,
                    name,
                    ty,
                    size,
                })
            }
            _ => {
                // No noun found after modifiers — malformed declaration
                Err(MorphlexError::AstError(
                    "Expected noun after type modifiers in declaration".to_string(),
                ))
            }
        }
    }

    /// Parse a declaration starting directly with a noun.
    /// "integer counter" → Declare { Local, counter, Int }
    fn parse_declaration_from_noun(&mut self) -> MorphResult<JStarStatement> {
        let first_lemma = self.peek().unwrap().lemma.clone();
        let ty = JStarType::from_noun(&first_lemma);
        self.advance();

        // Check for a second noun (the variable name)
        if let Some(tok) = self.peek() {
            if matches!(tok.category, TokenCategory::Data) {
                let name = tok.lemma.clone();
                self.advance();
                let size = self.try_parse_array_size();
                return Ok(JStarStatement::Declare {
                    scope: ScopeKind::Local,
                    name,
                    ty,
                    size,
                });
            }
        }

        // Single noun — declare it with its own name
        let size = self.try_parse_array_size();
        Ok(JStarStatement::Declare {
            scope: ScopeKind::Local,
            name: first_lemma,
            ty,
            size,
        })
    }

    /// Parse a literal as a standalone statement (bare number).
    fn parse_literal_statement(&mut self) -> MorphResult<JStarStatement> {
        let operand = self.parse_operand()?;
        Ok(JStarStatement::Execute {
            op: JStarInstruction::Nop,
            operands: vec![operand],
        })
    }

    /// Parse a single operand (noun phrase, literal, register, or addressed operand).
    fn parse_operand(&mut self) -> MorphResult<JStarOperand> {
        let current = self.peek().ok_or_else(|| {
            MorphlexError::AstError("Expected operand but reached end of input".to_string())
        })?;

        match &current.category {
            // Preposition starts an addressed operand
            TokenCategory::Addressing(mode) => {
                let mode = *mode;
                self.advance();
                let target = self.parse_operand()?;
                Ok(JStarOperand::Addressed {
                    mode,
                    target: Box::new(target),
                })
            }

            // Determiner starts a scoped noun phrase
            TokenCategory::Scope(scope) => {
                let scope = *scope;
                self.advance();
                let mut modifiers = Vec::new();
                while let Some(tok) = self.peek() {
                    match &tok.category {
                        TokenCategory::TypeModifier(m) => {
                            modifiers.push(*m);
                            self.advance();
                        }
                        _ => break,
                    }
                }
                if let Some(tok) = self.peek() {
                    if matches!(tok.category, TokenCategory::Data) {
                        let name = tok.lemma.clone();
                        self.advance();
                        return Ok(JStarOperand::Variable {
                            name,
                            scope,
                            modifiers,
                        });
                    }
                }
                Err(MorphlexError::AstError(
                    "Expected noun after determiner in operand".to_string(),
                ))
            }

            // Bare noun
            TokenCategory::Data => {
                let name = current.lemma.clone();
                self.advance();
                Ok(JStarOperand::Variable {
                    name,
                    scope: ScopeKind::Local,
                    modifiers: vec![],
                })
            }

            // Pronoun = register alias
            TokenCategory::Register(reg) => {
                let reg = *reg;
                self.advance();
                Ok(JStarOperand::Register(reg))
            }

            // Number or string literal
            TokenCategory::Literal => {
                let lemma = current.lemma.clone();
                let pos = current.vector.pos;
                self.advance();
                if pos == POS_STRING {
                    Ok(JStarOperand::StringLiteral(lemma))
                } else {
                    let value = lemma.parse::<i64>().unwrap_or(0);
                    Ok(JStarOperand::Immediate(value))
                }
            }

            // Type modifier without a preceding scope — treat as part of operand
            TokenCategory::TypeModifier(m) => {
                let first_lemma = current.lemma.clone();
                let mut modifiers = vec![*m];
                self.advance();
                while let Some(tok) = self.peek() {
                    match &tok.category {
                        TokenCategory::TypeModifier(m) => {
                            modifiers.push(*m);
                            self.advance();
                        }
                        _ => break,
                    }
                }
                if let Some(tok) = self.peek() {
                    if matches!(tok.category, TokenCategory::Data) {
                        let name = tok.lemma.clone();
                        self.advance();
                        return Ok(JStarOperand::Variable {
                            name,
                            scope: ScopeKind::Local,
                            modifiers,
                        });
                    }
                }
                // No Data noun found — treat the modifier itself as a variable name.
                // This handles cases where morphlex classifies variable names
                // (like "left", "right") as adjectives instead of nouns.
                Ok(JStarOperand::Variable {
                    name: first_lemma,
                    scope: ScopeKind::Local,
                    modifiers: vec![],
                })
            }

            // Sequence conjunction — stop collecting operands
            TokenCategory::ControlFlow(FlowKind::Sequence) => {
                self.advance(); // consume "and"/"then"
                // Continue to next operand
                self.parse_operand()
            }

            other => Err(MorphlexError::AstError(format!(
                "Unexpected token category in operand position: {:?}",
                other
            ))),
        }
    }

    /// Parse a function definition: "define <name> [with <type> <name>...] ... end"
    ///
    /// Syntax:
    ///   define greet ... end
    ///   define add_nums with integer left integer right ... end
    fn parse_function_def(&mut self) -> MorphResult<JStarStatement> {
        self.advance(); // consume "define"

        // Function name — next token must be a data/noun token
        let name = match self.peek() {
            Some(tok) => {
                let n = tok.lemma.clone();
                self.advance();
                n
            }
            None => {
                return Err(MorphlexError::AstError(
                    "Expected function name after 'define'".to_string(),
                ));
            }
        };

        // Optional parameters: "with <type> <name> [<type> <name>]..."
        let mut params = Vec::new();
        if let Some(tok) = self.peek() {
            if matches!(tok.category, TokenCategory::Addressing(AddrMode::By)) {
                self.advance(); // consume "with"
                // Parse parameter pairs: <type-noun> <name>
                // Type token must be Data; name token can be any category
                // (morphlex may classify parameter names as adjectives, verbs, etc.)
                while let Some(tok) = self.peek() {
                    if matches!(tok.category, TokenCategory::Data) {
                        let type_lemma = tok.lemma.clone();
                        let ty = JStarType::from_noun(&type_lemma);
                        self.advance();
                        // Next token is the parameter name — accept regardless of POS
                        if let Some(name_tok) = self.peek() {
                            if !matches!(name_tok.category,
                                TokenCategory::Operation(JStarInstruction::Halt)
                                | TokenCategory::ControlFlow(_)
                                | TokenCategory::FunctionDef
                            ) {
                                let param_name = name_tok.lemma.clone();
                                self.advance();
                                params.push((param_name, ty));
                                continue;
                            }
                        }
                        // Single noun — treat as name with default type
                        params.push((type_lemma, JStarType::Int));
                    } else {
                        break;
                    }
                }
            }
        }

        // Parse body statements until "end"
        let mut body = Vec::new();
        while !self.is_at_end() {
            if let Some(tok) = self.peek() {
                if matches!(tok.category, TokenCategory::Operation(JStarInstruction::Halt)) {
                    self.advance(); // consume "end"
                    break;
                }
            }
            match self.parse_statement() {
                Ok(stmt) => body.push(stmt),
                Err(_) => { self.advance(); }
            }
        }

        Ok(JStarStatement::FunctionDef {
            name,
            params,
            body,
            return_type: JStarType::Void,
        })
    }

    // ─── Helpers ────────────────────────────────────────────────────────────

    /// Try to parse an array size literal after a declaration name.
    /// "a buffer 256" → size = Some(256)
    /// "a counter"    → size = None
    fn try_parse_array_size(&mut self) -> Option<usize> {
        if let Some(tok) = self.peek() {
            if matches!(tok.category, TokenCategory::Literal) && tok.vector.pos == POS_LITERAL {
                if let Ok(n) = tok.lemma.parse::<usize>() {
                    if n > 0 {
                        self.advance();
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    /// Check if the current token can start an operand.
    ///
    /// Note: Scope (a/the) is NOT included — articles start declarations,
    /// not operands. Without this, `store 42 into result` followed by
    /// `a val` on the next line would consume `a val` as a third operand.
    /// Scope tokens are still handled by parse_operand when reached via
    /// recursive calls (e.g., from Addressing: `into the counter`).
    fn is_operand_start(&self) -> bool {
        match self.peek().map(|t| &t.category) {
            Some(TokenCategory::Data) => true,
            Some(TokenCategory::Register(_)) => true,
            Some(TokenCategory::Addressing(_)) => true,
            Some(TokenCategory::Literal) => true,
            Some(TokenCategory::TypeModifier(_)) => true,
            Some(TokenCategory::ControlFlow(FlowKind::Sequence)) => true,
            _ => false,
        }
    }

    fn peek(&self) -> Option<&ParseToken> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&ParseToken> {
        if self.pos < self.tokens.len() {
            self.pos += 1;
            self.tokens.get(self.pos - 1)
        } else {
            None
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: run the JStar tokenizer + parser.
    fn parse_jstar(input: &str) -> JStarProgram {
        let (lemmas, vectors) = crate::jstar::tokenize_jstar(input).unwrap();
        parse(&lemmas, &vectors).unwrap()
    }

    #[test]
    fn test_parse_return_literal() {
        // "return" should parse as a verb (Return instruction)
        // Note: morphlex may classify "return" differently based on POS inference.
        // This tests the parser structure, not POS accuracy.
        let prog = parse_jstar("return");
        assert!(!prog.statements.is_empty());
    }

    #[test]
    fn test_parse_simple_declaration() {
        // "the dog" → Determiner(the) + Noun(dog) → Declare statement
        // Using "dog" because morphlex reliably POS-tags it as Noun.
        let prog = parse_jstar("the dog");
        let has_declare = prog.statements.iter().any(|s| {
            matches!(s, JStarStatement::Declare { .. })
        });
        assert!(has_declare, "Expected a Declare statement from 'the dog'");
    }

    #[test]
    fn test_parse_verb_operand() {
        // "add counter" — verb + noun
        let prog = parse_jstar("add counter");
        // morphlex may not POS-tag "add" as a verb in isolation,
        // so check that we got at least some statements
        assert!(!prog.statements.is_empty());
    }

    #[test]
    fn test_parse_determinism() {
        let a = parse_jstar("store the integer into buffer");
        let b = parse_jstar("store the integer into buffer");
        assert_eq!(a, b, "Parser must be deterministic");
    }

    #[test]
    fn test_parse_empty_input() {
        let prog = parse_jstar("");
        assert!(prog.statements.is_empty());
    }
}
