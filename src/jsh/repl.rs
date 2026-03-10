//! REPL — Interactive Read-Eval-Print Loop for jsh.
//!
//! Same parser as the compiler. Each line is:
//!   read → accumulate → morphlex tokenize → JStar parse → typecheck → codegen → execute
//!
//! Multi-line constructs (if/while/define) accumulate until the matching `end`.
//! Built-in commands are intercepted before the compiler pipeline.

use crate::types::MorphResult;
use super::builtins::{self, BuiltinResult};

/// Run the interactive REPL.
pub fn run() -> MorphResult<()> {
    println!("jsh — Jasterish Shell v0.2.0");
    println!("Type 'help' for commands, 'exit' to quit.\n");

    let stdin = std::io::stdin();
    let mut line = String::new();
    let mut buffer: Vec<String> = Vec::new();
    let mut nesting: i32 = 0;

    loop {
        // Prompt: "jsh> " for top level, "...> " for continuation
        if nesting > 0 {
            eprint!("...> ");
        } else {
            eprint!("jsh> ");
        }

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

        // Built-in commands only at top level (not inside multi-line blocks)
        if nesting == 0 {
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
                BuiltinResult::NotBuiltin => {}
            }
        }

        // Track nesting for multi-line constructs
        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        match first_word {
            "if" | "while" | "define" => nesting += 1,
            "end" => nesting -= 1,
            _ => {}
        }

        buffer.push(trimmed.to_string());

        // Execute when nesting returns to zero (or was never entered)
        if nesting <= 0 {
            nesting = 0;
            let source = buffer.join("\n");
            buffer.clear();

            match super::execute_jstar(&source) {
                Ok(result) => {
                    if !result.stdout.is_empty() {
                        print!("{}", result.stdout);
                    }
                    if !result.stderr.is_empty() {
                        eprint!("{}", result.stderr);
                    }
                    // Show non-zero exit codes as the "return value"
                    if result.exit_code != 0 {
                        println!("=> {}", result.exit_code);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    // On error, reset accumulator
                    buffer.clear();
                    nesting = 0;
                }
            }
        }
    }

    Ok(())
}
