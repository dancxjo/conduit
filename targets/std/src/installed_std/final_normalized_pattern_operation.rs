//! Installed finite Flow-to-final-Value normalized-pattern selection.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_semantic_catalog::FinalNormalizedPatternOperation;

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    let offer = conduit_std_offers::final_normalized_pattern_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.limits != offer.limits
    {
        return Err("planned final pattern differs from installed realization".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("final pattern value bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_FINAL_PATTERN_VALUES).contains(&maximum) {
        return Err("final pattern value bound is outside reviewed limits".into());
    }
    Ok(maximum)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let maximum = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(maximum + 1).map_err(|_| "final pattern item overflow")?,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(maximum as u32 + 1),
        host_requests: 0,
        sign_items: u16::try_from(maximum.saturating_mul(6) + 8)
            .map_err(|_| "final pattern sign overflow")?,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    Ok(InstalledOperation::FinalNormalizedPattern(
        FinalNormalizedPatternOperation::new(validate(placement)?),
    ))
}
