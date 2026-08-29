//! Browser installations for exact scalar and Boolean value entrances/results.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ConfigurationValue, HostOperationContractId, HostOperationRequirement, InfoBool,
    PlannedGear, Scalar, BOOL_ENCODED_LEN, PRESENTATION_RESOURCE_CLASS, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{HostedValueStore, ValueStorage};

const ARTIFACT: &str = "conduit-browser-runtime/installed-values@1";
const SCALAR_LITERAL_IMPLEMENTATION: &str = "browser/kernel-scalar-literal@1";
const BOOL_LITERAL_IMPLEMENTATION: &str = "browser/kernel-boolean-literal@1";
const SCALAR_PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-scalar@1";
const BOOL_PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-bool-value@1";

pub(super) static SCALAR_LITERAL: BrowserInstallation = BrowserInstallation {
    implementation_id: SCALAR_LITERAL_IMPLEMENTATION,
    offer: scalar_literal_offer,
    prepare: prepare_literal,
    perform: None,
};
pub(super) static BOOL_LITERAL: BrowserInstallation = BrowserInstallation {
    implementation_id: BOOL_LITERAL_IMPLEMENTATION,
    offer: bool_literal_offer,
    prepare: prepare_literal,
    perform: None,
};
pub(super) static SCALAR_PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: SCALAR_PRESENTATION_IMPLEMENTATION,
    offer: scalar_presentation_offer,
    prepare: prepare_presentation,
    perform: Some(perform_scalar_presentation),
};
pub(super) static BOOL_PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: BOOL_PRESENTATION_IMPLEMENTATION,
    offer: bool_presentation_offer,
    prepare: prepare_presentation,
    perform: Some(perform_bool_presentation),
};

fn scalar_literal_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::scalar_literal_contract(),
        SCALAR_LITERAL_IMPLEMENTATION,
        false,
    )
}
fn bool_literal_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::bool_literal_contract(),
        BOOL_LITERAL_IMPLEMENTATION,
        false,
    )
}
fn scalar_presentation_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::scalar_value_presentation_contract(),
        SCALAR_PRESENTATION_IMPLEMENTATION,
        true,
    )
}
fn bool_presentation_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::bool_value_presentation_contract(),
        BOOL_PRESENTATION_IMPLEMENTATION,
        true,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    implementation: &str,
    presentation: bool,
) -> conduit_core::CapabilityOffer {
    let host_operations = presentation
        .then(|| HostOperationRequirement {
            contract_id: HostOperationContractId::from(implementation),
            target_kind: Some(kind_id(implementation)),
            maximum_in_flight: 1,
            maximum_input_bytes: contract.limits.max_queue_bytes,
            maximum_output_bytes: 0,
        })
        .into_iter()
        .collect();
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::VALUE_PRIMITIVE_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: ARTIFACT,
        },
        host_operations,
        presentation
            .then(|| conduit_core::resource_requirement(PRESENTATION_RESOURCE_CLASS, 1))
            .into_iter()
            .collect(),
        Vec::new(),
    )
}

fn prepare_literal(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    let (offer, encoded): (_, Vec<u8>) = match placement.implementation_id.as_str() {
        SCALAR_LITERAL_IMPLEMENTATION => (
            scalar_literal_offer(),
            Scalar::from_raw_microunits(scalar_configuration(placement)?)
                .encode()
                .to_vec(),
        ),
        BOOL_LITERAL_IMPLEMENTATION => (
            bool_literal_offer(),
            InfoBool::new(bool_configuration(placement)?)
                .encode()
                .to_vec(),
        ),
        _ => return Err("unknown value literal installation".into()),
    };
    validate_placement(placement, &offer)?;
    let stored = values.store(&encoded).map_err(debug_error)?;
    Ok(BrowserOperation::source(stored))
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    let offer = match placement.implementation_id.as_str() {
        SCALAR_PRESENTATION_IMPLEMENTATION => scalar_presentation_offer(),
        BOOL_PRESENTATION_IMPLEMENTATION => bool_presentation_offer(),
        _ => return Err("unknown value presentation installation".into()),
    };
    validate_placement(placement, &offer)?;
    Ok(BrowserOperation::presentation(
        placement.host_operations[0].maximum_input_bytes,
        1,
    ))
}

fn perform_scalar_presentation(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Scalar::decode(input).map_err(debug_error)?;
    manifestation(
        conduit_semantic_catalog::SCALAR_VALUE_PRESENTATION_KIND,
        input,
    )
}

fn perform_bool_presentation(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    InfoBool::decode(input).map_err(debug_error)?;
    manifestation(
        conduit_semantic_catalog::BOOL_VALUE_PRESENTATION_KIND,
        input,
    )
}

fn manifestation(kind_id: &'static str, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id,
            canonical_value: input.to_vec(),
        }),
    })
}

fn scalar_configuration(placement: &PlannedGear) -> Result<i64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("value", ConfigurationValue::I64(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "scalar/literal is missing its exact value".into())
}

fn bool_configuration(placement: &PlannedGear) -> Result<bool, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("value", ConfigurationValue::Bool(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "boolean/literal is missing its exact value".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

const _: [(); SCALAR_ENCODED_LEN] = [(); 8];
const _: [(); BOOL_ENCODED_LEN] = [(); 1];
