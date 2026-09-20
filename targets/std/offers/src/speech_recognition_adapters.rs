//! Hosted std realizations for explicit Tongues single-shot/streaming adapters.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, CapabilityOfferBuilder,
    CapabilityRealization, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, SemanticCapabilityContract,
};

pub const SPEECH_WINDOW_TO_CLIP_STD_IMPLEMENTATION: &str = "std/speech-window-to-clip@1";
pub const SPEECH_RESULT_TO_EVENT_STREAM_STD_IMPLEMENTATION: &str =
    "std/speech-result-to-event-stream@1";

pub const SPEECH_WINDOW_PUSH_OPERATION: &str = "conduit.host/speech-window-push@1";
pub const SPEECH_WINDOW_CLOSE_OPERATION: &str = "conduit.host/speech-window-close@1";
pub const SPEECH_RESULT_TO_EVENT_OPERATION: &str = "conduit.host/speech-result-to-event-stream@1";

pub fn speech_window_to_clip_std_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::speech_window_to_clip_semantic_contract(),
        "std-speech-window-to-clip-v1",
        "std/speech-window-kernel@1",
        SPEECH_WINDOW_TO_CLIP_STD_IMPLEMENTATION,
        "conduit-std-host/speech-window-to-clip@1",
        vec![
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(SPEECH_WINDOW_PUSH_OPERATION),
                target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                    + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            },
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(SPEECH_WINDOW_CLOSE_OPERATION),
                target_kind: Some(kind_id(conduit_tongues::SPEECH_WINDOW_TO_CLIP_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: 1,
                maximum_output_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            },
        ],
    )
}

pub fn speech_result_to_event_stream_std_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::speech_result_to_event_stream_semantic_contract(),
        "std-speech-result-to-event-stream-v1",
        "std/speech-result-event-kernel@1",
        SPEECH_RESULT_TO_EVENT_STREAM_STD_IMPLEMENTATION,
        "conduit-std-host/speech-result-to-event-stream@1",
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(SPEECH_RESULT_TO_EVENT_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RESULT_TO_EVENT_STREAM_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
        }],
    )
}

fn offer(
    contract: SemanticCapabilityContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operations,
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
    fn offers_preserve_explicit_tongues_adapter_fronts() {
        let window = speech_window_to_clip_std_offer();
        let window_definition = conduit_tongues::speech_window_to_clip_definition();
        assert_eq!(window.kind_id, window_definition.kind_id);
        assert_eq!(window.inputs, window_definition.inputs);
        assert_eq!(window.outputs, window_definition.outputs);
        assert_eq!(window.host_operations.len(), 2);

        let stream = speech_result_to_event_stream_std_offer();
        let stream_definition = conduit_tongues::speech_result_to_event_stream_definition();
        assert_eq!(stream.kind_id, stream_definition.kind_id);
        assert_eq!(stream.inputs, stream_definition.inputs);
        assert_eq!(stream.outputs, stream_definition.outputs);
        assert_eq!(stream.host_operations.len(), 1);
    }
}
