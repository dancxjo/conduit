//! Portable Boolean presentation meaning.

use super::{KindTerminalBehavior, StandardKindContract};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
    BOOL_INFO_ID,
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
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/bool".to_string(),
    }
}

pub fn bool_presentation_semantic_contract() -> Kind {
    bool_presentation_contract().into_semantic_contract(BOOL_PRESENTATION_CONTRACT_REVISION)
}

#[cfg(feature = "plot-catalog")]
pub fn install_bool_presentation_catalog(
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    profile
        .insert_kind(bool_presentation_semantic_contract())
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
