# Requirements Specification: Jasterish (JStar) Compiler Self-Hosting Pipeline

**Document ID:** JSTAR-REQ-001  
**Version:** 1.0.0  
**Date:** 2026-05-28  
**Projects:** apps/ (morphlex + Jasterish), engine/nnos (consumer), pitchfork (agent/ consumer)  
**Highest-Value Context:** The deterministic foundation shared across nnos, apps, and pitchfork agent layer.

---

## 1. Purpose

Jasterish is a system-level machine language in which natural language tokens (from the morphlex deterministic NLP pipeline) **are** the instruction set. Verbs map to operations, nouns to data, adjectives to type modifiers, prepositions to addressing modes.

The compiler must be self-hosting:
- Phase 0–1: Rust bootstrap (jstar2) produces a working binary.
- Later phases: The compiler written in JStar (jstar3) compiles itself.

This Requirements Specification defines the mandatory properties of the self-hosting pipeline.

---

## 2. Core Functional Requirements

FR-001: The tokenizer must accept English text and numbers, run words through morphlex (morphology → AST → semantics → 12-byte TokenVector), and synthesize literal TokenVectors for numbers/strings while preserving original order.

FR-002: The instruction set must be derived directly from TokenVector fields (POS, semantic role, morph flags). No separate opcode table.

FR-003: The compiler must emit direct x86-64 machine code (no LLVM, no Cranelift) using the System V AMD64 ABI.

FR-004: The full pipeline (tokenize → parse → typecheck → IR → codegen → link) must produce a valid ELF-64 executable.

FR-005: Self-hosting T-Diagram must be achievable:
- jstar2 (Rust) compiles jstar3 source → produces jstar3 binary.
- jstar3 must be able to recompile its own source and produce an identical (or proven-equivalent) binary.

FR-006: All non-deterministic behavior is forbidden in the compiler core. Fixed input must produce bitwise-identical output (modulo intentional nonce for encryption layers).

---

## 3. Non-Functional Requirements

- **Determinism (mathematical)**: For any fixed input source + fixed morphlex database, the emitted binary must be identical across runs. Measured via data/text hash stability in T-Diagram.
- **Size efficiency**: Self-hosted binary target < 100 KB (current jstar2 is ~3.9 MB; jstar3 target 68 KB class).
- **Correctness**: Type system (Java 8 primitives mapping) must be enforced at compile time. No runtime type checks in emitted code.
- **Neuroscience alignment**: Token vectors as neural substrate (POS/role/morph as distributed representation). Compiler as a model of deterministic language-to-action mapping in prefrontal/basal ganglia circuits.

---

## 4. Self-Hosting Phase Requirements (T-Diagram Stabilization)

| Phase | Status (as of analysis) | Requirement |
|-------|-------------------------|-------------|
| Tokenization | DONE | morphlex + literal synthesis stable |
| Parsing | 90% | Grammar must accept full JStar surface syntax |
| Typecheck/IR | Skipped (direct codegen) | Direct lowering accepted if proven sound |
| Codegen | 85% | x86-64 emission with correct fixups |
| ELF Linking | DONE | Produces runnable binaries |
| Self-host validation | Partial | jstar3 must recompile its own source |

---

## 5. Integration Requirements (Consumers)

- apps/ (this repo): Primary development and self-hosting target.
- engine/nnos: Jasterish ports of validation layer and morph engine (see nnos specs).
- pitchfork/agent/: Inference runtime (SP2) and proof validator (SP3) depend on Morphlex + JStar token vectors for deterministic, proof-carrying code suggestions inside Forgejo.

Any change to the TokenVector format or codegen ABI is a breaking change for all three.

---

## 6. Verification Requirements

- 29+ unit tests (cargo test) must pass on every change.
- T-Diagram hash stability checks (data hash stable, text hash divergence analyzed).
- Bootstrap check script (`apps/scripts/jstar_bootstrap_check.sh`) must report green before any self-host promotion.
- Generated binaries must be runnable on target Linux (x86-64, glibc or musl as declared).

---

**End of Requirements Specification**

Design Specification (grammar, AST nodes, lowering strategy, register allocation, ELF emission details) and Technical Specification (exact TokenVector layout, x86 encoding tables, ABI contracts, self-host validation harness) follow in the triad.