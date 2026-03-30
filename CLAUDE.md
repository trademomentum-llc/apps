# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ABSOLUTE RULES

- **NEVER use curly/smart quotes in shell commands, code, or flag names.** Only straight ASCII quotes (' and "). Compromised characters in commands = compromised code. Zero tolerance.
- **NEVER chain unrelated commands with &&.** Run them separately.

## Project: morphlex

Deterministic natural language tokenizer and vector compiler. Processes English through a Clang-style compilation pipeline using Haskell/F#-style functional patterns (algebraic data types, pattern matching, monadic error handling).

Token vectors are 12-byte integer-packed objects mapped to Java's 8 primitives. No floats. Identity is a single i32. Comparison is ==.

## Build & Test Commands

```bash
cargo build              # dev build
cargo build --release     # optimized build (LTO + strip)
cargo test                # run all 29 unit tests
cargo test lexer          # run tests for a specific module
cargo test -- --nocapture # show println output in tests
cargo run -- tokenize "some text"   # quick pipeline test
cargo run -- compile-dict -o out.db.enc  # compile system dictionary
```

## Architecture

The pipeline mirrors Clang's compilation phases, adapted for NLP:

```
Raw Text -> Lexer -> Morphology -> AST -> Semantics -> Vectorizer -> Database -> Compact -> Encrypt
```

### Pipeline stages (src/)

| File | Phase | Clang Analogue | Input -> Output |
|------|-------|----------------|-----------------|
| `lexer.rs` | 1 | Lexer | `&str -> Vec<Token>` |
| `morphology.rs` | 2 | Preprocessor | `Vec<Token> -> Vec<MorphAnalysis>` |
| `ast.rs` | 3 | Parser | `Vec<MorphAnalysis> -> AstNode` |
| `semantics.rs` | 4 | Sema | `AstNode -> Vec<SemanticNode>` |
| `vectorizer.rs` | 5 | CodeGen | `Vec<SemanticNode> -> Vec<TokenVector>` |
| `database.rs` | 6-8 | Object file | Write -> Compact -> Encrypt |

### Core types (`types.rs`)

All data types are algebraic (Rust enums = Haskell/F# sum types):
- `TokenKind` -- lexer token classification
- `Morpheme` -- prefix/root/suffix/infix
- `PartOfSpeech` -- 10 POS tags
- `AstNode` -- recursive tree (Word | Phrase | Sentence | Document)
- `SemanticRole` -- 9 semantic roles (Agent, Action, Patient, etc.)
- `TokenVector` -- 12-byte integer-packed object (i32 id, i32 lemma_id, i8 pos, i8 role, i16 morph)
- `Recipe` / `RecipePattern` / `RecipeTransform` -- Moderne-style deterministic transform rules
- `MorphlexError` / `MorphResult<T>` -- monadic error type used throughout

### TokenVector layout (12 bytes, mapped to Java primitives)

| Offset | Size | Java Type | Rust Type | Field |
|--------|------|-----------|-----------|-------|
| 0 | 4 | int | i32 | id -- BLAKE3 hash of lexeme, truncated to i32 |
| 4 | 4 | int | i32 | lemma_id -- BLAKE3 hash of base form |
| 8 | 1 | byte | i8 | pos -- part of speech discriminant |
| 9 | 1 | byte | i8 | role -- semantic role discriminant |
| 10 | 2 | short | i16 | morph -- bitfield of morphological flags |

### Morph flags (i16 bitfield)

```
bit 0:  HAS_PREFIX       bit 7:  PREFIX_NEG
bit 1:  HAS_SUFFIX       bit 8:  PREFIX_REP
bit 2:  HAS_INFIX        bit 9:  SUFFIX_NOUN
bit 3:  IS_COMPOUND       bit 10: SUFFIX_VERB
bit 4:  IS_CONTRACTION    bit 11: SUFFIX_ADJ
bit 5:  IS_ROOT_ONLY      bit 12: SUFFIX_ADV
bit 6:  MULTI_ROOT
```

### Database binary format (v3)

```
[Header: 24 bytes]
  magic: "MORPHLEX" (8B)
  version: u32 (= 3)
  entry_count: u64
  flags: u32

[Lemma Table: variable]
  per entry: lemma_len (u16) + lemma bytes

[Vector Table: entry_count * 12 bytes]
  per entry: TokenVector as 12 packed bytes
```

### PQC encryption (three NIST FIPS standards)

All cryptography is post-quantum. NIST standard or better. No exceptions.

| Standard | Algorithm | Purpose |
|----------|-----------|---------|
| FIPS 203 | ML-KEM-1024 | Key encapsulation (quantum-resistant) |
| FIPS 204 | ML-DSA-65 | Digital signatures (tamper detection) |
| FIPS 197 + SP 800-38D | AES-256-GCM | Symmetric encryption |

Encrypted output format:
```
.db.enc file: [ML-KEM-1024 ciphertext: 1568B][AES-GCM nonce: 12B][AES-GCM ciphertext]
.sig file:    ML-DSA-65 signature over the .db.enc contents
.keys/ dir:   dk.bin (64B seed), sk.bin (32B seed), vk.bin (encoded verifying key)
```

### Recipe engine (Moderne/OpenRewrite style)

Recipes are deterministic pattern-match-and-transform rules:
- `RecipePattern` -- Suffix, Prefix, Exact, Pos, Any
- `RecipeTransform` -- SetPos, AddMorphFlags, SetLemmaId, Chain
- First match wins. Composable. No ambiguity.

## Key Design Principles

- **Deterministic**: Same input always produces the same output. No randomness in the pipeline.
- **Word-level vectorization**: Every word gets its own 12-byte vector. No phrase-level collapsing.
- **Int return**: Token identity is a single i32. Comparison is ==. No floats. No FPU.
- **Pure functions**: Each pipeline stage is Input -> Output with no side effects.
- **Pattern matching over if/else**: All branching uses Rust match (Haskell/F# style).
- **Monadic errors**: All fallible operations return MorphResult<T>.

## Entry points

- `morphlex::compile(input)` -- full pipeline, returns (Vec<String>, Vec<TokenVector>)
- `morphlex::compile_lexicon(words, db_path, enc_path)` -- full pipeline to encrypted database
- CLI subcommands: `tokenize`, `compile`, `compile-dict`, `inspect`
