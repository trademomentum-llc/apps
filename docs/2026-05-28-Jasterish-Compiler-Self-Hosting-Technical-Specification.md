# Technical Specification: Jasterish (JStar) Compiler Self-Hosting Pipeline

**Document ID:** JSTAR-TECH-001  
**Version:** 1.0.0  
**Date:** 2026-05-28  
**Predecessors:** JSTAR-REQ-001, JSTAR-DS-001

---

## 1. TokenVector Binary Layout (Canonical)

12 bytes, little-endian, no padding:

Offset | Size | Field          | Type | Meaning
-------|------|----------------|------|--------
0      | 4    | id             | i32  | morphlex token identifier
4      | 1    | pos            | i8   | Part-of-speech (verb=1, noun=2, ...)
5      | 1    | role           | i8   | Semantic role
6      | 2    | morph          | i16  | Morphology flags
8      | 4    | value_or_index | i32  | Literal value or symbol table index

For literals (numbers/strings), pos = POS_LITERAL, id carries synthetic value.

---

## 2. x86-64 Codegen Rules (Direct Emission)

- Registers: rax (accumulator), rdi/rsi/rdx/rcx/r8/r9 (args per SysV), rbp (frame), rsp (stack).
- Linear scan register allocation over SSA IR.
- While/for loops: Use label + conditional jump (jne/jmp pattern). Data section fixups applied before emission of any function body that references them.
- Syscalls: mov rax, syscall_nr; syscall; (Linux x86-64 ABI).
- No floating-point in bootstrap phase (only integer primitives).

Known defect (2026-04 snapshot): Nested while loops with data collection in self-hosted codegen can enter infinite loop due to incorrect fixup ordering for forward references in loop conditions. Root cause isolated to codegen.rs emit_while.

---

## 3. ELF-64 Output Contract (Bootstrap Phase)

- Static executable only (no dynamic linking or .interp).
- Sections: .text (RX), .data (RW), .rodata (R), minimal .symtab/.strtab for debug.
- Entry point: _start (emits syscall exit on top-level return).
- Alignment: 4K page for segments.

---

## 4. Self-Host Validation Harness

Mandatory preflight (apps/scripts/jstar_bootstrap_check.sh):

1. Build jstar2 (release).
2. jstar2 compiles compiler.jstr → jstar3 candidate.
3. Execute jstar3 on compiler.jstr with data collection enabled.
4. Compare output binary data hash against known stable value.
5. Run smoke tests (tokenize, compile small programs, execute).

Failure on step 3 or 4 blocks promotion of any self-hosted binary.

Current T-Diagram reality (2026-04 snapshot):
- jstar2 (Rust): 3.9 MB, full .data, functional.
- jstar3 (self-hosted): 68 KB, missing .data in some paths → hang on data-collection workloads.

---

## 5. Determinism & Proof Requirements

- Fixed morphlex database + fixed source → identical binary (modulo intentional nonce for any encrypted artifacts).
- When integrated with pitchfork/agent: every suggestion carries BLAKE3 proof over type/bounds/invariant claims + carbon cost (mg CO2).

---

## 6. Error Model

- All phases return MorphResult<T> (monadic).
- On fatal error: emit line + token offset + deterministic message. Never panic on well-formed input.

---

## 7. Integration Contracts

- apps/: Primary host.
- engine/nnos: Validation runtime module links against JStar emitted code for guard checks.
- pitchfork/agent/inference: Calls tokenize_jstar + agent_inference_run via FFI. Expects TokenVector stream as input.

---

**End of Technical Specification**

This completes the full triad for the Jasterish Compiler Self-Hosting Pipeline (apps/). All three documents now exist and should be referenced from PROJECT_SUMMARY.md and TODO.md.