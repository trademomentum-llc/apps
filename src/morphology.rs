//! Morphological Analyzer — Phase 2 of the morphlex pipeline.
//!
//! Decomposes tokens into morphemes (prefix, root, suffix).
//! Analogous to Clang's preprocessing + early parse phase.
//! Uses pattern matching (F#/Haskell style) over known affixes.

use crate::types::*;

/// Known English prefixes, ordered longest-first for greedy matching.
const PREFIXES: &[&str] = &[
    "anti", "auto", "counter", "de", "dis", "down", "extra", "fore",
    "hyper", "il", "im", "in", "inter", "ir", "mega", "mid", "mis",
    "mono", "multi", "non", "out", "over", "poly", "post", "pre",
    "pro", "re", "semi", "sub", "super", "trans", "tri", "ultra",
    "un", "under", "up",
];

/// Known English suffixes, ordered longest-first for greedy matching.
const SUFFIXES: &[&str] = &[
    "ation", "ment", "ness", "able", "ible", "ful", "less", "ous",
    "ive", "tion", "sion", "ing", "ings", "ence", "ance", "ity",
    "ist", "ism", "ize", "ise", "ify", "ary", "ory", "ery",
    "ally", "ily", "ly", "er", "or", "ed", "en", "es", "al", "s",
];

/// Minimum root length — we won't strip affixes if the remaining root
/// would be shorter than this.
const MIN_ROOT_LEN: usize = 3;


/// Analyze a stream of tokens into morphological decompositions.
/// Pure function: Vec<Token> → Vec<MorphAnalysis>.
pub fn analyze(tokens: &[Token]) -> MorphResult<Vec<MorphAnalysis>> {
    tokens
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Word | TokenKind::Contraction | TokenKind::Hyphenated))
        .map(|token| analyze_token(token))
        .collect()
}

/// Decompose a single token into morphemes using pattern matching.
fn analyze_token(token: &Token) -> MorphResult<MorphAnalysis> {
    let word = token.lexeme.to_lowercase();

    // Pattern match on token kind — Haskell-style case analysis
    match &token.kind {
        TokenKind::Contraction => analyze_contraction(token, &word),
        TokenKind::Hyphenated => analyze_hyphenated(token, &word),
        _ => analyze_simple_word(token, &word),
    }
}

/// Analyze a simple word by stripping known prefixes and suffixes.
fn analyze_simple_word(token: &Token, word: &str) -> MorphResult<MorphAnalysis> {
    let mut morphemes = Vec::new();
    let mut remaining = word.to_string();

    // Strip prefix (greedy, longest match first)
    let prefix = strip_prefix(&remaining);
    if let Some((pfx, rest)) = prefix {
        morphemes.push(Morpheme::Prefix(pfx.to_string()));
        remaining = rest.to_string();
    }

    // Strip suffix (greedy, longest match first)
    let suffix = strip_suffix(&remaining);
    if let Some((sfx, rest)) = suffix {
        // Root is what remains
        morphemes.push(Morpheme::Root(rest.to_string()));
        morphemes.push(Morpheme::Suffix(sfx.to_string()));
    } else {
        // No suffix found — entire remaining is root
        morphemes.push(Morpheme::Root(remaining.clone()));
    }

    // Lemma: the root morpheme (simplified — a full implementation would
    // use a dictionary lookup)
    let lemma = morphemes
        .iter()
        .find_map(|m| match m {
            Morpheme::Root(r) => Some(r.clone()),
            _ => None,
        })
        .unwrap_or_else(|| word.to_string());

    Ok(MorphAnalysis {
        original: token.clone(),
        morphemes,
        lemma,
    })
}

/// Analyze a contraction by splitting at the apostrophe.
fn analyze_contraction(token: &Token, word: &str) -> MorphResult<MorphAnalysis> {
    let parts: Vec<&str> = word.split('\'').collect();
    let mut morphemes = Vec::new();

    if let Some(base) = parts.first() {
        morphemes.push(Morpheme::Root(base.to_string()));
    }
    if let Some(clitic) = parts.get(1) {
        morphemes.push(Morpheme::Suffix(format!("'{clitic}")));
    }

    let lemma = parts.first().unwrap_or(&word).to_string();

    Ok(MorphAnalysis {
        original: token.clone(),
        morphemes,
        lemma,
    })
}

/// Analyze a hyphenated compound by splitting at hyphens.
fn analyze_hyphenated(token: &Token, word: &str) -> MorphResult<MorphAnalysis> {
    let parts: Vec<&str> = word.split('-').collect();
    let morphemes: Vec<Morpheme> = parts
        .iter()
        .map(|part| Morpheme::Root(part.to_string()))
        .collect();

    let lemma = word.replace('-', "");

    Ok(MorphAnalysis {
        original: token.clone(),
        morphemes,
        lemma,
    })
}

/// Try to strip a known prefix. Returns (prefix, remainder) or None.
fn strip_prefix(word: &str) -> Option<(&'static str, &str)> {
    // Sort by length descending to match greedily
    let mut sorted: Vec<&&str> = PREFIXES.iter().collect();
    sorted.sort_by(|a, b| b.len().cmp(&a.len()));

    for prefix in sorted {
        if word.starts_with(*prefix) {
            let rest = &word[prefix.len()..];
            if rest.len() >= MIN_ROOT_LEN {
                return Some((prefix, rest));
            }
        }
    }
    None
}

/// Try to strip a known suffix. Returns (suffix, remainder) or None.
fn strip_suffix(word: &str) -> Option<(&'static str, &str)> {
    let mut sorted: Vec<&&str> = SUFFIXES.iter().collect();
    sorted.sort_by(|a, b| b.len().cmp(&a.len()));

    for suffix in sorted {
        if word.ends_with(*suffix) {
            let rest = &word[..word.len() - suffix.len()];
            if rest.len() >= MIN_ROOT_LEN {
                return Some((suffix, rest));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    #[test]
    fn test_prefix_detection() {
        let tokens = lexer::lex("unhappy").unwrap();
        let analysis = analyze(&tokens).unwrap();
        assert_eq!(analysis.len(), 1);

        let morphemes = &analysis[0].morphemes;
        assert!(morphemes.iter().any(|m| matches!(m, Morpheme::Prefix(p) if p == "un")));
    }

    #[test]
    fn test_suffix_detection() {
        let tokens = lexer::lex("happiness").unwrap();
        let analysis = analyze(&tokens).unwrap();

        let morphemes = &analysis[0].morphemes;
        assert!(morphemes.iter().any(|m| matches!(m, Morpheme::Suffix(s) if s == "ness")));
    }

    #[test]
    fn test_contraction_split() {
        let tokens = lexer::lex("don't").unwrap();
        let analysis = analyze(&tokens).unwrap();
        assert_eq!(analysis[0].lemma, "don");
        assert_eq!(analysis[0].morphemes.len(), 2);
    }

    #[test]
    fn test_hyphenated_split() {
        let tokens = lexer::lex("well-known").unwrap();
        let analysis = analyze(&tokens).unwrap();
        assert_eq!(analysis[0].morphemes.len(), 2);
        assert!(matches!(&analysis[0].morphemes[0], Morpheme::Root(r) if r == "well"));
        assert!(matches!(&analysis[0].morphemes[1], Morpheme::Root(r) if r == "known"));
    }

    #[test]
    fn test_determinism() {
        let tokens = lexer::lex("unreasonable").unwrap();
        let a = analyze(&tokens).unwrap();
        let b = analyze(&tokens).unwrap();
        assert_eq!(a, b);
    }

}
