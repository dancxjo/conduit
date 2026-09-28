//! Parsing for the checked callable boundary of one authored Form.

use super::Parser;
use crate::surface_lex::{
    split_declaration, split_top_level_token, top_level_positions, top_level_token_positions,
};
use crate::syntax::{
    FormFront, RuntimePort, RuntimePortDirection, RuntimePortTemporal, ShorthandPair,
    StartupParameter,
};
use crate::{eof_span, FormError, Span};

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

pub(super) fn canonical_default_bound(value_type: &str) -> Option<u64> {
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
                front
                    .startup_parameters
                    .push(self.parse_startup(text, start)?);
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
            span: self.span(
                start + line.find(declaration).unwrap(),
                start + line.find(declaration).unwrap() + declaration.len(),
            ),
        })
    }
}
