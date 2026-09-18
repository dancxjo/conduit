//! Bounded installed realization of generated-text speech commitment.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
};

pub const SPEECH_COMMIT_STD_PROFILE: &str = "std/speech-commit@1";
pub const SPEECH_COMMIT_STD_IMPLEMENTATION: &str = "std/speech-commit@1";
pub const SPEECH_COMMIT_STD_ARTIFACT: &str = "conduit-std-host/speech-commit@1";
pub const SPEECH_COMMIT_PUSH_OPERATION: &str = "conduit.host/speech-commit-push@1";
pub const SPEECH_COMMIT_NEXT_OPERATION: &str = "conduit.host/speech-commit-next@1";
pub const SPEECH_COMMIT_CLOSE_OPERATION: &str = "conduit.host/speech-commit-close@1";

pub fn speech_commit_std_offer() -> CapabilityOffer {
    let contract = conduit_tongues::speech_commit_contract();
    let control = |contract_id| HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract_id),
        target_kind: Some(kind_id(conduit_tongues::SPEECH_COMMIT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_tongues::MAXIMUM_PENDING_SPEECH_BYTES as u32,
        maximum_output_bytes: conduit_tongues::MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("std-speech-commit-v1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SPEECH_COMMIT_STD_PROFILE),
            implementation_id: ImplementationId::from(SPEECH_COMMIT_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SPEECH_COMMIT_STD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![
            control(SPEECH_COMMIT_PUSH_OPERATION),
            control(SPEECH_COMMIT_NEXT_OPERATION),
            control(SPEECH_COMMIT_CLOSE_OPERATION),
        ],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_flow_and_exact_finite_operations() {
        let offer = speech_commit_std_offer();
        let contract = conduit_tongues::speech_commit_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_operations.len(), 3);
    }
}
