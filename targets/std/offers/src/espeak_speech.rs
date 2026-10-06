//! Real hosted speech synthesis with an exact admitted engine/voice closure.
//!
//! The compiled adapter artifact is distinct from the executable, libraries,
//! voice data and fixed options bound by the provider resource content identity.
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, ResourceContentRequirement,
};

pub const ESPEAK_STREAM_IMPLEMENTATION: &str = "std/espeak-ng-stream@3";
pub const ESPEAK_STREAM_PROFILE: &str = "std/espeak-ng-stream-s16le-22050-mono@2";
pub const ESPEAK_STREAM_OPERATION: &str = "conduit.host/espeak-ng-stream-next@2";
pub const ESPEAK_SPEECH_PROFILE: &str = "std/espeak-ng-s16le-22050-mono@1";
pub const ESPEAK_SPEECH_IMPLEMENTATION: &str = "std/espeak-ng-speech@2";
pub const ESPEAK_SPEECH_ARTIFACT: &str = "conduit-std-host/espeak-ng-speech@2";
pub const ESPEAK_SPEECH_OPERATION: &str = "conduit.host/espeak-ng-speech-next@1";
pub const ESPEAK_SPEECH_RESOURCE_CLASS: &str = "conduit.resource/espeak-ng-provider@1";
pub const ESPEAK_PROVIDER_CONTENT_PROFILE: &str = "std/espeak-ng-provider-closure@1";
pub const ESPEAK_EXECUTE_AUTHORITY: &str = "conduit.authority/process-execute@1";

/// Discovery supplies the verified bounded closure, not an ambient executable path.
/// Ordinary planning selects its exact content and process authority before Play.
pub fn espeak_speech_offer(provider: ResourceContentRequirement) -> CapabilityOffer {
    speech_offer(provider, false)
}
pub fn espeak_streaming_offer(provider: ResourceContentRequirement) -> CapabilityOffer {
    speech_offer(provider, true)
}
fn speech_offer(provider: ResourceContentRequirement, streaming: bool) -> CapabilityOffer {
    let contract = if streaming {
        conduit_tongues::streaming_synthesize_semantic_contract()
    } else {
        conduit_tongues::synthesize_semantic_contract()
    };
    let implementation = if streaming {
        ESPEAK_STREAM_IMPLEMENTATION
    } else {
        ESPEAK_SPEECH_IMPLEMENTATION
    };
    let profile = if streaming {
        ESPEAK_STREAM_PROFILE
    } else {
        ESPEAK_SPEECH_PROFILE
    };
    let call = if streaming {
        espeak_streaming_host_call()
    } else {
        espeak_speech_host_call()
    };
    // Authority scopes the declared Host Call target, not the enclosing gear.
    let subject_kind = kind_id(conduit_audio::AUDIO_PCM_INFO_ID);
    let operation = call.contract_id.clone();
    let mut resource = resource_requirement(ESPEAK_SPEECH_RESOURCE_CLASS, 1);
    resource.content = Some(provider);
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ESPEAK_SPEECH_ARTIFACT),
            host_calls: vec![call],
            resource_requirements: vec![resource],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(ESPEAK_EXECUTE_AUTHORITY),
                host_call_contract_id: operation,
                subject_kind,
            }],
        },
    )
    .build()
}

pub fn espeak_streaming_host_call() -> HostCallRequirement {
    let mut call = espeak_speech_host_call();
    call.contract_id = HostCallContractId::from(ESPEAK_STREAM_OPERATION);
    call.maximum_input_bytes = conduit_tongues::MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES as u32;
    call
}

/// Static dispatch budget; provider-specific content is checked separately.
pub fn espeak_speech_host_call() -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(ESPEAK_SPEECH_OPERATION),
        target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_output_bytes: crate::SPEECH_PCM_BLOCK_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        ResourceAccessMode, ResourceRetention, ResourceSemanticIdentity, ResourceSharing,
        ResourceVersionIdentity,
    };

    fn content(version: u8) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([version; 32]),
            content_profile: kind_id(ESPEAK_PROVIDER_CONTENT_PROFILE),
            maximum_bytes: 4_096,
            maximum_items: 3,
            retention: ResourceRetention::Boot,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: 0,
            sensitive: false,
        }
    }

    #[test]
    fn actual_provider_preserves_portable_meaning_and_finite_pcm_blocks() {
        let offer = espeak_speech_offer(content(2));
        let semantic = conduit_tongues::synthesize_contract();
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
        assert_eq!(offer.host_calls[0].maximum_input_bytes, 256);
        assert_eq!(
            offer.host_calls[0].maximum_output_bytes,
            crate::SPEECH_PCM_BLOCK_BYTES
        );
        assert_eq!(conduit_tongues::MAXIMUM_PCM_BYTES, 131_072);
        assert_ne!(
            offer.implementation.implementation_id.as_str(),
            crate::DETERMINISTIC_SPEECH_IMPLEMENTATION
        );
        assert_ne!(
            offer.implementation.artifact_id.as_str(),
            crate::DETERMINISTIC_SPEECH_ARTIFACT
        );
    }

    #[test]
    fn closure_changes_bind_resources_without_relabeling_the_compiled_adapter() {
        let first = espeak_speech_offer(content(2));
        let second = espeak_speech_offer(content(3));
        assert_eq!(
            first.implementation.artifact_id,
            ArtifactId::from(ESPEAK_SPEECH_ARTIFACT)
        );
        assert_eq!(
            first.implementation.artifact_id,
            second.implementation.artifact_id
        );
        assert_eq!(
            first.implementation.implementation_id,
            second.implementation.implementation_id
        );
        assert_ne!(first.resource_requirements, second.resource_requirements);
        assert_eq!(first.resource_requirements.len(), 1);
        assert_eq!(first.resource_requirements[0].units, 1);
        assert_eq!(
            first.resource_requirements[0].content.as_ref(),
            Some(&content(2))
        );
        let [authority] = first.authority_requirements.as_slice() else {
            panic!("the provider requires one exact process authority");
        };
        assert_eq!(
            authority.contract_id,
            AuthorityContractId::from(ESPEAK_EXECUTE_AUTHORITY)
        );
        assert_eq!(
            authority.host_call_contract_id,
            first.host_calls[0].contract_id
        );
        assert_eq!(
            Some(&authority.subject_kind),
            first.host_calls[0].target_kind.as_ref()
        );
        assert_eq!(
            authority.subject_kind,
            kind_id(conduit_audio::AUDIO_PCM_INFO_ID)
        );
    }
}
