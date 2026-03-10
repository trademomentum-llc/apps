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

/// Tokenize input for JStar: words through morphlex, numbers and strings as literals.
///
/// Returns (lemmas, vectors) where number/string tokens appear in their original
/// position with POS_LITERAL/POS_STRING and the literal value as their lemma.
///
/// String literals (text between double quotes) are extracted before morphlex
/// processing so they aren't decomposed into individual words.
pub fn tokenize_jstar(input: &str) -> MorphResult<(Vec<String>, Vec<TokenVector>)> {
    // Phase 0: Extract string literals and split input into segments.
    // Each segment between strings gets processed by morphlex separately.
    // Strings are interleaved at their original positions.
    struct Segment {
        kind: SegKind,
        text: String,
    }
    enum SegKind { Code, Str }

    let mut segments: Vec<Segment> = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            // Flush current code segment
            if !current.is_empty() {
                segments.push(Segment { kind: SegKind::Code, text: std::mem::take(&mut current) });
            }
            // Extract string literal
            let mut s = String::new();
            for c in chars.by_ref() {
                if c == '"' { break; }
                s.push(c);
            }
            segments.push(Segment { kind: SegKind::Str, text: s });
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        segments.push(Segment { kind: SegKind::Code, text: current });
    }

    // Process each segment
    let mut lemmas = Vec::new();
    let mut vectors = Vec::new();

    for seg in &segments {
        match seg.kind {
            SegKind::Str => {
                lemmas.push(seg.text.clone());
                vectors.push(TokenVector {
                    id: crate::vectorizer::hash_to_i32(&seg.text),
                    lemma_id: crate::vectorizer::hash_to_i32(&seg.text),
                    pos: token_map::POS_STRING,
                    role: 0,
                    morph: 0,
                });
            }
            SegKind::Code => {
                let all_tokens = crate::lexer::lex(&seg.text)?;

                let mut word_tokens: Vec<Token> = Vec::new();
                let mut number_lexemes: Vec<String> = Vec::new();

                enum Slot { Word(usize), Number(usize) }
                let mut order: Vec<Slot> = Vec::new();

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
                        _ => {}
                    }
                }

                // Run word tokens through the full morphlex pipeline
                let morphs = crate::morphology::analyze(&word_tokens)?;
                let word_lemmas: Vec<String> = morphs.iter().map(|m| m.lemma.clone()).collect();

                let word_vectors = if morphs.is_empty() {
                    Vec::new()
                } else {
                    let tree = crate::ast::build(&morphs)?;
                    let semnodes = crate::semantics::annotate(&tree)?;
                    crate::vectorizer::vectorize(&semnodes)?
                };

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
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Monotonic counter to guarantee unique binary names across parallel tests.
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    /// Uses a hash of source + thread ID to generate a unique binary name per test.
    #[cfg(target_os = "linux")]
    fn compile_and_run(source: &str) -> i32 {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("jstar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(format!("t_{}", n));
        compile_source(source, &binary).unwrap();
        let output = std::process::Command::new(&binary)
            .output()
            .expect("Failed to run compiled binary");
        let _ = std::fs::remove_file(&binary);
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
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("jstar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(format!("tc_{}", n));
        compile_source(source, &binary).unwrap();
        let output = std::process::Command::new(&binary)
            .output()
            .expect("Failed to run compiled binary");
        let _ = std::fs::remove_file(&binary);
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

    // ── String literal tests ────────────────────────────────────────────

    #[test]
    fn test_tokenize_jstar_string_literal() {
        let (lemmas, vectors) = tokenize_jstar("print \"hello world\"").unwrap();
        assert_eq!(lemmas.len(), 2);
        assert_eq!(lemmas[1], "hello world");
        assert_eq!(vectors[1].pos, token_map::POS_STRING);
    }

    #[test]
    fn test_tokenize_jstar_mixed_strings_and_numbers() {
        let (lemmas, vectors) = tokenize_jstar("print \"hi\" print 42").unwrap();
        assert_eq!(lemmas.len(), 4);
        assert_eq!(lemmas[1], "hi");
        assert_eq!(vectors[1].pos, token_map::POS_STRING);
        assert_eq!(lemmas[3], "42");
        assert_eq!(vectors[3].pos, token_map::POS_LITERAL);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_debug_print_string_binary() {
        let source = "print \"hello\"";
        let (lemmas, vectors) = tokenize_jstar(source).unwrap();
        let ast = parser::parse(&lemmas, &vectors).unwrap();
        let typed = typechecker::check(&ast).unwrap();
        let ir_prog = ir::lower(&typed).unwrap();
        let mc = codegen::generate(&ir_prog).unwrap();

        eprintln!("string_data: {:?}", String::from_utf8_lossy(&ir_prog.string_data));
        eprintln!("mc.data: {:?}", String::from_utf8_lossy(&mc.data));
        eprintln!("mc.text len: {}", mc.text.len());

        // Check for 0x48 0xBE pattern in text
        for i in 0..mc.text.len().saturating_sub(10) {
            if mc.text[i] == 0x48 && mc.text[i+1] == 0xBE {
                let val = u64::from_le_bytes(mc.text[i+2..i+10].try_into().unwrap());
                eprintln!("mov rsi, imm64 at offset {}: value={:#x}", i, val);
            }
        }

        // Compile to binary and inspect
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);
        let dir = std::env::temp_dir().join("jstar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(format!("test_dbg_{:016x}", hasher.finish()));
        let _ = std::fs::remove_file(&binary);
        compile_source(source, &binary).unwrap();

        let output = std::process::Command::new(&binary).output().unwrap();
        eprintln!("exit: {:?}, stdout: {:?}, stderr: {:?}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr));
        assert!(!output.stdout.is_empty(), "Should produce stdout output");
    }

    #[test]
    fn test_print_string_ir() {
        let (lemmas, vectors) = tokenize_jstar("print \"hello\"").unwrap();
        assert_eq!(lemmas.len(), 2);
        let ast = parser::parse(&lemmas, &vectors).unwrap();
        let typed = typechecker::check(&ast).unwrap();
        let ir = ir::lower(&typed).unwrap();
        assert!(!ir.string_data.is_empty(), "string_data should contain hello + newline");
        assert_eq!(&ir.string_data, b"hello\n");
        // Check that PrintStr instruction exists
        let has_print_str = ir.functions[0].blocks.iter().any(|b| {
            b.instructions.iter().any(|i| matches!(i, ir::IrInst::PrintStr { .. }))
        });
        assert!(has_print_str, "Should have PrintStr instruction");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_string() {
        let stdout = compile_and_capture("print \"hello\"");
        assert_eq!(stdout, "hello\n", "print string literal");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_print_string_and_number() {
        let stdout = compile_and_capture("print \"answer:\" print 42");
        assert_eq!(stdout, "answer:\n42\n", "print string then number");
    }

    // ── Function definition and call tests ──────────────────────────────

    #[test]
    fn test_parse_function_def() {
        let prog = crate::jstar::parser::parse(
            &["define", "greet", "print", "42", "end"]
                .iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &["define", "greet", "print", "42", "end"]
                .iter().map(|w| {
                    crate::types::TokenVector {
                        id: crate::vectorizer::hash_to_i32(w),
                        lemma_id: crate::vectorizer::hash_to_i32(w),
                        pos: if *w == "42" { token_map::POS_LITERAL } else { 0 },
                        role: 0,
                        morph: 0,
                    }
                }).collect::<Vec<_>>(),
        ).unwrap();
        let has_func_def = prog.statements.iter().any(|s| {
            matches!(s, grammar::JStarStatement::FunctionDef { .. })
        });
        assert!(has_func_def, "Should parse function definition");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_function_call_return() {
        // Define a function that returns 42, call it, return the result
        let exit = compile_and_run(
            "define answer\nreturn 42\nend\ncall answer\nreturn it"
        );
        assert_eq!(exit, 42, "function call should return 42");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_function_with_print() {
        let stdout = compile_and_capture(
            "define greet\nprint \"hello\"\nend\ncall greet"
        );
        assert_eq!(stdout, "hello\n", "function with print should output hello");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_function_call_with_args() {
        // Define a function that adds its two parameters
        let exit = compile_and_run(
            "define adder with integer left integer right\nadd left right\nreturn it\nend\ncall adder 17 25\nreturn it"
        );
        assert_eq!(exit, 42, "function with args: 17 + 25 = 42");
    }

    // ── Comparison operator expression tests ─────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_equal_true() {
        // equal 5 5 => 1 (true)
        let exit = compile_and_run("equal 5 5\nreturn it");
        assert_eq!(exit, 1, "5 == 5 should be 1");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_equal_false() {
        // equal 5 3 => 0 (false)
        let exit = compile_and_run("equal 5 3\nreturn it");
        assert_eq!(exit, 0, "5 == 3 should be 0");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_less_true() {
        // less 3 5 => 1 (true: 3 < 5)
        let exit = compile_and_run("less 3 5\nreturn it");
        assert_eq!(exit, 1, "3 < 5 should be 1");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_less_false() {
        // less 5 3 => 0 (false: 5 < 3)
        let exit = compile_and_run("less 5 3\nreturn it");
        assert_eq!(exit, 0, "5 < 3 should be 0");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_greater_true() {
        // greater 7 2 => 1 (true: 7 > 2)
        let exit = compile_and_run("greater 7 2\nreturn it");
        assert_eq!(exit, 1, "7 > 2 should be 1");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_greater_false() {
        // greater 2 7 => 0 (false: 2 > 7)
        let exit = compile_and_run("greater 2 7\nreturn it");
        assert_eq!(exit, 0, "2 > 7 should be 0");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_compare_expression() {
        // compare 5 0 => 1 (5 != 0), compare 0 0 => 0 (0 != 0 is false)
        let exit = compile_and_run("compare 5 0\nreturn it");
        assert_eq!(exit, 1, "5 != 0 should be 1");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_compare_equal_values() {
        let exit = compile_and_run("compare 0 0\nreturn it");
        assert_eq!(exit, 0, "0 != 0 should be 0");
    }

    // ── If/else branch tests ─────────────────────────────────────────────

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_else_true_branch() {
        // condition true => if body runs, else body skipped
        let exit = compile_and_run(
            "a result\nstore 0 into result\nif compare 1 0\nstore 42 into result\nelse\nstore 99 into result\nend\nreturn result"
        );
        assert_eq!(exit, 42, "if-true should run if-body, not else-body");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_else_false_branch() {
        // condition false => if body skipped, else body runs
        let exit = compile_and_run(
            "a result\nstore 0 into result\nif compare 0 0\nstore 42 into result\nelse\nstore 99 into result\nend\nreturn result"
        );
        assert_eq!(exit, 99, "if-false should run else-body, not if-body");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_e2e_if_else_with_return() {
        // Direct return from branches
        let exit = compile_and_run(
            "if compare 5 0\nreturn 42\nelse\nreturn 99\nend"
        );
        assert_eq!(exit, 42, "if-true branch should return 42");
    }
}
