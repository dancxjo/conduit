//! Exact generic keyboard realization offer owned by the hosted std Host.
pub mod button;

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    INPUT_RESOURCE_CLASS,
};
use conduit_human::{KEY_EVENT_ENCODED_LEN, KEY_EVENT_INFO_ID};

pub const NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/input-next-key-event@1";
pub const HOSTED_KEYBOARD_EXECUTION_PROFILE: &str = "conduit.std/input-keyboard-kernel-hosted@1";
pub const HOSTED_KEYBOARD_IMPLEMENTATION: &str = "std/kernel-input-keyboard-hosted@1";

pub fn next_key_event_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT),
        target_kind: Some(kind_id(KEY_EVENT_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: 0,
        maximum_output_bytes: KEY_EVENT_ENCODED_LEN as u32,
    }
}

pub fn hosted_keyboard_offer(capability: &str, artifact: &str) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::keyboard_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(HOSTED_KEYBOARD_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(HOSTED_KEYBOARD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![next_key_event_host_operation_requirement()],
        resource_requirements: vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_portable_face_and_bounded_effect() {
        let contract = conduit_semantic_catalog::keyboard_contract();
        let offer = hosted_keyboard_offer("proof-keyboard", "proof/keyboard@1");
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_operations[0].maximum_output_bytes, 3);
        assert_eq!(offer.resource_requirements.len(), 1);
    }
}
