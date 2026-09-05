//! JStar Shell (jsh) — interactive REPL and script execution.
//!
//! Same parser as the compiler, two input modes:
//!   - REPL: read line → parse → typecheck → codegen → execute
//!   - Script: batch execution of .jsh files
//!
//! The shell shares the full JStar compiler pipeline.

pub mod builtins;
pub mod repl;
pub mod scripting;
pub mod shell;

use crate::types::{MorphResult, MorphlexError};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Result of executing a JStar program.
pub struct ExecutionResult {
    /// Captured stdout from the program.
    pub stdout: String,
    /// Captured stderr from the program.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
}

/// Compile JStar source to a temp binary, execute it, and return the result.
///
/// This is the Phase 7 codegen+execute bridge used by both the REPL and scripting.
pub fn execute_jstar(source: &str) -> MorphResult<ExecutionResult> {
    if !unsafe_execute_enabled() {
        return Err(MorphlexError::CodegenError(
            "JSH native execution is disabled by default; set MORPHLEX_JSH_UNSAFE_EXECUTE=1 only for audited local development".to_string(),
        ));
    }

    // Generate a unique temp path based on source hash + thread ID
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    // Include timestamp for uniqueness across invocations with same source
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        d.as_nanos().hash(&mut hasher)
    }
    let hash = hasher.finish();

    let dir = std::env::temp_dir().join("jsh_exec");
    std::fs::create_dir_all(&dir).map_err(MorphlexError::IoError)?;
    let binary = dir.join(format!("jsh_{:016x}", hash));

    // Remove stale binary if it exists
    let _ = std::fs::remove_file(&binary);

    // Compile through the full JStar pipeline
    crate::jstar::compile_source(source, &binary, crate::jstar::codegen::Arch::X86_64)?;

    // Execute the compiled binary
    let output = std::process::Command::new(&binary)
        .output()
        .map_err(MorphlexError::IoError)?;

    // Clean up the temp binary
    let _ = std::fs::remove_file(&binary);

    Ok(ExecutionResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

fn unsafe_execute_enabled() -> bool {
    matches!(
        std::env::var("MORPHLEX_JSH_UNSAFE_EXECUTE").as_deref(),
        Ok("1" | "true" | "yes")
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::execute_jstar;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static EXECUTION_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct UnsafeExecuteGuard {
        previous: Option<OsString>,
    }

    impl UnsafeExecuteGuard {
        fn new() -> Self {
            let previous = std::env::var_os("MORPHLEX_JSH_UNSAFE_EXECUTE");
            unsafe {
                std::env::set_var("MORPHLEX_JSH_UNSAFE_EXECUTE", "1");
            }
            Self { previous }
        }
    }

    impl Drop for UnsafeExecuteGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe {
                    std::env::set_var("MORPHLEX_JSH_UNSAFE_EXECUTE", value);
                },
                None => unsafe {
                    std::env::remove_var("MORPHLEX_JSH_UNSAFE_EXECUTE");
                },
            }
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_execute_jstar_return() {
        let _lock = EXECUTION_ENV_LOCK.lock().unwrap();
        let _guard = UnsafeExecuteGuard::new();
        let result = execute_jstar("return 42").unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_execute_jstar_print() {
        let _lock = EXECUTION_ENV_LOCK.lock().unwrap();
        let _guard = UnsafeExecuteGuard::new();
        let result = execute_jstar("print 99").unwrap();
        assert_eq!(result.stdout.trim(), "99");
        unsafe {
            std::env::remove_var("MORPHLEX_JSH_UNSAFE_EXECUTE");
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_execute_jstar_multiline() {
        let _lock = EXECUTION_ENV_LOCK.lock().unwrap();
        let _guard = UnsafeExecuteGuard::new();
        let result = execute_jstar("a counter\nstore 10 into counter\nprint counter").unwrap();
        assert_eq!(result.stdout.trim(), "10");
    }
}
