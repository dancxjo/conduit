//! Validate exact planned facts against one installed browser Back.
use conduit_core::CapabilityOffer;

pub(in super::super) fn validate_placement(
    placement: &conduit_core::PlannedGear,
    offer: &CapabilityOffer,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.realization_properties != offer.realization_properties
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
    {
        return Err("planned browser Gear does not match its installed capability".into());
    }
    Ok(())
}
