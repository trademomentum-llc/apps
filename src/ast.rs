//! AST Builder — Phase 3 of the morphlex pipeline.
//!
//! Constructs an Abstract Syntax Tree from morphological analysis.
//! Analogous to Clang's Parser producing clang::Decl / clang::Stmt nodes.
//! Uses algebraic data types and pattern matching (Haskell/F# style).

use crate::types::*;

/// Build an AST from morphological analyses.
/// Pure function: Vec<MorphAnalysis> → AstNode.
pub fn build(analyses: &[MorphAnalysis]) -> MorphResult<AstNode> {
    let word_nodes: Vec<AstNode> = analyses
        .iter()
        .map(|a| {
            let pos = infer_pos(a);
            AstNode::Word(WordNode {
                analysis: a.clone(),
                pos,
            })
        })
        .collect();

    // Group words into phrases using a simple deterministic algorithm.
    // This is a recursive descent approach, matching Clang's parser strategy.
    let phrases = group_into_phrases(&word_nodes)?;

    Ok(AstNode::Sentence(phrases))
}

/// Infer part of speech from morphological structure.
/// Pattern matching over morpheme patterns — the Haskell/F# approach.
fn infer_pos(analysis: &MorphAnalysis) -> PartOfSpeech {
    let suffixes: Vec<&str> = analysis
        .morphemes
        .iter()
        .filter_map(|m| match m {
            Morpheme::Suffix(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();

    // Pattern match on suffix patterns to determine POS
    // This mirrors how Haskell would use guards and pattern matching
    for suffix in &suffixes {
        match *suffix {
            // Noun suffixes
            "ness" | "ment" | "tion" | "sion" | "ation" | "ity" | "ence" | "ance"
            | "ist" | "ism" | "ery" => return PartOfSpeech::Noun,

            // Adjective suffixes
            "able" | "ible" | "ful" | "less" | "ous" | "ive" | "al" | "ary" | "ory" => {
                return PartOfSpeech::Adjective
            }

            // Adverb suffixes
            "ly" | "ally" | "ily" => return PartOfSpeech::Adverb,

            // Verb suffixes
            "ize" | "ise" | "ify" | "en" => return PartOfSpeech::Verb,

            // Verb inflections
            "ing" | "ed" => return PartOfSpeech::Verb,

            // Plural / possessive — likely noun
            "s" | "es" => return PartOfSpeech::Noun,

            // Comparative/superlative — adjective
            "er" => return PartOfSpeech::Adjective,

            _ => {}
        }
    }

    // Default heuristic for bare roots: check common function words
    let lemma = analysis.lemma.to_lowercase();
    match lemma.as_str() {
        // Determiners
        "the" | "a" | "an" | "this" | "that" | "these" | "those" | "my" | "your"
        | "his" | "her" | "its" | "our" | "their" | "some" | "any" | "no" | "every"
        | "each" | "all" | "both" | "few" | "many" | "much" | "several" => {
            PartOfSpeech::Determiner
        }

        // Pronouns
        "i" | "me" | "we" | "us" | "you" | "he" | "him" | "she" | "it" | "they"
        | "them" | "who" | "whom" | "what" | "which" | "myself" | "yourself" => {
            PartOfSpeech::Pronoun
        }

        // Prepositions
        "in" | "on" | "at" | "to" | "for" | "with" | "by" | "from" | "of" | "about"
        | "into" | "through" | "during" | "before" | "after" | "above" | "below"
        | "between" | "under" | "over" | "up" | "down" | "out" | "off" | "near" => {
            PartOfSpeech::Preposition
        }

        // Conjunctions
        "and" | "but" | "or" | "nor" | "so" | "yet" | "because" | "although"
        | "while" | "if" | "unless" | "until" | "since" | "whether" => {
            PartOfSpeech::Conjunction
        }

        // Common verbs (bare form)
        "is" | "am" | "are" | "was" | "were" | "be" | "been" | "being" | "have"
        | "has" | "had" | "do" | "does" | "did" | "will" | "would" | "shall"
        | "should" | "may" | "might" | "can" | "could" | "must" | "go" | "get"
        | "make" | "know" | "think" | "take" | "come" | "see" | "want" | "give"
        | "use" | "find" | "tell" | "say" | "said" => PartOfSpeech::Verb,

        // Interjections
        "oh" | "ah" | "wow" | "ouch" | "hey" | "hello" | "hi" | "yes"
        | "please" | "thanks" => PartOfSpeech::Interjection,

        // Particles
        "not" | "n't" => PartOfSpeech::Particle,

        // Default: assume noun (most open-class words are nouns)
        _ => PartOfSpeech::Noun,
    }
}

/// Group word nodes into phrases using deterministic rules.
/// This is a simplified recursive-descent phrase parser.
fn group_into_phrases(nodes: &[AstNode]) -> MorphResult<Vec<AstNode>> {
    let mut phrases: Vec<AstNode> = Vec::new();
    let mut i = 0;

    while i < nodes.len() {
        let (phrase, consumed) = match_phrase(&nodes[i..])?;
        phrases.push(phrase);
        i += consumed;
    }

    Ok(phrases)
}

/// Try to match a phrase starting at the given position.
/// Returns (phrase_node, number_of_nodes_consumed).
fn match_phrase(nodes: &[AstNode]) -> MorphResult<(AstNode, usize)> {
    if nodes.is_empty() {
        return Err(MorphlexError::AstError("Empty node list".to_string()));
    }

    let first_pos = get_pos(&nodes[0]);

    match first_pos {
        // Determiner/Adjective/Noun → try to build a noun phrase
        Some(PartOfSpeech::Determiner)
        | Some(PartOfSpeech::Adjective)
        | Some(PartOfSpeech::Noun)
        | Some(PartOfSpeech::Pronoun) => {
            let (children, consumed) = collect_noun_phrase(nodes);
            Ok((
                AstNode::Phrase(PhraseNode {
                    kind: PhraseKind::NounPhrase,
                    children,
                }),
                consumed,
            ))
        }

        // Verb → verb phrase
        Some(PartOfSpeech::Verb) => {
            let (children, consumed) = collect_verb_phrase(nodes);
            Ok((
                AstNode::Phrase(PhraseNode {
                    kind: PhraseKind::VerbPhrase,
                    children,
                }),
                consumed,
            ))
        }

        // Preposition → prepositional phrase
        Some(PartOfSpeech::Preposition) => {
            let (children, consumed) = collect_prepositional_phrase(nodes);
            Ok((
                AstNode::Phrase(PhraseNode {
                    kind: PhraseKind::PrepositionalPhrase,
                    children,
                }),
                consumed,
            ))
        }

        // Adverb → adverbial phrase
        Some(PartOfSpeech::Adverb) => Ok((
            AstNode::Phrase(PhraseNode {
                kind: PhraseKind::AdverbialPhrase,
                children: vec![nodes[0].clone()],
            }),
            1,
        )),

        // Anything else — wrap as single-word phrase
        _ => Ok((nodes[0].clone(), 1)),
    }
}

/// Collect a noun phrase: (Det)? (Adj)* Noun+
fn collect_noun_phrase(nodes: &[AstNode]) -> (Vec<AstNode>, usize) {
    let mut children = Vec::new();
    let mut i = 0;

    // Optional determiner
    if i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Determiner) {
        children.push(nodes[i].clone());
        i += 1;
    }

    // Zero or more adjectives
    while i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Adjective) {
        children.push(nodes[i].clone());
        i += 1;
    }

    // One or more nouns (compound nouns)
    while i < nodes.len()
        && matches!(
            get_pos(&nodes[i]),
            Some(PartOfSpeech::Noun) | Some(PartOfSpeech::Pronoun)
        )
    {
        children.push(nodes[i].clone());
        i += 1;
    }

    // Must consume at least one node
    if children.is_empty() {
        children.push(nodes[0].clone());
        i = 1;
    }

    (children, i)
}

/// Collect a verb phrase: Verb (Adv)* (NounPhrase)?
fn collect_verb_phrase(nodes: &[AstNode]) -> (Vec<AstNode>, usize) {
    let mut children = Vec::new();
    let mut i = 0;

    // The verb itself
    if i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Verb) {
        children.push(nodes[i].clone());
        i += 1;
    }

    // Optional adverbs
    while i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Adverb) {
        children.push(nodes[i].clone());
        i += 1;
    }

    // Optional particle (e.g., "not")
    if i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Particle) {
        children.push(nodes[i].clone());
        i += 1;
    }

    (children, i)
}

/// Collect a prepositional phrase: Prep (NounPhrase)
fn collect_prepositional_phrase(nodes: &[AstNode]) -> (Vec<AstNode>, usize) {
    let mut children = Vec::new();
    let mut i = 0;

    // The preposition
    if i < nodes.len() && get_pos(&nodes[i]) == Some(PartOfSpeech::Preposition) {
        children.push(nodes[i].clone());
        i += 1;
    }

    // Followed by potential noun phrase components
    while i < nodes.len()
        && matches!(
            get_pos(&nodes[i]),
            Some(PartOfSpeech::Determiner)
                | Some(PartOfSpeech::Adjective)
                | Some(PartOfSpeech::Noun)
                | Some(PartOfSpeech::Pronoun)
        )
    {
        children.push(nodes[i].clone());
        i += 1;
    }

    (children, i)
}

/// Extract PartOfSpeech from an AstNode.
fn get_pos(node: &AstNode) -> Option<PartOfSpeech> {
    match node {
        AstNode::Word(w) => Some(w.pos.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, morphology};

    fn pipeline(input: &str) -> AstNode {
        let tokens = lexer::lex(input).unwrap();
        let morphs = morphology::analyze(&tokens).unwrap();
        build(&morphs).unwrap()
    }

    #[test]
    fn test_simple_sentence() {
        let ast = pipeline("the big dog");
        match &ast {
            AstNode::Sentence(phrases) => {
                assert!(!phrases.is_empty());
                // Should produce at least one noun phrase
                assert!(phrases.iter().any(|p| matches!(
                    p,
                    AstNode::Phrase(PhraseNode {
                        kind: PhraseKind::NounPhrase,
                        ..
                    })
                )));
            }
            _ => panic!("Expected Sentence node"),
        }
    }

    #[test]
    fn test_determinism() {
        let a = pipeline("the quick brown fox");
        let b = pipeline("the quick brown fox");
        assert_eq!(a, b);
    }
}
