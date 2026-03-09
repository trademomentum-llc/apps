//! morphlex — Deterministic natural language tokenizer and vector compiler.
//!
//! Pipeline: Lexer → Morphology → AST → Semantics → Vectorizer → Database
//!
//! Architecture modeled after Clang's compilation phases.
//! Logic follows Haskell/F# patterns: algebraic data types, pattern matching,
//! pure functions, monadic error handling.
//!
//! Token vectors are 12-byte integer-packed objects (no floats).
//! Identity is a single i32. Comparison is `==`.

pub mod ast;
pub mod database;
pub mod lexer;
pub mod morphology;
pub mod semantics;
pub mod types;
pub mod vectorizer;
pub mod jstar;
pub mod jsh;

use std::path::Path;
use types::*;

/// Run the full pipeline on input text. Returns (lemmas, vectors).
pub fn compile(input: &str) -> MorphResult<(Vec<String>, Vec<TokenVector>)> {
    let tokens = lexer::lex(input)?;
    let morphs = morphology::analyze(&tokens)?;
    let tree = ast::build(&morphs)?;
    let semnodes = semantics::annotate(&tree)?;
    let vectors = vectorizer::vectorize(&semnodes)?;

    // Extract lemmas parallel to vectors
    let lemmas: Vec<String> = extract_lemmas_from_ast(&semnodes);

    Ok((lemmas, vectors))
}

/// Extract lemmas from semantic nodes in word-level order.
fn extract_lemmas_from_ast(nodes: &[SemanticNode]) -> Vec<String> {
    let mut lemmas = Vec::new();
    for node in nodes {
        collect_word_lemmas(&node.ast, &mut lemmas);
    }
    lemmas
}

fn collect_word_lemmas(node: &AstNode, lemmas: &mut Vec<String>) {
    match node {
        AstNode::Word(w) => lemmas.push(w.analysis.lemma.clone()),
        AstNode::Phrase(p) => {
            for child in &p.children {
                collect_word_lemmas(child, lemmas);
            }
        }
        AstNode::Sentence(s) => {
            for child in s {
                collect_word_lemmas(child, lemmas);
            }
        }
        AstNode::Document(d) => {
            for child in d {
                collect_word_lemmas(child, lemmas);
            }
        }
    }
}

/// Run the full pipeline and write to a PQC-encrypted database.
/// Returns the PQC key bundle.
pub fn compile_to_database(
    input: &str,
    db_path: &Path,
    encrypted_path: &Path,
) -> MorphResult<database::PqcKeyBundle> {
    let (lemmas, vectors) = compile(input)?;
    database::write_database(&vectors, &lemmas, db_path)?;
    database::compact(db_path)?;
    database::encrypt(db_path, encrypted_path)
}

/// Compile a word list (one word per line) into the database.
pub fn compile_lexicon(
    words: &[String],
    db_path: &Path,
    encrypted_path: &Path,
) -> MorphResult<database::PqcKeyBundle> {
    let mut all_vectors = Vec::new();
    let mut all_lemmas = Vec::new();

    for word in words {
        match compile(word) {
            Ok((lemmas, vectors)) => {
                all_lemmas.extend(lemmas);
                all_vectors.extend(vectors);
            }
            Err(e) => {
                eprintln!("Warning: skipping '{}': {}", word, e);
            }
        }
    }

    // Deduplicate by id (deterministic — same word always gets same id)
    let mut seen = std::collections::HashSet::new();
    let mut deduped_vectors = Vec::new();
    let mut deduped_lemmas = Vec::new();
    for (tv, lemma) in all_vectors.into_iter().zip(all_lemmas.into_iter()) {
        if seen.insert(tv.id) {
            deduped_vectors.push(tv);
            deduped_lemmas.push(lemma);
        }
    }

    // Sort by id for binary search lookups
    let mut paired: Vec<_> = deduped_vectors
        .into_iter()
        .zip(deduped_lemmas.into_iter())
        .collect();
    paired.sort_by_key(|(tv, _)| tv.id);
    let (sorted_vectors, sorted_lemmas): (Vec<_>, Vec<_>) = paired.into_iter().unzip();

    database::write_database(&sorted_vectors, &sorted_lemmas, db_path)?;
    let size = database::compact(db_path)?;
    eprintln!(
        "Database compiled: {} vectors, {} bytes ({} bytes/vector)",
        sorted_vectors.len(),
        size,
        TOKEN_VECTOR_SIZE,
    );

    database::encrypt(db_path, encrypted_path)
}
