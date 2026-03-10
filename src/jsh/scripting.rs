//! Script Execution — batch mode for .jsh files.
//!
//! Reads a .jsh file, processes each line through the JStar compiler pipeline.
//! Built-in commands work in script mode too.
//! Supports shebang: #!/usr/bin/env jsh

use crate::types::{MorphResult, MorphlexError};
use super::builtins::{self, BuiltinResult};
use std::path::Path;

/// Execute a .jsh script file.
///
/// Built-in commands (echo, cd, pwd, etc.) are intercepted and executed inline.
/// All remaining lines are collected as JStar source, compiled as a single
/// program, and executed via the full pipeline (codegen → ELF → execute).
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

    let mut jstar_lines: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Try built-in commands first — execute them immediately
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

        // Accumulate JStar lines
        jstar_lines.push(trimmed.to_string());
    }

    // Compile and execute all JStar lines as a single program
    if !jstar_lines.is_empty() {
        let source = jstar_lines.join("\n");
        let result = super::execute_jstar(&source)?;

        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }
        if result.exit_code != 0 {
            std::process::exit(result.exit_code);
        }
    }

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

    #[test]
    #[cfg(target_os = "linux")]
    fn test_run_script_jstar_code() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_jstar.jsh");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "#!/usr/bin/env jsh").unwrap();
            writeln!(f, "# JStar code in script").unwrap();
            writeln!(f, "print 42").unwrap();
        }
        let result = run_script(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_run_script_multiline_jstar() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_multiline.jsh");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "a counter").unwrap();
            writeln!(f, "store 10 into counter").unwrap();
            writeln!(f, "add counter 5").unwrap();
            writeln!(f, "store it into counter").unwrap();
            writeln!(f, "print counter").unwrap();
        }
        let result = run_script(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }
}
