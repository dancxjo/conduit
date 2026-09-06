//! Ordinary indicator output over an acquired provider resource, not stdout.
use conduit_core::{kind_id, resource_requirement, CapabilityOffer, HostOperationRequirement};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};
pub const IMPLEMENTATION: &str = "std/kernel-indicator-resource@1";
pub const ARTIFACT: &str = "conduit-std-host/indicator-resource@1";
pub const OPERATION: &str = "conduit.host/indicator-state@1";
pub const RESOURCE_CLASS: &str = "conduit.resource/indicator-output@1";

/// Publish only when the Host holds the corresponding acquired resource.
pub fn offer() -> CapabilityOffer {
    realization_offer(
        conduit_semantic_catalog::indicator_state_presentation_contract(),
        conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_REVISION,
        RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: OPERATION.into(),
            target_kind: Some(kind_id(
                conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND,
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 0,
        }],
        vec![resource_requirement(RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}
