use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, ResourceRequirement, INPUT_RESOURCE_CLASS, KEY_EVENT_ENCODED_LEN,
    KEY_EVENT_INFO_ID,
};

pub const KEYBOARD_KIND: &str = "input/keyboard";
pub const KEYBOARD_PORT: &str = "key";
pub const KEYBOARD_CONTRACT_REVISION: &str = "conduit.input/keyboard@1";
pub const NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/input-next-key-event@1";
/// Generic installed-kernel source whose platform adapter supplies admitted
/// portable key events through `NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT`.
/// Hosts remain responsible for advertising the concrete adapter artifact and
/// resources; the standard catalog deliberately does not offer this by itself.
pub const HOSTED_KEYBOARD_EXECUTION_PROFILE: &str = "conduit.std/input-keyboard-kernel-hosted@1";
pub const HOSTED_KEYBOARD_IMPLEMENTATION: &str = "std/kernel-input-keyboard-hosted@1";
pub const KEYBOARD_MAX_QUEUE_ITEMS: u16 = 8;
pub const KEYBOARD_MAX_QUEUE_BYTES: u32 =
    KEYBOARD_MAX_QUEUE_ITEMS as u32 * KEY_EVENT_ENCODED_LEN as u32;

pub fn keyboard_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(KEYBOARD_KIND),
        plain_name: "Keyboard".to_string(),
        summary: "Produce a bounded flow of portable key transitions.".to_string(),
        inputs: Vec::new(),
        outputs: keyboard_outputs(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: KEYBOARD_MAX_QUEUE_ITEMS,
            max_queue_bytes: KEYBOARD_MAX_QUEUE_BYTES,
        },
        terminal_behavior: TerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "keyboard: input/keyboard".to_string(),
    }
}

pub fn keyboard_contract_revision() -> KindContractRevision {
    KindContractRevision::from(KEYBOARD_CONTRACT_REVISION)
}

pub fn keyboard_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(KEYBOARD_PORT),
        value_kind: kind_id(KEY_EVENT_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
    }]
}

/// Exact bounded operation needed by a concrete admitted keyboard source.
/// Device disappearance fails this operation; the ordinary kernel then fails
/// or cancels the enclosing Play rather than inventing successful closure.
pub fn next_key_event_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT),
        target_kind: Some(kind_id(KEY_EVENT_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: 0,
        maximum_output_bytes: KEY_EVENT_ENCODED_LEN as u32,
    }
}

pub fn keyboard_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)]
}

/// Builds a host-owned offer for the generic installed keyboard source. The
/// caller names the concrete capability and adapter artifact; no ambient offer
/// is installed merely because the semantic catalog knows the Kind.
pub fn hosted_keyboard_offer(capability: &str, artifact: &str) -> CapabilityOffer {
    let contract = keyboard_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: keyboard_contract_revision(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(HOSTED_KEYBOARD_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(HOSTED_KEYBOARD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![next_key_event_host_operation_requirement()],
        resource_requirements: keyboard_resource_requirements(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_keyboard_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    let contract = keyboard_contract();
    startup.insert(KindSignature {
        kind: KEYBOARD_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: keyboard_contract_revision(),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_is_a_finite_typed_closing_source() {
        let contract = keyboard_contract();
        assert!(contract.inputs.is_empty());
        assert_eq!(contract.outputs, keyboard_outputs());
        assert_eq!(contract.outputs[0].value_kind.as_str(), KEY_EVENT_INFO_ID);
        assert_eq!(
            contract.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(contract.limits.max_queue_items, 8);
        assert_eq!(contract.limits.max_queue_bytes, 24);
        let operation = next_key_event_host_operation_requirement();
        assert_eq!(operation.maximum_in_flight, 1);
        assert_eq!(operation.maximum_input_bytes, 0);
        assert_eq!(operation.maximum_output_bytes, 3);
        assert_eq!(keyboard_resource_requirements().len(), 1);
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn keyboard_catalog_has_exact_semantic_face_without_an_implementation_offer() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
        let definition = profile.get(&kind_id(KEYBOARD_KIND)).unwrap();
        assert_eq!(definition.outputs, keyboard_outputs());
        assert!(crate::supported_nucleus_offers()
            .iter()
            .all(|offer| offer.kind_id.as_str() != KEYBOARD_KIND));
    }
}
