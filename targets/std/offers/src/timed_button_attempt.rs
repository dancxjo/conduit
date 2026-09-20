//! Exact finite pressed-button attempt offer for the hosted std Host.

use conduit_core::{
    monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement, ArtifactId,
    CapabilityId, CapabilityOffer, CapabilityOfferBuilder, CapabilityRealization,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES, TIMER_RESOURCE_CLASS,
};

pub const TIMED_BUTTON_ATTEMPT_STD_PROFILE: &str = "std/pressed-button-attempt-kernel-hosted@1";
pub const TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION: &str = "std/kernel-pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_STD_ARTIFACT: &str = "conduit-std-host/pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION: &str =
    "conduit.host/observe-pressed-button-instant@1";

pub fn timed_button_attempt_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::timed_button_attempt_semantic_contract();
    let mut deadline = monotonic_timer_host_operation_requirement();
    deadline.target_kind = Some(contract.kind_id.clone());
    let target_kind = contract.kind_id.clone();
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from("pressed-button-attempt"),
            execution_profile_id: ExecutionProfileId::from(TIMED_BUTTON_ATTEMPT_STD_PROFILE),
            implementation_id: ImplementationId::from(TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TIMED_BUTTON_ATTEMPT_STD_ARTIFACT),
            host_operations: vec![
                deadline,
                HostOperationRequirement {
                    contract_id: HostOperationContractId::from(
                        TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION,
                    ),
                    target_kind: Some(target_kind),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                },
            ],
            resource_requirements: vec![
                monotonic_timer_resource_requirement(),
                conduit_core::resource_requirement(TIMER_RESOURCE_CLASS, 1),
            ],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_front_and_admits_clock_work() {
        let definition = conduit_semantic_catalog::timed_button_attempt_definition();
        let offer = timed_button_attempt_std_offer();
        assert_eq!(offer.inputs, definition.inputs);
        assert_eq!(offer.outputs, definition.outputs);
        assert_eq!(offer.startup_parameters.len(), 3);
        assert_eq!(offer.host_operations.len(), 2);
        assert!(offer
            .host_operations
            .iter()
            .all(|requirement| requirement.target_kind == Some(definition.kind_id.clone())));
        assert_eq!(offer.resource_requirements.len(), 2);
        assert!(offer
            .resource_requirements
            .iter()
            .all(|requirement| requirement.units == 1));
        assert!(offer.authority_requirements.is_empty());
    }
}
