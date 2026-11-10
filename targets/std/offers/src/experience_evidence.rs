use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};

pub const EXPERIENCE_EVIDENCE_IMPLEMENTATION: &str = "std/experience-project-evidence@1";

pub fn experience_evidence_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_evidence_contract(),
        Back {
            capability_id: "experience-project-evidence".into(),
            execution_profile_id: "conduit.std/experience-project-evidence-kernel@1".into(),
            implementation_id: EXPERIENCE_EVIDENCE_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-project-evidence@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub const EXPERIENCE_MODEL_REQUEST_IMPLEMENTATION: &str = "std/experience-model-request@1";

pub fn experience_model_request_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::experience_model_request_contract(),
        Back {
            capability_id: "experience-model-request".into(),
            execution_profile_id: "conduit.std/experience-model-request-kernel@1".into(),
            implementation_id: EXPERIENCE_MODEL_REQUEST_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/experience-model-request@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
