//! Pure finite PCM expansion; no microphone or playback authority.
use conduit_core::*;
pub const PCM_CLIP_FRAMES_IMPLEMENTATION: &str = "std/kernel-pcm-clip-frames@1";
pub const PCM_CLIP_FRAMES_PROFILE: &str = "conduit.std/pcm-clip-frames@1";
pub const PCM_CLIP_FRAMES_ARTIFACT: &str = "conduit-std-host/pcm-clip-frames@1";
pub fn pcm_clip_frames_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_audio::pcm_clip_frames_semantic_contract(),
        Back {
            capability_id: "pcm-clip-frames-v1".into(),
            execution_profile_id: PCM_CLIP_FRAMES_PROFILE.into(),
            implementation_id: PCM_CLIP_FRAMES_IMPLEMENTATION.into(),
            artifact_id: PCM_CLIP_FRAMES_ARTIFACT.into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
