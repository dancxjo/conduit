//! Repository-owned deterministic proof realization of portable speech synthesis.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const DETERMINISTIC_SPEECH_PROFILE: &str = "conduit-proof/speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_SPEECH_IMPLEMENTATION: &str = "conduit-proof/deterministic-speech@1";
pub const DETERMINISTIC_STREAMING_SPEECH_PROFILE: &str =
    "conduit-proof/streaming-speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION: &str =
    "conduit-proof/deterministic-streaming-speech@1";
pub const DETERMINISTIC_SPEECH_ARTIFACT: &str = "conduit-std-host/proof-deterministic-speech@1";
pub const DETERMINISTIC_SPEECH_OPERATION: &str = "conduit.host/proof-speech-next@1";
const _: () =
    assert!(conduit_tongues::MAXIMUM_PCM_BYTES == crate::AUDIO_CONVERT_PCM_INPUT_MAXIMUM_BYTES);
pub const SPEECH_FRAMES_PER_BLOCK: u16 = crate::AUDIO_CONVERT_PCM_INPUT_FRAMES_PER_BLOCK;
pub const SPEECH_PCM_BLOCK_BYTES: u32 = crate::AUDIO_CONVERT_PCM_MAXIMUM_INPUT_BYTES;
pub const SPEECH_MAXIMUM_BLOCKS: u16 = crate::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BLOCKS;

pub fn deterministic_speech_offer() -> CapabilityOffer {
    speech_offer(
        "proof-deterministic-speech-s16le-22050-mono",
        DETERMINISTIC_SPEECH_PROFILE,
        DETERMINISTIC_SPEECH_IMPLEMENTATION,
        DETERMINISTIC_SPEECH_ARTIFACT,
        false,
    )
}

pub fn deterministic_streaming_speech_offer() -> CapabilityOffer {
    speech_offer(
        "proof-deterministic-streaming-speech-s16le-22050-mono",
        DETERMINISTIC_STREAMING_SPEECH_PROFILE,
        DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION,
        DETERMINISTIC_SPEECH_ARTIFACT,
        true,
    )
}

fn speech_offer(
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    streaming: bool,
) -> CapabilityOffer {
    let contract = if streaming {
        conduit_tongues::streaming_synthesize_semantic_contract()
    } else {
        conduit_tongues::synthesize_semantic_contract()
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(DETERMINISTIC_SPEECH_OPERATION),
                target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: if streaming {
                    conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES
                } else {
                    conduit_tongues::MAXIMUM_TEXT_BYTES
                },
                maximum_output_bytes: SPEECH_PCM_BLOCK_BYTES,
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
    fn deterministic_offer_preserves_portable_speech_front_and_bounds_each_block() {
        let offer = deterministic_speech_offer();
        let contract = conduit_tongues::synthesize_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
        assert_eq!(
            offer.host_calls[0].maximum_output_bytes,
            SPEECH_PCM_BLOCK_BYTES
        );
        assert!(offer.resource_requirements.is_empty());
    }

    #[test]
    fn streaming_offer_preserves_speakable_segment_and_pcm_flow_contract() {
        let offer = deterministic_streaming_speech_offer();
        let contract = conduit_tongues::streaming_synthesize_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(
            offer.host_calls[0].maximum_input_bytes,
            conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES
        );
        assert!(offer.resource_requirements.is_empty());
    }
}
