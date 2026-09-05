//! Exact finite pressed-button attempt offer for the hosted std Host.

use conduit_core::{
    monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement, ArtifactId,
    CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES, TIMER_RESOURCE_CLASS,
};

pub const TIMED_BUTTON_ATTEMPT_STD_PROFILE: &str = "std/pressed-button-attempt-kernel-hosted@1";
pub const TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION: &str = "std/kernel-pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_STD_ARTIFACT: &str = "conduit-std-host/pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION: &str =
    "conduit.host/observe-pressed-button-instant@1";

pub fn timed_button_attempt_std_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::timed_button_attempt_definition();
    let mut deadline = monotonic_timer_host_operation_requirement();
    deadline.target_kind = Some(contract.kind_id.clone());
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "maximum-transitions".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "maximum-presses".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "timeout-ms".into(),
                value_type: "Duration".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from("pressed-button-attempt"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TIMED_BUTTON_ATTEMPT_STD_PROFILE),
            implementation_id: ImplementationId::from(TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TIMED_BUTTON_ATTEMPT_STD_ARTIFACT),
        },
        host_operations: vec![
            deadline,
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(
                    TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION,
                ),
                target_kind: Some(contract.kind_id),
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
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u16,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                * (conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u32 + 1),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_face_and_admits_clock_work() {
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
