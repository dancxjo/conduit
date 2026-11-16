//! Finite audible cue realization; publishing an offer grants no playback.
use conduit_core::*;
pub const STARTUP_CHIME_PROFILE: &str = "std/startup-chime-alsa-s16le-48000-stereo@1";
pub const STARTUP_CHIME_IMPLEMENTATION: &str = "std/kernel-startup-chime@1";
pub const STARTUP_CHIME_ARTIFACT: &str = "conduit-std-host/startup-chime@1";
pub const STARTUP_CHIME_OPERATION: &str = "conduit.host/startup-chime@1";

pub fn startup_chime_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::startup_chime_contract()
            .into_semantic_contract(conduit_semantic_catalog::STARTUP_CHIME_REVISION),
        Back {
            capability_id: "startup-chime".into(),
            execution_profile_id: STARTUP_CHIME_PROFILE.into(),
            implementation_id: STARTUP_CHIME_IMPLEMENTATION.into(),
            artifact_id: STARTUP_CHIME_ARTIFACT.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: STARTUP_CHIME_OPERATION.into(),
                target_kind: Some(BOOL_INFO_ID.into()),
                maximum_in_flight: 1,
                maximum_input_bytes: BOOL_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(
                crate::AUDIO_PLAYBACK_RESOURCE_CLASS,
                1,
            )],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: crate::AUDIO_PLAYBACK_AUTHORITY_CONTRACT.into(),
                host_call_contract_id: STARTUP_CHIME_OPERATION.into(),
                subject_kind: BOOL_INFO_ID.into(),
            }],
        },
    )
    .build()
}
