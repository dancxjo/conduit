//! Class-neutral endpoint Back description for an initialized native provider.
//! Root separately supplies actual attachment resources, lifecycle and grants.
use super::{endpoint_read_contract::*, endpoint_read_factory::*};
use alloc::vec;
use conduit_core::*;

/// Describe the bounded operation without creating an initialized provider,
/// resource possession or authority. The caller identifies its actual artifact.
pub fn offer(contract: &EndpointReadContract, artifact_id: ArtifactId) -> CapabilityOffer {
    let kind = contract.kind();
    conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(ENDPOINT_READ_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ENDPOINT_READ_PROFILE.into(), implementation_id: ENDPOINT_READ_IMPLEMENTATION.into(), artifact_id,
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: ENDPOINT_READ_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: ENDPOINT_READ_MAXIMUM_BYTES, maximum_output_bytes: ENDPOINT_READ_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(ENDPOINT_READ_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: ENDPOINT_READ_AUTHORITY.into(), host_call_contract_id: ENDPOINT_READ_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    }
}

/// Describe an initialized capture provider's admitted instance capacity.
/// Each instance retains the same single-call and 4 KiB transport contract.
pub fn capture_offer(
    contract: &EndpointReadContract,
    artifact_id: ArtifactId,
    capability_id: CapabilityId,
    maximum_instances: u16,
) -> Result<CapabilityOffer, &'static str> {
    if !(1..=8).contains(&maximum_instances) {
        return Err("usb-endpoint-capture-capacity");
    }
    let mut description = offer(contract, artifact_id);
    description.capability_id = capability_id;
    description.limits.max_active_instances = maximum_instances;
    Ok(description)
}

#[cfg(test)]
#[path = "endpoint_read_offer_tests.rs"]
mod tests;
