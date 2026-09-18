//! Explicitly initialized hosted Piper realization of portable speech synthesis.

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer,
};

pub const PIPER_SPEECH_PROFILE: &str = "std/piper-s16le-22050-mono-p25@1";
pub const PIPER_SPEECH_IMPLEMENTATION: &str = "std/hosted-piper-speech@1";
pub const PIPER_STREAMING_SPEECH_PROFILE: &str = "std/piper-streaming-s16le-22050-mono-p25@1";
pub const PIPER_STREAMING_SPEECH_IMPLEMENTATION: &str = "std/hosted-piper-streaming-speech@1";
pub const PIPER_SPEECH_ARTIFACT: &str = "conduit-std-host/piper-speech@1";
pub const PIPER_SPEECH_OPERATION: &str = "conduit.host/piper-speech-next@1";
pub const PIPER_PROCESS_RESOURCE_CLASS: &str = "conduit.resource/piper-process-slot@1";
pub const DETERMINISTIC_SPEECH_PROFILE: &str = "conduit-proof/speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_SPEECH_IMPLEMENTATION: &str = "conduit-proof/deterministic-speech@1";
pub const DETERMINISTIC_STREAMING_SPEECH_PROFILE: &str =
    "conduit-proof/streaming-speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION: &str =
    "conduit-proof/deterministic-streaming-speech@1";
pub const DETERMINISTIC_SPEECH_ARTIFACT: &str = "conduit-std-host/proof-deterministic-speech@1";
const _: () = assert!(
    conduit_tongues::MAXIMUM_PCM_BYTES == crate::AUDIO_CONVERT_PCM_INPUT_MAXIMUM_BYTES
);
pub const PIPER_FRAMES_PER_BLOCK: u16 = crate::AUDIO_CONVERT_PCM_INPUT_FRAMES_PER_BLOCK;
pub const PIPER_MAXIMUM_FRAMES: u32 = conduit_tongues::MAXIMUM_PCM_BYTES / 2;
pub const PIPER_PCM_BLOCK_BYTES: u32 = crate::AUDIO_CONVERT_PCM_MAXIMUM_INPUT_BYTES;
pub const PIPER_MAXIMUM_BLOCKS: u16 = crate::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BLOCKS;

pub fn piper_speech_offer() -> CapabilityOffer {
    speech_offer(
        "speech-synthesize-piper-s16le-22050-mono",
        PIPER_SPEECH_PROFILE,
        PIPER_SPEECH_IMPLEMENTATION,
        PIPER_SPEECH_ARTIFACT,
        true,
        false,
    )
}

pub fn deterministic_speech_offer() -> CapabilityOffer {
    speech_offer(
        "proof-deterministic-speech-s16le-22050-mono",
        DETERMINISTIC_SPEECH_PROFILE,
        DETERMINISTIC_SPEECH_IMPLEMENTATION,
        DETERMINISTIC_SPEECH_ARTIFACT,
        false,
        false,
    )
}

pub fn piper_streaming_speech_offer() -> CapabilityOffer {
    speech_offer(
        "speech-synthesize-stream-piper-s16le-22050-mono",
        PIPER_STREAMING_SPEECH_PROFILE,
        PIPER_STREAMING_SPEECH_IMPLEMENTATION,
        PIPER_SPEECH_ARTIFACT,
        true,
        true,
    )
}

pub fn deterministic_streaming_speech_offer() -> CapabilityOffer {
    speech_offer(
        "proof-deterministic-streaming-speech-s16le-22050-mono",
        DETERMINISTIC_STREAMING_SPEECH_PROFILE,
        DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION,
        DETERMINISTIC_SPEECH_ARTIFACT,
        false,
        true,
    )
}

fn speech_offer(
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    requires_process: bool,
    streaming: bool,
) -> CapabilityOffer {
    let contract = if streaming {
        conduit_tongues::streaming_synthesize_contract()
    } else {
        conduit_tongues::synthesize_contract()
    };
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-output-bytes".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(PIPER_SPEECH_OPERATION),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: if streaming {
                conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES
            } else {
                conduit_tongues::MAXIMUM_TEXT_BYTES
            },
            maximum_output_bytes: PIPER_PCM_BLOCK_BYTES,
        }],
        resource_requirements: requires_process
            .then(|| resource_requirement(PIPER_PROCESS_RESOURCE_CLASS, 1))
            .into_iter()
            .collect(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piper_offer_preserves_portable_speech_face_and_bounds_each_block() {
        let offer = piper_speech_offer();
        let contract = conduit_tongues::synthesize_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
        assert_eq!(
            offer.host_operations[0].maximum_output_bytes,
            PIPER_PCM_BLOCK_BYTES
        );
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(
            offer.resource_requirements[0].class_id.as_str(),
            PIPER_PROCESS_RESOURCE_CLASS
        );
    }

    #[test]
    fn deterministic_proof_and_piper_realization_have_distinct_identities() {
        let piper = piper_speech_offer();
        let deterministic = deterministic_speech_offer();
        assert_eq!(piper.kind_id, deterministic.kind_id);
        assert_ne!(piper.capability_id, deterministic.capability_id);
        assert_ne!(piper.implementation, deterministic.implementation);
        assert!(deterministic.resource_requirements.is_empty());
    }

    #[test]
    fn streaming_offer_preserves_speakable_segment_and_pcm_flow_contract() {
        let offer = piper_streaming_speech_offer();
        let contract = conduit_tongues::streaming_synthesize_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(
            offer.host_operations[0].maximum_input_bytes,
            conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES
        );
        assert_eq!(offer.resource_requirements.len(), 1);
    }
}
