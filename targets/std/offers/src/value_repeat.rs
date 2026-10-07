//! Exact prepared offer for finite repetition of one admitted immutable Value.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId, StructuredInfoType,
};
pub const VALUE_REPEAT_EXECUTION_PROFILE: &str = "conduit.std/value-repeat-prepared@1";
pub const VALUE_REPEAT_CAPACITY2_EXECUTION_PROFILE: &str =
    "conduit.std/value-repeat-prepared-capacity2@1";
pub const VALUE_REPEAT_IMPLEMENTATION: &str = "std/value-repeat-finite@1";
pub const VALUE_REPEAT_ARTIFACT: &str = "conduit-std-host/value-repeat-finite@1";
pub fn value_repeat_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    let contract = conduit_semantic_catalog::value_repeat_semantic_contract(value, schema)?;
    repeat_offer(value, contract, false)
}
pub fn value_repeat_capacity2_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    let contract =
        conduit_semantic_catalog::value_repeat_capacity2_semantic_contract(value, schema)?;
    repeat_offer(value, contract, true)
}
fn repeat_offer(
    value: &CheckedValueContract,
    contract: conduit_core::Kind,
    capacity2: bool,
) -> Result<CapabilityOffer, &'static str> {
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "value-repeat-{}-{}{}",
                value.value_kind.as_str(),
                value.maximum_bytes,
                if capacity2 { "-capacity2" } else { "" }
            )),
            execution_profile_id: ExecutionProfileId::from(if capacity2 {
                VALUE_REPEAT_CAPACITY2_EXECUTION_PROFILE
            } else {
                VALUE_REPEAT_EXECUTION_PROFILE
            }),
            implementation_id: ImplementationId::from(VALUE_REPEAT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(VALUE_REPEAT_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
