//! Exact installed Back for one selected Todo checkpoint generation.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION,
    budget,
    prepare,
};

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = conduit_todo_plot::todo_checkpoint_kind();
    let [resource] = placement.resources.as_slice() else {
        return Err("Todo checkpoint requires one exact resource".into());
    };
    let content = resource
        .content
        .as_ref()
        .ok_or("Todo checkpoint resource has no content")?;
    let offer = conduit_std_offers::todo_checkpoint_offer(content.contract.clone())
        .map_err(str::to_string)?;
    if placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.semantic_contract != kind.semantic_contract()
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || resource.class_id != offer.resource_requirements[0].class_id
        || resource.units != 1
        || placement.authority.len() != 1
        || placement.authority[0].contract_id.as_str()
            != conduit_std_offers::TODO_CHECKPOINT_AUTHORITY
        || !placement.configuration.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Todo checkpoint differs from exact std offer".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (2 * conduit_todo_plot::STATE_MAX_BYTES) as u32,
        host_requests: 1,
        sign_items: 4,
        maximum_value_bytes: conduit_todo_plot::STATE_MAX_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::TodoCheckpoint(
        crate::todo_checkpoint_call::TodoCheckpointBack::new(),
    ))
}
