│                                                                                                                               │
│ Jasterish (JStar) Compiler -- Implementation Plan                                                                             │
│                                                                                                                               │
│ Context                                                                                                                       │
│                                                                                                                               │
│ morphlex Phase 1 is complete: a deterministic NLP tokenizer that produces 12-byte integer-packed token vectors from English   │
│ text. The vector database is the foundation for Jasterish -- a system-level machine language where natural language tokens    │
│ ARE the instruction set. The token's POS, semantic role, and morph flags determine what kind of instruction it is. This       │
│ eliminates the gap between human language and machine code: verbs are operations, nouns are data, adjectives are modifiers,   │
│ prepositions are relations.                                                                                                   │
│                                                                                                                               │
│ The compiler must be self-hosting: we bootstrap in Rust, then Jasterish compiles itself. Machine code emission is direct      │
│ x86-64 (no LLVM, no Cranelift) so the self-hosted compiler has zero external dependencies.                                    │
│                                                                                                                               │
│ Prerequisites (before Phase 0)                                                                                                │
│                                                                                                                               │
│ - Fix morphology edge cases in src/morphology.rs (MIN_ROOT_LEN=3 verification, "over"->"ov" fix)                              │
│ - Test compile-dict end-to-end against /usr/share/dict/words                                                                  │
│ - Verify determinism: same input = identical database (minus nonce)                                                           │
│ - These are TODO.md Phase 2 items that must be solid before building a language on top                                        │
│                                                                                                                               │
│ Module Layout                                                                                                                 │
│                                                                                                                               │
│ src/                                                                                                                          │
│   jstar/           # Compiler                                                                                                 │
│     mod.rs         # Public API, re-exports                                                                                   │
│     token_map.rs   # morphlex TokenVector -> JStar instruction mapping                                                        │
│     grammar.rs     # Language grammar definition, AST types                                                                   │
│     parser.rs      # Recursive descent parser (.jstr -> JStar AST)                                                            │
│     typechecker.rs # Type system (Java's 8 primitives)                                                                        │
│     codegen.rs     # x86-64 machine code emission                                                                             │
│     linker.rs      # ELF binary assembly                                                                                      │
│     ir.rs          # Intermediate representation (between AST and machine code)                                               │
│   jsh/             # Shell                                                                                                    │
│     mod.rs         # Public API                                                                                               │
│     repl.rs        # Interactive REPL mode                                                                                    │
│     scripting.rs   # Script execution mode (.jsh files)                                                                       │
│     builtins.rs    # Built-in shell commands                                                                                  │
│                                                                                                                               │
│ Semantic Category Mapping (the core insight)                                                                                  │
│                                                                                                                               │
│ The morphlex TokenVector already classifies every English word. Jasterish uses this classification directly:                  │
│                                                                                                                               │
│ ┌──────────────┬────────────────────────────┬─────────────────────────────────────────────────────┐                           │
│ │ POS Category │         JStar Role         │                      Examples                       │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Verb         │ Operation/Instruction      │ "add", "store", "jump", "compare", "return"         │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Noun         │ Data declaration/reference │ "integer", "buffer", "counter", "result"            │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Adjective    │ Type modifier/qualifier    │ "unsigned", "static", "mutable", "volatile"         │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Adverb       │ Execution modifier         │ "immediately", "conditionally", "repeatedly"        │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Preposition  │ Relation/addressing mode   │ "into", "from", "at", "through"                     │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Determiner   │ Scope/lifetime             │ "the" (global), "a" (local), "this" (self)          │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Conjunction  │ Control flow join          │ "and" (sequence), "or" (branch), "if" (conditional) │                           │
│ ├──────────────┼────────────────────────────┼─────────────────────────────────────────────────────┤                           │
│ │ Pronoun      │ Register/reference alias   │ "it" (accumulator), "that" (last result)            │                           │
│ └──────────────┴────────────────────────────┴─────────────────────────────────────────────────────┘                           │
│                                                                                                                               │
│ Type System (Java's 8 primitives)                                                                                             │
│                                                                                                                               │
│ boolean  ->  i8   (0 or 1)                                                                                                    │
│ byte     ->  i8   (-128 to 127)                                                                                               │
│ short    ->  i16  (-32768 to 32767)                                                                                           │
│ int      ->  i32  (default integer)                                                                                           │
│ long     ->  i64                                                                                                              │
│ float    ->  f32                                                                                                              │
│ double   ->  f64                                                                                                              │
│ char     ->  u16  (UTF-16 code unit)                                                                                          │
│                                                                                                                               │
│ Type checking uses the pos and morph fields from TokenVector to infer and validate types.                                     │
│                                                                                                                               │
│ Implementation Phases                                                                                                         │
│                                                                                                                               │
│ Phase 0: Token-to-Instruction Mapping                                                                                         │
│                                                                                                                               │
│ File: src/jstar/token_map.rs                                                                                                  │
│ - Define JStarInstruction enum (Move, Add, Sub, Mul, Div, Compare, Jump, JumpIf, Call, Return, Load, Store, Push, Pop,        │
│ Syscall, Nop)                                                                                                                 │
│ - Map morphlex i32 token IDs to instruction variants using the vector DB                                                      │
│ - Resolution function: fn resolve(id: i32, pos: i8, role: i8, morph: i16) -> JStarInstruction                                 │
│ - Instruction categories derived from POS: verb -> operation, noun -> data, etc.                                              │
│ - Build the instruction vocabulary from the compiled morphlex database                                                        │
│                                                                                                                               │
│ Phase 1: Grammar & AST                                                                                                        │
│                                                                                                                               │
│ File: src/jstar/grammar.rs                                                                                                    │
│ - Define JStar AST node types (algebraic data types, pattern matching -- same style as morphlex)                              │
│ - Grammar is POS-driven structured English:                                                                                   │
│ statement  := verb_phrase (noun_phrase)? (prep_phrase)*                                                                       │
│ verb_phrase := adverb? verb                                                                                                   │
│ noun_phrase := determiner? adjective* noun                                                                                    │
│ prep_phrase := preposition noun_phrase                                                                                        │
│ - Example: "store the unsigned integer into buffer" parses as:                                                                │
│   - verb: store (Operation::Store)                                                                                            │
│   - noun_phrase: the unsigned integer (Type::u32, scope: global)                                                              │
│   - prep_phrase: into buffer (destination addressing)                                                                         │
│                                                                                                                               │
│ Phase 2: Recursive Descent Parser                                                                                             │
│                                                                                                                               │
│ File: src/jstar/parser.rs                                                                                                     │
│ - Input: .jstr source text -> morphlex pipeline -> token stream -> JStar AST                                                  │
│ - POS-driven recursive descent (POS tag determines which parse rule to invoke)                                                │
│ - The parser consumes TokenVectors, not raw characters                                                                        │
│ - Reuses morphlex's compile() function as the front-end lexer                                                                 │
│ - Error recovery: monadic MorphResult<T> error handling throughout                                                            │
│                                                                                                                               │
│ Phase 3: Type Checker                                                                                                         │
│                                                                                                                               │
│ File: src/jstar/typechecker.rs                                                                                                │
│ - Walk the JStar AST, infer and validate types                                                                                │
│ - Type inference from adjective modifiers: "unsigned" -> unsigned variant, "long" -> i64                                      │
│ - Default type is int (i32) when no modifier present                                                                          │
│ - Type errors are compile-time (no runtime type checks in emitted code)                                                       │
│                                                                                                                               │
│ Phase 4: Intermediate Representation                                                                                          │
│                                                                                                                               │
│ File: src/jstar/ir.rs                                                                                                         │
│ - Three-address code IR between AST and machine code                                                                          │
│ - SSA (Static Single Assignment) form for optimization passes                                                                 │
│ - IR instructions map 1:1 to semantic operations, not yet to x86 instructions                                                 │
│ - Keeps the codegen phase clean: IR -> x86 is a straightforward lowering                                                      │
│                                                                                                                               │
│ Phase 5: x86-64 Code Generation                                                                                               │
│                                                                                                                               │
│ File: src/jstar/codegen.rs                                                                                                    │
│ - Direct x86-64 machine code emission (no LLVM, no Cranelift)                                                                 │
│ - Instruction encoding: REX prefixes, ModR/M, SIB, displacement, immediate                                                    │
│ - Register allocation: linear scan over SSA IR                                                                                │
│ - System V AMD64 ABI for function calls (rdi, rsi, rdx, rcx, r8, r9 for args; rax for return)                                 │
│ - Syscall interface for I/O and process control                                                                               │
│                                                                                                                               │
│ Phase 6: ELF Linker                                                                                                           │
│                                                                                                                               │
│ File: src/jstar/linker.rs                                                                                                     │
│ - Assemble machine code into ELF64 executable format                                                                          │
│ - ELF header + program headers + .text section + .data section                                                                │
│ - Minimal: no dynamic linking in bootstrap phase (static executables only)                                                    │
│ - Output: native Linux x86-64 binary                                                                                          │
│                                                                                                                               │
│ Phase 7: JStar Shell (REPL)                                                                                                   │
│                                                                                                                               │
│ Files: src/jsh/repl.rs, src/jsh/builtins.rs                                                                                   │
│ - Same parser as the compiler, two input modes (interactive vs script)                                                        │
│ - REPL: read line -> parse -> typecheck -> codegen -> execute (JIT-style)                                                     │
│ - Built-in commands: file operations, process management, environment                                                         │
│ - .jsh extension for shell scripts                                                                                            │
│                                                                                                                               │
│ Phase 8: Shell Scripting                                                                                                      │
│                                                                                                                               │
│ File: src/jsh/scripting.rs                                                                                                    │
│ - Batch execution of .jsh files                                                                                               │
│ - Shebang support: #!/usr/bin/env jsh                                                                                         │
│ - Environment variable access, piping, redirection                                                                            │
│ - Integration with the REPL built-ins                                                                                         │
│                                                                                                                               │
│ Phase 9: Self-Hosting Preparation                                                                                             │
│                                                                                                                               │
│ - Write the JStar compiler in JStar itself (.jstr source files)                                                               │
│ - The Rust bootstrap compiler compiles the JStar compiler source                                                              │
│ - The JStar-compiled compiler must produce identical output to the Rust bootstrap                                             │
│ - This is the T-diagram: Rust compiles JStar1, JStar1 compiles JStar2, JStar1 output == JStar2 output                         │
│                                                                                                                               │
│ Phase 10: Standard Library                                                                                                    │
│                                                                                                                               │
│ - Core operations library written in JStar                                                                                    │
│ - Math, string manipulation, I/O, memory management                                                                           │
│ - All built on the 8 primitive types                                                                                          │
│ - Distributed as compiled .jstr modules                                                                                       │
│                                                                                                                               │
│ Phase 11: Self-Hosting Verification                                                                                           │
│                                                                                                                               │
│ - Remove Rust bootstrap dependency                                                                                            │
│ - JStar compiler compiles itself                                                                                              │
│ - Triple-verified: Bootstrap(Rust) -> JStar1 -> JStar2, verify JStar1 == JStar2                                               │
│ - The language is now self-sustaining                                                                                         │
│                                                                                                                               │
│ Critical Files to Modify                                                                                                      │
│                                                                                                                               │
│ ┌──────────────┬─────────────────────────────────────────────────────────────────────────┐                                    │
│ │     File     │                                 Change                                  │                                    │
│ ├──────────────┼─────────────────────────────────────────────────────────────────────────┤                                    │
│ │ Cargo.toml   │ No new dependencies for Phase 0-6 (pure Rust, no external codegen libs) │                                    │
│ ├──────────────┼─────────────────────────────────────────────────────────────────────────┤                                    │
│ │ src/lib.rs   │ Add pub mod jstar; and pub mod jsh;                                     │                                    │
│ ├──────────────┼─────────────────────────────────────────────────────────────────────────┤                                    │
│ │ src/main.rs  │ Add CLI subcommands: jstar compile, jsh                                 │                                    │
│ ├──────────────┼─────────────────────────────────────────────────────────────────────────┤                                    │
│ │ src/types.rs │ May add JStar-specific types or extend existing enums                   │                                    │
│ └──────────────┴─────────────────────────────────────────────────────────────────────────┘                                    │
│                                                                                                                               │
│ Verification Plan                                                                                                             │
│                                                                                                                               │
│ 1. Phase 0: Unit test that known verbs (add, store, jump) resolve to correct instructions via token DB lookup                 │
│ 2. Phase 1-2: Parse .jstr source snippets and verify AST structure matches expected parse trees                               │
│ 3. Phase 3: Type-check valid and invalid programs, verify correct acceptance/rejection                                        │
│ 4. Phase 5-6: Compile a minimal program (return 42), run the ELF binary, verify exit code is 42                               │
│ 5. Phase 7: REPL evaluates add 1 to 2 and prints 3                                                                            │
│ 6. Phase 9-11: Self-hosting triple test (bootstrap == stage1 == stage2 output)                                                │
│ 7. All tests: cargo test runs the full suite including JStar tests                                                            │
│                                                                                                                               │
│ Design Principles (inherited from morphlex)                                                                                   │
│                                                                                                                               │
│ - Deterministic: same source -> same binary, always                                                                           │
│ - Pattern matching over if/else (Rust match everywhere)                                                                       │
│ - Monadic errors: MorphResult<T> throughout                                                                                   │
│ - Pure functions: each phase is Input -> Output                                                                               │
│ - No floats in the token system (the language supports f32/f64 as data types, but instruction dispatch is always integer)     │
│ - PQC: any key material or signing in the toolchain uses the same NIST PQC stack    
