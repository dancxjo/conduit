//! Exact hosted ALSA microphone clip-source offer.

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId,
};

pub const MICROPHONE_CLIP_PROFILE: &str = "std/alsa-microphone-clip-s16le-16000-mono@1";
pub const MICROPHONE_CLIP_IMPLEMENTATION: &str = "std/alsa-microphone-clip@1";
pub const MICROPHONE_CLIP_ARTIFACT: &str = "conduit-std-host/alsa-microphone-clip@1";
pub const MICROPHONE_CLIP_OPERATION: &str = "conduit.host/capture-microphone-clip@1";
pub const MICROPHONE_CAPTURE_RESOURCE_CLASS: &str = "conduit.resource/audio-capture-alsa-hw@1";
pub const MICROPHONE_CAPTURE_AUTHORITY: &str = "conduit.authority/audio-capture@1";

pub fn microphone_clip_offer() -> CapabilityOffer {
    let operation = HostCallContractId::from(MICROPHONE_CLIP_OPERATION);
    BackOfferBuilder::new(
        conduit_semantic_catalog::microphone_clip_source_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("audio-capture-alsa-microphone-clip"),
            execution_profile_id: ExecutionProfileId::from(MICROPHONE_CLIP_PROFILE),
            implementation_id: ImplementationId::from(MICROPHONE_CLIP_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MICROPHONE_CLIP_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: operation.clone(),
                target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: 16,
                maximum_output_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
            }],
            resource_requirements: vec![resource_requirement(MICROPHONE_CAPTURE_RESOURCE_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(MICROPHONE_CAPTURE_AUTHORITY),
                host_call_contract_id: operation,
                subject_kind: kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
            }],
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_keeps_endpoint_resource_and_capture_authority_explicit() {
        let offer = microphone_clip_offer();
        assert_eq!(offer.inputs.len(), 1);
        assert_eq!(offer.outputs.len(), 1);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(
            offer.host_calls[0].maximum_output_bytes,
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        );
    }
}
