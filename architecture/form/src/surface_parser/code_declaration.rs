//! Lossless syntax for named finite codes.

use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, is_operation};
use crate::syntax::{CodeMappingSyntax, CodeStorageSyntax, CodeSyntax};
use crate::{FormError, Span};

pub(super) fn parse_code(parser: &mut Parser<'_>) -> Result<CodeSyntax, (FormError, Span)> {
    let header_line = parser.lines[parser.index];
    let (header, start) = header_line.statement();
    let declaration = header
        .strip_prefix("code ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let (name, realization) = declaration
        .split_once(" = ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let (value_type, storage) = realization
        .split_once(" as ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let first_discriminant = match storage.strip_prefix("u8 from ") {
        Some(value) => value
            .parse::<u8>()
            .map_err(|_| parser.invalid_statement(header, start))?,
        None if storage == "u8" => 0,
        None => return Err(parser.invalid_statement(header, start)),
    };
    if !is_operation(name) || !is_name(value_type) {
        return Err(parser.invalid_statement(header, start));
    }

    let name = parser.spanned_at(name, header, start);
    let value_type = parser.spanned_at(value_type, header, start);
    let mut mappings = Vec::new();
    let mut end = header_line;
    parser.index += 1;
    while parser.index < parser.lines.len() {
        let line = parser.lines[parser.index];
        let (text, line_start) = line.statement();
        if text.is_empty() {
            parser.index += 1;
            continue;
        }
        if line.text.len() == line.text.trim_start().len() {
            break;
        }
        if !is_name(text) {
            return Err(parser.invalid_statement(text, line_start));
        }
        mappings.push(CodeMappingSyntax {
            variant: parser.spanned_at(text, text, line_start),
            span: parser.line_span(line),
        });
        end = line;
        parser.index += 1;
    }
    Ok(CodeSyntax {
        name,
        value_type,
        storage: CodeStorageSyntax::U8,
        first_discriminant,
        mappings,
        span: parser.span(start, end.start + end.text.len()),
    })
}
