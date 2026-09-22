//! Exact hosted projection from a recognition result to bounded text.

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    ResourceRequirement,
};

pub const WHISPER_SPEECH_PROFILE: &str = "std/whisper-s16le-16000-mono@1";
pub const WHISPER_SPEECH_IMPLEMENTATION: &str = "std/hosted-whisper-speech@1";
pub const WHISPER_SPEECH_ARTIFACT: &str = "conduit-std-host/whisper-speech@1";
pub const WHISPER_SPEECH_OPERATION: &str = "conduit.host/whisper-speech-recognize@1";
pub const WHISPER_PROCESS_RESOURCE_CLASS: &str = "conduit.resource/whisper-process-slot@1";
pub const WHISPER_CLIP_SPEECH_PROFILE: &str = "std/whisper-clip-s16le-16000-mono@1";
pub const WHISPER_CLIP_SPEECH_IMPLEMENTATION: &str = "std/hosted-whisper-clip-speech@1";
pub const WHISPER_CLIP_SPEECH_ARTIFACT: &str = "conduit-std-host/whisper-clip-speech@1";
pub const WHISPER_CLIP_SPEECH_OPERATION: &str = "conduit.host/whisper-clip-speech-recognize@1";

pub fn whisper_speech_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::speech_recognition_contract(),
        Identity {
            capability: "speech-recognize-whisper-s16le-16000-mono",
            profile: WHISPER_SPEECH_PROFILE,
            implementation: WHISPER_SPEECH_IMPLEMENTATION,
            artifact: WHISPER_SPEECH_ARTIFACT,
        },
        HostCallRequirement {
            contract_id: HostCallContractId::from(WHISPER_SPEECH_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNITION_RESULT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        },
        vec![resource_requirement(WHISPER_PROCESS_RESOURCE_CLASS, 1)],
    )
}

pub fn whisper_clip_speech_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::speech_clip_recognition_contract(),
        Identity {
            capability: "speech-recognize-clip-whisper-s16le-16000-mono",
            profile: WHISPER_CLIP_SPEECH_PROFILE,
            implementation: WHISPER_CLIP_SPEECH_IMPLEMENTATION,
            artifact: WHISPER_CLIP_SPEECH_ARTIFACT,
        },
        HostCallRequirement {
            contract_id: HostCallContractId::from(WHISPER_CLIP_SPEECH_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNITION_RESULT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
        },
        vec![resource_requirement(WHISPER_PROCESS_RESOURCE_CLASS, 1)],
    )
}

pub const RECOGNITION_TO_TEXT_STD_PROFILE: &str = "std/recognition-to-text-hosted@1";
pub const RECOGNITION_TO_TEXT_STD_IMPLEMENTATION: &str = "std/recognition-to-text@1";
pub const RECOGNITION_TO_TEXT_STD_ARTIFACT: &str = "conduit-std-host/recognition-to-text@1";
pub const RECOGNITION_TO_TEXT_OPERATION: &str = "conduit.host/recognition-to-text@1";
pub const COMMITTED_TURN_TO_TEXT_OPERATION: &str = "conduit.host/committed-turn-to-text@1";

pub fn recognition_to_text_std_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::speech_recognition_to_text_contract(),
        Identity {
            capability: "std-recognition-to-text-v1",
            profile: RECOGNITION_TO_TEXT_STD_PROFILE,
            implementation: RECOGNITION_TO_TEXT_STD_IMPLEMENTATION,
            artifact: RECOGNITION_TO_TEXT_STD_ARTIFACT,
        },
        HostCallRequirement {
            contract_id: HostCallContractId::from(RECOGNITION_TO_TEXT_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::SPEECH_RECOGNITION_TO_TEXT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
        },
        Vec::new(),
    )
}

pub fn committed_turn_to_text_std_offer() -> CapabilityOffer {
    offer(
        conduit_tongues::committed_turn_to_text_contract(),
        Identity {
            capability: "std-committed-turn-to-text-v1",
            profile: RECOGNITION_TO_TEXT_STD_PROFILE,
            implementation: RECOGNITION_TO_TEXT_STD_IMPLEMENTATION,
            artifact: RECOGNITION_TO_TEXT_STD_ARTIFACT,
        },
        HostCallRequirement {
            contract_id: HostCallContractId::from(COMMITTED_TURN_TO_TEXT_OPERATION),
            target_kind: Some(kind_id(conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32,
            maximum_output_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
        },
        Vec::new(),
    )
}

#[derive(Clone, Copy)]
struct Identity<'a> {
    capability: &'a str,
    profile: &'a str,
    implementation: &'a str,
    artifact: &'a str,
}

fn offer(
    contract: conduit_tongues::SpeechRecognitionContract,
    identity: Identity<'_>,
    host_call: HostCallRequirement,
    resources: Vec<ResourceRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract.into_semantic_capability_contract(),
        Back {
            capability_id: CapabilityId::from(identity.capability),
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
            host_calls: vec![host_call],
            resource_requirements: resources,
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_front_and_finite_bounds() {
        let offer = recognition_to_text_std_offer();
        let contract = conduit_tongues::speech_recognition_to_text_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls.len(), 1);
    }

    #[test]
    fn committed_turn_offer_preserves_the_flow_front_and_finite_bounds() {
        let offer = committed_turn_to_text_std_offer();
        let contract = conduit_tongues::committed_turn_to_text_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls.len(), 1);
    }

    #[test]
    fn whisper_offer_preserves_portable_recognition_and_process_bounds() {
        let offer = whisper_speech_offer();
        let contract = conduit_tongues::speech_recognition_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
    }

    #[test]
    fn whisper_clip_offer_preserves_the_distinct_bounded_clip_contract() {
        let offer = whisper_clip_speech_offer();
        let contract = conduit_tongues::speech_clip_recognition_contract();
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(
            offer.host_calls[0].maximum_input_bytes,
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        );
    }
}
