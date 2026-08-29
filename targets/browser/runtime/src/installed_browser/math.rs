//! Browser installations for exact bounded scalar transforms.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ConfigurationValue, HostOperationContractId, HostOperationRequirement, PlannedGear,
    Scalar, SCALAR_ENCODED_LEN,
};
use conduit_kernel::HostedValueStore;

const ARTIFACT: &str = "conduit-browser-runtime/installed-math@1";
const CLAMP_IMPLEMENTATION: &str = "browser/kernel-math-clamp@1";
const SCALE_IMPLEMENTATION: &str = "browser/kernel-math-scale@1";
const DEADBAND_IMPLEMENTATION: &str = "browser/kernel-math-deadband@1";

pub(super) static CLAMP: BrowserInstallation =
    installation(CLAMP_IMPLEMENTATION, clamp_offer, perform);
pub(super) static SCALE: BrowserInstallation =
    installation(SCALE_IMPLEMENTATION, scale_offer, perform);
pub(super) static DEADBAND: BrowserInstallation =
    installation(DEADBAND_IMPLEMENTATION, deadband_offer, perform);

const fn installation(
    implementation_id: &'static str,
    offer: fn() -> conduit_core::CapabilityOffer,
    perform: fn(&PlannedGear, &[u8]) -> Result<BrowserHostResult, String>,
) -> BrowserInstallation {
    BrowserInstallation {
        implementation_id,
        offer,
        prepare,
        perform: Some(perform),
    }
}

fn clamp_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::math_clamp_contract(),
        conduit_semantic_catalog::MATH_CLAMP_CONTRACT_REVISION,
        CLAMP_IMPLEMENTATION,
    )
}
fn scale_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::math_scale_contract(),
        conduit_semantic_catalog::MATH_SCALE_CONTRACT_REVISION,
        SCALE_IMPLEMENTATION,
    )
}
fn deadband_offer() -> conduit_core::CapabilityOffer {
    offer(
        conduit_semantic_catalog::math_deadband_contract(),
        conduit_semantic_catalog::MATH_DEADBAND_CONTRACT_REVISION,
        DEADBAND_IMPLEMENTATION,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    implementation: &str,
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
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(implementation),
            target_kind: Some(kind_id(implementation)),
            maximum_in_flight: 1,
            maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: SCALAR_ENCODED_LEN as u32,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    let offer = installed_offer(placement.implementation_id.as_str())
        .ok_or_else(|| "unknown math installation".to_string())?;
    validate_placement(placement, &offer)?;
    transform(placement, Scalar::ZERO)?;
    Ok(BrowserOperation::unary(SCALAR_ENCODED_LEN as u32, 1))
}

fn installed_offer(implementation: &str) -> Option<conduit_core::CapabilityOffer> {
    Some(match implementation {
        CLAMP_IMPLEMENTATION => clamp_offer(),
        SCALE_IMPLEMENTATION => scale_offer(),
        DEADBAND_IMPLEMENTATION => deadband_offer(),
        _ => return None,
    })
}

fn perform(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let input = Scalar::decode(input).map_err(debug_error)?;
    Ok(BrowserHostResult {
        output: Some(transform(placement, input)?.encode().to_vec()),
        manifestation: None,
    })
}

fn transform(placement: &PlannedGear, input: Scalar) -> Result<Scalar, String> {
    match placement.kind_id.as_str() {
        conduit_semantic_catalog::MATH_CLAMP_KIND => conduit_semantic_catalog::clamp_scalar(
            input,
            configuration(placement, conduit_semantic_catalog::CLAMP_MINIMUM_KEY)?,
            configuration(placement, conduit_semantic_catalog::CLAMP_MAXIMUM_KEY)?,
        ),
        conduit_semantic_catalog::MATH_SCALE_KIND => conduit_semantic_catalog::scale_scalar(
            input,
            configuration(placement, conduit_semantic_catalog::SCALE_GAIN_KEY)?,
        ),
        conduit_semantic_catalog::MATH_DEADBAND_KIND => conduit_semantic_catalog::deadband_scalar(
            input,
            configuration(placement, conduit_semantic_catalog::DEADBAND_RADIUS_KEY)?,
        ),
        _ => return Err("unsupported scalar transform".into()),
    }
    .map_err(debug_error)
}

fn configuration(placement: &PlannedGear, key: &str) -> Result<Scalar, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::I64(value)) if found == key => {
                Some(Scalar::from_raw_microunits(*value))
            }
            _ => None,
        })
        .ok_or_else(|| format!("math configuration '{key}' is missing"))
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
