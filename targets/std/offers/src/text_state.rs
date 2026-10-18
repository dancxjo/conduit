//! Hosted std realizations of bounded retained text state.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId, Kind,
};

pub const TEXT_STATE_HOST_CALL: &str = "conduit.host/text-state@1";
pub const TEXT_EDIT_DOCUMENT_STD_IMPLEMENTATION: &str = "std/kernel-text-edit-document@1";
pub const TEXT_EDIT_STD_IMPLEMENTATION: &str = "std/kernel-text-edit@1";
pub const TEXT_SUBMIT_LINES_STD_IMPLEMENTATION: &str = "std/kernel-text-submit-lines@1";

pub fn text_edit_document_std_offer() -> CapabilityOffer {
    let mut realization = offer(
        conduit_semantic_catalog::text_edit_document_semantic_contract(),
        TEXT_EDIT_DOCUMENT_STD_IMPLEMENTATION,
    );
    realization.host_calls[0].target_kind =
        Some(kind_id(conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND));
    realization.host_calls[0].maximum_input_bytes =
        conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES;
    realization.host_calls[0].maximum_output_bytes =
        conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES;
    realization
}

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
            host_calls: vec![HostCallRequirement {
                contract_id: TEXT_STATE_HOST_CALL.into(),
                target_kind: Some(kind_id("text/bounded-state-output@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
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
    fn document_offer_admits_canonical_edit_and_snapshot_bounds() {
        let offer = text_edit_document_std_offer();
        let contract = conduit_semantic_catalog::text_edit_document_semantic_contract();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.host_calls[0].maximum_input_bytes, 272);
        assert_eq!(offer.host_calls[0].maximum_output_bytes, 264);
        assert_eq!(
            offer.host_calls[0].target_kind.as_ref().unwrap().as_str(),
            conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND
        );
    }

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
            assert_eq!(offer.host_calls.len(), 1);
            assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
        }
    }
}
