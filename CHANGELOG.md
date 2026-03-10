# Changelog

All notable changes to the morphlex + JStar project.

Format: [Semantic Versioning](https://semver.org/). Each entry includes the date, what changed, and why.

---

## [0.5.0] - 2026-03-10

### Added
- **Byte/char operations** -- `movzx` (0x0F 0xB6) for byte loads, 8-bit `mov` (0x88) for byte stores
  - `is_byte_type()` check on Store/Load IR instructions dispatches to byte-width codegen
  - Supports JStarType::Byte, JStarType::Boolean, JStarType::Char
- **Fixed-size stack arrays** -- `a buffer 256` declares 256 bytes on the stack
  - `store 42 into buffer at 0` / `load from buffer at 0` -- indexed read/write
  - `StoreIndexed` / `LoadIndexed` IR instructions with SIB-encoded x86-64 addressing
  - `size: Option<usize>` field on Declare AST nodes flows through parser -> typechecker -> IR -> codegen
  - SIB byte encoding: scale=1 for bytes, scale=8 for qwords; index=rcx, base=rax
- **Hex literals** -- `0x2A` preprocessed to decimal before morphlex (keeps NLP pipeline pure)
  - `preprocess_hex_literals()` converts `0x[0-9a-fA-F]+` to decimal strings
  - Case-insensitive: `0xFF`, `0XFF`, `0xAbCd` all work
- **Multi-file compilation** -- `compile_multi()` concatenates source files before compilation
  - CLI: `jstar compile --input main.jstr --include lib.jstr`
- **ETXTBSY race fix** -- `run_binary()` retry-with-backoff (10ms increments) for kernel exec lock race
- **10 new tests** (185 total) -- arrays, hex literals, hex preprocessing, byte operations

### Changed
- `src/jstar/grammar.rs` -- `Declare` gains `size: Option<usize>` field (both untyped and typed variants)
- `src/jstar/parser.rs` -- `try_parse_array_size()` helper checks for literal after declaration name
- `src/jstar/ir.rs` -- `StoreIndexed`/`LoadIndexed` instructions; `AddrMode` import; size-aware Alloca; indexed addressing detection in Store/Load lowering
- `src/jstar/codegen.rs` -- Byte store/load instructions; indexed store/load with SIB encoding; `is_byte_type` dispatch on Store/Load; `LoadIndexed` pre-allocation
- `src/jstar/mod.rs` -- `preprocess_hex_literals()`; `compile_multi()`; `run_binary()` ETXTBSY retry
- `src/main.rs` -- `--include` flag on `jstar compile`

### Milestone
**Data structures and hex literals complete.** The language can manipulate byte arrays, use hex constants, and compile multi-file projects.

---

## [0.2.5] - 2026-03-10

### Added
- **Function definitions and calls** -- `define <name> [with <type> <param>...] <body> end` / `call <name> [args...]`
  - `define answer\nreturn 42\nend\ncall answer\nreturn it` -> exit code 42
  - `define adder with integer left integer right\nadd left right\nreturn it\nend\ncall adder 17 25\nreturn it` -> exit code 42
  - Functions use System V AMD64 ABI (args in rdi/rsi/rdx/rcx/r8/r9, return in rax)
  - `call rel32` fixups resolved after all functions are emitted
- **String literals and sys_write** -- `print "hello"` writes to stdout via syscall
  - Segment-based tokenizer splits input into Code/Str segments before morphlex processing
  - String data accumulated in .data section, addresses patched by linker
  - `mov rsi, imm64` pattern (0x48 0xBE) scanned and patched with data_vaddr
- **Parser robustness for non-keyword identifiers** -- `call` now accepts any token as function name regardless of POS; TypeModifier tokens (adjective-classified variable names like "left", "right") fall back to variable names instead of erroring; function def parameter names accept any non-control-flow token
- **6 new e2e tests** (125 total) -- string printing, function calls, function args, mixed strings and numbers

### Changed
- `src/jstar/linker.rs` -- Single PT_LOAD segment (R+W+X) instead of dual text/data segments. Simpler ELF layout, avoids kernel mapping issues
- `src/jstar/ir.rs` -- `lower_to_function()` preserves param Alloca instructions instead of clearing them
- `src/jstar/codegen.rs` -- `MachineCode` gains `data_vaddr` field; `CodeGen` tracks `function_offsets`, `call_fixups`, `is_entry_point` for multi-function emission
- `src/jstar/grammar.rs` -- Added `FunctionDef` statement, `StringLiteral` operand, `TypedStatement::FunctionDef`, `TypedOperand::StringLiteral`
- `src/jstar/token_map.rs` -- Added `FunctionDef` category, `POS_STRING` constant, "define"/"function"/"with" keywords

### Fixed
- **ETXTBSY race condition** -- Thread ID added to binary name hash; stale binaries removed before writing
- **Parameter alloca loss** -- `lower_to_function()` was clearing `current_insts` which contained param allocas from the caller
- **Function name POS mismatch** -- "call answer" failed because morphlex classified "answer" as a verb (Operation), not an operand

### Milestone
**Functions and strings work end-to-end.** The language has procedures, arguments, return values, and I/O.

---

## [0.2.2] - 2026-03-09

### Added
- **Stack-allocated variables** -- declare, store, load, and use variables in arithmetic
  - `a counter` / `store 42 into counter` / `return counter` -> exit code 42
  - `a counter` / `store 10 into counter` / `add counter 5` / `return it` -> exit code 15
  - `a result` / `add 17 8` / `store it into result` / `return result` -> exit code 25
- **Keyword hash table expansion** -- determiners ("a", "an", "the"), prepositions ("into", "from", "to", "at"), and pronouns ("it", "that") added to BLAKE3 hash table for O(1) resolution regardless of morphlex POS classification
- **IR variable tracking** -- `variables: HashMap<String, VReg>` in Lowerer maps declared variable names to their alloca vregs. Variable operands resolve to `IrValue::Reg(alloca_vreg)` instead of `IrValue::Named(name)`
- **2 new tests** (91 total) -- `test_lower_variable_store_and_return`, `test_lower_undeclared_variable_uses_named`

### Changed
- `src/jstar/token_map.rs` -- KEYWORD_TABLE expanded with 12 new entries (determiners, prepositions, pronouns)
- `src/jstar/ir.rs` -- Lowerer tracks `variables: HashMap<String, VReg>`. Declare records name->vreg. Variable operands resolve to `Reg(alloca_vreg)` for declared variables
- `src/jstar/codegen.rs` -- Store/Load instructions now use rbp-relative addressing for `Reg(vreg)` (stack slots) instead of pointer dereference. Preserves indirect addressing for `Imm(addr)` operands

### Milestone
**Variables work end-to-end: declare, store, load, arithmetic with variables, accumulator-to-variable transfer.** The language has state.

### Design decisions
1. **Stack-slot addressing over pointer dereference** -- In bootstrap, all variables are stack-allocated. `Store { addr: Reg(vreg) }` emits `mov [rbp+offset], value` (direct stack access) instead of `mov rax, [rbp+offset]; mov [rax], value` (pointer dereference). Simpler, faster, correct for the current memory model.
2. **BLAKE3 hash table for function words** -- "a", "the", "into" etc. are too short/ambiguous for morphlex's suffix-based POS heuristics. The hash table resolves them deterministically by i32 identity, same pattern that already works for "return" and "integer".

---

## [0.2.1] - 2026-03-09

### Added
- **Arithmetic operations end-to-end** -- add, subtract, multiply, divide with literal operands produce correct native binaries
  - `add 3 5 return it` -> exit code 8
  - `subtract 10 3 return it` -> exit code 7
  - `multiply 4 7 return it` -> exit code 28
  - `divide 20 4 return it` -> exit code 5
- **Accumulator tracking in IR** -- `last_result: Option<VReg>` in Lowerer struct tracks the vreg from the most recent Execute statement. Pronoun "it" (Register::Accumulator) resolves to this vreg, enabling natural chaining: "compute something, return it"

### Changed
- `src/jstar/ir.rs` -- `lower_execute()` now returns `(Vec<IrInst>, VReg)` tuple. All callers destructure and update `self.last_result`. Register operands resolve via `IrValue::Reg(last_result)` instead of placeholder `IrValue::Imm(0)`

### Milestone
**All four arithmetic operations compile to native x86-64 ELF binaries and produce correct results via exit codes.** The language computes.

### Design decisions
1. **Accumulator pronoun over explicit variables** -- "it" is the natural English way to refer back to the last result. This maps directly to the accumulator pattern in hardware. No variable declarations needed for simple pipelines.
2. **Exit codes for verification** -- Linux exit codes (0-255) provide a zero-dependency way to verify computation correctness during bootstrap. Stdout write syscalls come later.

---

## [0.2.0] - 2026-03-09

### Added
- **JStar compiler (Phases 0-6)** -- a system-level machine language built on morphlex token vectors
  - `src/jstar/token_map.rs` -- i32 BLAKE3 keyword hash table for O(1) instruction resolution. No string matching. `tv.id = BLAKE3(original_lexeme)` catches keywords like "return" and "integer" even when morphology misanalyzes them (re-+turn, in-+teg+-er)
  - `src/jstar/grammar.rs` -- POS-driven AST types. JStarType maps to Java's 8 primitives (boolean, byte, short, int, long, float, double, char)
  - `src/jstar/parser.rs` -- Recursive descent parser. POS tag of each token determines which parse rule fires: verb=operation, noun=data, adjective=modifier, determiner=scope, conjunction=control flow
  - `src/jstar/typechecker.rs` -- Type inference from noun lemmas and adjective modifiers. Symbol table tracks declared variables. Immediate values get the smallest fitting type
  - `src/jstar/ir.rs` -- Three-address code intermediate representation in SSA form. Virtual registers, basic blocks with terminators (Return, Halt, Jump, Branch)
  - `src/jstar/codegen.rs` -- Direct x86-64 machine code emission. REX prefixes, ModR/M encoding, stack-based virtual register allocation. System V AMD64 ABI. No LLVM, no Cranelift
  - `src/jstar/linker.rs` -- ELF64 binary assembly. Header + program headers + .text + .data. Static linking only. Sets executable permissions
  - `src/jstar/mod.rs` -- `tokenize_jstar()` wraps morphlex pipeline for words and synthesizes `POS_LITERAL` tokens for numbers. `compile_source()` and `compile_file()` run the full pipeline
- **jsh shell**
  - `src/jsh/repl.rs` -- Interactive REPL: read line -> tokenize -> parse -> typecheck -> display
  - `src/jsh/scripting.rs` -- .jsh script execution with shebang support (`#!/usr/bin/env jsh`)
  - `src/jsh/builtins.rs` -- Built-in commands: help, exit, pwd, cd, ls, cat, echo, env, tokenize
- **CLI subcommands** -- `jstar compile`, `jstar parse`, `jsh` (REPL or script mode)
- **`POS_LITERAL` (value 10)** in token_map.rs -- routes number tokens through resolve() to `TokenCategory::Literal`, since morphlex NLP pipeline drops numbers at the morphology stage
- **`vectorizer::hash_to_i32` made public** -- canonical BLAKE3-to-i32 hash used by JStar tokenizer for synthetic number vectors
- **54 new tests** across all compiler phases (token_map, grammar, parser, typechecker, IR, codegen, linker, jsh builtins/scripting, tokenizer)

### Changed
- `src/lib.rs` -- Added `pub mod jstar` and `pub mod jsh`
- `src/main.rs` -- Added JStar compile/parse subcommands and jsh launcher
- Parser signature: `parse(lemmas, vectors)` (2-arg) instead of 3-arg. Original lexemes not needed because keyword hash table resolves from `tv.id`
- All JStar/jsh call sites use `tokenize_jstar()` instead of `crate::compile()` to preserve number literals

### Milestone
**"return 42" compiles to a native x86-64 ELF binary, runs, exits with code 42.** The compiler is standing.

### Design decisions
1. **i32 keyword hash over string exception lists** -- morphlex legitimately decomposes "return" as re-+turn and "integer" as in-+teg+-er. Rather than adding exception lists (which create latency), the keyword hash table checks `tv.id` (BLAKE3 of the original word) in O(1). The vector IS the identity.
2. **JStar tokenizer separate from morphlex pipeline** -- morphlex is NLP; it correctly drops numbers. JStar wraps it and interleaves synthetic number tokens with `POS_LITERAL`, keeping the NLP pipeline pure.
3. **`gen` avoided as variable name** -- reserved keyword in Rust 2024 edition. Used `emitter` instead in codegen.rs.

---

## [0.1.0] - 2026-03-08

### Added
- **morphlex core pipeline** -- deterministic natural language tokenizer and vector compiler
  - `src/lexer.rs` -- Phase 1: raw text to token stream. Handles words, contractions (don't), hyphenated compounds (well-known), numbers, punctuation, whitespace
  - `src/morphology.rs` -- Phase 2: morpheme decomposition. 36 prefixes, 34 suffixes, greedy longest-match, MIN_ROOT_LEN=3
  - `src/ast.rs` -- Phase 3: POS inference from morphological signals, phrase grouping (NP, VP, PP, AdvP), recursive tree structure
  - `src/semantics.rs` -- Phase 4: SVO semantic role assignment (Agent, Action, Patient, Instrument, Location, Temporal, Modifier, Quantifier, Connector)
  - `src/vectorizer.rs` -- Phase 5: 12-byte integer-packed TokenVector per word. BLAKE3 hash for id/lemma_id. Moderne/OpenRewrite-style recipe engine for deterministic transforms
  - `src/database.rs` -- Phases 6-8: flat binary format v3 (header + lemma table + vector table), compactor (exact-size truncation), PQC encryption
  - `src/types.rs` -- Core algebraic data types: Token, TokenKind, Morpheme, MorphAnalysis, PartOfSpeech, AstNode, SemanticNode, TokenVector, Recipe, MorphlexError
  - `src/lib.rs` -- Public API: `compile(input)` returns (lemmas, vectors), `compile_lexicon(words)` writes encrypted database
  - `src/main.rs` -- CLI: `tokenize`, `compile`, `compile-dict`, `inspect`
- **TokenVector** -- 12-byte integer-packed object mapped to Java's 8 primitives
  - `i32 id` -- BLAKE3 hash of lexeme, truncated. The token's address
  - `i32 lemma_id` -- BLAKE3 hash of base form
  - `i8 pos` -- part of speech discriminant (0-9)
  - `i8 role` -- semantic role discriminant (0-8)
  - `i16 morph` -- 13-bit morphological flag bitfield
- **PQC encryption** -- all post-quantum, all NIST standard
  - ML-KEM-1024 (FIPS 203) for key encapsulation
  - ML-DSA-65 (FIPS 204) for digital signatures / tamper detection
  - AES-256-GCM (FIPS 197 + SP 800-38D) for symmetric encryption
- **Recipe engine** -- Moderne/OpenRewrite-style deterministic pattern-match-and-transform rules. Suffix/Prefix/Exact/Pos/Any patterns. SetPos/AddMorphFlags/SetLemmaId/Chain transforms. First match wins
- **30 passing tests** across all pipeline stages

### Design decisions
1. **12-byte integer vectors over 512-byte float vectors** -- identity is `==` on i32, not cosine similarity on f64[64]. No FPU needed. 42x smaller per token.
2. **Word-level vectorization** -- every word gets its own 12-byte vector. No phrase-level collapsing. This enables the programming language: each word maps to exactly one instruction/operand.
3. **Clang-style pipeline** -- each phase is a pure function (Input -> Output). Lexer -> Morphology -> AST -> Semantics -> Vectorizer. Same architecture that compiles C++ adapted for NLP.
4. **Monadic error handling** -- `MorphResult<T> = Result<T, MorphlexError>` throughout. Every fallible operation returns the monad. Pattern matching over if/else everywhere.
5. **PQC-only cryptography** -- no classical crypto anywhere. NIST FIPS standards or better. The database is immutable once encrypted.
