//! Browser installations for the typed finite Morse composition verbs.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    PlannedGear,
};
use conduit_kernel::HostedValueStore;

const ARTIFACT: &str = "conduit-browser-runtime/installed-morse-composition@1";
const TEXT_CHARACTERS_IMPLEMENTATION: &str = "browser/kernel-text-characters@1";
const LOOKUP_IMPLEMENTATION: &str = "browser/kernel-morse-lookup@1";
const INTERSPERSE_IMPLEMENTATION: &str = "browser/kernel-morse-intersperse@1";
const FLATTEN_IMPLEMENTATION: &str = "browser/kernel-morse-flatten@1";
const SYMBOLS_TO_PATTERN_IMPLEMENTATION: &str = "browser/kernel-morse-symbols-to-pattern@1";
const PATTERN_TO_SYMBOLS_IMPLEMENTATION: &str = "browser/kernel-morse-pattern-to-symbols@1";
const SYMBOLS_TO_TEXT_IMPLEMENTATION: &str = "browser/kernel-morse-symbols-to-text@1";

pub(super) static TEXT_CHARACTERS: BrowserInstallation = installation(
    TEXT_CHARACTERS_IMPLEMENTATION,
    text_characters_offer,
    perform_text_characters,
);
pub(super) static LOOKUP: BrowserInstallation =
    installation(LOOKUP_IMPLEMENTATION, lookup_offer, perform_lookup);
pub(super) static INTERSPERSE: BrowserInstallation = installation(
    INTERSPERSE_IMPLEMENTATION,
    intersperse_offer,
    perform_intersperse,
);
pub(super) static FLATTEN: BrowserInstallation =
    installation(FLATTEN_IMPLEMENTATION, flatten_offer, perform_flatten);
pub(super) static SYMBOLS_TO_PATTERN: BrowserInstallation = installation(
    SYMBOLS_TO_PATTERN_IMPLEMENTATION,
    symbols_to_pattern_offer,
    perform_symbols_to_pattern,
);
pub(super) static PATTERN_TO_SYMBOLS: BrowserInstallation = installation(
    PATTERN_TO_SYMBOLS_IMPLEMENTATION,
    pattern_to_symbols_offer,
    perform_pattern_to_symbols,
);
pub(super) static SYMBOLS_TO_TEXT: BrowserInstallation = installation(
    SYMBOLS_TO_TEXT_IMPLEMENTATION,
    symbols_to_text_offer,
    perform_symbols_to_text,
);

const fn installation(
    implementation_id: &'static str,
    offer: fn() -> CapabilityOffer,
    perform: fn(&PlannedGear, &[u8]) -> Result<BrowserHostResult, String>,
) -> BrowserInstallation {
    BrowserInstallation {
        implementation_id,
        offer,
        prepare,
        perform: Some(perform),
    }
}

fn text_characters_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_characters_semantics(),
        TEXT_CHARACTERS_IMPLEMENTATION,
    )
}
fn lookup_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_lookup_semantics(),
        LOOKUP_IMPLEMENTATION,
    )
}
fn intersperse_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_intersperse_semantics(),
        INTERSPERSE_IMPLEMENTATION,
    )
}
fn flatten_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_flatten_semantics(),
        FLATTEN_IMPLEMENTATION,
    )
}
fn symbols_to_pattern_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_symbols_to_pattern_semantics(),
        SYMBOLS_TO_PATTERN_IMPLEMENTATION,
    )
}
fn pattern_to_symbols_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_pattern_to_symbols_semantics(),
        PATTERN_TO_SYMBOLS_IMPLEMENTATION,
    )
}
fn symbols_to_text_offer() -> CapabilityOffer {
    offer(
        conduit_text::morse_symbols_to_text_semantics(),
        SYMBOLS_TO_TEXT_IMPLEMENTATION,
    )
}

fn offer(contract: conduit_text::MorseKindContract, implementation: &str) -> CapabilityOffer {
    let maximum_input_bytes = contract
        .limits
        .max_queue_bytes
        .min(super::MAXIMUM_BROWSER_VALUE_BYTES as u32);
    let mut offer = CapabilityOffer {
        startup_parameters: contract
            .configuration
            .iter()
            .map(|(name, _)| FaceStartupParameter {
                name: (*name).into(),
                value_type: "Count".into(),
                has_default: true,
            })
            .collect(),
        shorthand: Some((
            contract.inputs[0].port_id.clone(),
            contract.outputs[0].port_id.clone(),
        )),
        capability_id: CapabilityId::from(implementation),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(implementation),
            target_kind: Some(kind_id(implementation)),
            maximum_in_flight: 1,
            maximum_input_bytes,
            maximum_output_bytes: maximum_input_bytes,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    };
    offer.limits.max_queue_bytes = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32;
    offer
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    let offer = installed_offer(placement.implementation_id.as_str())
        .ok_or_else(|| "unknown Morse composition installation".to_string())?;
    validate_placement(placement, &offer)?;
    Ok(BrowserOperation::unary(
        placement.host_operations[0].maximum_input_bytes,
        1,
    ))
}

fn installed_offer(implementation: &str) -> Option<CapabilityOffer> {
    Some(match implementation {
        TEXT_CHARACTERS_IMPLEMENTATION => text_characters_offer(),
        LOOKUP_IMPLEMENTATION => lookup_offer(),
        INTERSPERSE_IMPLEMENTATION => intersperse_offer(),
        FLATTEN_IMPLEMENTATION => flatten_offer(),
        SYMBOLS_TO_PATTERN_IMPLEMENTATION => symbols_to_pattern_offer(),
        PATTERN_TO_SYMBOLS_IMPLEMENTATION => pattern_to_symbols_offer(),
        SYMBOLS_TO_TEXT_IMPLEMENTATION => symbols_to_text_offer(),
        _ => return None,
    })
}

fn result(output: Vec<u8>) -> Result<BrowserHostResult, String> {
    if output.len() > super::MAXIMUM_BROWSER_VALUE_BYTES {
        return Err("Morse composition output exceeded the browser value bound".into());
    }
    Ok(BrowserHostResult {
        output: Some(output),
        manifestation: None,
    })
}

fn perform_text_characters(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let text = core::str::from_utf8(input).map_err(|_| "text/characters input is not UTF-8")?;
    result(conduit_text::morse_characters_from_text(text).map_err(debug_error)?)
}
fn perform_lookup(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    result(conduit_text::morse_lookup_characters(input).map_err(debug_error)?)
}
fn perform_intersperse(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    result(conduit_text::morse_intersperse_gaps(input).map_err(debug_error)?)
}
fn perform_flatten(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    result(conduit_text::morse_flatten_groups(input).map_err(debug_error)?)
}
fn perform_symbols_to_pattern(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    result(
        conduit_text::morse_symbols_to_pattern(input, unit_millis(placement)?)
            .map_err(debug_error)?,
    )
}
fn perform_pattern_to_symbols(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    result(conduit_text::morse_pattern_to_symbols(input).map_err(debug_error)?)
}
fn perform_symbols_to_text(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    result(conduit_text::morse_symbols_to_text(input).map_err(debug_error)?)
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
        .ok_or_else(|| "Morse timing Gear has invalid unit-ms".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
