//! Hosted std realization of exact bounded two-Flow zip.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, ImplementationId,
};

pub const FLOW_ZIP_EXECUTION_PROFILE: &str = "conduit.std/flow-zip-prepared@1";
pub const FLOW_ZIP_IMPLEMENTATION: &str = "std/kernel-flow-zip@1";
pub const FLOW_ZIP_ARTIFACT: &str = "conduit-std-host/flow-zip@1";
pub const FLOW_ZIP_MAXIMUM_INPUT_BYTES: u32 = 4 * 1024;

pub fn flow_zip_offer(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    if left.maximum_bytes > FLOW_ZIP_MAXIMUM_INPUT_BYTES
        || right.maximum_bytes > FLOW_ZIP_MAXIMUM_INPUT_BYTES
    {
        return Err("std flow/zip specialization exceeds its prepared input bounds");
    }
    let contract = conduit_semantic_catalog::flow_zip_semantic_contract(left, right)?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "flow-zip-{}-{}-{}-{}",
                left.value_kind.as_str(),
                left.maximum_bytes,
                right.value_kind.as_str(),
                right.maximum_bytes
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_ZIP_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(FLOW_ZIP_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_ZIP_ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, KindSemanticLaw};

    #[test]
    fn offer_preserves_both_exact_input_contracts_without_host_effects() {
        let left = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let right = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let offer = flow_zip_offer(&left, &right).unwrap();
        assert!(offer.host_calls.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert_eq!(offer.inputs[0].value_kind, left.value_kind);
        assert_eq!(offer.inputs[1].value_kind, right.value_kind);
        assert_eq!(
            offer
                .semantic_contract
                .laws
                .iter()
                .filter(|law| matches!(law, KindSemanticLaw::TerminalTransduction(_)))
                .count(),
            2
        );
    }
}
