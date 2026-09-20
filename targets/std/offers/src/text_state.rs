//! Hosted std realizations of bounded retained text state.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, Kind,
};

pub const TEXT_STATE_HOST_OPERATION: &str = "conduit.host/text-state@1";
pub const TEXT_EDIT_STD_IMPLEMENTATION: &str = "std/kernel-text-edit@1";
pub const TEXT_SUBMIT_LINES_STD_IMPLEMENTATION: &str = "std/kernel-text-submit-lines@1";

pub fn text_edit_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_edit_semantic_contract(),
        TEXT_EDIT_STD_IMPLEMENTATION,
    )
}

pub fn text_submit_lines_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_submit_lines_semantic_contract(),
        TEXT_SUBMIT_LINES_STD_IMPLEMENTATION,
    )
}

fn offer(contract: Kind, implementation: &'static str) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-std-host/text-state@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: TEXT_STATE_HOST_OPERATION.into(),
                target_kind: Some(kind_id("text/bounded-state-output@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: 4,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
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
    fn offers_preserve_the_portable_bounded_fronts() {
        for (offer, contract) in [
            (
                text_edit_std_offer(),
                conduit_semantic_catalog::text_edit_contract(),
            ),
            (
                text_submit_lines_std_offer(),
                conduit_semantic_catalog::text_submit_lines_contract(),
            ),
        ] {
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
        }
    }
}
