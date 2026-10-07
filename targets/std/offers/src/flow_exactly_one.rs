//! Exact prepared offer for a singleton closing Flow to Value.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId, StructuredInfoType,
};
pub const FLOW_EXACTLY_ONE_EXECUTION_PROFILE: &str = "conduit.std/flow-exactly-one-prepared@1";
pub const FLOW_EXACTLY_ONE_IMPLEMENTATION: &str = "std/flow-exactly-one@1";
pub const FLOW_EXACTLY_ONE_ARTIFACT: &str = "conduit-std-host/flow-exactly-one@1";
pub fn flow_exactly_one_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    let contract = conduit_semantic_catalog::flow_exactly_one_semantic_contract(value, schema)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "flow-exactly-one-{}-{}",
                value.value_kind.as_str(),
                value.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_EXACTLY_ONE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(FLOW_EXACTLY_ONE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_EXACTLY_ONE_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
