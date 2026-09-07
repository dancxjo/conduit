//! Exact hosted projection from a recognition result to bounded text.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
};

pub const RECOGNITION_TO_TEXT_STD_PROFILE: &str = "std/recognition-to-text-hosted@1";
pub const RECOGNITION_TO_TEXT_STD_IMPLEMENTATION: &str = "std/recognition-to-text@1";
pub const RECOGNITION_TO_TEXT_STD_ARTIFACT: &str = "conduit-std-host/recognition-to-text@1";
pub const RECOGNITION_TO_TEXT_OPERATION: &str = "conduit.host/recognition-to-text@1";

pub fn recognition_to_text_std_offer() -> CapabilityOffer {
    let contract = conduit_tongues::speech_recognition_to_text_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("std-recognition-to-text-v1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(RECOGNITION_TO_TEXT_STD_PROFILE),
            implementation_id: ImplementationId::from(RECOGNITION_TO_TEXT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(RECOGNITION_TO_TEXT_STD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(RECOGNITION_TO_TEXT_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNITION_TO_TEXT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_face_and_finite_bounds() {
        let offer = recognition_to_text_std_offer();
        let contract = conduit_tongues::speech_recognition_to_text_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_operations.len(), 1);
    }
}
