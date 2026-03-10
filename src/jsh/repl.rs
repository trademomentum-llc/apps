//! REPL — Interactive Read-Eval-Print Loop for jsh.
//!
//! Same parser as the compiler. Each line is:
//!   read → expand vars → parse shell ops → accumulate → compile → execute
//!
//! Multi-line constructs (if/while/define) accumulate until the matching `end`.
//! Built-in commands are intercepted before the compiler pipeline.
//! Shell features: $VAR expansion, set/unset/export, >/>>/<, pipes.

use crate::types::MorphResult;
use super::builtins::{self, BuiltinResult};
use super::shell::{self, ShellState};

/// Run the interactive REPL.
pub fn run() -> MorphResult<()> {
    println!("jsh — Jasterish Shell v0.3.0");
    println!("Type 'help' for commands, 'exit' to quit.\n");

    let stdin = std::io::stdin();
    let mut line = String::new();
    let mut buffer: Vec<String> = Vec::new();
    let mut nesting: i32 = 0;
    let mut state = ShellState::new();

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

        // Expand $VAR references
        let expanded = state.expand_vars(trimmed);
        let expanded = expanded.trim();

        // Variable assignment commands (set/unset/export) at top level only
        if nesting == 0 {
            if let Some(msg) = state.try_set_var(expanded) {
                if !msg.is_empty() {
                    println!("{}", msg);
                }
                continue;
            }
        }

        // Parse pipeline and redirection
        let pipeline = shell::parse_pipeline(expanded);

        // Single command (no pipes) — use existing flow
        if pipeline.segments.len() == 1 {
            let seg = &pipeline.segments[0];
            let cmd = &seg.command;

            // Built-in commands only at top level
            if nesting == 0 {
                match builtins::try_builtin(cmd) {
                    BuiltinResult::Output(text) => {
                        let output = format!("{}\n", text);
                        if let Err(e) = shell::write_output(&output, seg) {
                            eprintln!("Redirect error: {}", e);
                        }
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
            let first_word = cmd.split_whitespace().next().unwrap_or("");
            match first_word {
                "if" | "while" | "define" => nesting += 1,
                "end" => nesting -= 1,
                _ => {}
            }

            buffer.push(cmd.to_string());

            // Execute when nesting returns to zero
            if nesting <= 0 {
                nesting = 0;
                let source = buffer.join("\n");
                buffer.clear();

                match super::execute_jstar(&source) {
                    Ok(result) => {
                        if !result.stdout.is_empty() {
                            if let Err(e) = shell::write_output(&result.stdout, seg) {
                                eprintln!("Redirect error: {}", e);
                            }
                        }
                        if !result.stderr.is_empty() {
                            eprint!("{}", result.stderr);
                        }
                        if result.exit_code != 0 {
                            println!("=> {}", result.exit_code);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        buffer.clear();
                        nesting = 0;
                    }
                }
            }
        } else {
            // Pipeline: chain commands, passing stdout → stdin
            execute_pipeline(&pipeline, &mut state);
        }
    }

    Ok(())
}

/// Execute a multi-stage pipeline, feeding stdout of each stage to the next.
fn execute_pipeline(pipeline: &shell::Pipeline, state: &mut ShellState) {
    let mut pipe_input: Option<String> = None;

    for (i, seg) in pipeline.segments.iter().enumerate() {
        let is_last = i == pipeline.segments.len() - 1;
        let cmd = &seg.command;

        // Check for stdin redirect on first segment
        let input = if i == 0 {
            match shell::read_input(seg) {
                Ok(data) => data.or(pipe_input.take()),
                Err(e) => {
                    eprintln!("Redirect error: {}", e);
                    return;
                }
            }
        } else {
            pipe_input.take()
        };

        // Try builtin first
        match builtins::try_builtin(cmd) {
            BuiltinResult::Output(text) => {
                if is_last {
                    let output = format!("{}\n", text);
                    if let Err(e) = shell::write_output(&output, seg) {
                        eprintln!("Redirect error: {}", e);
                    }
                } else {
                    pipe_input = Some(format!("{}\n", text));
                }
                continue;
            }
            BuiltinResult::Ok => continue,
            BuiltinResult::Exit(code) => {
                println!("Goodbye.");
                std::process::exit(code);
            }
            BuiltinResult::NotBuiltin => {}
        }

        // Execute as JStar, with optional piped input available as env var
        if let Some(ref input_data) = input {
            // Make piped input available via $PIPE_INPUT
            state.try_set_var(&format!("set PIPE_INPUT {}", input_data.trim()));
        }

        match super::execute_jstar(cmd) {
            Ok(result) => {
                if is_last {
                    if !result.stdout.is_empty() {
                        if let Err(e) = shell::write_output(&result.stdout, seg) {
                            eprintln!("Redirect error: {}", e);
                        }
                    }
                    if !result.stderr.is_empty() {
                        eprint!("{}", result.stderr);
                    }
                    if result.exit_code != 0 {
                        println!("=> {}", result.exit_code);
                    }
                } else {
                    pipe_input = Some(result.stdout);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                return;
            }
        }
    }
}
