//! Parsing for the checked callable boundary of one authored Form.

use super::Parser;
use crate::surface_lex::{
    split_declaration, split_top_level, split_top_level_token, top_level_positions,
    top_level_token_positions,
};
use crate::syntax::{
    FormFront, RuntimePort, RuntimePortDirection, RuntimePortTemporal, ShorthandPair,
    StartupParameter, TypeParameter,
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

fn split_type_bound(value_type: &str) -> Option<(&str, Option<u64>)> {
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
        _ => None,
    }
}

fn split_default(text: &str) -> (&str, Option<&str>) {
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
    pub(super) fn parse_front(&mut self, open: usize) -> Result<FormFront, (FormError, Span)> {
        let mut front = FormFront::default();
        while self.index < self.lines.len() {
            let line = self.lines[self.index];
            let (text, start) = line.statement();
            if text == ") {" || text == "){" {
                front.span = Some(self.span(open, start + text.find(')').unwrap() + 1));
                self.index += 1;
                return Ok(front);
            }
            if text.is_empty() || text.starts_with('#') {
                self.index += 1;
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

    fn parse_startup(
        &self,
        text: &str,
        start: usize,
    ) -> Result<StartupParameter, (FormError, Span)> {
        let (left, default) = split_default(text);
        let (name, value_type) =
            split_declaration(left).ok_or_else(|| self.invalid_statement(text, start))?;
        let span = self.span(start, start + text.len());
        let (value_type, refinements) = self.parse_value_refinements(value_type, text, start)?;
        let (value_type, explicit_bound) =
            split_type_bound(value_type).ok_or_else(|| self.invalid_statement(text, start))?;
        let (value_type, temporal) =
            parse_port_type(value_type).ok_or_else(|| self.invalid_statement(text, start))?;
        if !matches!(
            temporal,
            RuntimePortTemporal::Value | RuntimePortTemporal::OptionalValue
        ) {
            return Err(self.invalid_statement(text, start));
        }
        Ok(StartupParameter {
            name: self.spanned_at(name, text, start),
            value_type: self.spanned_at(value_type, text, start),
            optional: temporal == RuntimePortTemporal::OptionalValue,
            maximum_bytes: explicit_bound.or_else(|| canonical_default_bound(value_type)),
            refinements,
            default: default
                .map(|value| self.expression_at(value, text, start))
                .transpose()?,
            span,
        })
    }

    fn parse_front_runtime(
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

    fn parse_value_refinements<'b>(
        &self,
        source: &'b str,
        line: &str,
        start: usize,
    ) -> Result<(&'b str, Vec<crate::ValueRefinement>), (FormError, Span)> {
        let Some((value_type, refinement)) = source.split_once(" where ") else {
            return Ok((source, Vec::new()));
        };
        let value_type = value_type.trim();
        let refinement = refinement.trim();
        let clauses = split_top_level_token(refinement, " and ");
        if clauses.is_empty() || clauses.iter().any(|clause| clause.trim().is_empty()) {
            return Err(self.invalid_statement(line, start));
        }
        let refinement_offset = line
            .find(refinement)
            .expect("refinement is an exact slice of the declaration line");
        let mut refinements = Vec::with_capacity(clauses.len());
        let mut search_from = 0;
        for clause in clauses {
            let clause = clause.trim();
            let relative = refinement[search_from..]
                .find(clause)
                .map(|offset| search_from + offset)
                .expect("refinement clause is an exact slice of the refinement");
            search_from = relative + clause.len();
            let clause_start = start + refinement_offset + relative;
            let span = self.span(clause_start, clause_start + clause.len());
            if let Some(pattern) = clause
                .strip_prefix("pattern(r\"")
                .and_then(|tail| tail.strip_suffix("\")"))
            {
                let pattern_offset = clause_start
                    + clause
                        .find(pattern)
                        .expect("pattern is an exact slice of the refinement clause");
                refinements.push(crate::ValueRefinement::TextPattern {
                    source: self.spanned(pattern, pattern_offset),
                    span,
                });
                continue;
            }
            if let Some(interval) = clause
                .strip_prefix("range(")
                .and_then(|tail| tail.strip_suffix(')'))
            {
                let values = split_top_level(interval, ',');
                let [minimum, maximum] = values.as_slice() else {
                    return Err(self.invalid_statement(line, start));
                };
                let (minimum, minimum_endpoint) = parse_interval_endpoint(minimum)
                    .ok_or_else(|| self.invalid_statement(line, start))?;
                let (maximum, maximum_endpoint) = parse_interval_endpoint(maximum)
                    .ok_or_else(|| self.invalid_statement(line, start))?;
                let minimum_offset = clause_start + clause.find(minimum).unwrap();
                let maximum_offset = clause_start + clause.rfind(maximum).unwrap();
                refinements.push(crate::ValueRefinement::Range {
                    minimum: self.spanned(minimum, minimum_offset),
                    maximum: self.spanned(maximum, maximum_offset),
                    minimum_endpoint,
                    maximum_endpoint,
                    span,
                });
                continue;
            }
            if let Some(body) = clause
                .strip_prefix("member(")
                .and_then(|tail| tail.strip_suffix(')'))
            {
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
                    let offset = clause_start + "member(".len() + relative;
                    members.push(self.spanned(value, offset));
                }
                refinements.push(crate::ValueRefinement::Membership { members, span });
                continue;
            }
            return Err(self.invalid_statement(line, start));
        }
        Ok((value_type, refinements))
    }
}

fn parse_interval_endpoint(source: &str) -> Option<(&str, crate::RefinementIntervalEndpoint)> {
    let source = source.trim();
    let (value, endpoint) = source.rsplit_once(' ')?;
    let endpoint = match endpoint {
        "inclusive" => crate::RefinementIntervalEndpoint::Inclusive,
        "exclusive" => crate::RefinementIntervalEndpoint::Exclusive,
        _ => return None,
    };
    let value = value.trim();
    (!value.is_empty()).then_some((value, endpoint))
}
