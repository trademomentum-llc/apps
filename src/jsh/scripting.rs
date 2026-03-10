//! Script Execution — batch mode for .jsh files.
//!
//! Reads a .jsh file, processes each line through the JStar compiler pipeline.
//! Built-in commands work in script mode too.
//! Supports: shebang, $VAR expansion, set/unset/export, >/>>/<, pipes.

use crate::types::{MorphResult, MorphlexError};
use super::builtins::{self, BuiltinResult};
use super::shell::{self, ShellState};
use std::path::Path;

/// Execute a .jsh script file.
///
/// Built-in commands (echo, cd, pwd, etc.) are intercepted and executed inline.
/// Shell features ($VAR, set/unset/export, redirection, pipes) are processed.
/// All remaining lines are collected as JStar source, compiled as a single
/// program, and executed via the full pipeline (codegen -> ELF -> execute).
pub fn run_script(path: &Path) -> MorphResult<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| MorphlexError::IoError(e))?;

    let mut lines = content.lines().peekable();
    let mut state = ShellState::new();

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

        // Expand $VAR references
        let expanded = state.expand_vars(trimmed);
        let expanded = expanded.trim().to_string();

        // Variable assignment commands
        if let Some(msg) = state.try_set_var(&expanded) {
            if !msg.is_empty() {
                eprintln!("{}", msg);
            }
            continue;
        }

        // Parse pipeline and redirection
        let pipeline = shell::parse_pipeline(&expanded);

        if pipeline.segments.len() == 1 {
            let seg = &pipeline.segments[0];
            let cmd = &seg.command;

            // Try built-in commands first — execute them immediately
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
                    std::process::exit(code);
                }
                BuiltinResult::NotBuiltin => {}
            }

            // Accumulate JStar lines
            jstar_lines.push(cmd.to_string());
        } else {
            // Flush any accumulated JStar code first
            if !jstar_lines.is_empty() {
                let source = jstar_lines.join("\n");
                jstar_lines.clear();
                execute_and_display(&source, None)?;
            }
            // Execute pipeline
            execute_script_pipeline(&pipeline, &mut state);
        }
    }

    // Compile and execute all remaining JStar lines as a single program
    if !jstar_lines.is_empty() {
        let source = jstar_lines.join("\n");
        execute_and_display(&source, None)?;
    }

    Ok(())
}

/// Compile and execute JStar source, optionally redirecting output.
fn execute_and_display(source: &str, redirect: Option<&shell::Redirect>) -> MorphResult<()> {
    let result = super::execute_jstar(source)?;

    if !result.stdout.is_empty() {
        match redirect {
            Some(seg) => {
                if let Err(e) = shell::write_output(&result.stdout, seg) {
                    eprintln!("Redirect error: {}", e);
                }
            }
            None => print!("{}", result.stdout),
        }
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    if result.exit_code != 0 {
        std::process::exit(result.exit_code);
    }

    Ok(())
}

/// Execute a pipeline within a script context.
fn execute_script_pipeline(pipeline: &shell::Pipeline, state: &mut ShellState) {
    let mut pipe_input: Option<String> = None;

    for (i, seg) in pipeline.segments.iter().enumerate() {
        let is_last = i == pipeline.segments.len() - 1;
        let cmd = &seg.command;

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
                std::process::exit(code);
            }
            BuiltinResult::NotBuiltin => {}
        }

        if let Some(ref input_data) = input {
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
                        eprintln!("Pipeline stage exited with code {}", result.exit_code);
                    }
                } else {
                    pipe_input = Some(result.stdout);
                }
            }
            Err(e) => {
                eprintln!("Error in pipeline: {}", e);
                return;
            }
        }
    }
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

    #[test]
    fn test_run_script_with_variables() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_vars.jsh");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "set GREETING hello world").unwrap();
            writeln!(f, "echo $GREETING").unwrap();
        }
        let result = run_script(&path);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_run_script_with_redirect() {
        let dir = std::env::temp_dir();
        let script_path = dir.join("test_jsh_redirect.jsh");
        let output_path = dir.join("test_jsh_redirect_out.txt");
        let _ = std::fs::remove_file(&output_path);
        {
            let mut f = std::fs::File::create(&script_path).unwrap();
            writeln!(f, "echo hello redirect > {}", output_path.display()).unwrap();
        }
        let result = run_script(&script_path);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "hello redirect\n");
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_run_script_with_pipeline() {
        let dir = std::env::temp_dir();
        let script_path = dir.join("test_jsh_pipeline.jsh");
        let output_path = dir.join("test_jsh_pipeline_out.txt");
        let _ = std::fs::remove_file(&output_path);
        {
            let mut f = std::fs::File::create(&script_path).unwrap();
            // echo produces output, piped to next stage which redirects to file
            writeln!(f, "echo piped output > {}", output_path.display()).unwrap();
        }
        let result = run_script(&script_path);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "piped output\n");
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&output_path);
    }
}
