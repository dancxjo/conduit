//! Parsing for the checked callable boundary of one authored Form.

mod kind_parameter;
mod startup;

use super::Parser;
use crate::surface_lex::{
    split_declaration, split_top_level, split_top_level_token, top_level_positions,
    top_level_token_positions,
};
use crate::syntax::{
    Expression, FormFront, KindParameter, RuntimePort, RuntimePortDirection, RuntimePortTemporal,
    ShorthandPair, TypeParameter,
};
use crate::{eof_span, FormError, Span};
use alloc::vec::Vec;

pub(super) fn parse_port_type(value_type: &str) -> Option<(&str, RuntimePortTemporal)> {
    let (value_type, temporal) = if let Some(value_type) = value_type
        .strip_prefix('$')
        .and_then(|value| value.strip_suffix('?'))
    {
        (value_type, RuntimePortTemporal::CurrentOptional)
    } else if let Some(value_type) = value_type.strip_prefix('$') {
        (value_type, RuntimePortTemporal::Current)
    } else if let Some(value_type) = value_type.strip_suffix("...|") {
        (value_type, RuntimePortTemporal::Flow { closes: true })
    } else if let Some(value_type) = value_type.strip_suffix("...") {
        (value_type, RuntimePortTemporal::Flow { closes: false })
    } else if let Some(value_type) = value_type.strip_suffix('?') {
        (value_type, RuntimePortTemporal::OptionalValue)
    } else {
        (value_type, RuntimePortTemporal::Value)
    };
    (!value_type.is_empty()
        && !value_type.starts_with('$')
        && !value_type.starts_with("&&")
        && value_type != "&"
        && !value_type.ends_with("...")
        && !value_type.ends_with("...|")
        && !value_type.ends_with('?'))
    .then_some((value_type, temporal))
}

pub(super) fn parse_finite_bound(source: &str) -> Option<u64> {
    let source = source.trim();
    let (digits, multiplier) = if let Some(value) = source.strip_suffix("KiB") {
        (value, 1_024_u64)
    } else if let Some(value) = source.strip_suffix("MiB") {
        (value, 1_048_576_u64)
    } else {
        (source.strip_suffix('B')?, 1_u64)
    };
    digits
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(multiplier))
        .filter(|value| *value > 0)
}

pub(super) fn split_type_bound(value_type: &str) -> Option<(&str, Option<u64>)> {
    let parts = split_top_level_token(value_type, "<=");
    match parts.as_slice() {
        [value_type] => Some((value_type.trim(), None)),
        [value_type, bound] => Some((value_type.trim(), Some(parse_finite_bound(bound)?))),
        _ => None,
    }
}

pub(crate) fn canonical_default_bound(value_type: &str) -> Option<u64> {
    match value_type {
        "Text" | "value/text" => Some(256),
        "Bytes" | "value/bytes" => Some(65_536),
        "Boolean" | conduit_core::BOOL_INFO_ID => Some(1),
        "U64" | "info/u64" | "I64" | "info/i64" => Some(8),
        _ => None,
    }
}

pub(super) fn split_default(text: &str) -> (&str, Option<&str>) {
    top_level_positions(text, '=')
        .into_iter()
        .find(|position| {
            *position == 0 || !matches!(text.as_bytes()[position - 1], b'<' | b'>' | b'!' | b'=')
        })
        .map_or((text, None), |position| {
            (&text[..position], Some(&text[position + 1..]))
        })
}

impl Parser<'_> {
    pub(super) fn parse_front(
        &mut self,
        open: usize,
    ) -> Result<(FormFront, Option<Expression>), (FormError, Span)> {
        let mut front = FormFront::default();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == ") {" || text == "){" {
                front.span = Some(self.span(open, start + text.find(')').unwrap() + 1));
                self.index += 1;
                return Ok((front, None));
            }
            if let Some(body) = text
                .strip_prefix(')')
                .and_then(|tail| tail.trim().strip_prefix('='))
            {
                let body = body.trim();
                if body.is_empty() {
                    return Err((
                        FormError::InvalidSyntax(
                            "expression-bodied form requires one finite expression after '='"
                                .into(),
                        ),
                        self.line_span(line),
                    ));
                }
                let expression = self.expression_at(body, text, start)?;
                front.span = Some(self.span(open, start + text.find(')').unwrap() + 1));
                self.index += 1;
                return Ok((front, Some(expression)));
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
                continue;
            }
            if let Some(declaration) = text.strip_suffix("kind (") {
                let name = declaration
                    .trim()
                    .strip_suffix(':')
                    .map(str::trim)
                    .filter(|name| crate::surface_lex::is_name(name))
                    .ok_or_else(|| self.invalid_statement(text, start))?;
                let parameter_start = start;
                let name = self.spanned_at(name, text, start);
                self.index += 1;
                let parameter_front = self.parse_kind_parameter_front(parameter_start)?;
                let close = self.lines[self.index - 1];
                front.kind_parameters.push(KindParameter {
                    name,
                    front: parameter_front,
                    span: self.span(parameter_start, close.start + close.text.len()),
                });
                continue;
            }
            if text.contains(">>") {
                self.parse_front_runtime(text, start, &mut front)?;
            } else if !top_level_positions(text, '>').is_empty() {
                return Err((
                    FormError::InvalidSyntax(
                        "'>' is not a Conduitese cord or fore; use '>>'".into(),
                    ),
                    self.span(start, start + text.len()),
                ));
            } else {
                let (left, default) = split_default(text);
                let declaration = split_declaration(left);
                if declaration.is_some_and(|(_, value_type)| value_type == "type") {
                    if default.is_some() {
                        return Err(self.invalid_statement(text, start));
                    }
                    let (name, _) = declaration.expect("type declaration was recognized");
                    front.type_parameters.push(TypeParameter {
                        name: self.spanned_at(name, text, start),
                        span: self.span(start, start + text.len()),
                    });
                } else {
                    front
                        .startup_parameters
                        .push(self.parse_startup(text, start)?);
                }
            }
            self.index += 1;
        }
        Err((FormError::IncompleteForm, eof_span(self.source)))
    }

    pub(super) fn parse_front_runtime(
        &self,
        text: &str,
        start: usize,
        front: &mut FormFront,
    ) -> Result<(), (FormError, Span)> {
        let arrows = top_level_token_positions(text, ">>");
        if arrows.len() != 1 {
            return Err((
                FormError::InvalidSyntax("malformed front arrows".into()),
                self.span(start, start + text.len()),
            ));
        }
        let arrow = arrows[0];
        let left = text[..arrow].trim();
        let right = text[arrow + 2..].trim();
        match (left.is_empty(), right.is_empty()) {
            (true, false) => front.runtime_ports.push(self.runtime_port(
                right,
                text,
                start,
                RuntimePortDirection::Input,
            )?),
            (false, true) => front.runtime_ports.push(self.runtime_port(
                left,
                text,
                start,
                RuntimePortDirection::Output,
            )?),
            (false, false) => {
                if front.shorthand.is_some() {
                    return Err((
                        FormError::InvalidSyntax("more than one shorthand front pair".into()),
                        self.span(start, start + text.len()),
                    ));
                }
                let input = self.runtime_port(left, text, start, RuntimePortDirection::Input)?;
                let output = self.runtime_port(right, text, start, RuntimePortDirection::Output)?;
                front.shorthand = Some(ShorthandPair {
                    input_port: input.name.clone(),
                    output_port: output.name.clone(),
                    span: self.span(start, start + text.len()),
                });
                front.runtime_ports.extend([input, output]);
            }
            (true, true) => return Err(self.invalid_statement(text, start)),
        }
        Ok(())
    }

    fn runtime_port(
        &self,
        declaration: &str,
        line: &str,
        start: usize,
        direction: RuntimePortDirection,
    ) -> Result<RuntimePort, (FormError, Span)> {
        let (name, value_type) =
            split_declaration(declaration).ok_or_else(|| self.invalid_statement(line, start))?;
        let (value_type, refinements) = self.parse_value_refinements(value_type, line, start)?;
        let (value_type, explicit_bound) =
            split_type_bound(value_type).ok_or_else(|| self.invalid_statement(line, start))?;
        let (value_type, temporal) =
            parse_port_type(value_type).ok_or_else(|| self.invalid_statement(line, start))?;
        Ok(RuntimePort {
            name: self.spanned_at(name, line, start),
            value_type: self.spanned_at(value_type, line, start),
            direction,
            temporal,
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            refinements,
            span: self.span(
                start + line.find(declaration).unwrap(),
                start + line.find(declaration).unwrap() + declaration.len(),
            ),
        })
    }

    pub(super) fn parse_value_refinements<'b>(
        &self,
        source: &'b str,
        line: &str,
        start: usize,
    ) -> Result<(&'b str, Vec<crate::ValueRefinement>), (FormError, Span)> {
        let Some(first_relation) = next_refinement(source) else {
            return Ok((source, Vec::new()));
        };
        let value_type = source[..first_relation].trim();
        let source_offset = line.find(source).unwrap_or(0);
        let mut cursor = first_relation;
        let mut refinements = Vec::new();
        while cursor < source.len() {
            let remaining = &source[cursor..];
            let (negated, relation, body_start) = if remaining.starts_with(" not in ") {
                (true, "in", cursor + " not in ".len())
            } else if remaining.starts_with(" in ") {
                (false, "in", cursor + " in ".len())
            } else if remaining.starts_with(" !~ ") {
                (true, "pattern", cursor + " !~ ".len())
            } else if remaining.starts_with(" ~ ") {
                (false, "pattern", cursor + " ~ ".len())
            } else {
                return Err(self.invalid_statement(line, start));
            };
            if relation == "pattern" {
                let (pattern, case_insensitive, anchored_start, anchored_end, consumed) =
                    slash_pattern(&source[body_start..])
                        .ok_or_else(|| self.invalid_statement(line, start))?;
                let clause_end = body_start + consumed;
                let pattern_offset = start
                    + source_offset
                    + body_start
                    + source[body_start..clause_end].find(pattern).unwrap();
                refinements.push(crate::ValueRefinement::TextPattern {
                    source: self.spanned(pattern, pattern_offset),
                    case_insensitive,
                    anchored_start,
                    anchored_end,
                    negated,
                    span: self.span(
                        start + source_offset + cursor,
                        start + source_offset + clause_end,
                    ),
                });
                cursor = clause_end;
                continue;
            }
            if source[body_start..].starts_with('[') {
                let consumed = bracketed_members(&source[body_start..])
                    .ok_or_else(|| self.invalid_statement(line, start))?;
                let clause_end = body_start + consumed;
                let body = &source[body_start + 1..clause_end - 1];
                let values = split_top_level(body, ',');
                if values.is_empty() || values.len() > conduit_core::MAX_MEMBERSHIP_VALUES {
                    return Err(self.invalid_statement(line, start));
                }
                let mut members = Vec::with_capacity(values.len());
                let mut member_search = 0;
                for value in values {
                    let value = value.trim();
                    if value.is_empty() {
                        return Err(self.invalid_statement(line, start));
                    }
                    let relative = body[member_search..]
                        .find(value)
                        .map(|offset| member_search + offset)
                        .unwrap();
                    member_search = relative + value.len();
                    let offset = start + source_offset + body_start + 1 + relative;
                    members.push(self.spanned(value, offset));
                }
                refinements.push(crate::ValueRefinement::Membership {
                    members,
                    negated,
                    span: self.span(
                        start + source_offset + cursor,
                        start + source_offset + clause_end,
                    ),
                });
                cursor = clause_end;
                continue;
            }
            if negated {
                return Err(self.invalid_statement(line, start));
            }
            let tail = &source[body_start..];
            let consumed = next_refinement(tail).unwrap_or(tail.len());
            let interval = tail[..consumed].trim();
            let (minimum, maximum, maximum_endpoint) =
                if let Some((minimum, maximum)) = interval.split_once("..=") {
                    (
                        minimum.trim(),
                        maximum.trim(),
                        crate::RefinementIntervalEndpoint::Inclusive,
                    )
                } else if let Some((minimum, maximum)) = interval.split_once("..") {
                    (
                        minimum.trim(),
                        maximum.trim(),
                        crate::RefinementIntervalEndpoint::Exclusive,
                    )
                } else {
                    return Err(self.invalid_statement(line, start));
                };
            if minimum.is_empty() && maximum.is_empty() {
                return Err(self.invalid_statement(line, start));
            }
            let clause_end = body_start + consumed;
            let minimum = (!minimum.is_empty()).then(|| {
                let offset = start + source_offset + body_start + interval.find(minimum).unwrap();
                self.spanned(minimum, offset)
            });
            let maximum = (!maximum.is_empty()).then(|| {
                let offset = start + source_offset + body_start + interval.rfind(maximum).unwrap();
                self.spanned(maximum, offset)
            });
            refinements.push(crate::ValueRefinement::Range {
                minimum,
                maximum,
                minimum_endpoint: crate::RefinementIntervalEndpoint::Inclusive,
                maximum_endpoint,
                span: self.span(
                    start + source_offset + cursor,
                    start + source_offset + clause_end,
                ),
            });
            cursor = clause_end;
        }
        Ok((value_type, refinements))
    }
}

fn next_refinement(source: &str) -> Option<usize> {
    [" not in ", " in ", " !~ ", " ~ "]
        .into_iter()
        .filter_map(|token| source.find(token))
        .min()
}

fn bracketed_members(source: &str) -> Option<usize> {
    if !source.starts_with('[') {
        return None;
    }
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' && quoted {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if character == ']' && !quoted {
            return Some(offset + 1);
        }
    }
    None
}

fn slash_pattern(source: &str) -> Option<(&str, bool, bool, bool, usize)> {
    if !source.starts_with('/') {
        return None;
    }
    let mut escaped = false;
    let mut class = false;
    for (offset, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' => class = true,
            ']' => class = false,
            '/' if !class => {
                let flags_end = source[offset + 1..]
                    .find(|character: char| !character.is_ascii_alphabetic())
                    .map_or(source.len(), |relative| offset + 1 + relative);
                let flags = &source[offset + 1..flags_end];
                let case_insensitive = match flags {
                    "" => false,
                    "i" => true,
                    _ => return None,
                };
                let mut pattern = &source[1..offset];
                let anchored_start = pattern.starts_with('^');
                if anchored_start {
                    pattern = &pattern[1..];
                }
                let anchored_end = pattern.ends_with('$') && trailing_dollar_is_anchor(pattern);
                if anchored_end {
                    pattern = &pattern[..pattern.len() - 1];
                }
                if pattern.is_empty() {
                    return None;
                }
                return Some((
                    pattern,
                    case_insensitive,
                    anchored_start,
                    anchored_end,
                    flags_end,
                ));
            }
            _ => {}
        }
    }
    None
}

fn trailing_dollar_is_anchor(pattern: &str) -> bool {
    let preceding_backslashes = pattern[..pattern.len().saturating_sub(1)]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count();
    preceding_backslashes % 2 == 0
}
