//! Lossless syntax for named finite Forms.

use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, is_operation};
use crate::syntax::{
    QuantityFormSyntax, TypeFormMappingSyntax, TypeFormStorageSyntax, TypeFormSyntax,
};
use crate::{PlotError, Span};

pub(super) fn parse_type_form(
    parser: &mut Parser<'_>,
) -> Result<TypeFormSyntax, (PlotError, Span)> {
    let header_line = parser.lines[parser.index];
    let (header, start) = header_line.statement();
    let declaration = header
        .strip_prefix("form ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let (name, realization) = declaration
        .split_once(" = ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let (value_type, storage) = realization
        .split_once(" as ")
        .ok_or_else(|| parser.invalid_statement(header, start))?;
    let quantity = if let Some((integer, details)) = storage.split_once('(') {
        if !matches!(integer, "i16le" | "u16le") {
            return Err(parser.invalid_statement(header, start));
        }
        let details = details
            .strip_suffix(')')
            .ok_or_else(|| parser.invalid_statement(header, start))?;
        let (unit, exponent) = details
            .split_once(',')
            .ok_or_else(|| parser.invalid_statement(header, start))?;
        Some(QuantityFormSyntax {
            unit: parser.spanned_at(unit.trim(), header, start),
            exponent: exponent
                .trim()
                .parse()
                .map_err(|_| parser.invalid_statement(header, start))?,
        })
    } else {
        None
    };
    let first_discriminant = match storage.strip_prefix("u8 from ") {
        Some(value) => value
            .parse::<u8>()
            .map_err(|_| parser.invalid_statement(header, start))?,
        None if matches!(storage, "u8" | "u8 bits") => 0,
        None if quantity.is_some() => 0,
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
        mappings.push(TypeFormMappingSyntax {
            variant: parser.spanned_at(text, text, line_start),
            span: parser.line_span(line),
        });
        end = line;
        parser.index += 1;
    }
    Ok(TypeFormSyntax {
        name,
        value_type,
        storage: if storage == "u8 bits" {
            TypeFormStorageSyntax::U8Bits
        } else if storage.starts_with("i16le(") {
            TypeFormStorageSyntax::I16Le
        } else if storage.starts_with("u16le(") {
            TypeFormStorageSyntax::U16Le
        } else {
            TypeFormStorageSyntax::U8
        },
        quantity,
        first_discriminant,
        mappings,
        span: parser.span(start, end.start + end.text.len()),
    })
}
