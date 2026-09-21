//! Direct optimized Morse browser installation.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::HostedValueStore;

pub(super) const DIRECT_IMPLEMENTATION: &str = "browser/kernel-text-morse-direct@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-morse@1";
const HOST_CALL: &str = "conduit.host/browser-text-to-morse@1";

pub(super) static DIRECT: BrowserInstallation = BrowserInstallation {
    implementation_id: DIRECT_IMPLEMENTATION,
    offer: direct_offer,
    prepare,
    perform: Some(perform),
};

fn direct_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_text::text_morse_semantics().into_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("browser/text-morse-direct@1"),
            execution_profile_id: ExecutionProfileId::from("browser/kernel-text-morse-direct@1"),
            implementation_id: ImplementationId::from(DIRECT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(HOST_CALL),
                target_kind: Some(kind_id("text/morse-pattern")),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
                maximum_output_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, _values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate_placement(placement, &direct_offer())?;
    unit_millis(placement)?;
    Ok(BrowserBack::unary(
        placement.host_calls[0].maximum_input_bytes,
        1,
    ))
}

fn perform(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let text = core::str::from_utf8(input).map_err(|_| "text/morse input is not UTF-8")?;
    let encoded = conduit_text::MorsePattern::from_text(text, unit_millis(placement)?)
        .and_then(|pattern| pattern.encode())
        .map_err(|error| format!("encode Morse pattern: {error:?}"))?;
    Ok(BrowserHostResult {
        output: Some(encoded),
        manifestation: None,
    })
}

pub(super) fn unit_millis(placement: &PlannedGear) -> Result<u16, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_offer_preserves_the_portable_morse_contract() {
        let offer = direct_offer();
        let semantic = conduit_text::text_morse_semantics().into_semantic_contract();
        assert_eq!(offer.startup_parameters, semantic.startup_parameters);
        assert_eq!(offer.shorthand, semantic.shorthand);
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
        assert_eq!(offer.host_calls.len(), 1);
    }
}
