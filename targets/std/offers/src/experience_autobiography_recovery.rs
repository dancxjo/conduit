//! Explicitly selected recoverable model dependency; ordinary retention stays strict.
use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};
pub const EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_IMPLEMENTATION: &str =
    "std/experience-autobiography-retain-recoverable@1";
pub fn experience_autobiography_recovery_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_autobiography_recovery_contract(),
        Back {
            capability_id: "experience-autobiography-retain-recoverable".into(),
            execution_profile_id:
                "conduit.std/experience-autobiography-retain-recoverable-kernel@1".into(),
            implementation_id: EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-autobiography-retain-recoverable@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
