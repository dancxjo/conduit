//! Validate the selected Home Face Back before preparing its kernel state.
use super::{
    ambient_home_back::AmbientHomeBack,
    back::{BackBudget, BackFactory, InstalledBack},
};
use conduit_core::PlannedGear;
use conduit_semantic_catalog::AMBIENT_HOME_OBSERVATION_BYTES;
use conduit_std_offers::{resident_ambient_home_face_offer, AMBIENT_HOME_FACE_IMPLEMENTATION};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: AMBIENT_HOME_FACE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) fn label(placement: &PlannedGear) -> Result<&str, String> {
    let offer = resident_ambient_home_face_offer()?;
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
        return Err("Home Face placement differs from its installed offer".into());
    }
    conduit_core::face_data_label(&placement.configuration)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    label(placement)?;
    // The incoming Cord owns the value. Both calls borrow it and return no bytes.
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 2,
        sign_items: 256,
        maximum_value_bytes: AMBIENT_HOME_OBSERVATION_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    Ok(InstalledBack::AmbientHome(
        AmbientHomeBack::prepare_resident(placement.placement_id.clone())?,
    ))
}
