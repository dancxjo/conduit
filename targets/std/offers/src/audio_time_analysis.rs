//! Exact std profile for ordinary bounded PCM observations.
use conduit_core::*;
pub const AUDIO_TIME_ANALYSIS_IMPLEMENTATION: &str = "std/kernel-audio-time-analysis@1";
pub const AUDIO_TIME_ANALYSIS_PROFILE: &str = "conduit.std/audio-time-analysis-f64@1";
pub const AUDIO_TIME_ANALYSIS_ARTIFACT: &str = "conduit-std-host/audio-time-analysis@1";
pub fn audio_time_analysis_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::audio_time_analysis_semantic_contract(),
        Back {
            capability_id: "audio-time-analysis-v1".into(),
            execution_profile_id: AUDIO_TIME_ANALYSIS_PROFILE.into(),
            implementation_id: AUDIO_TIME_ANALYSIS_IMPLEMENTATION.into(),
            artifact_id: AUDIO_TIME_ANALYSIS_ARTIFACT.into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
