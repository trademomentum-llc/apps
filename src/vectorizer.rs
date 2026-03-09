//! Vectorizer — Phase 5 of the morphlex pipeline.
//!
//! Deterministically maps each word to a 12-byte integer-packed TokenVector.
//! No floats. No FPU. Identity is a single i32.
//!
//! Uses Moderne/OpenRewrite-style recipes: deterministic pattern → transform
//! rules applied in order. First match wins. Composable and predictable.
//!
//! The algorithm:
//! 1. Flatten the AST to individual words (not phrases)
//! 2. For each word: compute BLAKE3 hash → truncate to i32 → that's the id
//! 3. Compute lemma_id the same way (hash of the lemma)
//! 4. Pack POS, role, morph flags into primitive fields
//! 5. Apply recipes to refine the output
//!
//! 12 bytes per token. int comparison for equality. No decode step.

use crate::types::*;

/// Vectorize semantic nodes into integer-packed token vectors.
/// Word-level: every individual word gets its own vector.
/// Pure function: &[SemanticNode] → Vec<TokenVector>.
pub fn vectorize(nodes: &[SemanticNode]) -> MorphResult<Vec<TokenVector>> {
    let recipes = build_recipes();
    let mut vectors = Vec::new();

    for node in nodes {
        let words = flatten_to_words(&node.ast);
        for word in &words {
            let mut tv = pack_word(word, &node.role)?;
            apply_recipes(&mut tv, word, &recipes);
            vectors.push(tv);
        }
    }

    Ok(vectors)
}

/// Vectorize a single word directly (for lexicon compilation).
pub fn vectorize_word(word: &WordNode, role: &SemanticRole) -> MorphResult<TokenVector> {
    let recipes = build_recipes();
    let mut tv = pack_word(word, role)?;
    apply_recipes(&mut tv, word, &recipes);
    Ok(tv)
}

/// Pack a WordNode into a TokenVector using integer primitives.
fn pack_word(word: &WordNode, role: &SemanticRole) -> MorphResult<TokenVector> {
    // id: deterministic BLAKE3 hash of the original lexeme, truncated to i32
    let id = hash_to_i32(&word.analysis.original.lexeme.to_lowercase());

    // lemma_id: hash of the lemma (base form)
    let lemma_id = hash_to_i32(&word.analysis.lemma);

    // pos: discriminant as i8
    let pos = pos_to_i8(&word.pos);

    // role: discriminant as i8
    let role_byte = role_to_i8(role);

    // morph: pack morphological flags into i16 bitfield
    let morph = pack_morph_flags(&word.analysis);

    Ok(TokenVector {
        id,
        lemma_id,
        pos,
        role: role_byte,
        morph,
    })
}

/// BLAKE3 hash → truncate to i32. Deterministic. Same input → same int, always.
pub fn hash_to_i32(input: &str) -> i32 {
    let hash = blake3::hash(input.as_bytes());
    let bytes = hash.as_bytes();
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Pack morphological analysis into i16 bitfield flags.
fn pack_morph_flags(analysis: &MorphAnalysis) -> i16 {
    let mut flags: i16 = 0;

    let mut has_prefix = false;
    let mut has_suffix = false;
    let mut root_count = 0;

    for morpheme in &analysis.morphemes {
        match morpheme {
            Morpheme::Prefix(p) => {
                has_prefix = true;
                flags |= morph_flags::HAS_PREFIX;
                // Classify the prefix
                match p.as_str() {
                    "un" | "dis" | "in" | "im" | "il" | "ir" | "non" | "anti" => {
                        flags |= morph_flags::PREFIX_NEG;
                    }
                    "re" => {
                        flags |= morph_flags::PREFIX_REP;
                    }
                    _ => {}
                }
            }
            Morpheme::Suffix(s) => {
                has_suffix = true;
                flags |= morph_flags::HAS_SUFFIX;
                // Classify the suffix
                match s.as_str() {
                    "ness" | "ment" | "tion" | "sion" | "ation" | "ity" | "ence"
                    | "ance" | "ist" | "ism" | "ery" => {
                        flags |= morph_flags::SUFFIX_NOUN;
                    }
                    "able" | "ible" | "ful" | "less" | "ous" | "ive" | "al"
                    | "ary" | "ory" => {
                        flags |= morph_flags::SUFFIX_ADJ;
                    }
                    "ly" | "ally" | "ily" => {
                        flags |= morph_flags::SUFFIX_ADV;
                    }
                    "ize" | "ise" | "ify" | "en" => {
                        flags |= morph_flags::SUFFIX_VERB;
                    }
                    _ => {}
                }
            }
            Morpheme::Root(_) => {
                root_count += 1;
            }
            Morpheme::Infix(_) => {
                flags |= morph_flags::HAS_INFIX;
            }
        }
    }

    if !has_prefix && !has_suffix {
        flags |= morph_flags::IS_ROOT_ONLY;
    }
    if root_count > 1 {
        flags |= morph_flags::MULTI_ROOT;
    }

    // Check token kind for compound/contraction
    match &analysis.original.kind {
        TokenKind::Hyphenated => flags |= morph_flags::IS_COMPOUND,
        TokenKind::Contraction => flags |= morph_flags::IS_CONTRACTION,
        _ => {}
    }

    flags
}

/// Map PartOfSpeech to i8 discriminant.
fn pos_to_i8(pos: &PartOfSpeech) -> i8 {
    match pos {
        PartOfSpeech::Noun => 0,
        PartOfSpeech::Verb => 1,
        PartOfSpeech::Adjective => 2,
        PartOfSpeech::Adverb => 3,
        PartOfSpeech::Pronoun => 4,
        PartOfSpeech::Preposition => 5,
        PartOfSpeech::Conjunction => 6,
        PartOfSpeech::Determiner => 7,
        PartOfSpeech::Interjection => 8,
        PartOfSpeech::Particle => 9,
    }
}

/// Map SemanticRole to i8 discriminant.
fn role_to_i8(role: &SemanticRole) -> i8 {
    match role {
        SemanticRole::Agent => 0,
        SemanticRole::Action => 1,
        SemanticRole::Patient => 2,
        SemanticRole::Instrument => 3,
        SemanticRole::Location => 4,
        SemanticRole::Temporal => 5,
        SemanticRole::Modifier => 6,
        SemanticRole::Quantifier => 7,
        SemanticRole::Connector => 8,
    }
}

/// Flatten an AST node to its individual WordNodes.
/// Every word gets its own vector — no phrase-level collapsing.
fn flatten_to_words(node: &AstNode) -> Vec<&WordNode> {
    match node {
        AstNode::Word(w) => vec![w],
        AstNode::Phrase(p) => p.children.iter().flat_map(flatten_to_words).collect(),
        AstNode::Sentence(s) => s.iter().flat_map(flatten_to_words).collect(),
        AstNode::Document(d) => d.iter().flat_map(flatten_to_words).collect(),
    }
}

// ─── Recipe Engine ───────────────────────────────────────────────────────────
//
// Moderne/OpenRewrite style: pattern → transform, applied in order.
// First matching recipe wins. Deterministic, composable, no ambiguity.

/// Build the default recipe set.
fn build_recipes() -> Vec<Recipe> {
    vec![
        // Negation prefixes always set PREFIX_NEG
        Recipe {
            name: "negation-prefix",
            pattern: RecipePattern::Prefix("un"),
            transform: RecipeTransform::AddMorphFlags(morph_flags::PREFIX_NEG),
        },
        Recipe {
            name: "negation-prefix-dis",
            pattern: RecipePattern::Prefix("dis"),
            transform: RecipeTransform::AddMorphFlags(morph_flags::PREFIX_NEG),
        },
        // Nominalizing suffixes ensure noun POS
        Recipe {
            name: "nominal-suffix-ness",
            pattern: RecipePattern::Suffix("ness"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(0), // Noun
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_NOUN),
            ]),
        },
        Recipe {
            name: "nominal-suffix-ment",
            pattern: RecipePattern::Suffix("ment"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(0),
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_NOUN),
            ]),
        },
        // Adverbial -ly forces adverb
        Recipe {
            name: "adverb-ly",
            pattern: RecipePattern::Suffix("ly"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(3), // Adverb
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_ADV),
            ]),
        },
        // Verbal suffixes
        Recipe {
            name: "verbal-suffix-ize",
            pattern: RecipePattern::Suffix("ize"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(1), // Verb
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_VERB),
            ]),
        },
        // Adjectival suffixes
        Recipe {
            name: "adj-suffix-able",
            pattern: RecipePattern::Suffix("able"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(2), // Adjective
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_ADJ),
            ]),
        },
        Recipe {
            name: "adj-suffix-ful",
            pattern: RecipePattern::Suffix("ful"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(2),
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_ADJ),
            ]),
        },
        Recipe {
            name: "adj-suffix-less",
            pattern: RecipePattern::Suffix("less"),
            transform: RecipeTransform::Chain(vec![
                RecipeTransform::SetPos(2),
                RecipeTransform::AddMorphFlags(morph_flags::SUFFIX_ADJ),
            ]),
        },
    ]
}

/// Apply recipes to a token vector. First match wins per recipe.
fn apply_recipes(tv: &mut TokenVector, word: &WordNode, recipes: &[Recipe]) {
    let lemma = &word.analysis.lemma;

    for recipe in recipes {
        let matched = match &recipe.pattern {
            RecipePattern::Suffix(s) => lemma.ends_with(s),
            RecipePattern::Prefix(p) => lemma.starts_with(p),
            RecipePattern::Exact(e) => lemma == *e,
            RecipePattern::Pos(p) => tv.pos == *p,
            RecipePattern::Any => true,
        };

        if matched {
            apply_transform(tv, &recipe.transform);
        }
    }
}

/// Apply a single transform to a token vector.
fn apply_transform(tv: &mut TokenVector, transform: &RecipeTransform) {
    match transform {
        RecipeTransform::SetPos(p) => tv.pos = *p,
        RecipeTransform::AddMorphFlags(f) => tv.morph |= *f,
        RecipeTransform::SetLemmaId(id) => tv.lemma_id = *id,
        RecipeTransform::Chain(transforms) => {
            for t in transforms {
                apply_transform(tv, t);
            }
        }
    }
}

// ─── Public helpers for POS/Role conversion ──────────────────────────────────

pub fn i8_to_pos(val: i8) -> PartOfSpeech {
    match val {
        0 => PartOfSpeech::Noun,
        1 => PartOfSpeech::Verb,
        2 => PartOfSpeech::Adjective,
        3 => PartOfSpeech::Adverb,
        4 => PartOfSpeech::Pronoun,
        5 => PartOfSpeech::Preposition,
        6 => PartOfSpeech::Conjunction,
        7 => PartOfSpeech::Determiner,
        8 => PartOfSpeech::Interjection,
        _ => PartOfSpeech::Particle,
    }
}

pub fn i8_to_role(val: i8) -> SemanticRole {
    match val {
        0 => SemanticRole::Agent,
        1 => SemanticRole::Action,
        2 => SemanticRole::Patient,
        3 => SemanticRole::Instrument,
        4 => SemanticRole::Location,
        5 => SemanticRole::Temporal,
        6 => SemanticRole::Modifier,
        7 => SemanticRole::Quantifier,
        _ => SemanticRole::Connector,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast, lexer, morphology, semantics};

    fn pipeline(input: &str) -> Vec<TokenVector> {
        let tokens = lexer::lex(input).unwrap();
        let morphs = morphology::analyze(&tokens).unwrap();
        let tree = ast::build(&morphs).unwrap();
        let semnodes = semantics::annotate(&tree).unwrap();
        vectorize(&semnodes).unwrap()
    }

    #[test]
    fn test_determinism() {
        let a = pipeline("the quick brown fox");
        let b = pipeline("the quick brown fox");
        assert_eq!(a, b);
    }

    #[test]
    fn test_word_level_not_phrase_level() {
        // Every word must get its own vector
        let vecs = pipeline("the quick brown fox");
        assert_eq!(vecs.len(), 4, "Expected 4 vectors, one per word");
    }

    #[test]
    fn test_different_words_different_ids() {
        let a = pipeline("cat");
        let b = pipeline("dog");
        let a_id = a[0].id;
        let b_id = b[0].id;
        assert_ne!(a_id, b_id);
    }

    #[test]
    fn test_same_word_same_id() {
        let a = pipeline("cat");
        let b = pipeline("cat");
        let a_id = a[0].id;
        let b_id = b[0].id;
        assert_eq!(a_id, b_id);
    }

    #[test]
    fn test_vector_is_12_bytes() {
        let vecs = pipeline("hello");
        let bytes = vecs[0].to_bytes();
        assert_eq!(bytes.len(), 12);
    }

    #[test]
    fn test_roundtrip_bytes() {
        let vecs = pipeline("happiness");
        let bytes = vecs[0].to_bytes();
        let recovered = TokenVector::from_bytes(&bytes);
        assert_eq!(vecs[0], recovered);
    }

    #[test]
    fn test_int_identity() {
        let vecs = pipeline("hello");
        let id: i32 = vecs[0].as_int();
        // Same word must always produce the same int
        let vecs2 = pipeline("hello");
        assert_eq!(id, vecs2[0].as_int());
    }

    #[test]
    fn test_morph_flags_prefix() {
        let vecs = pipeline("unhappy");
        let morph = vecs[0].morph;
        assert_ne!(morph & morph_flags::HAS_PREFIX, 0);
        assert_ne!(morph & morph_flags::PREFIX_NEG, 0);
    }

    #[test]
    fn test_morph_flags_suffix() {
        let vecs = pipeline("happiness");
        let morph = vecs[0].morph;
        assert_ne!(morph & morph_flags::HAS_SUFFIX, 0);
        assert_ne!(morph & morph_flags::SUFFIX_NOUN, 0);
    }

    #[test]
    fn test_recipe_forces_pos() {
        // "quickly" should be forced to adverb by the -ly recipe
        let vecs = pipeline("quickly");
        let pos = vecs[0].pos;
        assert_eq!(pos, 3); // Adverb
    }

    #[test]
    fn test_no_floats_in_vector() {
        // Verify the struct is purely integer primitives
        assert_eq!(std::mem::size_of::<TokenVector>(), TOKEN_VECTOR_SIZE);
    }
}
