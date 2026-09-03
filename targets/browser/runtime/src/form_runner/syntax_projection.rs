//! Finite browser projection of canonical Conduit syntax-highlight spans.

use conduit_form::{highlight_syntax, SyntaxHighlightKind, SyntaxHighlightRefusal};
use serde::Serialize;

pub(super) const SYNTAX_PROJECTION_VERSION: &str = "conduit.syntax-highlight-projection@1";
const SYNTAX_KINDS: [&str; 10] = [
    "whitespace",
    "comment",
    "keyword",
    "name",
    "identity",
    "string",
    "number",
    "literal",
    "operator",
    "delimiter",
];

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(super) struct SyntaxProjection {
    pub protocol: &'static str,
    pub source_bytes: u32,
    pub kinds: &'static [&'static str; 10],
    /// Compact tuples keep every admitted Book projection inside the shared
    /// 128 KiB application-presentation aggregate bound.
    pub spans: Vec<(u32, u32, u8)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum SyntaxKind {
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

pub(super) fn project(source: &str) -> Result<SyntaxProjection, String> {
    let source_bytes = u32::try_from(source.len())
        .map_err(|_| "Book syntax source length exceeds u32".to_owned())?;
    let spans = highlight_syntax(source)
        .map_err(refusal)?
        .into_iter()
        .map(|span| {
            let start = u32::try_from(span.start)
                .map_err(|_| "Book syntax span start exceeds u32".to_owned())?;
            let end = u32::try_from(span.end)
                .map_err(|_| "Book syntax span end exceeds u32".to_owned())?;
            Ok((start, end, SyntaxKind::from(span.kind) as u8))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(SyntaxProjection {
        protocol: SYNTAX_PROJECTION_VERSION,
        source_bytes,
        kinds: &SYNTAX_KINDS,
        spans,
    })
}

fn refusal(reason: SyntaxHighlightRefusal) -> String {
    match reason {
        SyntaxHighlightRefusal::SourceTooLarge => "Book syntax source exceeds its byte bound",
        SyntaxHighlightRefusal::TooManyTokens => "Book syntax source exceeds its token bound",
        SyntaxHighlightRefusal::TooManySpans => "Book syntax source exceeds its span bound",
    }
    .to_owned()
}

impl From<SyntaxHighlightKind> for SyntaxKind {
    fn from(kind: SyntaxHighlightKind) -> Self {
        match kind {
            SyntaxHighlightKind::Whitespace => Self::Whitespace,
            SyntaxHighlightKind::Comment => Self::Comment,
            SyntaxHighlightKind::Keyword => Self::Keyword,
            SyntaxHighlightKind::Name => Self::Name,
            SyntaxHighlightKind::Identity => Self::Identity,
            SyntaxHighlightKind::String => Self::String,
            SyntaxHighlightKind::Number => Self::Number,
            SyntaxHighlightKind::Literal => Self::Literal,
            SyntaxHighlightKind::Operator => Self::Operator,
            SyntaxHighlightKind::Delimiter => Self::Delimiter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_runner::abi::{INPUT_BYTES, OUTPUT_BYTES};

    #[test]
    fn incomplete_source_projects_exact_utf8_byte_spans() {
        let source = "form café { value=text/constant(value=\"still typing\n";
        let projection = project(source).unwrap();
        assert_eq!(projection.source_bytes, source.len() as u32);
        assert_eq!(projection.spans.last().unwrap().2, SyntaxKind::String as u8);
        for (start, end, _) in projection.spans {
            assert!(source.is_char_boundary(start as usize));
            assert!(source.is_char_boundary(end as usize));
        }
    }

    #[test]
    fn worst_case_book_input_fits_the_bounded_output_arena() {
        let source = "x ".repeat(INPUT_BYTES / 2);
        let encoded = serde_json::to_vec(&project(&source).unwrap()).unwrap();
        assert!(
            encoded.len() <= OUTPUT_BYTES,
            "{} > {OUTPUT_BYTES}",
            encoded.len()
        );
    }
}
