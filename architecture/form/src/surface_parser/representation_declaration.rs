//! Lossless syntax for named finite representations.

use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, is_operation};
use crate::syntax::{
    RepresentationMappingSyntax, RepresentationStorageSyntax, RepresentationSyntax,
};
use crate::{FormError, Span};

pub(super) fn parse_representation(
    parser: &mut Parser<'_>,
) -> Result<RepresentationSyntax, (FormError, Span)> {
    let header_line = parser.lines[parser.index];
    let (header, start) = header_line.statement();
    let declaration = header
        .strip_prefix("representation ")
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
        mappings.push(RepresentationMappingSyntax {
            variant: parser.spanned_at(text, text, line_start),
            span: parser.line_span(line),
        });
        end = line;
        parser.index += 1;
    }
    Ok(RepresentationSyntax {
        name,
        value_type,
        storage: RepresentationStorageSyntax::U8,
        first_discriminant,
        mappings,
        span: parser.span(start, end.start + end.text.len()),
    })
}
