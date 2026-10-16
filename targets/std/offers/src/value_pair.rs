//! Exact hosted implementation of one atomic typed Value pair.
use conduit_core::{
    Back, BackOfferBuilder, CapabilityOffer, CheckedValueContract, StructuredInfoType,
};
pub const VALUE_PAIR_IMPLEMENTATION: &str = "std/kernel-value-pair@1";
pub const VALUE_PAIR_PROFILE: &str = "std/value-pair-prepared@1";
pub const VALUE_PAIR_ARTIFACT: &str = "conduit-composite/value-pair@1";
pub fn value_pair_offer(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    let contract =
        conduit_semantic_catalog::value_pair_semantic_contract(left, left_type, right, right_type)?;
    let identity = format!(
        "std/value-pair/{}/{}-{}",
        contract.outputs[0].value_kind.as_str(),
        left.maximum_bytes,
        right.maximum_bytes
    );
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: identity.into(),
            execution_profile_id: VALUE_PAIR_PROFILE.into(),
            implementation_id: VALUE_PAIR_IMPLEMENTATION.into(),
            artifact_id: VALUE_PAIR_ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
