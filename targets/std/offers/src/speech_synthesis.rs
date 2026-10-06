//! Repository-owned deterministic proof realization of portable speech synthesis.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const DETERMINISTIC_SPEECH_PROFILE: &str = "conduit-proof/speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_SPEECH_IMPLEMENTATION: &str = "conduit-proof/deterministic-speech@2";
pub const DETERMINISTIC_STREAMING_SPEECH_PROFILE: &str =
    "conduit-proof/streaming-speech-s16le-22050-mono-p25@1";
pub const DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION: &str =
    "conduit-proof/deterministic-streaming-speech@2";
pub const DETERMINISTIC_SPEECH_ARTIFACT: &str = "conduit-std-host/proof-deterministic-speech@2";
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
    let mut offer = BackOfferBuilder::new(
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
    .build();
    offer.realization_properties = vec![proof_english_coverage(artifact)];
    offer
}

/// This deterministic fixture produces proof audio, not a linguistic accuracy
/// claim. Its declared English scope is finite and belongs to this artifact.
fn proof_english_coverage(artifact: &str) -> conduit_core::StructuredConfigurationValue {
    use conduit_language::{LanguageCoverage, LanguageId};
    use conduit_plot::rust_binding::BoundedSequence;
    let coverage = LanguageCoverage::new(
        artifact.into(),
        BoundedSequence::try_from_iter([
            LanguageId::new("language/english".into()).expect("fixture Language")
        ])
        .expect("one fixture Language"),
        BoundedSequence::try_from_iter([]).expect("no private provider mappings"),
        "deterministic-proof-english@1".into(),
        BoundedSequence::try_from_iter([]).expect("no exact fixture varieties"),
        false,
    )
    .expect("finite fixture coverage");
    conduit_language::language_coverage_property(coverage).expect("native fixture property")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_offers_declare_only_their_exact_english_fixture_scope() {
        use conduit_language::{LanguageId, LanguageRequest, LanguageVarietyPolicy};
        for offer in [
            deterministic_speech_offer(),
            deterministic_streaming_speech_offer(),
        ] {
            let property = &offer.realization_properties[0];
            use conduit_plot::rust_binding::NativeRustBinding;
            let coverage =
                conduit_language::LanguageCoverage::decode(property.canonical_value()).unwrap();
            assert_eq!(
                coverage.evidence(),
                offer.implementation.artifact_id.as_str()
            );
            let request = |language: &str| {
                LanguageRequest::new(
                    LanguageId::new(language.into()).unwrap(),
                    None,
                    LanguageVarietyPolicy::LanguageSufficient,
                )
                .unwrap()
            };
            assert!(conduit_language::admit_language_coverage(
                &request("language/english"),
                Some(&coverage)
            )
            .is_ok());
            assert_eq!(
                conduit_language::admit_language_coverage(
                    &request("language/french"),
                    Some(&coverage)
                ),
                Err(conduit_language::LanguageCoverageRefusal::Language)
            );
        }
    }

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
