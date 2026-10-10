//! Compile-time Info substitution into existing authored scalar contracts.
use super::{error, integer};
use crate::prelude::*;
use crate::{SpannedText, SyntaxCheckDiagnostic, ValueRefinement};
use alloc::collections::BTreeMap;

pub(super) fn substitute(
    refinements: &[ValueRefinement],
    values: &BTreeMap<String, u16>,
) -> Result<Vec<ValueRefinement>, SyntaxCheckDiagnostic> {
    refinements
        .iter()
        .map(|refinement| {
            Ok(match refinement {
                ValueRefinement::Range {
                    minimum,
                    maximum,
                    minimum_endpoint,
                    maximum_endpoint,
                    span,
                } => ValueRefinement::Range {
                    minimum: minimum
                        .as_ref()
                        .map(|value| operand(value, values))
                        .transpose()?,
                    maximum: maximum
                        .as_ref()
                        .map(|value| operand(value, values))
                        .transpose()?,
                    minimum_endpoint: *minimum_endpoint,
                    maximum_endpoint: *maximum_endpoint,
                    span: *span,
                },
                ValueRefinement::Membership {
                    members,
                    negated,
                    span,
                } => ValueRefinement::Membership {
                    members: members
                        .iter()
                        .map(|value| operand(value, values))
                        .collect::<Result<_, _>>()?,
                    negated: *negated,
                    span: *span,
                },
                _ => refinement.clone(),
            })
        })
        .collect()
}

fn operand(
    source: &SpannedText,
    values: &BTreeMap<String, u16>,
) -> Result<SpannedText, SyntaxCheckDiagnostic> {
    // Quoted members and other domains' literals keep their existing meaning.
    // Only operands referencing actual Info bindings enter this evaluator.
    if source.text.starts_with('"')
        || source.text.starts_with('\'')
        || !mentions(&source.text, values)
    {
        return Ok(source.clone());
    }
    let expression =
        crate::surface_parser::parse_integer_spanned(source).map_err(|(refusal, span)| {
            error(
                span,
                alloc::format!("invalid native Info refinement operand: {refusal:?}"),
            )
        })?;
    let normalized = integer::normalize(&expression, values)?;
    Ok(SpannedText {
        text: normalized
            .literal_value()
            .expect("normalized integer")
            .to_string(),
        span: source.span,
    })
}

fn mentions(source: &str, values: &BTreeMap<String, u16>) -> bool {
    source
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .any(|name| values.contains_key(name))
}

pub(super) fn uses(refinements: &[ValueRefinement], parameter: &str) -> bool {
    let mut values = BTreeMap::new();
    values.insert(parameter.into(), 0);
    refinements.iter().any(|refinement| match refinement {
        ValueRefinement::Range {
            minimum, maximum, ..
        } => minimum
            .iter()
            .chain(maximum)
            .any(|source| mentions(&source.text, &values)),
        ValueRefinement::Membership { members, .. } => members.iter().any(|source| {
            !source.text.starts_with('"')
                && !source.text.starts_with('\'')
                && mentions(&source.text, &values)
        }),
        _ => false,
    })
}

#[cfg(test)]
mod tests;
