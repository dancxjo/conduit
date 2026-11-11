use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};

pub const EXPERIENCE_PROMPT_IMPLEMENTATION: &str = "std/experience-context-to-prompt@1";

pub fn experience_prompt_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_prompt_contract(),
        Back {
            capability_id: "experience-context-to-prompt".into(),
            execution_profile_id: "conduit.std/experience-context-to-prompt-kernel@1".into(),
            implementation_id: EXPERIENCE_PROMPT_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-context-to-prompt@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
