//! Jasterish (JStar) — a system-level machine language built on morphlex token vectors.
//!
//! The compiler pipeline:
//!   .jstr source → tokenize_jstar → parse → typecheck → IR → codegen → link → ELF
//!
//! The core insight: morphlex POS/role/morph fields ARE the instruction encoding.
//! Verbs = operations, Nouns = data, Adjectives = modifiers, Prepositions = relations.
//!
//! tokenize_jstar() wraps the morphlex NLP pipeline but adds number/literal support.
//! Words go through morphlex (morphology → AST → semantics → vectorizer).
//! Numbers get synthetic TokenVectors with POS_LITERAL, interleaved in original order.

pub mod token_map;
pub mod grammar;
pub mod parser;
pub mod typechecker;
pub mod ir;
pub mod codegen;
pub mod linker;

use crate::types::*;
use std::path::Path;

// ─── JStar-Specific Tokenization ───────────────────────────────────────────
//
// The morphlex NLP pipeline drops numbers (they're not words). JStar needs
// them as literal operands. This tokenizer runs words through morphlex and
// synthesizes TokenVector entries for numbers, preserving original order.

/// Tokenize input for JStar: words through morphlex, numbers as literals.
///
/// Returns (lemmas, vectors) where number tokens appear in their original
/// position with POS_LITERAL and the numeric string as their lemma.
pub fn tokenize_jstar(input: &str) -> MorphResult<(Vec<String>, Vec<TokenVector>)> {
    let all_tokens = crate::lexer::lex(input)?;

    // Separate words from numbers, tracking original order
    enum Slot {
        Word(usize),
        Number(usize),
    }

    let mut order: Vec<Slot> = Vec::new();
    let mut word_tokens: Vec<Token> = Vec::new();
    let mut number_lexemes: Vec<String> = Vec::new();

    for token in &all_tokens {
        match token.kind {
            TokenKind::Word | TokenKind::Contraction | TokenKind::Hyphenated => {
                order.push(Slot::Word(word_tokens.len()));
                word_tokens.push(token.clone());
            }
            TokenKind::Number => {
                order.push(Slot::Number(number_lexemes.len()));
                number_lexemes.push(token.lexeme.clone());
            }
            _ => {} // skip whitespace, punctuation
        }
    }

    // Run word tokens through the full morphlex pipeline
    let morphs = crate::morphology::analyze(&word_tokens)?;
    let word_lemmas: Vec<String> = morphs.iter().map(|m| m.lemma.clone()).collect();

    // Only run the rest of the pipeline if there are words
    let word_vectors = if morphs.is_empty() {
        Vec::new()
    } else {
        let tree = crate::ast::build(&morphs)?;
        let semnodes = crate::semantics::annotate(&tree)?;
        crate::vectorizer::vectorize(&semnodes)?
    };

    // Interleave words and numbers in original order
    let mut lemmas = Vec::new();
    let mut vectors = Vec::new();

    for slot in &order {
        match slot {
            Slot::Word(i) => {
                if *i < word_lemmas.len() {
                    lemmas.push(word_lemmas[*i].clone());
                    vectors.push(word_vectors[*i]);
                }
            }
            Slot::Number(i) => {
                let raw = &number_lexemes[*i];
                let clean = raw.replace(',', "");
                lemmas.push(clean.clone());
                vectors.push(TokenVector {
                    id: crate::vectorizer::hash_to_i32(&raw.to_lowercase()),
                    lemma_id: crate::vectorizer::hash_to_i32(&clean),
                    pos: token_map::POS_LITERAL,
                    role: 0,
                    morph: 0,
                });
            }
        }
    }

    Ok((lemmas, vectors))
}

// ─── Compiler Pipeline ─────────────────────────────────────────────────────

/// Compile a .jstr source file to a native ELF binary.
pub fn compile_file(source_path: &Path, output_path: &Path) -> MorphResult<()> {
    let source = std::fs::read_to_string(source_path)
        .map_err(MorphlexError::IoError)?;
    compile_source(&source, output_path)
}

/// Compile JStar source text to a native ELF binary.
pub fn compile_source(source: &str, output_path: &Path) -> MorphResult<()> {
    // Phase 0: Tokenize (morphlex + number literals)
    let (lemmas, vectors) = tokenize_jstar(source)?;

    // Phase 1-2: Parse token stream into JStar AST
    let ast = parser::parse(&lemmas, &vectors)?;

    // Phase 3: Type check
    let typed_ast = typechecker::check(&ast)?;

    // Phase 4: Lower to IR
    let ir_program = ir::lower(&typed_ast)?;

    // Phase 5: Generate x86-64 machine code
    let machine_code = codegen::generate(&ir_program)?;

    // Phase 6: Link into ELF binary
    linker::link(&machine_code, output_path)?;

    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::token_map::*;

    #[test]
    fn test_tokenize_jstar_return_42() {
        let (lemmas, vectors) = tokenize_jstar("return 42").unwrap();
        assert_eq!(lemmas.len(), 2);
        assert_eq!(vectors.len(), 2);

        // "return" resolved by keyword table
        let cat0 = resolve(&vectors[0], &lemmas[0]);
        assert_eq!(cat0, TokenCategory::Operation(JStarInstruction::Return));

        // "42" is a literal
        let cat1 = resolve(&vectors[1], &lemmas[1]);
        assert_eq!(cat1, TokenCategory::Literal);
        assert_eq!(lemmas[1], "42");
    }

    #[test]
    fn test_tokenize_jstar_numbers_preserved() {
        let (lemmas, vectors) = tokenize_jstar("add 3 to 5").unwrap();
        // Should have 4 tokens: add, 3, to, 5
        assert_eq!(lemmas.len(), 4);
        assert_eq!(vectors.len(), 4);
        assert_eq!(lemmas[1], "3");
        assert_eq!(lemmas[3], "5");
    }

    #[test]
    fn test_tokenize_jstar_only_numbers() {
        let (lemmas, vectors) = tokenize_jstar("42").unwrap();
        assert_eq!(lemmas.len(), 1);
        assert_eq!(lemmas[0], "42");
        assert_eq!(vectors[0].pos, POS_LITERAL);
    }

    #[test]
    fn test_tokenize_jstar_empty() {
        let (lemmas, vectors) = tokenize_jstar("").unwrap();
        assert!(lemmas.is_empty());
        assert!(vectors.is_empty());
    }

    #[test]
    fn test_tokenize_jstar_determinism() {
        let a = tokenize_jstar("return 42").unwrap();
        let b = tokenize_jstar("return 42").unwrap();
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
    }
}
