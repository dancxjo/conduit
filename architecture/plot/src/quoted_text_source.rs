//! Bounded source custody for decoded quoted Text, reusable by domain parsers.
use crate::{prelude::*, Span, SpannedText, MAXIMUM_PLOT_SOURCE_BYTES};
use core::ops::Range;

pub struct QuotedTextSourceMap<'a> {
    source: &'a str,
    decoded: String,
    scalars: Vec<(Range<usize>, Range<usize>)>,
    body_start: usize,
}
impl<'a> QuotedTextSourceMap<'a> {
    /// Correlates an exact quoted token with original Source. The same decoder
    /// used by startup and expression checking owns the five supported escapes.
    /// No mapping or allocation happens during Play.
    pub fn new(source: &'a str, token: &SpannedText, maximum_decoded_bytes: usize) -> Option<Self> {
        if source.len() > MAXIMUM_PLOT_SOURCE_BYTES
            || maximum_decoded_bytes > MAXIMUM_PLOT_SOURCE_BYTES
            || token.text.len() > maximum_decoded_bytes.checked_mul(2)?.checked_add(2)?
            || source.get(token.span.start..token.span.end)? != token.text
        {
            return None;
        }
        let mut scalars = Vec::new();
        let mut interior_quote = false;
        let decoded = crate::text_value::parse_quoted_text_mapped(
            &token.text,
            maximum_decoded_bytes,
            |decoded, raw| {
                interior_quote |= raw.len() == 1 && token.text.as_bytes()[raw.start] == b'"';
                scalars.push((
                    decoded,
                    token.span.start + raw.start..token.span.start + raw.end,
                ));
            },
        )?;
        if interior_quote {
            return None;
        }
        Some(Self {
            source,
            decoded,
            scalars,
            body_start: token.span.start + 1,
        })
    }
    pub fn decoded(&self) -> &str {
        &self.decoded
    }
    /// UTF-8 boundaries only. An escaped scalar maps to the whole authored
    /// escape; combined segments map across all their original scalar spellings.
    pub fn source_span(&self, decoded: Range<usize>) -> Option<Span> {
        if decoded.start > decoded.end || decoded.end > self.decoded.len() {
            return None;
        }
        let start = if decoded.start == self.decoded.len() {
            self.scalars
                .last()
                .map_or(self.body_start, |(_, raw)| raw.end)
        } else {
            self.scalars
                .iter()
                .find(|(part, _)| part.start == decoded.start)?
                .1
                .start
        };
        let end = if decoded.end == 0 {
            self.body_start
        } else {
            self.scalars
                .iter()
                .find(|(part, _)| part.end == decoded.end)?
                .1
                .end
        };
        source_span(self.source, start..end)
    }
}

/// Exact byte range and one-based Unicode scalar locations in retained Source.
pub fn source_span(source: &str, bytes: Range<usize>) -> Option<Span> {
    if source.len() > MAXIMUM_PLOT_SOURCE_BYTES
        || bytes.start > bytes.end
        || source.get(bytes.clone()).is_none()
    {
        return None;
    }
    let location = |end| {
        let mut line = 1;
        let mut column = 1;
        for character in source[..end].chars() {
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        (line, column)
    };
    let (line, column) = location(bytes.start);
    let (end_line, end_column) = location(bytes.end);
    Some(Span {
        start: bytes.start,
        end: bytes.end,
        line,
        column,
        end_line,
        end_column,
    })
}
