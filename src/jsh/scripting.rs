//! Script Execution — batch mode for .jsh files.
//!
//! Reads a .jsh file, processes each line through the JStar compiler pipeline.
//! Built-in commands work in script mode too.
//! Supports shebang: #!/usr/bin/env jsh

use crate::types::{MorphResult, MorphlexError};
use super::builtins::{self, BuiltinResult};
use std::path::Path;

/// Execute a .jsh script file.
pub fn run_script(path: &Path) -> MorphResult<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| MorphlexError::IoError(e))?;

    let mut lines = content.lines().peekable();

    // Skip shebang line if present
    if let Some(first) = lines.peek() {
        if first.starts_with("#!") {
            lines.next();
        }
    }

    for (line_num, line) in lines.enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
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
                std::process::exit(code);
            }
            BuiltinResult::NotBuiltin => {}
        }

        // Compile and execute as JStar
        match compile_and_run(trimmed) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error at line {}: {}", line_num + 1, e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Compile a single line of JStar and execute it.
/// Currently: compile to typed AST (full execution comes with Phase 7).
fn compile_and_run(input: &str) -> MorphResult<()> {
    let (lemmas, vectors) = crate::compile(input)?;
    let program = crate::jstar::parser::parse(&lemmas, &vectors)?;
    let _typed = crate::jstar::typechecker::check(&program)?;
    // TODO: Phase 7 — codegen and execute (JIT-style)
    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_run_script_with_shebang() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_script.jsh");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "#!/usr/bin/env jsh").unwrap();
            writeln!(f, "# This is a comment").unwrap();
            writeln!(f, "echo hello from script").unwrap();
        }
        // Should not error
        let result = run_script(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_run_empty_script() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_empty.jsh");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "# empty script").unwrap();
        }
        let result = run_script(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }
}
