//! Finite PCM Flow offers retain the same microphone endpoint and authority.
use conduit_core::{
    kind_id, resource_requirement, AuthorityRequirement, Back, BackOfferBuilder, CapabilityOffer,
    HostCallRequirement,
};
pub const MICROPHONE_FLOW_IMPLEMENTATION: &str = "std/alsa-microphone-flow@1";
pub const CONVERTED_MICROPHONE_FLOW_IMPLEMENTATION: &str = "std/alsa-plug-microphone-flow@1";
pub const MICROPHONE_FLOW_OPERATION: &str = "conduit.host/capture-microphone-flow@1";
pub fn microphone_flow_offer(converted: bool) -> CapabilityOffer {
    let (implementation, capability, profile, artifact) = if converted {
        (
            CONVERTED_MICROPHONE_FLOW_IMPLEMENTATION,
            "audio-capture-alsa-plug-flow",
            "std/alsa-plug-microphone-flow-s16le-16000-mono@1",
            "conduit-std-host/alsa-plug-microphone-flow@1",
        )
    } else {
        (
            MICROPHONE_FLOW_IMPLEMENTATION,
            "audio-capture-alsa-flow",
            "std/alsa-microphone-flow-s16le-16000-mono@1",
            "conduit-std-host/alsa-microphone-flow@1",
        )
    };
    let operation = MICROPHONE_FLOW_OPERATION.into();
    BackOfferBuilder::new(
        conduit_semantic_catalog::audio_capture_finite_contract()
            .into_semantic_contract(conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_REVISION),
        Back {
            capability_id: capability.into(),
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: operation,
                target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: 16,
                maximum_output_bytes: conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_BLOCK_BYTES,
            }],
            resource_requirements: vec![resource_requirement(
                crate::MICROPHONE_CAPTURE_RESOURCE_CLASS,
                1,
            )],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: crate::MICROPHONE_CAPTURE_AUTHORITY.into(),
                host_call_contract_id: MICROPHONE_FLOW_OPERATION.into(),
                subject_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
            }],
        },
    )
    .build()
}
