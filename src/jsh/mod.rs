//! JStar Shell (jsh) — interactive REPL and script execution.
//!
//! Same parser as the compiler, two input modes:
//!   - REPL: read line → parse → typecheck → codegen → execute
//!   - Script: batch execution of .jsh files
//!
//! The shell shares the full JStar compiler pipeline.

pub mod repl;
pub mod scripting;
pub mod builtins;
