//! Exact allocation-free PCM representation profile; no device or synthesis effect.
use conduit_core::*;
pub const PCM16_PACKING_IMPLEMENTATION: &str = "std/kernel-pcm16-packing@1";
pub const PCM16_PACKING_PROFILE: &str = "std/pcm16-packing-prepared@1";
pub const PCM16_PACKING_ARTIFACT: &str = "conduit-audio/pcm16-packing@1";
pub fn pcm16_packing_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::pcm16_packing_semantic_contract(),
        Back {
            capability_id: "std/pcm16-packing@1".into(),
            execution_profile_id: PCM16_PACKING_PROFILE.into(),
            implementation_id: PCM16_PACKING_IMPLEMENTATION.into(),
            artifact_id: PCM16_PACKING_ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
