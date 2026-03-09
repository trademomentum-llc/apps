//! REPL — Interactive Read-Eval-Print Loop for jsh.
//!
//! Same parser as the compiler. Each line is:
//!   read → morphlex tokenize → JStar parse → typecheck → codegen → execute
//!
//! Built-in commands are intercepted before the compiler pipeline.

use crate::types::MorphResult;
use super::builtins::{self, BuiltinResult};

/// Run the interactive REPL.
pub fn run() -> MorphResult<()> {
    println!("jsh — Jasterish Shell v0.1.0");
    println!("Type 'help' for commands, 'exit' to quit.\n");

    let stdin = std::io::stdin();
    let mut line = String::new();

    loop {
        // Prompt
        eprint!("jsh> ");

        // Read
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                continue;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try built-in commands first
        match builtins::try_builtin(trimmed) {
            BuiltinResult::Output(text) => {
                println!("{}", text);
                continue;
            }
            BuiltinResult::Ok => continue,
            BuiltinResult::Exit(code) => {
                println!("Goodbye.");
                std::process::exit(code);
            }
            BuiltinResult::NotBuiltin => {
                // Fall through to JStar compiler
            }
        }

        // Compile and execute as JStar
        match eval_line(trimmed) {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}

/// Evaluate a single line of JStar code.
/// Currently: parse and display the AST (codegen execution comes later).
fn eval_line(input: &str) -> MorphResult<String> {
    // Run through JStar tokenizer (morphlex + number literals)
    let (lemmas, vectors) = crate::jstar::tokenize_jstar(input)?;

    // Parse into JStar AST
    let program = crate::jstar::parser::parse(&lemmas, &vectors)?;

    // Type check
    let typed = crate::jstar::typechecker::check(&program)?;

    // For now, display the typed program (full codegen+exec comes later)
    Ok(format!("{:#?}", typed))
}
