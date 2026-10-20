//! Wording for the exact checked constraints of a Face action argument.
use alloc::{format, string::String, vec::Vec};
use conduit_core::{IntervalEndpoint, ValueConstraint};

pub(super) fn contract_constraints_clause(contract: &conduit_core::CheckedValueContract) -> String {
    let constraints = contract
        .constraints
        .iter()
        .map(constraint_clause)
        .collect::<Vec<_>>()
        .join("; ");
    if constraints.is_empty() {
        "no additional value constraint".into()
    } else {
        constraints
    }
}

fn constraint_clause(constraint: &ValueConstraint) -> String {
    match constraint {
        ValueConstraint::ByteLength { minimum, maximum } => {
            format!("between {minimum} and {maximum} bytes")
        }
        ValueConstraint::UnsignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause("value", minimum.as_ref(), maximum.as_ref(), *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::SignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause("value", minimum.as_ref(), maximum.as_ref(), *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::FixedIntegerRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "fixed-width integer",
            minimum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            maximum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::QuantityRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "quantity",
            minimum.as_ref().map(|value| format!("{}e{} {}", value.coefficient(), value.exponent(), value.unit().semantic_id())).as_ref(),
            maximum.as_ref().map(|value| format!("{}e{} {}", value.coefficient(), value.exponent(), value.unit().semantic_id())).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::NearQuantity { limit, .. } => format!(
            "certified maximum numerical error {} relative to the retained exact source",
            limit.canonical_literal().unwrap_or_else(|_| format!("{}e{} {}", limit.coefficient(), limit.exponent(), limit.unit().symbol()))
        ),
        ValueConstraint::FloatFinite => "a finite IEEE-754 value".into(),
        ValueConstraint::FloatRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "IEEE-754 value",
            minimum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            maximum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::CanonicalMembership { members, negated } => format!(
            "{} the canonical values {}",
            if *negated { "none of" } else { "one of" },
            members
                .iter()
                .map(|member| canonical_member(member))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValueConstraint::TextPattern {
            pattern,
            anchored_start,
            anchored_end,
            negated,
        } => format!(
            "{}the exact checked portable text pattern as a {} with {} states, start state {}, at most {} characters, and at most {} match steps",
            if *negated { "not " } else { "" },
            pattern_profile(*anchored_start, *anchored_end),
            pattern.states.len(),
            pattern.start_state,
            pattern.maximum_input_characters,
            pattern.maximum_match_steps,
        ),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn pattern_profile(anchored_start: bool, anchored_end: bool) -> &'static str {
    match (anchored_start, anchored_end) {
        (false, false) => "bounded search",
        (true, false) => "prefix match",
        (false, true) => "suffix match",
        (true, true) => "whole-value match",
    }
}

fn range_clause<T: core::fmt::Display>(
    noun: &str,
    minimum: Option<&T>,
    maximum: Option<&T>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> String {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => format!(
            "a {noun} from {} {minimum} through {} {maximum}",
            endpoint_word(minimum_endpoint),
            endpoint_word(maximum_endpoint)
        ),
        (Some(minimum), None) => {
            format!(
                "a {noun} from {} {minimum} with no upper semantic bound",
                endpoint_word(minimum_endpoint)
            )
        }
        (None, Some(maximum)) => {
            format!(
                "a {noun} with no lower semantic bound through {} {maximum}",
                endpoint_word(maximum_endpoint)
            )
        }
        (None, None) => format!("a {noun} with no semantic bounds"),
    }
}

fn endpoint_word(endpoint: IntervalEndpoint) -> &'static str {
    match endpoint {
        IntervalEndpoint::Inclusive => "inclusive",
        IntervalEndpoint::Exclusive => "exclusive",
    }
}

fn canonical_member(member: &[u8]) -> String {
    match core::str::from_utf8(member) {
        Ok(text) if !text.chars().any(char::is_control) => format!("{text:?}"),
        _ => {
            let mut encoded = String::with_capacity(member.len() * 2 + 2);
            encoded.push_str("0x");
            for byte in member {
                use core::fmt::Write;
                write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
            }
            encoded
        }
    }
}
