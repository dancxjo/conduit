//! Class-neutral control Back description shared by product and proof roots.
//! Describing a Back creates neither possession nor authority.
use super::{control_contract::*, control_factory::*};
use alloc::vec;
use conduit_core::*;

pub fn offer(contract: &ControlContract, artifact_id: ArtifactId) -> CapabilityOffer {
    let kind = contract.kind();
    conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(CONTROL_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: CONTROL_PROFILE.into(), implementation_id: CONTROL_IMPLEMENTATION.into(), artifact_id,
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: CONTROL_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: CONTROL_MAXIMUM_BYTES, maximum_output_bytes: CONTROL_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(CONTROL_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: CONTROL_AUTHORITY.into(), host_call_contract_id: CONTROL_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    }
}
