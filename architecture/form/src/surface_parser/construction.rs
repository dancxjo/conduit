use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, top_level_positions};
use crate::syntax::{ConstructionRole, ConstructionSyntax, LocalValue};
use crate::{eof_span, FormError, Span};

pub(super) fn parse_construction(
    parser: &mut Parser<'_>,
    role: ConstructionRole,
    spelling: &str,
) -> Result<ConstructionSyntax, (FormError, Span)> {
    let header_line = parser.lines[parser.index];
    let (header, header_start) = header_line.statement();
    let prefix = format!("{spelling} ");
    let rest = header
        .strip_prefix(&prefix)
        .expect("construction role was selected from its prefix");
    let Some(open) = rest.find('{') else {
        return Err((
            FormError::InvalidSyntax(format!("expected '{{' to open {spelling} document")),
            parser.line_span(header_line),
        ));
    };
    let name_text = rest[..open].trim();
    if !is_name(name_text) || !rest[open + 1..].trim().is_empty() {
        return Err((
            FormError::InvalidSyntax(format!("invalid {spelling} document header")),
            parser.line_span(header_line),
        ));
    }
    let name_start = header_start + prefix.len() + rest[..open].find(name_text).unwrap_or(0);
    let start = header_start;
    parser.index += 1;
    let mut declarations = Vec::new();
    while parser.index < parser.lines.len() {
        let line = parser.lines[parser.index];
        let (text, line_start) = line.statement();
        if text == "}" {
            parser.index += 1;
            return Ok(ConstructionSyntax {
                role,
                name: parser.spanned(name_text, name_start),
                declarations,
                span: parser.span(start, line.start + line.text.len()),
            });
        }
        if text.is_empty() || text.starts_with('#') {
            parser.index += 1;
            continue;
        }
        let Some(equal) = top_level_positions(text, '=').first().copied() else {
            return Err(parser.invalid_statement(text, line_start));
        };
        let name = text[..equal].trim();
        let value = text[equal + 1..].trim();
        if !is_name(name) || value.is_empty() {
            return Err(parser.invalid_statement(text, line_start));
        }
        declarations.push(LocalValue {
            name: parser.spanned_at(name, text, line_start),
            value: parser.expression_at(value, text, line_start)?,
            span: parser.span(line_start, line_start + text.len()),
        });
        parser.index += 1;
    }
    Err((FormError::MissingBlockEnd, eof_span(parser.source)))
}
