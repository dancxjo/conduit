//! Exact installed offer validation for a reported original-generation Face.
use super::{
    back::{BackBudget, BackFactory, InstalledBack},
    face_contribution_back::FaceContributionBack,
};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::REPORTED_GENERATION_FACE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) fn label(placement: &PlannedGear) -> Result<&str, String> {
    let offer = conduit_std_offers::reported_generation_face_offer()?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.semantic_contract != offer.semantic_contract
        || placement.capability_id != offer.capability_id
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.authority.is_empty()
        || placement.configuration.len() != 1
    {
        return Err("reported generation Face placement differs from its installed offer".into());
    }
    conduit_core::face_data_label(&placement.configuration)
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    label(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 2,
        sign_items: 256,
        maximum_value_bytes: conduit_semantic_catalog::reported_generation_maximum_bytes()?,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let budget = budget(placement)?;
    Ok(InstalledBack::ReportedGenerationFace(
        FaceContributionBack::prepare_resident(
            placement.placement_id.clone(),
            budget.maximum_value_bytes,
        )?,
    ))
}

#[cfg(test)]
#[path = "reported_generation_factory_tests.rs"]
mod tests;
