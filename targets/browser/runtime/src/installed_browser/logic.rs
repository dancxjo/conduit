//! Browser installations for finite exact Boolean decisions.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ConfigurationValue, HostOperationContractId, HostOperationRequirement, InfoBool,
    PlannedGear, BOOL_ENCODED_LEN,
};
use conduit_kernel::{HostedValueStore, ValueStorage};

const ARTIFACT: &str = "conduit-browser-runtime/installed-logic@1";
const COMPARE_IMPLEMENTATION: &str = "browser/kernel-logic-compare@1";
const NOT_IMPLEMENTATION: &str = "browser/kernel-logic-not@1";
const SELECT_IMPLEMENTATION: &str = "browser/kernel-logic-select-scalar@1";

pub(super) static COMPARE: BrowserInstallation = BrowserInstallation {
    implementation_id: COMPARE_IMPLEMENTATION,
    offer: compare_offer,
    prepare: prepare_compare,
    perform: None,
};
pub(super) static NOT: BrowserInstallation = BrowserInstallation {
    implementation_id: NOT_IMPLEMENTATION,
    offer: not_offer,
    prepare: prepare_not,
    perform: Some(perform_not),
};
pub(super) static SELECT: BrowserInstallation = BrowserInstallation {
    implementation_id: SELECT_IMPLEMENTATION,
    offer: select_offer,
    prepare: prepare_select,
    perform: None,
};

fn compare_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::logic_compare_scalar_contract(),
        conduit_semantic_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        COMPARE_IMPLEMENTATION,
        Vec::new(),
    )
}

fn not_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::logic_not_contract(),
        conduit_semantic_catalog::LOGIC_NOT_CONTRACT_REVISION,
        NOT_IMPLEMENTATION,
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NOT_IMPLEMENTATION),
            target_kind: Some(kind_id(NOT_IMPLEMENTATION)),
            maximum_in_flight: 1,
            maximum_input_bytes: BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: BOOL_ENCODED_LEN as u32,
        }],
    )
}

fn select_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::logic_select_scalar_contract(),
        conduit_semantic_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        SELECT_IMPLEMENTATION,
        Vec::new(),
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    implementation: &str,
    operations: Vec<HostOperationRequirement>,
) -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        contract,
        revision,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: ARTIFACT,
        },
        operations,
        Vec::new(),
        Vec::new(),
    )
}

fn prepare_compare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &compare_offer())?;
    let operator = comparison_operator(placement)?;
    let false_value = values
        .store(&InfoBool::FALSE.encode())
        .map_err(debug_error)?;
    let true_value = values
        .store(&InfoBool::TRUE.encode())
        .map_err(debug_error)?;
    Ok(BrowserOperation::compare_scalar(
        operator,
        false_value,
        true_value,
    ))
}

fn prepare_not(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &not_offer())?;
    Ok(BrowserOperation::unary(BOOL_ENCODED_LEN as u32, 1))
}

fn prepare_select(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &select_offer())?;
    Ok(BrowserOperation::select_scalar())
}

fn perform_not(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let value = InfoBool::decode(input).map_err(debug_error)?;
    Ok(BrowserHostResult {
        output: Some(InfoBool::new(!value.get()).encode().to_vec()),
        manifestation: None,
    })
}

fn comparison_operator(
    placement: &PlannedGear,
) -> Result<conduit_semantic_catalog::ScalarComparison, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (conduit_semantic_catalog::COMPARE_OPERATOR_KEY, ConfigurationValue::Text(value)) => {
                conduit_semantic_catalog::ScalarComparison::parse(value)
            }
            _ => None,
        })
        .ok_or_else(|| "logic/compare operator is invalid".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
