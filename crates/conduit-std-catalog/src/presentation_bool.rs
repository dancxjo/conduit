//! Portable Boolean presentation meaning.

use super::{StandardKindContract, TerminalBehavior};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, BOOL_INFO_ID,
};

pub const BOOL_PRESENTATION_KIND: &str = "presentation/bool";
pub const BOOL_PRESENTATION_CONTRACT_REVISION: &str = "conduit.presentation/bool@1";

pub fn bool_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(BOOL_PRESENTATION_KIND),
        plain_name: "Present current Boolean".to_string(),
        summary: "Manifest each current Boolean through an admitted presenter effect.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/bool".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_bool_presentation_catalog(
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, KindDefinition};
    let contract = bool_presentation_contract();
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(BOOL_PRESENTATION_CONTRACT_REVISION),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::<ConfigurationField>::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_presenter_is_exact_current_boolean_to_admitted_effect() {
        let contract = bool_presentation_contract();
        assert_eq!(contract.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Current);
        assert!(contract.outputs.is_empty());
    }
}
