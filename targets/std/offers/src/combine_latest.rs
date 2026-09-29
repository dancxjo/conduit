//! Hosted std realization of exact bounded combine-latest state.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId,
};

pub const COMBINE_LATEST_EXECUTION_PROFILE: &str = "conduit.std/combine-latest-prepared@1";
pub const COMBINE_LATEST_IMPLEMENTATION: &str = "std/kernel-combine-latest@1";
pub const COMBINE_LATEST_ARTIFACT: &str = "conduit-std-host/combine-latest@1";
pub const COMBINE_LATEST_MAXIMUM_INPUT_BYTES: u32 = 4 * 1024;

pub fn combine_latest_offer(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if left.maximum_bytes > COMBINE_LATEST_MAXIMUM_INPUT_BYTES
        || right.maximum_bytes > COMBINE_LATEST_MAXIMUM_INPUT_BYTES
    {
        return Err("std state/combine-latest specialization exceeds its prepared input bounds");
    }
    let contract = conduit_semantic_catalog::combine_latest_semantic_contract(left, right)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "combine-latest-{}-{}-{}-{}",
                left.value_kind.as_str(),
                left.maximum_bytes,
                right.value_kind.as_str(),
                right.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(COMBINE_LATEST_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(COMBINE_LATEST_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COMBINE_LATEST_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
