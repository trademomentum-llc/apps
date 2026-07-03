# Design Specification: Jasterish (JStar) Compiler Self-Hosting Pipeline

**Document ID:** JSTAR-DS-001  
**Version:** 1.0.0  
**Date:** 2026-05-28  
**Predecessor:** JSTAR-REQ-001

---

## 1. Compiler Pipeline Design

The pipeline is deliberately modeled on Clang phases but adapted to a natural-language token substrate.

```
Raw Text
  │
  ▼
Lexer (words → morphlex; numbers/strings → synthetic literals)
  │
  ▼
TokenVector Stream (12-byte packed: i32 id + i8 pos/role + i16 morph + ...)
  │
  ▼
Parser (POS-driven recursive descent)
  │
  ▼
AST (grammar.rs algebraic data types)
  │
  ▼
Typechecker (adjective-driven primitive mapping: unsigned long → u64, etc.)
  │
  ▼
IR (three-address, SSA form for optimization)
  │
  ▼
Codegen (direct x86-64, linear-scan register allocation, System V ABI)
  │
  ▼
Linker (ELF-64: header + program headers + .text + .data)
  │
  ▼
Executable Binary
```

---

## 2. TokenVector as Instruction Encoding (Core Insight)

The morphlex 12-byte vector already classifies every token. JStar reuses this classification directly:

- Verb → Operation (Move, Add, Jump, Call, Syscall, ...)
- Noun → Data / Register / Memory operand
- Adjective → Type modifier (unsigned, volatile, static)
- Preposition → Addressing mode (into, from, at)
- Conjunction → Control flow join (and = seq, or = branch)

This eliminates a separate instruction set architecture. The language **is** the classification produced by the deterministic NLP front-end.

---

## 3. Type System Design

Maps to Java's 8 primitives for familiarity and small encoding:

- boolean → i8 (0/1)
- byte → i8
- short → i16
- int → i32 (default)
- long → i64
- float → f32
- double → f64
- char → u16

Type inference walks adjective modifiers on noun phrases. Default is i32. Compile-time only.

---

## 4. Self-Hosting Strategy (T-Diagram)

1. jstar2 (Rust bootstrap) is the trusted base.
2. It compiles the JStar source of jstar3.
3. jstar3 must be able to compile the same JStar source and produce a binary that is either identical or proven equivalent via the proof validator (see pitchfork agent/SP3).
4. Once jstar3 is stable, jstar2 can be retired or kept only for bootstrap verification.

Current measured state (T-Diagram):
- jstar2: 3.9 MB, data hash stable, functional.
- jstar3 target: ~68 KB class, direct codegen (no full IR yet in all paths).

---

## 5. Key Modules and Responsibilities

- `lexer.rs` / `token_map.rs`: morphlex bridge + literal synthesis.
- `grammar.rs`: AST nodes (statement, verb_phrase, noun_phrase, etc.).
- `parser.rs`: POS-driven recursive descent.
- `typechecker.rs`: adjective → primitive mapping + scope rules.
- `ir.rs`: SSA form (optional optimization layer).
- `codegen.rs`: REX/ModR/M/SIB encoding, linear scan allocation, syscall emission.
- `linker.rs`: Minimal ELF-64 writer (static only in bootstrap phase).

---

## 6. Error Handling Design

- Monadic `MorphResult<T>` (or equivalent) throughout.
- No panic in compiler core on valid input.
- Clear, deterministic error messages with source location (line + token offset).

---

## 7. Integration Points (Highest-Value Consumers)

- nnos: Validation layer and morphogenetic engine ports (JStar as the systems language for the neurodivergent OS daemons).
- pitchfork/agent/inference: Morphlex tokenizer + ONNX runtime for proof-carrying suggestions inside Forgejo.
- apps/: The primary development and self-hosting vehicle.

Any change to TokenVector layout or calling convention must be coordinated across all three.

---

**End of Design Specification**

Technical Specification (exact 12-byte layout, x86 opcode tables used, ELF section layout, self-host validation harness, carbon tracking) completes the triad.