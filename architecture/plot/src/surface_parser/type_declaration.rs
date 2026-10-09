//! Lossless syntax for authored nominal semantic Types.

mod integer;

use super::Parser;
use crate::prelude::*;
use crate::surface_lex::{is_name, split_declaration};
use crate::surface_parser::front::{canonical_default_bound, split_type_bound};
use crate::syntax::{
    TypeDefinitionSyntax, TypeExpressionSyntax, TypeFieldSyntax, TypeSyntax, TypeVariantCaseSyntax,
    TypeVariantPayloadSyntax,
};
use crate::{PlotError, Span};

impl Parser<'_> {
    pub(super) fn parse_type_declaration(&mut self) -> Result<TypeSyntax, (PlotError, Span)> {
        let header_line = self.lines[self.index];
        let (header, start) = header_line.statement();
        let declaration = header
            .strip_prefix("type ")
            .ok_or_else(|| self.invalid_statement(header, start))?;
        let (name, body) = split_type_header(declaration)
            .map(|(name, body)| (name.trim(), body.trim()))
            .ok_or_else(|| self.invalid_statement(header, start))?;
        let (name_text, parameter_names) =
            split_generic_application(name).ok_or_else(|| self.invalid_statement(header, start))?;
        if !is_name(name_text) {
            return Err(self.invalid_statement(header, start));
        }
        let name = self.spanned_at(name_text, header, start);
        let parameters = parameter_names
            .into_iter()
            .map(|parameter| {
                let (name, annotation) = parameter
                    .split_once(':')
                    .map_or((parameter, None), |(name, annotation)| {
                        (name.trim(), Some(annotation.trim()))
                    });
                if !is_name(name) {
                    return Err(self.invalid_statement(header, start));
                }
                Ok(crate::NativeTypeParameterSyntax {
                    name: self.spanned_at(name, header, start),
                    value_type: annotation
                        .map(|source| {
                            self.parse_type_expression(source, header, start)
                                .map(Box::new)
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, (PlotError, Span)>>()?;
        let declaration_start = start;

        let (definition, invariants) = if body == "{" {
            self.index += 1;
            let (fields, invariants) = self.parse_type_fields(true)?;
            (TypeDefinitionSyntax::Record(fields), invariants)
        } else if body.is_empty() {
            self.index += 1;
            (
                TypeDefinitionSyntax::Variant(self.parse_type_variants()?),
                Vec::new(),
            )
        } else {
            let (body, invariant) = body
                .split_once(" where ")
                .map_or((body, None), |(value_type, invariant)| {
                    (value_type.trim(), Some(invariant.trim()))
                });
            if invariant.is_some_and(str::is_empty) {
                return Err(self.invalid_statement(header, start));
            }
            let value_type = self.parse_type_expression(body, header, start)?;
            let invariants = invariant
                .map(|source| self.expression_at(source, header, start))
                .transpose()?
                .into_iter()
                .collect();
            self.index += 1;
            (TypeDefinitionSyntax::Scalar(value_type), invariants)
        };
        let end = self.lines[self.index.saturating_sub(1)];
        Ok(TypeSyntax {
            name,
            parameters,
            generic_context: None,
            definition,
            invariants,
            span: self.span(declaration_start, end.start + end.text.len()),
        })
    }

    fn parse_type_fields(
        &mut self,
        allow_invariants: bool,
    ) -> Result<(Vec<TypeFieldSyntax>, Vec<crate::Expression>), (PlotError, Span)> {
        let mut fields = Vec::new();
        let mut invariants = Vec::new();
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
                        PlotError::InvalidSyntax("semantic record type must have a field".into()),
                        self.line_span(line),
                    ));
                }
                return Ok((fields, invariants));
            }
            if let Some(source) = text.strip_prefix("where ") {
                if !allow_invariants || fields.is_empty() || source.trim().is_empty() {
                    return Err(self.invalid_statement(text, start));
                }
                if invariants.len() >= conduit_core::MAXIMUM_STRUCTURED_RECORD_FIELDS {
                    return Err((
                        PlotError::InvalidSyntax(
                            "semantic record type has too many where laws".into(),
                        ),
                        self.line_span(line),
                    ));
                }
                invariants.push(self.expression_at(source, text, start)?);
                self.index += 1;
                continue;
            }
            if !invariants.is_empty() {
                return Err((
                    PlotError::InvalidSyntax(
                        "semantic record fields must precede its where laws".into(),
                    ),
                    self.line_span(line),
                ));
            }
            if fields.len() >= conduit_core::MAXIMUM_STRUCTURED_RECORD_FIELDS {
                return Err((
                    PlotError::InvalidSyntax("semantic record type has too many fields".into()),
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
        Err((PlotError::MissingBlockEnd, crate::eof_span(self.source)))
    }

    fn parse_type_variants(&mut self) -> Result<Vec<TypeVariantCaseSyntax>, (PlotError, Span)> {
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
                    PlotError::InvalidSyntax("semantic variant type has too many cases".into()),
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
                TypeVariantPayloadSyntax::Record(self.parse_type_fields(false)?.0)
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
                PlotError::InvalidSyntax("semantic variant type must have a case".into()),
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
    ) -> Result<TypeExpressionSyntax, (PlotError, Span)> {
        self.parse_type_expression_at_depth(source, line, start, 0)
    }

    fn parse_type_expression_at_depth(
        &self,
        source: &str,
        line: &str,
        start: usize,
        depth: usize,
    ) -> Result<TypeExpressionSyntax, (PlotError, Span)> {
        if depth >= 32 {
            return Err(self.invalid_statement(line, start));
        }
        let source = source.trim();
        let offset = start + line.find(source).unwrap_or(0);
        let span = self.span(offset, offset + source.len());
        if let Some(rest) = source.strip_prefix("sequence ") {
            let (element, minimum_items, maximum_items) =
                if let Some((element, maximum)) = rest.rsplit_once(" <= ") {
                    (
                        element,
                        crate::NativeIntegerExpressionSyntax::Literal { value: 0, span },
                        self.parse_integer_extent(
                            maximum,
                            offset + source.len() - maximum.len(),
                            false,
                        )?,
                    )
                } else if let Some((element, bounds)) = rest.rsplit_once(" in ") {
                    let (minimum, maximum) = bounds
                        .split_once("..=")
                        .ok_or_else(|| self.invalid_statement(line, start))?;
                    let bounds_offset = offset + source.len() - bounds.len();
                    let minimum = self.parse_integer_extent(minimum, bounds_offset, true)?;
                    let maximum = self.parse_integer_extent(
                        maximum,
                        bounds_offset + bounds.len() - maximum.len(),
                        false,
                    )?;
                    if let (Some(minimum), Some(maximum)) =
                        (minimum.literal_value(), maximum.literal_value())
                    {
                        if minimum > maximum {
                            return Err(self.invalid_statement(line, start));
                        }
                    }
                    (element, minimum, maximum)
                } else {
                    return Err(self.invalid_statement(line, start));
                };
            return Ok(TypeExpressionSyntax::Sequence {
                element: Box::new(self.parse_type_expression_at_depth(
                    element,
                    line,
                    start,
                    depth + 1,
                )?),
                minimum_items: Box::new(minimum_items),
                maximum_items: Box::new(maximum_items),
                span,
            });
        }
        if let Some(rest) = source.strip_prefix("collection ") {
            let (element, length) = rest
                .rsplit_once(" = ")
                .ok_or_else(|| self.invalid_statement(line, start))?;
            let length =
                self.parse_integer_extent(length, offset + source.len() - length.len(), true)?;
            return Ok(TypeExpressionSyntax::Collection {
                element: Box::new(self.parse_type_expression_at_depth(
                    element,
                    line,
                    start,
                    depth + 1,
                )?),
                length: Box::new(length),
                span,
            });
        }
        if let Some(value) = source.strip_prefix('&') {
            return Ok(TypeExpressionSyntax::DataReference {
                value: Box::new(self.parse_type_expression_at_depth(
                    value,
                    line,
                    start,
                    depth + 1,
                )?),
                span,
            });
        }
        if let Some(value) = source.strip_suffix('?') {
            return Ok(TypeExpressionSyntax::Optional {
                value: Box::new(self.parse_type_expression_at_depth(
                    value,
                    line,
                    start,
                    depth + 1,
                )?),
                span,
            });
        }
        if let Some((value_type, argument_sources)) =
            split_generic_application(source).filter(|(_, arguments)| !arguments.is_empty())
        {
            if !is_name(value_type) && !value_type.split('/').all(is_name) {
                return Err(self.invalid_statement(line, start));
            }
            let arguments = argument_sources
                .into_iter()
                .map(|argument| self.parse_native_argument(argument, line, start, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(TypeExpressionSyntax::Reference {
                value_type: self.spanned(value_type, offset),
                arguments,
                maximum_bytes: None,
                refinements: Vec::new(),
                span,
            });
        }
        let (reference, refinements) = self.parse_value_refinements(source, line, start)?;
        let (reference, explicit_bound) =
            split_type_bound(reference).ok_or_else(|| self.invalid_statement(line, start))?;
        let (value_type, argument_sources) = split_generic_application(reference)
            .ok_or_else(|| self.invalid_statement(line, start))?;
        if value_type.is_empty()
            || value_type.chars().any(char::is_whitespace)
            || (!is_name(value_type) && !value_type.split('/').all(is_name))
        {
            return Err(self.invalid_statement(line, start));
        }
        let arguments = argument_sources
            .into_iter()
            .map(|argument| self.parse_native_argument(argument, line, start, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TypeExpressionSyntax::Reference {
            value_type: self.spanned(value_type, offset),
            arguments,
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            refinements,
            span,
        })
    }
}

fn split_generic_application(source: &str) -> Option<(&str, Vec<&str>)> {
    let source = source.trim();
    let Some(open) = source.find('<') else {
        return Some((source, Vec::new()));
    };
    if !source.ends_with('>') || open == 0 {
        return None;
    }
    let name = source[..open].trim();
    let body = &source[open + 1..source.len() - 1];
    let mut arguments = Vec::new();
    let mut depth = 0_u16;
    let mut parentheses = 0_u16;
    let mut brackets = 0_u16;
    let mut start = 0;
    for (index, character) in body.char_indices() {
        match character {
            '<' if body.as_bytes().get(index + 1) != Some(&b'=')
                && body.as_bytes().get(index.wrapping_sub(1)) != Some(&b'.') =>
            {
                depth = depth.checked_add(1)?;
            }
            '>' => depth = depth.checked_sub(1)?,
            '(' => parentheses = parentheses.checked_add(1)?,
            ')' => parentheses = parentheses.checked_sub(1)?,
            '[' => brackets = brackets.checked_add(1)?,
            ']' => brackets = brackets.checked_sub(1)?,
            ',' if depth == 0 && parentheses == 0 && brackets == 0 => {
                let argument = body[start..index].trim();
                if argument.is_empty() {
                    return None;
                }
                if arguments.len() >= 15 {
                    return None;
                }
                arguments.push(argument);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 || parentheses != 0 || brackets != 0 {
        return None;
    }
    let argument = body[start..].trim();
    if argument.is_empty() {
        return None;
    }
    arguments.push(argument);
    Some((name, arguments))
}

impl Parser<'_> {
    fn parse_integer_extent(
        &self,
        source: &str,
        offset: usize,
        allow_zero: bool,
    ) -> Result<crate::NativeIntegerExpressionSyntax, (PlotError, Span)> {
        let expression = integer::parse(self, source, offset)?;
        if expression.literal_value().is_some_and(|value| {
            (!allow_zero && value == 0)
                || usize::from(value) > conduit_core::MAXIMUM_STRUCTURED_COLLECTION_ITEMS
        }) {
            return Err((
                PlotError::InvalidSyntax(
                    "native collection bound exceeds its finite profile".into(),
                ),
                expression.span(),
            ));
        }
        Ok(expression)
    }
}

fn split_type_header(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0_u16;
    for (index, character) in source.char_indices() {
        match character {
            '<' if source.as_bytes().get(index + 1) != Some(&b'=')
                && source.as_bytes().get(index.wrapping_sub(1)) != Some(&b'.') =>
            {
                depth = depth.checked_add(1)?
            }
            '>' => depth = depth.checked_sub(1)?,
            '=' if depth == 0 => return Some((&source[..index], &source[index + 1..])),
            _ => {}
        }
    }
    None
}

impl Parser<'_> {
    fn parse_native_argument(
        &self,
        source: &str,
        line: &str,
        start: usize,
        depth: usize,
    ) -> Result<crate::NativeTypeArgumentSyntax, (PlotError, Span)> {
        // Names remain unresolved Type syntax until the declared parameter kind
        // selects Type or Info resolution. Capitalization has no semantic role.
        if source.starts_with('(')
            || source.chars().next().is_some_and(|c| c.is_ascii_digit())
            || source.contains('+')
            || source.contains('*')
        {
            let offset = start
                + (source.as_ptr() as usize)
                    .checked_sub(line.as_ptr() as usize)
                    .filter(|offset| *offset <= line.len())
                    .unwrap_or_else(|| line.find(source).unwrap_or(0));
            return integer::parse(self, source, offset)
                .map(|value| crate::NativeTypeArgumentSyntax::Value(Box::new(value)));
        }
        self.parse_type_expression_at_depth(source, line, start, depth)
            .map(|value| crate::NativeTypeArgumentSyntax::Type(Box::new(value)))
    }
}

pub(crate) use integer::parse_integer_spanned;
