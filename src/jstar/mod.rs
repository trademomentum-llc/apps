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
    // Filter out comment lines (starting with #) before tokenization
    let filtered: String = input
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let all_tokens = crate::lexer::lex(&filtered)?;

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

    // ── Control flow end-to-end tests ───────────────────────────────────

    /// Helper: compile JStar source to a temp binary and run it, return exit code.
    /// Uses a hash of the source to generate a unique binary name per test.
    #[cfg(target_os = "linux")]
    fn compile_and_run(source: &str) -> i32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let hash = hasher.finish();

        let dir = std::env::temp_dir().join("jstar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(format!("test_{:016x}", hash));
        compile_source(source, &binary).unwrap();

        let output = std::process::Command::new(&binary)
            .output()
            .expect("Failed to run compiled binary");
        output.status.code().unwrap_or(-1)
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_true() {
        // counter=1, compare counter 0 => 1!=0 => true => body runs => counter=42
        let exit = compile_and_run(
            "a counter\nstore 1 into counter\nif compare counter 0\nstore 42 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 42, "if-true should execute body, exit 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_false() {
        // counter=0, compare counter 0 => 0!=0 => false => body skipped => counter=0
        let exit = compile_and_run(
            "a counter\nstore 0 into counter\nif compare counter 0\nstore 42 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 0, "if-false should skip body, exit 0");
    }

    /// Helper: compile JStar source and capture stdout from execution.
    #[cfg(target_os = "linux")]
    fn compile_and_capture(source: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let hash = hasher.finish();

        let dir = std::env::temp_dir().join("jstar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(format!("test_cap_{:016x}", hash));
        compile_source(source, &binary).unwrap();

        let output = std::process::Command::new(&binary)
            .output()
            .expect("Failed to run compiled binary");
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_literal() {
        let stdout = compile_and_capture("print 42");
        assert_eq!(stdout.trim(), "42", "print 42 should output '42'");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_zero() {
        let stdout = compile_and_capture("print 0");
        assert_eq!(stdout.trim(), "0", "print 0 should output '0'");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_variable() {
        let stdout = compile_and_capture(
            "a counter\nstore 99 into counter\nprint counter"
        );
        assert_eq!(stdout.trim(), "99", "print variable should output '99'");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_arithmetic() {
        let stdout = compile_and_capture("add 3 5\nprint it");
        assert_eq!(stdout.trim(), "8", "add 3 5; print it should output '8'");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_while_countdown() {
        // counter=5, while counter!=0: counter = subtract counter 1
        // Loop runs 5 times, counter reaches 0
        let exit = compile_and_run(
            "a counter\nstore 5 into counter\nwhile compare counter 0\nsubtract counter 1\nstore it into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 0, "while-countdown should exit 0");
    }

    // ── Multiple statements / sequence execution ────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_multi_statement_sequence() {
        // Declare, store, arithmetic, store result, return — 5 statements in sequence
        let exit = compile_and_run(
            "a counter\nstore 10 into counter\nadd counter 20\nstore it into counter\nreturn counter"
        );
        assert_eq!(exit, 30, "multi-statement sequence: 10 + 20 = 30");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_multiple_prints() {
        // Multiple print statements in sequence
        let stdout = compile_and_capture(
            "print 1\nprint 2\nprint 3"
        );
        assert_eq!(stdout, "1\n2\n3\n", "three sequential prints");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_declare_compute_print_return() {
        // Full pipeline: declare, store, compute, print, return
        let stdout = compile_and_capture(
            "a result\nstore 7 into result\nadd result 3\nstore it into result\nprint result\nreturn result"
        );
        assert_eq!(stdout.trim(), "10", "declare + compute + print");
    }

    // ── Codegen correctness: each arithmetic instruction ────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_multiply() {
        let exit = compile_and_run("multiply 6 7\nreturn it");
        assert_eq!(exit, 42, "6 * 7 = 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_divide() {
        let exit = compile_and_run("divide 84 2\nreturn it");
        assert_eq!(exit, 42, "84 / 2 = 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_subtract() {
        let exit = compile_and_run("subtract 50 8\nreturn it");
        assert_eq!(exit, 42, "50 - 8 = 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_chained_arithmetic() {
        // add 10 20 -> 30, subtract it 5 -> 25, multiply it 2 -> 50
        let exit = compile_and_run(
            "add 10 20\nsubtract it 5\nmultiply it 2\nreturn it"
        );
        assert_eq!(exit, 50, "chained arithmetic: (10+20-5)*2 = 50");
    }

    // ── Codegen correctness: variable load/store patterns ───────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_two_variables() {
        // Two separate variables
        let exit = compile_and_run(
            "a counter\na result\nstore 10 into counter\nstore 32 into result\nadd counter result\nreturn it"
        );
        assert_eq!(exit, 42, "two variables: 10 + 32 = 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_variable_overwrite() {
        // Store, overwrite, return
        let exit = compile_and_run(
            "a counter\nstore 99 into counter\nstore 42 into counter\nreturn counter"
        );
        assert_eq!(exit, 42, "variable overwrite: last store wins");
    }

    // ── Codegen correctness: control flow edge cases ────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_while_accumulate() {
        // while loop counts 3 to 0, accumulate counter values into result
        // result += counter each iteration: 3 + 2 + 1 = 6
        let exit = compile_and_run(
            "a counter\na result\nstore 3 into counter\nstore 0 into result\nwhile compare counter 0\nadd result counter\nstore it into result\nsubtract counter 1\nstore it into counter\nend\nreturn result"
        );
        assert_eq!(exit, 6, "while accumulate: 3+2+1 = 6");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_after_while() {
        // Sequence: while loop, then if, then return
        let exit = compile_and_run(
            "a counter\nstore 3 into counter\nwhile compare counter 0\nsubtract counter 1\nstore it into counter\nend\nif compare counter 0\nstore 99 into counter\nend\nreturn counter"
        );
        // After while: counter=0. compare 0 0 => 0!=0 => false. Body skipped. Return 0.
        assert_eq!(exit, 0, "if-after-while: condition false, skip body");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_in_loop() {
        // Print inside a while loop
        let stdout = compile_and_capture(
            "a counter\nstore 3 into counter\nwhile compare counter 0\nprint counter\nsubtract counter 1\nstore it into counter\nend"
        );
        assert_eq!(stdout, "3\n2\n1\n", "print inside loop: 3, 2, 1");
    }

    // ── Codegen correctness: print large numbers ────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_large_number() {
        let stdout = compile_and_capture("print 12345");
        assert_eq!(stdout.trim(), "12345", "print 5-digit number");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_multiply_result() {
        let stdout = compile_and_capture("multiply 111 111\nprint it");
        assert_eq!(stdout.trim(), "12321", "111 * 111 = 12321");
    }

    // ── v0.3.0: Else branches ───────────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_else_true() {
        // Condition true → body runs, else skipped
        let exit = compile_and_run(
            "a counter\nstore 1 into counter\nif compare counter 0\nstore 42 into counter\nelse\nstore 99 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 42, "if-else-true: body runs, exit 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_else_false() {
        // Condition false → else body runs
        let exit = compile_and_run(
            "a counter\nstore 0 into counter\nif compare counter 0\nstore 42 into counter\nelse\nstore 99 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 99, "if-else-false: else body runs, exit 99");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_nested_if_else() {
        // Nested: outer false → else → inner true → body
        let exit = compile_and_run(
            "a counter\nstore 0 into counter\na result\nstore 0 into result\nif compare counter 0\nstore 1 into result\nelse\nstore 1 into counter\nif compare counter 0\nstore 42 into result\nelse\nstore 99 into result\nend\nend\nreturn result"
        );
        assert_eq!(exit, 42, "nested if-else: outer-else inner-true");
    }

    // ── v0.3.0: Nested control flow ─────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_inside_while() {
        // While loop with if inside: count down from 3, add only odd values to result
        // counter=3 (odd, add 3), counter=2 (even, skip), counter=1 (odd, add 1) → result=4
        let exit = compile_and_run(
            "a counter\na result\na remainder\nstore 3 into counter\nstore 0 into result\nwhile compare counter 0\nstore counter into remainder\nsubtract remainder 2\nif compare remainder 0\nadd result counter\nstore it into result\nend\nsubtract counter 1\nstore it into counter\nend\nreturn result"
        );
        // counter=3: remainder=3-2=1, 1!=0 → true → result=0+3=3
        // counter=2: remainder=2-2=0, 0!=0 → false → skip
        // counter=1: remainder=1-2=-1, -1!=0 → true → result=3+1=4
        assert_eq!(exit, 4, "if inside while: sum odd-offset values");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_inside_if() {
        // Nested if blocks
        let exit = compile_and_run(
            "a counter\nstore 5 into counter\nif compare counter 0\nif compare counter 3\nstore 42 into counter\nend\nend\nreturn counter"
        );
        // counter=5, 5!=0 → true → inner: 5!=3 → true → counter=42
        assert_eq!(exit, 42, "if inside if: both true");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_while_inside_while() {
        // Outer loop runs 2 times, inner loop runs 3 times each → result = 2*3 = 6
        let exit = compile_and_run(
            "a outer\na inner\na result\nstore 2 into outer\nstore 0 into result\nwhile compare outer 0\nstore 3 into inner\nwhile compare inner 0\nadd result 1\nstore it into result\nsubtract inner 1\nstore it into inner\nend\nsubtract outer 1\nstore it into outer\nend\nreturn result"
        );
        assert_eq!(exit, 6, "while inside while: 2 * 3 = 6");
    }

    // ── v0.3.0: Comparison operators ────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_equal_true() {
        let exit = compile_and_run(
            "a counter\nstore 5 into counter\nif equal counter 5\nstore 42 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 42, "equal 5 5 → true → body runs");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_equal_false() {
        let exit = compile_and_run(
            "a counter\nstore 5 into counter\nif equal counter 3\nstore 42 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 5, "equal 5 3 → false → body skipped");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_less_true() {
        let exit = compile_and_run(
            "a counter\nstore 3 into counter\nif less counter 5\nstore 42 into counter\nend\nreturn counter"
        );
        assert_eq!(exit, 42, "less 3 5 → true → body runs");
    }

    // ── v0.3.0: Boolean literals ────────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_boolean_true() {
        let exit = compile_and_run(
            "a value\nstore true into value\nreturn value"
        );
        assert_eq!(exit, 1, "true literal = 1");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_boolean_false() {
        let exit = compile_and_run(
            "a value\nstore false into value\nreturn value"
        );
        assert_eq!(exit, 0, "false literal = 0");
    }

    // ── v0.3.0: Comments ────────────────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_comment_ignored() {
        let exit = compile_and_run(
            "# this is a comment\nreturn 42"
        );
        assert_eq!(exit, 42, "comment line ignored");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_comment_between_statements() {
        let exit = compile_and_run(
            "a counter\nstore 42 into counter\n# this should be ignored\nreturn counter"
        );
        assert_eq!(exit, 42, "comment between statements ignored");
    }
}
