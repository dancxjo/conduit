//! An explicitly selected Space-key realization of ordinary button meaning.
use conduit_core::{
    resource_requirement, ArtifactId, AuthorityRequirement, CapabilityId, CapabilityOffer,
    CapabilityOfferBuilder, CapabilityRealization, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ResourceRequirement, SemanticCapabilityContract,
    INPUT_RESOURCE_CLASS,
};

pub const IMPLEMENTATION: &str = "std/kernel-space-button@1";
pub const ARTIFACT: &str = "conduit-std-host/space-button@1";
pub const MAPPER: &str = "std/kernel-button-indicator-state@1";
pub const INDICATOR: &str = "std/stdout-indicator-state@1";
pub const NEXT_TRANSITION_HOST_OPERATION: &str = "conduit.host/input-next-button-transition@1";

pub fn mapper_offer() -> CapabilityOffer {
    button_offer(
        conduit_semantic_catalog::button_indicator_state_semantic_contract(),
        MAPPER,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// Current Boolean state manifested through the existing stdout host operation.
pub fn indicator_offer() -> CapabilityOffer {
    let stdout = crate::bool_presentation_offer();
    button_offer(
        conduit_semantic_catalog::indicator_state_presentation_semantic_contract(),
        INDICATOR,
        stdout.host_operations,
        stdout.resource_requirements,
        Vec::new(),
    )
}

/// Advertise only with a currently acquired keyboard input resource/adapter.
pub fn offer() -> CapabilityOffer {
    button_offer(
        conduit_semantic_catalog::button_source_semantic_contract(),
        IMPLEMENTATION,
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

fn button_offer(
    contract: SemanticCapabilityContract,
    implementation: &'static str,
    host_operations: Vec<HostOperationRequirement>,
    resource_requirements: Vec<ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
) -> CapabilityOffer {
    CapabilityOfferBuilder::new(
        contract,
        CapabilityRealization {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_operations,
            resource_requirements,
            authority_requirements,
        },
    )
    .build()
}
