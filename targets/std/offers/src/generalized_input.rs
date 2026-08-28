//! Exact deterministic generalized-input offers owned by the hosted std Host.

use alloc::{format, vec, vec::Vec};
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

extern crate alloc;

pub const GENERALIZED_INPUT_PROFILE: &str = "std/generalized-input-deterministic@1";
pub const GENERALIZED_INPUT_ARTIFACT: &str = "conduit-std-host/generalized-input@1";
pub const GENERALIZED_INPUT_HOST_OPERATION: &str = "conduit.host/generalized-input@1";

pub fn generalized_input_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            conduit_semantic_catalog::DETERMINISTIC_GAMEPAD_KIND,
            conduit_semantic_catalog::deterministic_gamepad_outputs(),
        ),
        offer(
            conduit_semantic_catalog::DETERMINISTIC_POINTER_TOUCH_KIND,
            conduit_semantic_catalog::deterministic_pointer_touch_outputs(),
        ),
    ]
}

fn offer(kind: &str, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::GENERALIZED_INPUT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(GENERALIZED_INPUT_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(GENERALIZED_INPUT_ARTIFACT),
        },
        inputs: vec![],
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(GENERALIZED_INPUT_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 8,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_portable_faces_and_finite_effects() {
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
