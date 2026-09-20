//! Exact deterministic generalized-input offers owned by the hosted std Host.

use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, Kind,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

extern crate alloc;

pub const GENERALIZED_INPUT_PROFILE: &str = "std/generalized-input-deterministic@1";
pub const GENERALIZED_INPUT_ARTIFACT: &str = "conduit-std-host/generalized-input@1";
pub const GENERALIZED_INPUT_HOST_OPERATION: &str = "conduit.host/generalized-input@1";

pub fn generalized_input_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(conduit_semantic_catalog::deterministic_gamepad_semantic_contract()),
        offer(conduit_semantic_catalog::deterministic_pointer_touch_semantic_contract()),
    ]
}

fn offer(contract: Kind) -> CapabilityOffer {
    let identity = format!("std/{}@1", contract.kind_id.as_str());
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(identity.clone()),
            execution_profile_id: ExecutionProfileId::from(GENERALIZED_INPUT_PROFILE),
            implementation_id: ImplementationId::from(identity),
            artifact_id: ArtifactId::from(GENERALIZED_INPUT_ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(GENERALIZED_INPUT_HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_portable_fronts_and_finite_effects() {
        let offers = generalized_input_std_offers();
        assert_eq!(offers.len(), 2);
        assert_eq!(
            offers[0].outputs,
            conduit_semantic_catalog::deterministic_gamepad_outputs()
        );
        assert_eq!(
            offers[1].outputs,
            conduit_semantic_catalog::deterministic_pointer_touch_outputs()
        );
        for offer in offers {
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
            assert_eq!(offer.limits.max_queue_items, 8);
            assert!(offer.authority_requirements.is_empty());
        }
    }
}
