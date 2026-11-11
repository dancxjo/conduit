use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};

pub const EXPERIENCE_EVIDENCE_IMPLEMENTATION: &str = "std/experience-project-evidence@1";

pub const EXPERIENCE_EVIDENCE_FLOW_IMPLEMENTATION: &str = "std/experience-project-evidence-flow@1";

pub fn experience_evidence_offer() -> CapabilityOffer {
    offer(false)
}

pub fn experience_evidence_flow_offer() -> CapabilityOffer {
    offer(true)
}

fn offer(flow: bool) -> CapabilityOffer {
    let (contract, capability, profile, implementation, artifact) = if flow {
        (
            conduit_semantic_catalog::experience_evidence_flow_contract(),
            "experience-project-evidence-flow",
            "conduit.std/experience-project-evidence-flow-kernel@1",
            EXPERIENCE_EVIDENCE_FLOW_IMPLEMENTATION,
            "conduit-std-host/experience-project-evidence-flow@1",
        )
    } else {
        (
            conduit_semantic_catalog::experience_evidence_contract(),
            "experience-project-evidence",
            "conduit.std/experience-project-evidence-kernel@1",
            EXPERIENCE_EVIDENCE_IMPLEMENTATION,
            "conduit-std-host/experience-project-evidence@1",
        )
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: capability.into(),
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
