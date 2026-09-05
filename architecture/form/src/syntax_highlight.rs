//! Finite lossless lexical highlighting for canonical Conduit source.

use crate::{tokenize_losslessly, MAXIMUM_FORM_SOURCE_BYTES};
use alloc::vec::Vec;

/// Every highlight span covers at least one source byte, so the source bound is
/// also a strict upper bound on the number of spans.
pub const MAXIMUM_SYNTAX_HIGHLIGHT_SPANS: usize = MAXIMUM_FORM_SOURCE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxHighlightKind {
    Whitespace,
    Comment,
    Keyword,
    Name,
    Identity,
    String,
    Number,
    Literal,
    Operator,
    Delimiter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxHighlightSpan {
    pub start: usize,
    pub end: usize,
    pub kind: SyntaxHighlightKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxHighlightRefusal {
    SourceTooLarge,
    TooManyTokens,
    TooManySpans,
}

/// Classifies exact UTF-8 byte spans without requiring the source to parse.
///
/// This deliberately starts from the Form surface's lossless tokenizer. An
/// incomplete quote, missing delimiter, or otherwise temporarily invalid edit
/// can therefore still be highlighted while syntax diagnostics remain a
/// separate concern.
pub fn highlight_syntax(source: &str) -> Result<Vec<SyntaxHighlightSpan>, SyntaxHighlightRefusal> {
    if source.len() > MAXIMUM_FORM_SOURCE_BYTES {
        return Err(SyntaxHighlightRefusal::SourceTooLarge);
    }
    // Preserve the parser's exact token-admission bound without changing CST
    // identity. Highlight segmentation is separately quote-aware because an
    // incomplete embedded string may legally contain whitespace while edited.
    tokenize_losslessly(source).map_err(|_| SyntaxHighlightRefusal::TooManyTokens)?;
    let mut spans = Vec::new();
    scan_source(source, &mut spans)?;
    Ok(spans)
}

fn scan_source(
    source: &str,
    spans: &mut Vec<SyntaxHighlightSpan>,
) -> Result<(), SyntaxHighlightRefusal> {
    let bytes = source.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        let start = offset;
        let character = source[offset..]
            .chars()
            .next()
            .expect("offset remains on a character boundary");
        let kind = if character.is_whitespace() {
            offset += character.len_utf8();
            while offset < bytes.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset remains on a character boundary");
                if !next.is_whitespace() {
                    break;
                }
                offset += next.len_utf8();
            }
            SyntaxHighlightKind::Whitespace
        } else if character == '#' {
            offset += character.len_utf8();
            while offset < bytes.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset remains on a character boundary");
                if next == '\n' {
                    break;
                }
                offset += next.len_utf8();
            }
            SyntaxHighlightKind::Comment
        } else if matches!(character, '\'' | '"') {
            offset = quoted_end(source, offset, character);
            SyntaxHighlightKind::String
        } else if let Some((length, kind)) = punctuation(&source[offset..]) {
            offset += length;
            kind
        } else {
            offset += character.len_utf8();
            while offset < bytes.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset remains on a character boundary");
                if next.is_whitespace()
                    || next == '#'
                    || matches!(next, '\'' | '"')
                    || punctuation(&source[offset..]).is_some()
                {
                    break;
                }
                offset += next.len_utf8();
            }
            classify_word(&source[start..offset])
        };
        push(spans, start, offset, kind)?;
    }
    Ok(())
}

fn quoted_end(text: &str, start: usize, quote: char) -> usize {
    let mut offset = start + quote.len_utf8();
    let mut escaped = false;
    while offset < text.len() {
        let character = text[offset..]
            .chars()
            .next()
            .expect("offset remains on a character boundary");
        offset += character.len_utf8();
        if character == quote && !escaped {
            break;
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    offset
}

fn punctuation(text: &str) -> Option<(usize, SyntaxHighlightKind)> {
    if text.starts_with("...|") {
        return Some((4, SyntaxHighlightKind::Operator));
    }
    if text.starts_with("...") {
        return Some((3, SyntaxHighlightKind::Operator));
    }
    let character = text.chars().next()?;
    match character {
        '(' | ')' | '{' | '}' | '[' | ']' | ',' => {
            Some((character.len_utf8(), SyntaxHighlightKind::Delimiter))
        }
        ':' | '=' | '>' | '|' | '$' => Some((character.len_utf8(), SyntaxHighlightKind::Operator)),
        _ => None,
    }
}

fn classify_word(word: &str) -> SyntaxHighlightKind {
    match word {
        "form" | "host" | "body" | "pool" => SyntaxHighlightKind::Keyword,
        "true" | "false" => SyntaxHighlightKind::Literal,
        _ if word.parse::<i128>().is_ok() || word.parse::<u128>().is_ok() => {
            SyntaxHighlightKind::Number
        }
        _ if word.contains('/') || word.contains('.') || word.contains('@') => {
            SyntaxHighlightKind::Identity
        }
        _ => SyntaxHighlightKind::Name,
    }
}

fn push(
    spans: &mut Vec<SyntaxHighlightSpan>,
    start: usize,
    end: usize,
    kind: SyntaxHighlightKind,
) -> Result<(), SyntaxHighlightRefusal> {
    if start == end {
        return Ok(());
    }
    if spans.len() == MAXIMUM_SYNTAX_HIGHLIGHT_SPANS {
        return Err(SyntaxHighlightRefusal::TooManySpans);
    }
    spans.push(SyntaxHighlightSpan { start, end, kind });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_syntax_document, MAXIMUM_FORM_TOKENS};
    use alloc::{string::String, vec};

    fn pieces<'a>(
        source: &'a str,
        spans: &'a [SyntaxHighlightSpan],
    ) -> Vec<(SyntaxHighlightKind, &'a str)> {
        spans
            .iter()
            .map(|span| (span.kind, &source[span.start..span.end]))
            .collect()
    }

    #[test]
    fn canonical_source_is_lossless_and_grammar_aware() {
        let source = "form hello (\n  name: value/text@1 = \"reader\"\n  tick: value/u64@1... >\n) {\n  count = 42\n  source: text/constant(value=\"hi\")\n  source.output > sink.input\n}\n";
        let spans = highlight_syntax(source).unwrap();
        let reconstructed: String = spans
            .iter()
            .map(|span| &source[span.start..span.end])
            .collect();
        assert_eq!(reconstructed, source);
        let pieces = pieces(source, &spans);
        for expected in [
            (SyntaxHighlightKind::Keyword, "form"),
            (SyntaxHighlightKind::Identity, "value/text@1"),
            (SyntaxHighlightKind::String, "\"reader\""),
            (SyntaxHighlightKind::Operator, "..."),
            (SyntaxHighlightKind::Number, "42"),
            (SyntaxHighlightKind::Identity, "text/constant"),
            (SyntaxHighlightKind::Identity, "source.output"),
        ] {
            assert!(
                pieces.contains(&expected),
                "missing {expected:?} in {pieces:?}"
            );
        }
    }

    #[test]
    fn incomplete_invalid_edit_still_highlights_exact_source() {
        let source = "form unfinished {\n  message: text/constant(value=\"still typing\n";
        assert!(!parse_syntax_document(source).diagnostics.is_empty());
        let spans = highlight_syntax(source).unwrap();
        assert_eq!(spans.first().unwrap().kind, SyntaxHighlightKind::Keyword);
        assert_eq!(spans.last().unwrap().kind, SyntaxHighlightKind::String);
        assert_eq!(&source[spans.last().unwrap().start..], "\"still typing\n");
    }

    #[test]
    fn unicode_comments_and_names_keep_utf8_byte_boundaries() {
        let source = "# café\nform élève {\n}\n";
        let spans = highlight_syntax(source).unwrap();
        for span in &spans {
            assert!(source.is_char_boundary(span.start));
            assert!(source.is_char_boundary(span.end));
        }
        assert!(pieces(source, &spans).contains(&(SyntaxHighlightKind::Comment, "# café")));
    }

    #[test]
    fn inline_comments_are_distinct_from_hashes_inside_strings() {
        let source = "form note { value = \"channel #7\" # visible note\n}\n";
        let spans = highlight_syntax(source).unwrap();
        let pieces = pieces(source, &spans);
        assert!(pieces.contains(&(SyntaxHighlightKind::String, "\"channel #7\"")));
        assert!(pieces.contains(&(SyntaxHighlightKind::Comment, "# visible note")));
    }

    #[test]
    fn source_and_token_pressure_refuse_distinctly() {
        let oversized = "x".repeat(MAXIMUM_FORM_SOURCE_BYTES + 1);
        assert_eq!(
            highlight_syntax(&oversized),
            Err(SyntaxHighlightRefusal::SourceTooLarge)
        );
        let token_pressure = "x ".repeat(MAXIMUM_FORM_TOKENS / 2 + 1);
        assert_eq!(
            highlight_syntax(&token_pressure),
            Err(SyntaxHighlightRefusal::TooManyTokens)
        );
    }

    #[test]
    fn punctuation_and_literals_remain_finite_without_regex_rewriting() {
        let source = "body demo { enabled=true list=[1,2] current=$value stream=value/text@1...| }";
        let spans = highlight_syntax(source).unwrap();
        let pieces = pieces(source, &spans);
        assert!(pieces.contains(&(SyntaxHighlightKind::Keyword, "body")));
        assert!(pieces.contains(&(SyntaxHighlightKind::Literal, "true")));
        assert!(pieces.contains(&(SyntaxHighlightKind::Operator, "$")));
        assert!(pieces.contains(&(SyntaxHighlightKind::Operator, "...|")));
        assert!(pieces.contains(&(SyntaxHighlightKind::Delimiter, "[")));
        assert_eq!(
            spans.len(),
            pieces.len(),
            "one bounded span describes each exact piece"
        );
        assert_ne!(spans, vec![]);
    }
}
