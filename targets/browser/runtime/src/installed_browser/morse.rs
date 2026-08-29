//! Direct optimized Morse browser installation.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, FaceStartupParameter, HostOperationContractId, HostOperationRequirement,
    ImplementationId, PlannedGear,
};
use conduit_kernel::HostedValueStore;

pub(super) const DIRECT_IMPLEMENTATION: &str = "browser/kernel-text-morse-direct@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-morse@1";
const HOST_OPERATION: &str = "conduit.host/browser-text-to-morse@1";

pub(super) static DIRECT: BrowserInstallation = BrowserInstallation {
    implementation_id: DIRECT_IMPLEMENTATION,
    offer: direct_offer,
    prepare,
    perform: Some(perform),
};

fn direct_offer() -> CapabilityOffer {
    let contract = conduit_text::text_morse_semantics();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: conduit_text::MORSE_UNIT_MILLIS_KEY.into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: Some((port_id("text"), port_id("pattern"))),
        capability_id: CapabilityId::from("browser/text-morse-direct@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/kernel-text-morse-direct@1"),
            implementation_id: ImplementationId::from(DIRECT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(kind_id("text/morse-pattern")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
            maximum_output_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &direct_offer())?;
    unit_millis(placement)?;
    Ok(BrowserOperation::unary(
        placement.host_operations[0].maximum_input_bytes,
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
