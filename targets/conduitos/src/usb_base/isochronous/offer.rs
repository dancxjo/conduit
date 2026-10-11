//! Class-neutral endpoint Back description for an initialized native provider.
//! Root separately supplies actual attachment resources, lifecycle and grants.
use super::{contract::*, factory::*};
use alloc::vec;
use conduit_core::*;

/// Describe the bounded operation without creating an initialized provider,
/// resource possession or authority. The caller identifies its actual artifact.
pub fn offer(contract: &IsochronousOutContract, artifact_id: ArtifactId) -> CapabilityOffer {
    let kind = contract.kind();
    conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(ISOCHRONOUS_OUT_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ISOCHRONOUS_OUT_PROFILE.into(), implementation_id: ISOCHRONOUS_OUT_IMPLEMENTATION.into(), artifact_id,
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: ISOCHRONOUS_OUT_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES, maximum_output_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(ISOCHRONOUS_OUT_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: ISOCHRONOUS_OUT_AUTHORITY.into(), host_call_contract_id: ISOCHRONOUS_OUT_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iso_offer_is_one_bounded_batch_call_and_one_attachment() {
        let contract = IsochronousOutContract::prepare().unwrap();
        let value = offer(&contract, "artifact/iso-test".into());
        assert_eq!(value.host_calls.len(), 1);
        assert_eq!(value.host_calls[0].maximum_in_flight, 1);
        assert_eq!(value.host_calls[0].maximum_input_bytes, 4096);
        assert_eq!(value.host_calls[0].maximum_output_bytes, 4096);
        assert_eq!(value.resource_requirements.len(), 1);
        assert_eq!(value.authority_requirements.len(), 1);
    }
}
