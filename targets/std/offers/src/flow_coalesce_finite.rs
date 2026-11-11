//! Exact finite Current replacement offer; execution uses the shared kernel.
use conduit_core::*;

pub const FLOW_COALESCE_FINITE_IMPLEMENTATION: &str = "std/kernel-flow-coalesce-finite@1";
pub fn flow_coalesce_finite_offer(value: &CheckedValueContract) -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::flow_coalesce_finite_contract(value),
        Back {
            capability_id: alloc_capability(value),
            execution_profile_id: "std/flow-coalesce-finite-kernel@1".into(),
            implementation_id: FLOW_COALESCE_FINITE_IMPLEMENTATION.into(),
            artifact_id: "conduit-kernel/flow-coalesce-latest@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
fn alloc_capability(value: &CheckedValueContract) -> CapabilityId {
    format!(
        "std-flow-coalesce-finite/{}/{}",
        value.value_kind.as_str(),
        value.maximum_bytes
    )
    .into()
}
