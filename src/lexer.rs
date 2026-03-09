//! Lexer — Phase 1 of the morphlex pipeline.
//!
//! Analogous to Clang's Lexer: takes raw source text and produces a stream
//! of tokens. Pure function: &str → Vec<Token>.

use crate::types::*;

/// Lex raw text into a token stream. Pure, deterministic function.
pub fn lex(input: &str) -> MorphResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some(&(start, ch)) = chars.peek() {
        let token = match ch {
            // Whitespace run
            c if c.is_whitespace() => {
                let end = consume_while(&mut chars, |c| c.is_whitespace());
                Token {
                    kind: TokenKind::Whitespace,
                    lexeme: input[start..end].to_string(),
                    span: Span { start, end },
                }
            }

            // Numeric
            c if c.is_ascii_digit() => {
                let end = consume_while(&mut chars, |c| c.is_ascii_digit() || c == '.' || c == ',');
                Token {
                    kind: TokenKind::Number,
                    lexeme: input[start..end].to_string(),
                    span: Span { start, end },
                }
            }

            // Alphabetic — could be word, contraction, or hyphenated
            c if c.is_alphabetic() => lex_word(input, &mut chars, start),

            // Punctuation (everything else that's not alphanumeric/whitespace)
            _ => {
                chars.next();
                let end = start + ch.len_utf8();
                Token {
                    kind: TokenKind::Punctuation,
                    lexeme: input[start..end].to_string(),
                    span: Span { start, end },
                }
            }
        };

        tokens.push(token);
    }

    Ok(tokens)
}

/// Lex a word token, handling contractions (don't) and hyphenated words (well-known).
fn lex_word(
    input: &str,
    chars: &mut std::iter::Peekable<std::str::CharIndices>,
    start: usize,
) -> Token {
    let mut end = consume_while(chars, |c| c.is_alphabetic());
    let mut kind = TokenKind::Word;

    // Check for contraction (apostrophe followed by letters)
    if let Some(&(_, '\'')) = chars.peek() {
        let apostrophe_pos = end;
        chars.next(); // consume apostrophe
        if chars.peek().is_some_and(|&(_, c)| c.is_alphabetic()) {
            end = consume_while(chars, |c| c.is_alphabetic());
            kind = TokenKind::Contraction;
        } else {
            // Apostrophe at end of word — not a contraction.
            // We already consumed the apostrophe, include it.
            end = apostrophe_pos + 1;
            kind = TokenKind::Word;
        }
    }

    // Check for hyphenated compound (hyphen followed by letters)
    if let Some(&(_, '-')) = chars.peek() {
        let hyphen_pos = end;
        chars.next(); // consume hyphen
        if chars.peek().is_some_and(|&(_, c)| c.is_alphabetic()) {
            end = consume_while(chars, |c| c.is_alphabetic());
            kind = TokenKind::Hyphenated;
        } else {
            end = hyphen_pos + 1;
        }
    }

    Token {
        kind,
        lexeme: input[start..end].to_string(),
        span: Span { start, end },
    }
}

/// Consume characters while predicate holds, return the end position.
fn consume_while(
    chars: &mut std::iter::Peekable<std::str::CharIndices>,
    predicate: impl Fn(char) -> bool,
) -> usize {
    let mut end = 0;
    while let Some(&(i, c)) = chars.peek() {
        if predicate(c) {
            end = i + c.len_utf8();
            chars.next();
        } else {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_words() {
        let tokens = lex("hello world").unwrap();
        let words: Vec<_> = tokens.iter().filter(|t| t.kind == TokenKind::Word).collect();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].lexeme, "hello");
        assert_eq!(words[1].lexeme, "world");
    }

    #[test]
    fn test_contraction() {
        let tokens = lex("don't").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Contraction);
        assert_eq!(tokens[0].lexeme, "don't");
    }

    #[test]
    fn test_hyphenated() {
        let tokens = lex("well-known").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Hyphenated);
        assert_eq!(tokens[0].lexeme, "well-known");
    }

    #[test]
    fn test_punctuation() {
        let tokens = lex("Hello, world!").unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &TokenKind::Word,
                &TokenKind::Punctuation, // comma
                &TokenKind::Whitespace,
                &TokenKind::Word,
                &TokenKind::Punctuation, // exclamation
            ]
        );
    }

    #[test]
    fn test_determinism() {
        // Same input must always produce same output
        let a = lex("The quick brown fox").unwrap();
        let b = lex("The quick brown fox").unwrap();
        assert_eq!(a, b);
    }
}
