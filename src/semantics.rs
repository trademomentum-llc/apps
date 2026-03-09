//! Semantic Analyzer — Phase 4 of the morphlex pipeline.
//!
//! Annotates AST nodes with semantic roles (Agent, Action, Patient, etc.).
//! Analogous to Clang's Sema phase that performs type checking and
//! semantic validation on the AST.
//!
//! Uses pattern matching to assign roles based on phrase structure
//! and position — a deterministic, rule-based approach.

use crate::types::*;

/// Annotate an AST with semantic roles.
/// Pure function: AstNode → Vec<SemanticNode>.
pub fn annotate(ast: &AstNode) -> MorphResult<Vec<SemanticNode>> {
    match ast {
        AstNode::Sentence(phrases) => annotate_sentence(phrases),
        AstNode::Document(sentences) => {
            let mut all = Vec::new();
            for sentence in sentences {
                all.extend(annotate(sentence)?);
            }
            Ok(all)
        }
        // Single phrase or word — annotate in isolation
        node => Ok(vec![SemanticNode {
            ast: node.clone(),
            role: infer_isolated_role(node),
        }]),
    }
}

/// Annotate a sentence's phrases using positional and structural rules.
///
/// English sentence structure (SVO) gives us deterministic role assignment:
///   [NounPhrase] [VerbPhrase] [NounPhrase] → Agent, Action, Patient
///
/// This is the core semantic analysis — pattern matching over phrase sequences.
fn annotate_sentence(phrases: &[AstNode]) -> MorphResult<Vec<SemanticNode>> {
    let mut result = Vec::new();
    let mut found_verb = false;
    let mut noun_phrase_count_before_verb = 0;

    // First pass: locate the verb to establish SVO structure
    let verb_index = phrases.iter().position(|p| is_verb_phrase(p));

    for (i, phrase) in phrases.iter().enumerate() {
        let role = match phrase {
            AstNode::Phrase(PhraseNode { kind, .. }) => {
                match kind {
                    PhraseKind::NounPhrase => {
                        if let Some(vi) = verb_index {
                            if i < vi {
                                noun_phrase_count_before_verb += 1;
                                // First NP before verb = Agent
                                SemanticRole::Agent
                            } else {
                                // NP after verb = Patient
                                SemanticRole::Patient
                            }
                        } else {
                            // No verb found — default to Agent for first, Modifier for rest
                            if noun_phrase_count_before_verb == 0 {
                                noun_phrase_count_before_verb += 1;
                                SemanticRole::Agent
                            } else {
                                SemanticRole::Modifier
                            }
                        }
                    }
                    PhraseKind::VerbPhrase => {
                        found_verb = true;
                        SemanticRole::Action
                    }
                    PhraseKind::PrepositionalPhrase => {
                        // Determine if location, temporal, or instrument
                        infer_prepositional_role(phrase)
                    }
                    PhraseKind::AdverbialPhrase => SemanticRole::Modifier,
                    PhraseKind::AdjectivalPhrase => SemanticRole::Modifier,
                }
            }
            // Non-phrase nodes
            AstNode::Word(w) => match w.pos {
                PartOfSpeech::Verb => {
                    found_verb = true;
                    SemanticRole::Action
                }
                PartOfSpeech::Conjunction => SemanticRole::Connector,
                PartOfSpeech::Adverb => SemanticRole::Modifier,
                PartOfSpeech::Adjective => SemanticRole::Modifier,
                PartOfSpeech::Determiner => SemanticRole::Quantifier,
                PartOfSpeech::Interjection => SemanticRole::Modifier,
                _ => {
                    if !found_verb {
                        SemanticRole::Agent
                    } else {
                        SemanticRole::Patient
                    }
                }
            },
            _ => SemanticRole::Modifier,
        };

        result.push(SemanticNode {
            ast: phrase.clone(),
            role,
        });
    }

    Ok(result)
}

/// Check if a node is a verb phrase.
fn is_verb_phrase(node: &AstNode) -> bool {
    matches!(
        node,
        AstNode::Phrase(PhraseNode {
            kind: PhraseKind::VerbPhrase,
            ..
        }) | AstNode::Word(WordNode {
            pos: PartOfSpeech::Verb,
            ..
        })
    )
}

/// Infer semantic role of a prepositional phrase from its preposition.
fn infer_prepositional_role(node: &AstNode) -> SemanticRole {
    let prep_lemma = extract_first_lemma(node);

    match prep_lemma.as_deref() {
        // Spatial prepositions → Location
        Some("in" | "on" | "at" | "near" | "above" | "below" | "between"
             | "under" | "over" | "into" | "through") => SemanticRole::Location,

        // Temporal prepositions → Temporal
        Some("before" | "after" | "during" | "until" | "since") => SemanticRole::Temporal,

        // Instrumental prepositions → Instrument
        Some("with" | "by") => SemanticRole::Instrument,

        // Default
        _ => SemanticRole::Modifier,
    }
}

/// Infer a semantic role for a node in isolation (no sentence context).
fn infer_isolated_role(node: &AstNode) -> SemanticRole {
    match node {
        AstNode::Word(w) => match w.pos {
            PartOfSpeech::Noun | PartOfSpeech::Pronoun => SemanticRole::Agent,
            PartOfSpeech::Verb => SemanticRole::Action,
            PartOfSpeech::Adjective | PartOfSpeech::Adverb => SemanticRole::Modifier,
            PartOfSpeech::Determiner => SemanticRole::Quantifier,
            PartOfSpeech::Conjunction => SemanticRole::Connector,
            PartOfSpeech::Preposition => SemanticRole::Location,
            _ => SemanticRole::Modifier,
        },
        AstNode::Phrase(p) => match p.kind {
            PhraseKind::NounPhrase => SemanticRole::Agent,
            PhraseKind::VerbPhrase => SemanticRole::Action,
            PhraseKind::PrepositionalPhrase => SemanticRole::Location,
            _ => SemanticRole::Modifier,
        },
        _ => SemanticRole::Modifier,
    }
}

/// Extract the lemma of the first word in a node (for preposition detection).
fn extract_first_lemma(node: &AstNode) -> Option<String> {
    match node {
        AstNode::Word(w) => Some(w.analysis.lemma.clone()),
        AstNode::Phrase(p) => p.children.first().and_then(|c| extract_first_lemma(c)),
        AstNode::Sentence(s) => s.first().and_then(|c| extract_first_lemma(c)),
        AstNode::Document(d) => d.first().and_then(|c| extract_first_lemma(c)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast, lexer, morphology};

    fn pipeline(input: &str) -> Vec<SemanticNode> {
        let tokens = lexer::lex(input).unwrap();
        let morphs = morphology::analyze(&tokens).unwrap();
        let tree = ast::build(&morphs).unwrap();
        annotate(&tree).unwrap()
    }

    #[test]
    fn test_svo_roles() {
        let nodes = pipeline("the dog chased the cat");
        // Should have Agent (the dog), Action (chased), Patient (the cat)
        assert!(nodes.iter().any(|n| n.role == SemanticRole::Agent));
        assert!(nodes.iter().any(|n| n.role == SemanticRole::Action));
        assert!(nodes.iter().any(|n| n.role == SemanticRole::Patient));
    }

    #[test]
    fn test_determinism() {
        let a = pipeline("the cat sat on the mat");
        let b = pipeline("the cat sat on the mat");
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.role, y.role);
        }
    }
}
