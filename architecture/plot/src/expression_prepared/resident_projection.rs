//! Finite immutable paths over already checked canonical input values.
use super::Refusal;
use alloc::{format, string::String, vec::Vec};
use conduit_core::ValidatedCanonicalStructuredValue as Value;
#[derive(Clone, Copy, Debug)]
pub enum StaticExpressionPathStep {
    Field(&'static str),
    Index(u16),
    Variant(&'static str),
}
pub(super) enum OwnedExpressionPathStep {
    Field(String),
    Index(u16),
    Variant(String),
}
pub(super) fn render(path: &[OwnedExpressionPathStep]) -> String {
    path.iter()
        .map(|step| {
            let op = match step {
                OwnedExpressionPathStep::Field(name) => format!("Field({name:?})"),
                OwnedExpressionPathStep::Index(index) => format!("Index({index})"),
                OwnedExpressionPathStep::Variant(name) => format!("Variant({name:?})"),
            };
            format!("conduit_plot::StaticExpressionPathStep::{op}")
        })
        .collect::<Vec<_>>()
        .join(",")
}
pub(super) fn valid(path: &[StaticExpressionPathStep]) -> bool {
    path.len() <= conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH
        && path.iter().all(|step| match step {
            StaticExpressionPathStep::Field(name) | StaticExpressionPathStep::Variant(name) => {
                !name.is_empty() && name.len() <= conduit_core::MAXIMUM_STRUCTURED_NAME_BYTES
            }
            StaticExpressionPathStep::Index(_) => true,
        })
}
pub(super) fn select<'a>(
    mut value: Value<'a>,
    path: &[StaticExpressionPathStep],
) -> Result<Value<'a>, Refusal> {
    for step in path {
        let representation = value
            .representation()
            .map_err(|_| Refusal::InvalidProgram)?;
        value = match step {
            StaticExpressionPathStep::Field(name) => representation.record_field(name),
            StaticExpressionPathStep::Index(index) => representation.collection_index(*index),
            StaticExpressionPathStep::Variant(name) => representation.variant_payload(name),
        }
        .map_err(|_| Refusal::InvalidProgram)?
        .ok_or(Refusal::InvalidProgram)?;
    }
    Ok(value)
}
