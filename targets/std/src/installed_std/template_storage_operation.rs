//! Kernel lifecycle for bounded named-pattern storage commands.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TEMPLATE_STORAGE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_semantic_catalog::TemplateStorageOperation;

pub(super) fn refusal_detail(refusal: super::template_storage_host::StorageRefusal) -> u16 {
    use super::template_storage_host::StorageRefusal::*;
    match refusal {
        Malformed => 1,
        DuplicateName => 2,
        Full => 3,
        CorruptRetainedTemplate => 4,
    }
}

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    let offer = conduit_std_offers::template_storage_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
    {
        return Err("planned template storage differs from installed realization".into());
    }
    if placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS
        || placement.resources[0].units != 1
    {
        return Err("planned template storage lacks its exact admitted slot".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-commands", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("template storage command bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS).contains(&maximum) {
        return Err("template storage command bound is outside reviewed limits".into());
    }
    Ok(maximum)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let maximum = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(maximum + 1).map_err(|_| "template value budget overflow")?,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(maximum as u32 + 1),
        host_requests: usize::try_from(maximum).map_err(|_| "template request budget overflow")?,
        sign_items: u16::try_from(maximum.saturating_mul(8))
            .map_err(|_| "template sign budget overflow")?,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let maximum_commands = validate(placement)?;
    Ok(InstalledOperation::TemplateStorage(
        TemplateStorageOperation::new(maximum_commands, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
    ))
}
