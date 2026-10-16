//! Exact std profile for ordinary bounded PCM pitch observations.
use conduit_core::*;
pub const AUDIO_PITCH_ANALYSIS_IMPLEMENTATION: &str = "std/kernel-audio-pitch-analysis@1";
pub const AUDIO_PITCH_ANALYSIS_PROFILE: &str = "conduit.std/audio-pitch-analysis-f64@1";
pub const AUDIO_PITCH_ANALYSIS_ARTIFACT: &str = "conduit-std-host/audio-pitch-analysis@1";
pub fn audio_pitch_analysis_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::audio_pitch_analysis_semantic_contract(),
        Back {
            capability_id: "audio-pitch-analysis-v1".into(),
            execution_profile_id: AUDIO_PITCH_ANALYSIS_PROFILE.into(),
            implementation_id: AUDIO_PITCH_ANALYSIS_IMPLEMENTATION.into(),
            artifact_id: AUDIO_PITCH_ANALYSIS_ARTIFACT.into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
