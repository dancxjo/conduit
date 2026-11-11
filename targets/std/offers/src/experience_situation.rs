use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};
pub const EXPERIENCE_SITUATION_IMPLEMENTATION: &str = "std/experience-situation-current@1";
pub fn experience_situation_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_situation_contract(),
        Back {
            capability_id: "experience-situation-current".into(),
            execution_profile_id: "conduit.std/experience-situation-current-kernel@1".into(),
            implementation_id: EXPERIENCE_SITUATION_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-situation-current@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
