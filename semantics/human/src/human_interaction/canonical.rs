use alloc::{string::String, vec::Vec};
use sha2::{Digest, Sha256};

use super::{InteractionDomain, InteractionFamily, InteractionValue, OptionAvailability};
use conduit_core::QuantityUnit;

pub(super) fn encode_family(output: &mut Vec<u8>, family: &InteractionFamily) {
    match family {
        InteractionFamily::Activate => output.push(0),
        InteractionFamily::Boolean => output.push(1),
        InteractionFamily::ChooseOne {
            value_kind,
            maximum_options,
        } => {
            output.push(2);
            field(output, value_kind.as_str().as_bytes());
            output.extend_from_slice(&maximum_options.to_le_bytes());
        }
        InteractionFamily::ChooseMany {
            value_kind,
            maximum_options,
            minimum_selections,
            maximum_selections,
        } => {
            output.push(3);
            field(output, value_kind.as_str().as_bytes());
            output.extend_from_slice(&maximum_options.to_le_bytes());
            output.extend_from_slice(&minimum_selections.to_le_bytes());
            output.extend_from_slice(&maximum_selections.to_le_bytes());
        }
        InteractionFamily::Scalar {
            unit,
            minimum,
            minimum_bound,
            maximum,
            maximum_bound,
            granularity,
        } => {
            output.push(4);
            encode_quantity_profile(output, *unit, *minimum, *maximum, *granularity);
            output.push(*minimum_bound as u8);
            output.push(*maximum_bound as u8);
        }
        InteractionFamily::RelativeAdjustment {
            unit,
            minimum_delta,
            maximum_delta,
            granularity,
        } => {
            output.push(5);
            encode_quantity_profile(output, *unit, *minimum_delta, *maximum_delta, *granularity);
        }
        InteractionFamily::Text {
            maximum_bytes,
            allow_empty,
        } => {
            output.push(6);
            output.extend_from_slice(&maximum_bytes.to_le_bytes());
            output.push(u8::from(*allow_empty));
        }
        InteractionFamily::Structured {
            value_kind,
            type_digest,
            maximum_bytes,
        } => {
            output.push(7);
            field(output, value_kind.as_str().as_bytes());
            output.extend_from_slice(type_digest);
            output.extend_from_slice(&maximum_bytes.to_le_bytes());
        }
    }
}

fn encode_quantity_profile(
    output: &mut Vec<u8>,
    unit: QuantityUnit,
    minimum: i64,
    maximum: i64,
    granularity: i64,
) {
    field(output, unit.semantic_id().as_bytes());
    output.extend_from_slice(&minimum.to_le_bytes());
    output.extend_from_slice(&maximum.to_le_bytes());
    output.extend_from_slice(&granularity.to_le_bytes());
}

pub(super) fn encode_domain(output: &mut Vec<u8>, domain: &InteractionDomain) {
    output.extend_from_slice(&domain.revision.to_le_bytes());
    let mut options = domain.options.iter().collect::<Vec<_>>();
    options.sort_by(|left, right| left.identity.cmp(&right.identity));
    output.extend_from_slice(&(options.len() as u16).to_le_bytes());
    for option in options {
        field(output, option.identity.as_bytes());
        encode_value(output, &option.value);
        match &option.availability {
            OptionAvailability::Available => output.push(0),
            OptionAvailability::Unavailable { reason_code } => {
                output.push(1);
                field(output, reason_code.as_bytes());
            }
        }
    }
}

pub(super) fn values(output: &mut Vec<u8>, items: &[InteractionValue]) {
    output.extend_from_slice(&(items.len() as u16).to_le_bytes());
    for value in items {
        encode_value(output, value);
    }
}

pub(super) fn encode_value(output: &mut Vec<u8>, value: &InteractionValue) {
    field(output, value.value_kind.as_str().as_bytes());
    field(output, &value.canonical_bytes);
}

pub(super) fn field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

pub(super) fn identity(domain: &str, canonical: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conduit.human-interaction@1\0");
    digest.update(domain.as_bytes());
    digest.update(b"\0");
    digest.update(canonical);
    let digest: [u8; 32] = digest.finalize().into();
    let mut output = String::with_capacity(domain.len() + 65);
    output.push_str(domain);
    output.push('/');
    for byte in digest {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
