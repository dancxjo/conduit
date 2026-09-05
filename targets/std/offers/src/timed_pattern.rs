//! Exact finite timed-pattern realization offers owned by the hosted std Host.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const ORDERED_EVENT_INTERVALS_STD_PROFILE: &str = "std/ordered-event-intervals-kernel-hosted@1";
pub const ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION: &str = "std/kernel-ordered-event-intervals@1";
pub const ORDERED_EVENT_INTERVALS_STD_ARTIFACT: &str = "conduit-std-host/ordered-event-intervals@1";
pub const ORDERED_EVENT_INTERVALS_HOST_OPERATION: &str = "conduit.host/ordered-event-intervals@1";

pub fn ordered_event_intervals_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::ordered_event_intervals_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("ordered-event-intervals"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(ORDERED_EVENT_INTERVALS_STD_PROFILE),
            implementation_id: ImplementationId::from(ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ORDERED_EVENT_INTERVALS_STD_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(ORDERED_EVENT_INTERVALS_HOST_OPERATION),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_timed_sequence_face() {
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
