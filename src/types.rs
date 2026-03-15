//! Core algebraic data types for the morphlex pipeline.
//!
//! Modeled after Clang's internal representations but adapted for natural language.
//! Uses Haskell/F#-style sum types (Rust enums) and product types (structs).

use serde::{Deserialize, Serialize};

// ─── Lexer Output ────────────────────────────────────────────────────────────

/// A position in the source text (Clang-style source location).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Raw token produced by the lexer — analogous to clang::Token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

/// Sum type for token classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    Word,
    Number,
    Punctuation,
    Whitespace,
    Contraction,  // e.g., "don't", "I'm"
    Hyphenated,   // e.g., "well-known"
    Unknown,
}

// ─── Morphological Analysis Output ───────────────────────────────────────────

/// A morpheme — the smallest meaningful unit of language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Morpheme {
    Prefix(String),   // un-, re-, pre-, dis-
    Root(String),     // the base form
    Suffix(String),   // -ness, -ing, -ed, -ly
    Infix(String),    // rare in English
}

/// Result of morphological decomposition for a single token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MorphAnalysis {
    pub original: Token,
    pub morphemes: Vec<Morpheme>,
    pub lemma: String,  // dictionary base form
}

// ─── AST Types ───────────────────────────────────────────────────────────────

/// Part of speech — the "type system" of natural language.
/// Analogous to clang::Type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartOfSpeech {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Pronoun,
    Preposition,
    Conjunction,
    Determiner,
    Interjection,
    Particle,
}

/// AST node for a single word — analogous to clang::Expr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordNode {
    pub analysis: MorphAnalysis,
    pub pos: PartOfSpeech,
}

/// Phrase type — groups of words that function as a unit.
/// Analogous to clang::Stmt grouping expressions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhraseKind {
    NounPhrase,
    VerbPhrase,
    AdjectivalPhrase,
    AdverbialPhrase,
    PrepositionalPhrase,
}

/// A phrase node in the AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhraseNode {
    pub kind: PhraseKind,
    pub children: Vec<AstNode>,
}

/// The AST — a recursive sum type (Haskell-style).
/// This is the central data structure, analogous to clang::Decl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstNode {
    Word(WordNode),
    Phrase(PhraseNode),
    Sentence(Vec<AstNode>),
    Document(Vec<AstNode>),
}

// ─── Semantic Analysis Output ────────────────────────────────────────────────

/// Semantic role — what function a word/phrase serves in meaning.
/// Analogous to Clang's semantic annotations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticRole {
    Agent,      // the doer
    Action,     // the verb/event
    Patient,    // the thing acted upon
    Instrument, // the means
    Location,   // where
    Temporal,   // when
    Modifier,   // descriptive
    Quantifier, // how many/much
    Connector,  // linking role
}

/// A semantically annotated AST node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticNode {
    pub ast: AstNode,
    pub role: SemanticRole,
}

// ─── Vector Output ───────────────────────────────────────────────────────────
//
// Mapped to Java's 8 primitives:
//   id:       int    (i32) — deterministic identity hash, the token's address
//   lemma_id: int    (i32) — dictionary index of the base form
//   pos:      byte   (i8)  — part of speech tag
//   role:     byte   (i8)  — semantic role tag
//   morph:    short  (i16) — packed morphological flags
//   length:   byte   (i8)  — character length of original word
//   syllables:byte   (i8)  — syllable count estimate
//
// Total: 12 bytes per token. The object IS the vector.
// Comparison is `==` on id (i32). No floats. No decode step.

/// Size of a TokenVector in bytes (packed primitives).
pub const TOKEN_VECTOR_SIZE: usize = 12;

/// A deterministic integer-packed token representation.
/// Each field maps to a Java primitive. The struct IS the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C, packed)]
pub struct TokenVector {
    pub id: i32,        // int    — deterministic BLAKE3-derived identity
    pub lemma_id: i32,  // int    — lemma dictionary index
    pub pos: i8,        // byte   — PartOfSpeech discriminant
    pub role: i8,       // byte   — SemanticRole discriminant
    pub morph: i16,     // short  — morphological flags (bitfield)
}

/// Morphological flag bits packed into the `morph` field (i16).
/// Each bit encodes a boolean property — no branching needed to check.
pub mod morph_flags {
    pub const HAS_PREFIX: i16    = 1 << 0;
    pub const HAS_SUFFIX: i16    = 1 << 1;
    pub const HAS_INFIX: i16     = 1 << 2;
    pub const IS_COMPOUND: i16   = 1 << 3;  // hyphenated
    pub const IS_CONTRACTION: i16 = 1 << 4;
    pub const IS_ROOT_ONLY: i16  = 1 << 5;  // bare root, no affixes
    pub const MULTI_ROOT: i16    = 1 << 6;  // compound with multiple roots
    pub const PREFIX_NEG: i16    = 1 << 7;  // negation prefix (un-, dis-, in-)
    pub const PREFIX_REP: i16    = 1 << 8;  // repetition prefix (re-)
    pub const SUFFIX_NOUN: i16   = 1 << 9;  // nominalizing suffix (-ness, -ment)
    pub const SUFFIX_VERB: i16   = 1 << 10; // verbalizing suffix (-ize, -ify)
    pub const SUFFIX_ADJ: i16    = 1 << 11; // adjectival suffix (-able, -ful)
    pub const SUFFIX_ADV: i16    = 1 << 12; // adverbial suffix (-ly)
}

impl TokenVector {
    /// Pack the vector into a 12-byte array (wire format).
    pub fn to_bytes(&self) -> [u8; TOKEN_VECTOR_SIZE] {
        let mut buf = [0u8; TOKEN_VECTOR_SIZE];
        buf[0..4].copy_from_slice(&self.id.to_le_bytes());
        buf[4..8].copy_from_slice(&self.lemma_id.to_le_bytes());
        buf[8] = self.pos as u8;
        buf[9] = self.role as u8;
        buf[10..12].copy_from_slice(&self.morph.to_le_bytes());
        buf
    }

    /// Unpack from a 12-byte array.
    pub fn from_bytes(buf: &[u8; TOKEN_VECTOR_SIZE]) -> Self {
        TokenVector {
            id: i32::from_le_bytes(buf[0..4].try_into().unwrap()),
            lemma_id: i32::from_le_bytes(buf[4..8].try_into().unwrap()),
            pos: buf[8] as i8,
            role: buf[9] as i8,
            morph: i16::from_le_bytes(buf[10..12].try_into().unwrap()),
        }
    }

    /// Identity comparison — single int compare, O(1).
    pub fn same_token(&self, other: &TokenVector) -> bool {
        self.id == other.id
    }

    /// Returns the int identity. This is the token's address in the system.
    pub fn as_int(&self) -> i32 {
        self.id
    }
}

// ─── Recipe Types ────────────────────────────────────────────────────────────
//
// Moderne/OpenRewrite-style recipes for deterministic lexing transforms.
// A recipe is: Pattern → Guard → Transform. No ambiguity.

/// A lexing recipe — a deterministic pattern-match-and-transform rule.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: &'static str,
    pub pattern: RecipePattern,
    pub transform: RecipeTransform,
}

/// What the recipe matches against.
#[derive(Debug, Clone)]
pub enum RecipePattern {
    /// Match a specific suffix on the lemma
    Suffix(&'static str),
    /// Match a specific prefix on the lemma
    Prefix(&'static str),
    /// Match an exact lemma
    Exact(&'static str),
    /// Match by part of speech
    Pos(i8),
    /// Match any token (fallback)
    Any,
}

/// What the recipe produces.
#[derive(Debug, Clone)]
pub enum RecipeTransform {
    /// Set the POS tag
    SetPos(i8),
    /// Set morph flags (OR into existing)
    AddMorphFlags(i16),
    /// Override the lemma_id with a specific value
    SetLemmaId(i32),
    /// Composite: apply multiple transforms
    Chain(Vec<RecipeTransform>),
}

// ─── Search Engine Types ────────────────────────────────────────────────────

/// Unique document identifier — BLAKE3 hash of content truncated to i32.
pub type DocId = i32;

/// Word position within a document (0-based index).
pub type WordPos = u32;

/// A single hit in the inverted index: where a token was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocHit {
    pub doc_id: DocId,
    pub position: WordPos,
    pub pos: i8,
    pub role: i8,
    pub morph: i16,
    pub id: i32, // exact lexeme hash (for exact-match bonus)
}

/// How query terms combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// All query terms must appear (intersection)
    All,
    /// Any query term may appear (union)
    Any,
}

/// Filter constraints on search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilter {
    pub pos: Option<i8>,
    pub role: Option<i8>,
    pub morph_mask: Option<i16>,
}

/// Configuration for a search query.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub mode: QueryMode,
    pub filter: SearchFilter,
    pub max_results: usize,
}

/// A single document's search result with integer score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub doc_id: DocId,
    pub score: i32,
    pub matched_positions: Vec<WordPos>,
    pub matched_lemmas: Vec<i32>,
}

/// Metadata stored for each indexed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocMeta {
    pub doc_id: DocId,
    pub word_count: u32,
    pub title: String,
}

// ─── Error Types ─────────────────────────────────────────────────────────────

/// Monadic error type — all pipeline errors flow through this.
/// Pattern: Result<T, MorphlexError> used like Either/Result in Haskell.
#[derive(Debug, thiserror::Error)]
pub enum MorphlexError {
    #[error("Lexer error at position {position}: {message}")]
    LexError { position: usize, message: String },

    #[error("Morphological analysis failed for '{token}': {message}")]
    MorphError { token: String, message: String },

    #[error("AST construction failed: {0}")]
    AstError(String),

    #[error("Semantic analysis failed: {0}")]
    SemanticError(String),

    #[error("Vectorization failed for '{token}': {message}")]
    VectorError { token: String, message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Crawler error: {0}")]
    CrawlError(String),

    #[error("Codegen error: {0}")]
    CodegenError(String),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Pipeline result type — the monadic container.
pub type MorphResult<T> = Result<T, MorphlexError>;
