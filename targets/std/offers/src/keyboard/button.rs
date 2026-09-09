//! An explicitly selected Space-key realization of ordinary button meaning.
use conduit_core::{
    resource_requirement, CapabilityOffer, HostOperationContractId, HostOperationRequirement,
    INPUT_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const IMPLEMENTATION: &str = "std/kernel-space-button@1";
pub const ARTIFACT: &str = "conduit-std-host/space-button@1";
pub const MAPPER: &str = "std/kernel-button-indicator-state@1";
pub const INDICATOR: &str = "std/stdout-indicator-state@1";
pub const NEXT_TRANSITION_HOST_OPERATION: &str = "conduit.host/input-next-button-transition@1";

pub fn mapper_offer() -> CapabilityOffer {
    realization_offer(
        conduit_semantic_catalog::button_indicator_state_contract(),
        conduit_semantic_catalog::BUTTON_INDICATOR_STATE_REVISION,
        RealizationOfferIdentity {
            capability: MAPPER,
            execution_profile: MAPPER,
            implementation: MAPPER,
            artifact: ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// Current Boolean state manifested through the existing stdout host operation.
pub fn indicator_offer() -> CapabilityOffer {
    let stdout = crate::bool_presentation_offer();
    realization_offer(
        conduit_semantic_catalog::indicator_state_presentation_contract(),
        conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_REVISION,
        RealizationOfferIdentity {
            capability: INDICATOR,
            execution_profile: INDICATOR,
            implementation: INDICATOR,
            artifact: ARTIFACT,
        },
        stdout.host_operations,
        stdout.resource_requirements,
        Vec::new(),
    )
}

/// Advertise only with a currently acquired keyboard input resource/adapter.
pub fn offer() -> CapabilityOffer {
    realization_offer(
        conduit_semantic_catalog::button_source_contract(),
        conduit_semantic_catalog::BUTTON_SOURCE_REVISION,
        RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(NEXT_TRANSITION_HOST_OPERATION),
            target_kind: Some(
                conduit_semantic_catalog::input_button_transition_type()
                    .profile()
                    .expect("reviewed button transition profile")
                    .value_kind()
                    .clone(),
            ),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
        }],
        vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}
