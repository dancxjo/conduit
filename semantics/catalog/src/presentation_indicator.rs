//! Portable manifestation of one finite Morse indicator pattern.

use super::{StandardKindContract, TerminalBehavior};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const INDICATOR_PRESENTATION_KIND: &str = "presentation/indicator";
pub const INDICATOR_PRESENTATION_CONTRACT_REVISION: &str =
    "conduit.presentation/indicator@1";

pub fn indicator_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(INDICATOR_PRESENTATION_KIND),
        plain_name: "Indicator pattern".to_string(),
        summary: "Manifest one finite timed indication pattern through an admitted presenter effect."
            .to_string(),
        inputs: indicator_presentation_inputs(),
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "light: presentation/indicator".to_string(),
    }
}

pub fn indicator_presentation_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("pattern"),
        value_kind: kind_id(conduit_text::MORSE_PATTERN_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
    }]
}

#[cfg(feature = "form-catalog")]
pub fn install_indicator_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};
    startup.insert(KindSignature {
        kind: INDICATOR_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(INDICATOR_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(
                INDICATOR_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: indicator_presentation_inputs(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_consumes_the_canonical_morse_pattern_without_platform_facts() {
        let contract = indicator_presentation_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_text::MORSE_PATTERN_VALUE_KIND
        );
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Value);
        assert!(contract.outputs.is_empty());
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
    }
}
