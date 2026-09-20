//! Exact finite timed-pattern realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const ORDERED_EVENT_INTERVALS_STD_PROFILE: &str = "std/ordered-event-intervals-kernel-hosted@1";
pub const ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION: &str = "std/kernel-ordered-event-intervals@1";
pub const ORDERED_EVENT_INTERVALS_STD_ARTIFACT: &str = "conduit-std-host/ordered-event-intervals@1";
pub const ORDERED_EVENT_INTERVALS_HOST_OPERATION: &str = "conduit.host/ordered-event-intervals@1";

pub fn ordered_event_intervals_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::ordered_event_intervals_semantic_contract();
    let target_kind = contract.kind_id.clone();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("ordered-event-intervals"),
            execution_profile_id: ExecutionProfileId::from(ORDERED_EVENT_INTERVALS_STD_PROFILE),
            implementation_id: ImplementationId::from(ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ORDERED_EVENT_INTERVALS_STD_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(ORDERED_EVENT_INTERVALS_HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_timed_sequence_front() {
        let contract = conduit_semantic_catalog::ordered_event_intervals_definition();
        let offer = ordered_event_intervals_std_offer();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}
