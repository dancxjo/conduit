use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};

pub const EXPERIENCE_INTEGRATION_IMPLEMENTATION: &str = "std/experience-integrate-current@1";

pub fn experience_integration_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_integration_contract(),
        Back {
            capability_id: "experience-integrate-current".into(),
            execution_profile_id: "conduit.std/experience-integrate-current-kernel@1".into(),
            implementation_id: EXPERIENCE_INTEGRATION_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-integrate-current@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
