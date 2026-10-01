//! Lossless syntax for authored nominal semantic Types.

use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, split_declaration};
use crate::surface_parser::front::{canonical_default_bound, split_type_bound};
use crate::syntax::{
    TypeDefinitionSyntax, TypeExpressionSyntax, TypeFieldSyntax, TypeSyntax, TypeVariantCaseSyntax,
    TypeVariantPayloadSyntax,
};
use crate::{FormError, Span};

impl Parser<'_> {
    pub(super) fn parse_type_declaration(&mut self) -> Result<TypeSyntax, (FormError, Span)> {
        let header_line = self.lines[self.index];
        let (header, start) = header_line.statement();
        let declaration = header
            .strip_prefix("type ")
            .ok_or_else(|| self.invalid_statement(header, start))?;
        let (name, body) = declaration
            .split_once('=')
            .map(|(name, body)| (name.trim(), body.trim()))
            .ok_or_else(|| self.invalid_statement(header, start))?;
        if !is_name(name) {
            return Err(self.invalid_statement(header, start));
        }
        let name = self.spanned_at(name, header, start);
        let declaration_start = start;

        let definition = if body == "{" {
            self.index += 1;
            TypeDefinitionSyntax::Record(self.parse_type_fields()?)
        } else if body.is_empty() {
            self.index += 1;
            TypeDefinitionSyntax::Variant(self.parse_type_variants()?)
        } else {
            let value_type = self.parse_type_expression(body, header, start)?;
            self.index += 1;
            TypeDefinitionSyntax::Scalar(value_type)
        };
        let end = self.lines[self.index.saturating_sub(1)];
        Ok(TypeSyntax {
            name,
            definition,
            span: self.span(declaration_start, end.start + end.text.len()),
        })
    }

    fn parse_type_fields(&mut self) -> Result<Vec<TypeFieldSyntax>, (FormError, Span)> {
        let mut fields = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text.is_empty() {
                self.index += 1;
                continue;
            }
            if text == "}" {
                self.index += 1;
                if fields.is_empty() {
                    return Err((
                        FormError::InvalidSyntax("semantic record type must have a field".into()),
                        self.line_span(line),
                    ));
                }
                return Ok(fields);
            }
            if fields.len() >= conduit_core::MAXIMUM_STRUCTURED_RECORD_FIELDS {
                return Err((
                    FormError::InvalidSyntax("semantic record type has too many fields".into()),
                    self.line_span(line),
                ));
            }
            let (name, value_type) =
                split_declaration(text).ok_or_else(|| self.invalid_statement(text, start))?;
            fields.push(TypeFieldSyntax {
                name: self.spanned_at(name, text, start),
                value_type: self.parse_type_expression(value_type, text, start)?,
                span: self.line_span(line),
            });
            self.index += 1;
        }
        Err((FormError::MissingBlockEnd, crate::eof_span(self.source)))
    }

    fn parse_type_variants(&mut self) -> Result<Vec<TypeVariantCaseSyntax>, (FormError, Span)> {
        let mut cases = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text.is_empty() {
                self.index += 1;
                continue;
            }
            if line.text.len() == line.text.trim_start().len() {
                break;
            }
            if cases.len() >= conduit_core::MAXIMUM_STRUCTURED_VARIANT_CASES {
                return Err((
                    FormError::InvalidSyntax("semantic variant type has too many cases".into()),
                    self.line_span(line),
                ));
            }
            let case = text.strip_prefix('|').map(str::trim).unwrap_or(text);
            let (tag, payload_source, has_fields) = if let Some(tag) = case.strip_suffix('{') {
                (tag.trim(), None, true)
            } else if let Some((tag, payload)) = case.split_once(char::is_whitespace) {
                (tag.trim(), Some(payload.trim()), false)
            } else {
                (case.trim(), None, false)
            };
            if !is_name(tag) {
                return Err(self.invalid_statement(text, start));
            }
            let case_start = start;
            let tag = self.spanned_at(tag, text, start);
            self.index += 1;
            let payload = if has_fields {
                TypeVariantPayloadSyntax::Record(self.parse_type_fields()?)
            } else if let Some(payload) = payload_source {
                TypeVariantPayloadSyntax::Type(self.parse_type_expression(payload, text, start)?)
            } else {
                TypeVariantPayloadSyntax::Unit
            };
            let end = self.lines[self.index.saturating_sub(1)];
            cases.push(TypeVariantCaseSyntax {
                tag,
                payload,
                span: self.span(case_start, end.start + end.text.len()),
            });
        }
        if cases.is_empty() {
            return Err((
                FormError::InvalidSyntax("semantic variant type must have a case".into()),
                crate::eof_span(self.source),
            ));
        }
        Ok(cases)
    }

    fn parse_type_expression(
        &self,
        source: &str,
        line: &str,
        start: usize,
    ) -> Result<TypeExpressionSyntax, (FormError, Span)> {
        let source = source.trim();
        let offset = start + line.find(source).unwrap_or(0);
        let span = self.span(offset, offset + source.len());
        if let Some(rest) = source.strip_prefix("sequence ") {
            let (element, maximum) = rest
                .rsplit_once(" <= ")
                .ok_or_else(|| self.invalid_statement(line, start))?;
            let maximum_items = maximum
                .parse::<u16>()
                .ok()
                .filter(|maximum| {
                    *maximum > 0
                        && usize::from(*maximum)
                            <= conduit_core::MAXIMUM_STRUCTURED_COLLECTION_ITEMS
                })
                .ok_or_else(|| self.invalid_statement(line, start))?;
            return Ok(TypeExpressionSyntax::Sequence {
                element: Box::new(self.parse_type_expression(element, line, start)?),
                minimum_items: 0,
                maximum_items,
                span,
            });
        }
        if let Some(value) = source.strip_prefix('&') {
            return Ok(TypeExpressionSyntax::DataReference {
                value: Box::new(self.parse_type_expression(value, line, start)?),
                span,
            });
        }
        if let Some(value) = source.strip_suffix('?') {
            return Ok(TypeExpressionSyntax::Optional {
                value: Box::new(self.parse_type_expression(value, line, start)?),
                span,
            });
        }
        let (value_type, refinements) = self.parse_value_refinements(source, line, start)?;
        let (value_type, explicit_bound) =
            split_type_bound(value_type).ok_or_else(|| self.invalid_statement(line, start))?;
        if value_type.is_empty()
            || value_type.chars().any(char::is_whitespace)
            || (!is_name(value_type) && !value_type.split('/').all(is_name))
        {
            return Err(self.invalid_statement(line, start));
        }
        Ok(TypeExpressionSyntax::Reference {
            value_type: self.spanned(value_type, offset),
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            refinements,
            span,
        })
    }
}
