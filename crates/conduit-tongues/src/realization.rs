use crate::{
    audio_play_contract, synthesize_contract, AUDIO_PLAY_KIND, MAXIMUM_PCM_BYTES,
    MAXIMUM_TEXT_BYTES,
};
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId,
    AuthorityRequirement, BootId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, ImplementationOffer,
    OfferGeneration, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

pub const SYNTHESIZE_OPERATION: &str = "conduit.host/speech-synthesize@1";
pub const PLAY_AUDIO_OPERATION: &str = "conduit.host/audio-playback@1";
pub const WRITE_WAV_OPERATION: &str = "conduit.host/audio-wav-write@1";
pub const AUDIO_OUTPUT_AUTHORITY: &str = "conduit.authority/audio-output@1";
pub const ARTIFACT_WRITE_AUTHORITY: &str = "conduit.authority/artifact-write@1";
pub const CPU_RESOURCE: &str = "conduit.resource/compute/cpu@1";
pub const PCM_BUFFER_RESOURCE: &str = "conduit.resource/audio/pcm-buffer@1";
pub const AUDIO_DEVICE_RESOURCE: &str = "conduit.resource/audio/output-device@1";
pub const ARTIFACT_RESOURCE: &str = "conduit.resource/storage/artifact@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputCondition {
    PrimaryPlayback,
    DegradedWavArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechRealizationFacts {
    pub condition: OutputCondition,
    pub output_guarantee: String,
    pub output_base_pool_id: String,
    pub maximum_text_bytes: u32,
    pub maximum_pcm_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechHostFixture {
    pub advertisement: HostAdvertisement,
    pub facts: SpeechRealizationFacts,
}

pub fn speech_host_fixture(condition: OutputCondition) -> SpeechHostFixture {
    let (
        host,
        boot,
        output_operation,
        output_impl,
        output_artifact,
        resource,
        authority,
        guarantee,
    ) = match condition {
        OutputCondition::PrimaryPlayback => (
            "tongues-primary-host",
            "tongues-primary-boot",
            PLAY_AUDIO_OPERATION,
            "std/audio-playback@1",
            "conduit-tongues/audio-playback@1",
            AUDIO_DEVICE_RESOURCE,
            AUDIO_OUTPUT_AUTHORITY,
            "submitted-to-admitted-output-device",
        ),
        OutputCondition::DegradedWavArtifact => (
            "tongues-degraded-host",
            "tongues-degraded-boot",
            WRITE_WAV_OPERATION,
            "std/wav-artifact@1",
            "conduit-tongues/wav-artifact@1",
            ARTIFACT_RESOURCE,
            ARTIFACT_WRITE_AUTHORITY,
            "bounded-wav-artifact-produced-not-played",
        ),
    };
    let synth = synthesize_contract();
    let present = audio_play_contract();
    let synthesis_operation =
        host_operation(SYNTHESIZE_OPERATION, MAXIMUM_TEXT_BYTES, MAXIMUM_PCM_BYTES);
    let mut output_operation_requirement = host_operation(output_operation, MAXIMUM_PCM_BYTES, 256);
    output_operation_requirement.target_kind = Some(kind_id(AUDIO_PLAY_KIND));
    SpeechHostFixture {
        advertisement: HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("conduit.host/tongues-brownfield@1"),
            resources: {
                let mut resources = vec![
                    resource_offer(&format!("{host}/cpu-0"), CPU_RESOURCE, 1),
                    resource_offer(&format!("{host}/pcm-0"), PCM_BUFFER_RESOURCE, 1),
                    resource_offer(&format!("{host}/output-0"), resource, 1),
                ];
                resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
                resources
            },
            capabilities: vec![
                CapabilityOffer {
                    startup_parameters: vec![FaceStartupParameter {
                        name: "maximum-output-bytes".into(),
                        value_type: "Count".into(),
                        has_default: true,
                    }],
                    shorthand: None,
                    capability_id: CapabilityId::from(format!("{host}/synthesize")),
                    kind_id: synth.kind_id,
                    kind_contract_revision: synth.kind_contract_revision,
                    inputs: synth.inputs,
                    outputs: synth.outputs,
                    implementation: ImplementationOffer {
                        execution_profile_id: ExecutionProfileId::from(
                            "conduit.speech/deterministic-hosted@1",
                        ),
                        implementation_id: ImplementationId::from(
                            "tongues/fixture-tts-adapter@5748f20e",
                        ),
                        artifact_id: ArtifactId::from("tongues-pipeline/text-to-speech@5748f20e"),
                    },
                    host_operations: vec![synthesis_operation],
                    resource_requirements: {
                        let mut requirements = vec![
                            resource_requirement(CPU_RESOURCE, 1),
                            resource_requirement(PCM_BUFFER_RESOURCE, 1),
                        ];
                        requirements.sort_by(|left, right| left.class_id.cmp(&right.class_id));
                        requirements
                    },
                    authority_requirements: vec![],
                    limits: synth.limits,
                },
                CapabilityOffer {
                    startup_parameters: vec![],
                    shorthand: None,
                    capability_id: CapabilityId::from(format!("{host}/output")),
                    kind_id: present.kind_id,
                    kind_contract_revision: present.kind_contract_revision,
                    inputs: present.inputs,
                    outputs: present.outputs,
                    implementation: ImplementationOffer {
                        execution_profile_id: ExecutionProfileId::from(
                            "conduit.audio/bounded-output@1",
                        ),
                        implementation_id: ImplementationId::from(output_impl),
                        artifact_id: ArtifactId::from(output_artifact),
                    },
                    host_operations: vec![output_operation_requirement.clone()],
                    resource_requirements: vec![resource_requirement(resource, 1)],
                    authority_requirements: vec![AuthorityRequirement {
                        contract_id: AuthorityContractId::from(authority),
                        host_operation_contract_id: output_operation_requirement.contract_id,
                        subject_kind: kind_id(AUDIO_PLAY_KIND),
                    }],
                    limits: present.limits,
                },
            ],
            planner_capabilities: vec![],
        },
        facts: SpeechRealizationFacts {
            condition,
            output_guarantee: guarantee.into(),
            output_base_pool_id: format!("{host}/output-0"),
            maximum_text_bytes: MAXIMUM_TEXT_BYTES,
            maximum_pcm_bytes: MAXIMUM_PCM_BYTES,
        },
    }
}

fn host_operation(contract: &str, input: u32, output: u32) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_and_degraded_outputs_share_face_but_not_guarantees_or_authority() {
        let primary = speech_host_fixture(OutputCondition::PrimaryPlayback);
        let degraded = speech_host_fixture(OutputCondition::DegradedWavArtifact);
        assert_eq!(
            primary.advertisement.capabilities[1].checked_face(),
            degraded.advertisement.capabilities[1].checked_face()
        );
        assert_ne!(
            primary.facts.output_guarantee,
            degraded.facts.output_guarantee
        );
        assert_ne!(
            primary.advertisement.capabilities[1].implementation,
            degraded.advertisement.capabilities[1].implementation
        );
        assert_ne!(
            primary.advertisement.capabilities[1].authority_requirements,
            degraded.advertisement.capabilities[1].authority_requirements
        );
    }
}
