//! An explicitly selected Space-key realization of ordinary button meaning.
use conduit_core::{
    resource_requirement, ArtifactId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    Kind, ResourceRequirement, INPUT_RESOURCE_CLASS,
};

pub const IMPLEMENTATION: &str = "std/kernel-space-button@1";
pub const ARTIFACT: &str = "conduit-std-host/space-button@1";
pub const MAPPER: &str = "std/kernel-button-indicator-state@1";
pub const INDICATOR: &str = "std/stdout-indicator-state@1";
pub const NEXT_TRANSITION_HOST_CALL: &str = "conduit.host/input-next-button-transition@1";

pub fn mapper_offer() -> CapabilityOffer {
    button_offer(
        conduit_semantic_catalog::button_indicator_state_semantic_contract(),
        MAPPER,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// Current Boolean state manifested through the existing stdout Host Call.
pub fn indicator_offer() -> CapabilityOffer {
    let stdout = crate::bool_presentation_offer();
    button_offer(
        conduit_semantic_catalog::indicator_state_presentation_semantic_contract(),
        INDICATOR,
        stdout.host_calls,
        stdout.resource_requirements,
        Vec::new(),
    )
}

/// Advertise only with a currently acquired keyboard input resource/adapter.
pub fn offer() -> CapabilityOffer {
    button_offer(
        conduit_semantic_catalog::button_source_semantic_contract(),
        IMPLEMENTATION,
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(NEXT_TRANSITION_HOST_CALL),
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

fn button_offer(
    contract: Kind,
    implementation: &'static str,
    host_calls: Vec<HostCallRequirement>,
    resource_requirements: Vec<ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls,
            resource_requirements,
            authority_requirements,
        },
    )
    .build()
}
