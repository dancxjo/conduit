//! Bounded, effect-free realization of recurring nominal pulse observations.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};
pub const PULSE_OBSERVE_PROFILE: &str = "std/pulse-observe-ordered-64@1";
pub const PULSE_OBSERVE_IMPLEMENTATION: &str = "std/kernel-pulse-observe@1";
pub const PULSE_OBSERVE_ARTIFACT: &str = "conduit-std-host/pulse-observe@1";

pub fn pulse_observe_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_time::pulse_observe_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("pulse-observe"),
            execution_profile_id: ExecutionProfileId::from(PULSE_OBSERVE_PROFILE),
            implementation_id: ImplementationId::from(PULSE_OBSERVE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(PULSE_OBSERVE_ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
