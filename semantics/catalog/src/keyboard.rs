use super::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, KEY_EVENT_ENCODED_LEN, KEY_EVENT_INFO_ID,
};

pub const KEYBOARD_KIND: &str = "input/keyboard";
pub const KEYBOARD_PORT: &str = "key";
pub const KEYBOARD_CONTRACT_REVISION: &str = "conduit.input/keyboard@1";
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
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn keyboard_catalog_has_exact_semantic_face_without_an_implementation_offer() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
        let definition = profile.get(&kind_id(KEYBOARD_KIND)).unwrap();
        assert_eq!(definition.outputs, keyboard_outputs());
        assert!(crate::supported_nucleus_contracts()
            .iter()
            .all(|contract| contract.kind_id.as_str() != KEYBOARD_KIND));
    }
}
