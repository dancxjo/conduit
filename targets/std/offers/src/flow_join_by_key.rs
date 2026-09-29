//! Hosted std realization of the exact finite keyed join.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId,
};

pub const FLOW_JOIN_BY_KEY_EXECUTION_PROFILE: &str = "conduit.std/flow-join-by-key-prepared@1";
pub const FLOW_JOIN_BY_KEY_IMPLEMENTATION: &str = "std/kernel-flow-join-by-key@1";
pub const FLOW_JOIN_BY_KEY_ARTIFACT: &str = "conduit-std-host/flow-join-by-key@1";
pub const FLOW_JOIN_BY_KEY_MAXIMUM_COMPONENT_BYTES: u32 = 4 * 1024;

pub fn flow_join_by_key_offer(
    key: &CheckedValueContract,
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if [key, left, right]
        .iter()
        .any(|contract| contract.maximum_bytes > FLOW_JOIN_BY_KEY_MAXIMUM_COMPONENT_BYTES)
    {
        return Err("std flow/join/by-key specialization exceeds its prepared component bounds");
    }
    let contract = conduit_semantic_catalog::flow_join_by_key_semantic_contract(key, left, right)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "flow-join-by-key-{}-{}-{}-{}-{}-{}",
                key.value_kind.as_str(),
                key.maximum_bytes,
                left.value_kind.as_str(),
                left.maximum_bytes,
                right.value_kind.as_str(),
                right.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_JOIN_BY_KEY_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(FLOW_JOIN_BY_KEY_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_JOIN_BY_KEY_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
