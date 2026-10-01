//! Hosted std offer for exact bounded Flow collection.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer,
    CheckedValueContract, ExecutionProfileId, ImplementationId, UNIT_INFO_ID,
};

pub const FLOW_COLLECT_EXECUTION_PROFILE: &str = "conduit.std/flow-collect-prepared@1";
pub const FLOW_COLLECT_IMPLEMENTATION: &str = "std/kernel-flow-collect@1";
pub const FLOW_COLLECT_ARTIFACT: &str = "conduit-std-host/flow-collect@1";
pub const FLOW_COLLECT_MAXIMUM_ELEMENT_BYTES: u32 = 4 * 1024;
pub const FLOW_COLLECT_MAXIMUM_ITEMS: u16 = 256;

/// Offers the allocation-prepared std realization with a canonical Unit
/// overflow disposition. Arbitrary typed overflow payload construction is not
/// inferred from a type contract.
pub fn flow_collect_offer(
    element: &CheckedValueContract,
    maximum_items: u16,
) -> Result<CapabilityOffer, &'static str> {
    if element.maximum_bytes > FLOW_COLLECT_MAXIMUM_ELEMENT_BYTES
        || maximum_items > FLOW_COLLECT_MAXIMUM_ITEMS
    {
        return Err("std flow/collect specialization exceeds its prepared bounds");
    }
    let overflow = CheckedValueContract::new(kind_id(UNIT_INFO_ID), 0, Vec::new())
        .map_err(|_| "std flow/collect Unit overflow contract is invalid")?;
    let contract = conduit_semantic_catalog::flow_collect_semantic_contract(
        element,
        maximum_items,
        &overflow,
    )?;
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!(
                "flow-collect-{}-{}-{}",
                element.value_kind.as_str(),
                element.maximum_bytes,
                maximum_items
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_COLLECT_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(FLOW_COLLECT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_COLLECT_ARTIFACT),
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

    #[test]
    fn offer_is_exact_effect_free_and_unit_overflow_specialized() {
        let element = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let offer = flow_collect_offer(&element, 4).unwrap();
        let law = offer.semantic_contract.bounded_collect().unwrap();
        assert_eq!(law.element, element);
        assert_eq!(law.maximum_items, 4);
        assert_eq!(law.overflow_disposition.value_kind.as_str(), UNIT_INFO_ID);
        assert!(offer.host_calls.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }

    #[test]
    fn offer_refuses_specializations_outside_the_std_storage_profile() {
        let too_large = CheckedValueContract::new(
            kind_id("value/bytes"),
            FLOW_COLLECT_MAXIMUM_ELEMENT_BYTES + 1,
            vec![],
        )
        .unwrap();
        assert!(flow_collect_offer(&too_large, 1).is_err());
        let ordinary = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        assert!(flow_collect_offer(&ordinary, FLOW_COLLECT_MAXIMUM_ITEMS + 1).is_err());
    }
}
