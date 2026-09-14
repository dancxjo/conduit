//! Exact hosted ALSA microphone clip-source offer.

use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};

pub const MICROPHONE_CLIP_PROFILE: &str = "std/alsa-microphone-clip-s16le-16000-mono@1";
pub const MICROPHONE_CLIP_IMPLEMENTATION: &str = "std/alsa-microphone-clip@1";
pub const MICROPHONE_CLIP_ARTIFACT: &str = "conduit-std-host/alsa-microphone-clip@1";
pub const MICROPHONE_CLIP_OPERATION: &str = "conduit.host/capture-microphone-clip@1";
pub const MICROPHONE_CAPTURE_RESOURCE_CLASS: &str = "conduit.resource/audio-capture-alsa-hw@1";
pub const MICROPHONE_CAPTURE_AUTHORITY: &str = "conduit.authority/audio-capture@1";

pub fn microphone_clip_offer() -> CapabilityOffer {
    let operation = HostOperationContractId::from(MICROPHONE_CLIP_OPERATION);
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("audio-capture-alsa-microphone-clip"),
        kind_id: kind_id(conduit_semantic_catalog::MICROPHONE_CLIP_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/microphone-clip-source@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("request"),
            value_kind: kind_id(conduit_text::TEXT_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("clip"),
            value_kind: kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MICROPHONE_CLIP_PROFILE),
            implementation_id: ImplementationId::from(MICROPHONE_CLIP_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MICROPHONE_CLIP_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 16,
            maximum_output_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        }],
        resource_requirements: vec![resource_requirement(MICROPHONE_CAPTURE_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MICROPHONE_CAPTURE_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_keeps_endpoint_resource_and_capture_authority_explicit() {
        let offer = microphone_clip_offer();
        assert_eq!(offer.inputs.len(), 1);
        assert_eq!(offer.outputs.len(), 1);
        assert_eq!(offer.host_operations.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(
            offer.host_operations[0].maximum_output_bytes,
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        );
    }
}
