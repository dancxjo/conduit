//! Hosted realization offers for the semantic stages unique to a spoken Mask.

use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, Kind,
};

pub const PRESENTATION_REQUEST_IMPLEMENTATION: &str = "std/spoken-mask-presentation-request@1";
pub const GENERATED_STREAM_SPEECH_IMPLEMENTATION: &str =
    "std/spoken-mask-generated-stream-speech@1";
pub const GENERATED_SPEECH_IMPLEMENTATION: &str = "std/spoken-mask-generated-speech@1";
pub const SPOKEN_ARTIFACT_IMPLEMENTATION: &str = "std/spoken-mask-wav-artifact@1";
pub const ARTIFACT_SHOW_IMPLEMENTATION: &str = "std/spoken-mask-artifact-show@1";
pub const NO_INTERACTION_IMPLEMENTATION: &str = "std/spoken-mask-no-interaction@1";
pub const VALIDATION_ENVELOPE_IMPLEMENTATION: &str = "std/generated-validation-envelope@2";
pub const GENERATED_VALIDATOR_IMPLEMENTATION: &str = "std/generated-semantic-validator@2";
pub const RETAIN_GENERATED_VALIDATION_IMPLEMENTATION: &str = "std/retain-generated-validation@1";
pub const PRESENTATION_REQUEST_OPERATION: &str = "conduit.host/spoken-mask-request@1";
pub const GENERATED_SPEECH_OPERATION: &str = "conduit.host/spoken-mask-speech@1";
pub const SPOKEN_ARTIFACT_OPERATION: &str = "conduit.host/spoken-mask-artifact@1";
pub const REGISTER_MANIFESTATION_OPERATION: &str = "conduit.host/spoken-mask-register@1";
pub const ARTIFACT_SHOW_OPERATION: &str = "conduit.host/spoken-mask-show@1";
pub const REGISTER_GENERATED_CANDIDATE_OPERATION: &str =
    "conduit.host/register-generated-candidate@1";
pub const RETAIN_GENERATED_ASSESSMENT_OPERATION: &str =
    "conduit.host/retain-generated-assessment@1";
pub const REGISTER_VALIDATION_REQUEST_OPERATION: &str =
    "conduit.host/register-generated-validation-request@1";
pub const BUILD_VALIDATION_ENVELOPE_OPERATION: &str =
    "conduit.host/build-generated-validation-envelope@2";
pub const ASSESS_GENERATED_ENVELOPE_OPERATION: &str =
    "conduit.host/assess-generated-validation-envelope@2";

pub fn spoken_mask_offers() -> Vec<CapabilityOffer> {
    vec![
        semantic_offer(
            conduit_presentation::GENERATED_VALIDATION_ENVELOPE_KIND,
            "generated-validation-envelope",
            VALIDATION_ENVELOPE_IMPLEMENTATION,
            vec![
                call(
                    BUILD_VALIDATION_ENVELOPE_OPERATION,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                    conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES as u32,
                ),
                call(
                    REGISTER_VALIDATION_REQUEST_OPERATION,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
                    0,
                ),
            ],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::GENERATED_VALIDATOR_KIND,
            "generated-semantic-validator",
            GENERATED_VALIDATOR_IMPLEMENTATION,
            vec![call(
                ASSESS_GENERATED_ENVELOPE_OPERATION,
                conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES as u32,
                conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            )],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::RETAIN_GENERATED_VALIDATION_KIND,
            "retain-generated-validation",
            RETAIN_GENERATED_VALIDATION_IMPLEMENTATION,
            vec![
                call(
                    REGISTER_GENERATED_CANDIDATE_OPERATION,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                    0,
                ),
                call(
                    RETAIN_GENERATED_ASSESSMENT_OPERATION,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                ),
            ],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::PRESENTATION_TO_GENERATIVE_REQUEST_KIND,
            "spoken-mask-presentation-request",
            PRESENTATION_REQUEST_IMPLEMENTATION,
            vec![call(
                PRESENTATION_REQUEST_OPERATION,
                conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
                conduit_presentation::MAX_GENERATIVE_PRESENTER_INPUT_BYTES as u32,
            )],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::GENERATED_MANIFESTATION_TO_SPEECH_KIND,
            "spoken-mask-generated-speech",
            GENERATED_SPEECH_IMPLEMENTATION,
            vec![call(
                GENERATED_SPEECH_OPERATION,
                conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                conduit_tongues::MAXIMUM_TEXT_BYTES,
            )],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::SPOKEN_ARTIFACT_KIND,
            "spoken-mask-wav-artifact",
            SPOKEN_ARTIFACT_IMPLEMENTATION,
            vec![targeted_call(
                SPOKEN_ARTIFACT_OPERATION,
                crate::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
                4_096,
                conduit_audio::AUDIO_PCM_INFO_ID,
            )],
            vec![resource_requirement(
                crate::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS,
                1,
            )],
            vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(
                    crate::AUDIO_WAV_ARTIFACT_AUTHORITY_CONTRACT,
                ),
                host_call_contract_id: HostCallContractId::from(SPOKEN_ARTIFACT_OPERATION),
                subject_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
            }],
        ),
        semantic_offer(
            conduit_presentation::ARTIFACT_ACKNOWLEDGED_SHOW_KIND,
            "spoken-mask-artifact-show",
            ARTIFACT_SHOW_IMPLEMENTATION,
            vec![
                call(
                    REGISTER_MANIFESTATION_OPERATION,
                    conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
                    0,
                ),
                call(ARTIFACT_SHOW_OPERATION, 4_096, 262_144),
            ],
            vec![],
            vec![],
        ),
        semantic_offer(
            conduit_presentation::CLOSING_NO_INTERACTION_KIND,
            "spoken-mask-no-interaction",
            NO_INTERACTION_IMPLEMENTATION,
            vec![],
            vec![],
            vec![],
        ),
    ]
}

/// Explicit closing-flow projection of one accepted outward Speech segment.
/// Canonical committed segments perform segmentation after validation.
pub fn generated_stream_speech_offer() -> CapabilityOffer {
    semantic_offer(
        conduit_presentation::GENERATED_MANIFESTATION_TO_SPEECH_STREAM_KIND,
        "spoken-mask-generated-stream-speech",
        GENERATED_STREAM_SPEECH_IMPLEMENTATION,
        vec![call(
            GENERATED_SPEECH_OPERATION,
            conduit_presentation::MAX_GENERATIVE_PRESENTER_OUTPUT_BYTES as u32,
            1024,
        )],
        vec![],
        vec![],
    )
}

fn semantic_offer(
    kind_identity: &str,
    capability: &str,
    implementation: &str,
    host_calls: Vec<HostCallRequirement>,
    resources: Vec<conduit_core::ResourceRequirement>,
    authority: Vec<AuthorityRequirement>,
) -> CapabilityOffer {
    let contract = contract(kind_identity);
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("std/spoken-mask-kernel@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-std-host/spoken-mask@1"),
            host_calls,
            resource_requirements: resources,
            authority_requirements: authority,
        },
    )
    .build()
}

fn contract(identity: &str) -> Kind {
    conduit_presentation::spoken_mask_kinds()
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == identity)
        .expect("spoken Mask offer uses one canonical stage Kind")
}

fn call(id: &str, input: u32, output: u32) -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(id),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

fn targeted_call(id: &str, input: u32, output: u32, target: &str) -> HostCallRequirement {
    HostCallRequirement {
        target_kind: Some(kind_id(target)),
        ..call(id, input, output)
    }
}
