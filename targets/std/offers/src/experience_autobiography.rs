use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};

pub const EXPERIENCE_AUTOBIOGRAPHY_RETAIN_IMPLEMENTATION: &str =
    "std/experience-autobiography-retain@1";
pub fn experience_autobiography_retain_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_autobiography_retain_contract(),
        Back {
            capability_id: "experience-autobiography-retain".into(),
            execution_profile_id: "conduit.std/experience-autobiography-retain-kernel@1".into(),
            implementation_id: EXPERIENCE_AUTOBIOGRAPHY_RETAIN_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-autobiography-retain@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub const EXPERIENCE_AUTOBIOGRAPHY_RECALL_IMPLEMENTATION: &str =
    "std/experience-autobiography-recall@1";
pub fn experience_autobiography_recall_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_autobiography_recall_contract(),
        Back {
            capability_id: "experience-autobiography-recall".into(),
            execution_profile_id: "conduit.std/experience-autobiography-recall-kernel@1".into(),
            implementation_id: EXPERIENCE_AUTOBIOGRAPHY_RECALL_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-autobiography-recall@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
