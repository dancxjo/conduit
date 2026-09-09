//! Native realization of the existing portable retained text contract.
use alloc::{format, vec};
use conduit_core::{CapabilityOffer, HostOperationRequirement, resource_requirement};

pub(crate) const TEXT_EDIT_IMPLEMENTATION: &str = "conduitos/kernel-text-edit@1";
pub(crate) const TEXT_EDIT_HOST_OPERATION: &str = "conduit.host/conduitos-text-edit@1";

pub(super) fn offer(build_id: &str) -> CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::text_edit_contract(),
        conduit_semantic_catalog::TEXT_EDIT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: TEXT_EDIT_IMPLEMENTATION,
            execution_profile: "conduitos/bounded-text-state@1",
            implementation: TEXT_EDIT_IMPLEMENTATION,
            artifact: "conduitos/text-state@1",
        },
        vec![HostOperationRequirement {
            contract_id: TEXT_EDIT_HOST_OPERATION.into(),
            target_kind: Some("text/bounded-state-output@1".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 4,
            maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
        }],
        vec![resource_requirement(
            "conduit.resource/runtime-memory@1",
            4096,
        )],
        vec![],
    );
    offer.implementation.artifact_id = format!("conduitos-build/{build_id}").into();
    offer
}
