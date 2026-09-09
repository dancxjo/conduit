//! Hosted std realizations of bounded retained text state.

use conduit_core::{kind_id, CapabilityOffer, HostOperationRequirement};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const TEXT_STATE_HOST_OPERATION: &str = "conduit.host/text-state@1";
pub const TEXT_EDIT_STD_IMPLEMENTATION: &str = "std/kernel-text-edit@1";
pub const TEXT_SUBMIT_LINES_STD_IMPLEMENTATION: &str = "std/kernel-text-submit-lines@1";

pub fn text_edit_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_edit_contract(),
        conduit_semantic_catalog::TEXT_EDIT_REVISION,
        TEXT_EDIT_STD_IMPLEMENTATION,
    )
}

pub fn text_submit_lines_std_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::text_submit_lines_contract(),
        conduit_semantic_catalog::TEXT_SUBMIT_LINES_REVISION,
        TEXT_SUBMIT_LINES_STD_IMPLEMENTATION,
    )
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    revision: &str,
    implementation: &'static str,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability: implementation,
            execution_profile: implementation,
            implementation,
            artifact: "conduit-std-host/text-state@1",
        },
        vec![HostOperationRequirement {
            contract_id: TEXT_STATE_HOST_OPERATION.into(),
            target_kind: Some(kind_id("text/bounded-state-output@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: 4,
            maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_EDITED_TEXT_BYTES,
        }],
        Vec::new(),
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_preserve_the_portable_bounded_faces() {
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
