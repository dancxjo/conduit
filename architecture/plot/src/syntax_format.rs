//! Bounded block formatting that preserves domain-owned literal bytes.
use crate::prelude::*;
use crate::{StartupCatalog, MAXIMUM_PLOT_SOURCE_BYTES};

mod opaque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxFormatRefusal {
    InvalidSource(crate::SyntaxCheckDiagnostic),
    DepthLimit,
    OutputLimit,
}

/// Normalize block indentation and exterior horizontal trivia. Every literal,
/// comment and non-trivia source byte retains its authored spelling. Formatting
/// does not admit domain values or replace an ASCII spelling with Unicode.
pub fn format_syntax(
    source: &str,
    startup: &StartupCatalog,
) -> Result<String, SyntaxFormatRefusal> {
    let document = crate::parse_syntax_document_with_glyph_notations(source, startup);
    require_syntax(&document)?;
    let scope = crate::resolve_glyph_notation_scope(&document, startup)
        .map_err(SyntaxFormatRefusal::InvalidSource)?;
    let opaque = opaque::ranges(source, &scope);
    let mut result = String::new();
    let mut depth = 0usize;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let (body, newline) = match body.strip_suffix('\r') {
            Some(body) if line.ends_with('\n') => (body, "\r\n"),
            _ => (body, "\n"),
        };
        // A multiline quoted value owns all bytes on a continuation line.
        if opaque::continuation(&opaque, offset) {
            append(&mut result, line)?;
            advance_depth(body, offset, &opaque, &mut depth)?;
            offset += line.len();
            continue;
        }
        let mut first = 0;
        while first < body.len() && matches!(body.as_bytes()[first], b' ' | b'\t') {
            first += 1;
        }
        let mut end = body.len();
        while end > first
            && matches!(body.as_bytes()[end - 1], b' ' | b'\t' | b'\r')
            && !opaque::inside(&opaque, offset + end - 1)
        {
            end -= 1;
        }
        let text = &body[first..end];
        let closing = text
            .chars()
            .take_while(|character| matches!(character, ')' | ']' | '}'))
            .count();
        let indentation = depth.saturating_sub(closing);
        if !text.is_empty() {
            append(&mut result, &"    ".repeat(indentation))?;
            append(&mut result, text)?;
        }
        append(&mut result, newline)?;
        advance_depth(text, offset + first, &opaque, &mut depth)?;
        offset += line.len();
    }
    // Surface grammar and exact family resolution remain the authority after
    // trivia changes; no unchecked formatter output reaches a user.
    require_syntax(&crate::parse_syntax_document_with_glyph_notations(
        &result, startup,
    ))?;
    Ok(result)
}

fn advance_depth(
    text: &str,
    offset: usize,
    ranges: &[(usize, usize)],
    depth: &mut usize,
) -> Result<(), SyntaxFormatRefusal> {
    for (relative, character) in text.char_indices() {
        if opaque::inside(ranges, offset + relative) {
            continue;
        }
        match character {
            '(' | '[' | '{' => {
                *depth += 1;
                if *depth > 64 {
                    return Err(SyntaxFormatRefusal::DepthLimit);
                }
            }
            ')' | ']' | '}' => *depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn require_syntax(document: &crate::SyntaxDocument) -> Result<(), SyntaxFormatRefusal> {
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(SyntaxFormatRefusal::InvalidSource(
            crate::SyntaxCheckDiagnostic {
                code: diagnostic.code,
                span: diagnostic.span,
                message: diagnostic.message.clone(),
            },
        ));
    }
    Ok(())
}

fn append(output: &mut String, value: &str) -> Result<(), SyntaxFormatRefusal> {
    if output.len().saturating_add(value.len()) > MAXIMUM_PLOT_SOURCE_BYTES {
        return Err(SyntaxFormatRefusal::OutputLimit);
    }
    output.push_str(value);
    Ok(())
}
