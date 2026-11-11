//! Explicit finite temporal retrieval through the ordinary std kernel.
use conduit_core::{Back, BackOfferBuilder, CapabilityOffer};
pub const TEMPORAL_EVIDENCE_FLOW_IMPLEMENTATION: &str = "std/temporal-evidence-flow@1";
pub fn temporal_evidence_flow_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_ai::temporal_evidence_flow_contract(),
        Back {
            capability_id: "temporal-evidence-flow".into(),
            execution_profile_id: "conduit.std/temporal-evidence-flow-kernel@1".into(),
            implementation_id: TEMPORAL_EVIDENCE_FLOW_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/temporal-evidence-flow@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
