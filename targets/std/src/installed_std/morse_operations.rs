//! Bounded std realization of the canonical text-to-Morse gallery path.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use super::text_operations::{TextPresentationOperation, TextTransformOperation};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome, ValueRef};

pub(super) static TEXT_MORSE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TEXT_MORSE_IMPLEMENTATION,
    budget: text_morse_budget,
    prepare: prepare_text_morse,
};

pub(super) static INDICATOR_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::INDICATOR_PRESENTATION_IMPLEMENTATION,
    budget: indicator_budget,
    prepare: prepare_indicator,
};

fn text_morse_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_text_morse(placement)?;
    Ok(OperationBudget {
        value_items: 4,
        value_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32 * 4,
        host_requests: 4,
        sign_items: 64,
        maximum_value_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
    })
}

fn prepare_text_morse(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_morse(placement)?;
    Ok(InstalledOperation::TextUpper(
        TextTransformOperation::bounded_input(4, conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32),
    ))
}

fn indicator_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_indicator(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 4,
        sign_items: 64,
        maximum_value_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
    })
}

fn prepare_indicator(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_indicator(placement)?;
    Ok(InstalledOperation::TextPresentation(
        TextPresentationOperation::bounded(4),
    ))
}

fn validate_text_morse(placement: &PlannedGear) -> Result<(), String> {
    validate(placement, &conduit_std_offers::text_morse_offer())?;
    unit_millis(placement).map(|_| ())
}

fn validate_indicator(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::indicator_presentation_offer(),
    )
}

fn validate(placement: &PlannedGear, offer: &conduit_core::CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err(format!(
            "planned {} executable identity does not match its installation",
            offer.kind_id.as_str()
        ));
    }
    Ok(())
}

fn unit_millis(placement: &PlannedGear) -> Result<u16, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (conduit_text::MORSE_UNIT_MILLIS_KEY, ConfigurationValue::U64(value)) => {
                u16::try_from(*value).ok()
            }
            _ => None,
        })
        .filter(|value| {
            (conduit_text::MINIMUM_MORSE_UNIT_MILLIS..=conduit_text::MAXIMUM_MORSE_UNIT_MILLIS)
                .contains(value)
        })
        .ok_or_else(|| "text/morse unit duration is missing or invalid".into())
}

pub(super) fn encode(
    placement: &PlannedGear,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Result<(), String> {
    let text = core::str::from_utf8(input).map_err(|_| "text/morse input is not UTF-8")?;
    *output = conduit_text::MorsePattern::from_text(text, unit_millis(placement)?)
        .and_then(|pattern| pattern.encode())
        .map_err(|error| format!("encode Morse pattern: {error:?}"))?;
    Ok(())
}

pub(super) fn completed_with_output(value: ValueRef) -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: Some(
            BoundedValueRef::new(value, conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32)
                .expect("Morse output was checked against its admitted bound"),
        ),
        failure: None,
    }
}
