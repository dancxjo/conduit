//! Portable pulse-observation presentation face.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
};

use super::{StandardKindContract, TerminalBehavior};

pub const PULSE_PRESENTATION_KIND: &str = "presentation/pulse";
pub const PULSE_PRESENTATION_CONTRACT_REVISION: &str = "conduit.presentation/pulse-observation@2";
pub const PULSE_TONE_PRESENTATION_KIND: &str = "sound/pulse-tone";
pub const PULSE_TONE_PRESENTATION_CONTRACT_REVISION: &str = "conduit.sound/pulse-tone@1";

pub fn pulse_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(PULSE_PRESENTATION_KIND),
        plain_name: "Pulse presentation".to_string(),
        summary: "Manifest each exact pulse observation while its Play remains alive.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("pulse"),
            value_kind: kind_id(conduit_time::PULSE_OBSERVATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: false },
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "light: presentation/pulse".to_string(),
    }
}

pub fn pulse_tone_presentation_contract() -> StandardKindContract {
    let mut contract = pulse_presentation_contract();
    contract.kind_id = kind_id(PULSE_TONE_PRESENTATION_KIND);
    contract.plain_name = "Pulse tone".to_string();
    contract.summary = "Manifest exact pulse observations as admitted tone events.".to_string();
    contract.browser_manifestation_honest = false;
    contract.example = "tone: sound/pulse-tone".to_string();
    contract
}

#[cfg(feature = "form-catalog")]
pub fn pulse_presentation_kind_definition() -> conduit_form::KindDefinition {
    use conduit_form::KindDefinition;
    let contract = pulse_presentation_contract();
    KindDefinition {
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(PULSE_PRESENTATION_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: vec![],
    }
}

#[cfg(feature = "form-catalog")]
pub fn pulse_tone_presentation_kind_definition() -> conduit_form::KindDefinition {
    let mut definition = pulse_presentation_kind_definition();
    definition.kind_id = kind_id(PULSE_TONE_PRESENTATION_KIND);
    definition.kind_contract_revision =
        KindContractRevision::from(PULSE_TONE_PRESENTATION_CONTRACT_REVISION);
    definition
}

#[cfg(feature = "form-catalog")]
pub fn install_pulse_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::KindSignature;
    for (kind, definition) in [
        (
            PULSE_PRESENTATION_KIND,
            pulse_presentation_kind_definition(),
        ),
        (
            PULSE_TONE_PRESENTATION_KIND,
            pulse_tone_presentation_kind_definition(),
        ),
    ] {
        startup.insert(KindSignature {
            kind: kind.to_string(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(definition)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_tone_are_distinct_finite_presentation_contracts() {
        let light = pulse_presentation_contract();
        let tone = pulse_tone_presentation_contract();
        assert_ne!(light.kind_id, tone.kind_id);
        assert_eq!(light.inputs, tone.inputs);
        assert_eq!(light.limits, tone.limits);
        assert!(light.browser_manifestation_honest);
        assert!(!tone.browser_manifestation_honest);
        assert_eq!(tone.kind_id.as_str(), PULSE_TONE_PRESENTATION_KIND);
    }
}
