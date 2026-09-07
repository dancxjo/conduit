//! Portable pulse-observation presentation face.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection,
};

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};

pub const PULSE_PRESENTATION_KIND: &str = "presentation/pulse";
pub const PULSE_PRESENTATION_CONTRACT_REVISION: &str = "conduit.presentation/pulse-observation@1";

pub fn pulse_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(PULSE_PRESENTATION_KIND),
        plain_name: "Pulse presentation".to_string(),
        summary: "Manifest a finite flow of exact pulse observations.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("pulse"),
            value_kind: kind_id(conduit_time::PULSE_OBSERVATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: true },
        }],
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(64),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: conduit_time::MAXIMUM_OBSERVED_PULSES.into(),
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: conduit_time::MAXIMUM_OBSERVED_PULSES,
            max_queue_bytes: conduit_time::PULSE_OBSERVATION_ENCODED_LEN as u32
                * conduit_time::MAXIMUM_OBSERVED_PULSES as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/pulse(4)".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn pulse_presentation_kind_definition() -> conduit_form::KindDefinition {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition};
    let contract = pulse_presentation_contract();
    KindDefinition {
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(PULSE_PRESENTATION_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: vec![ConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(64),
            validation: ConfigurationRule::U64Range {
                minimum: 1,
                maximum: conduit_time::MAXIMUM_OBSERVED_PULSES.into(),
            },
        }],
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_pulse_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindSignature, StartupParameterSignature};
    startup.insert(KindSignature {
        kind: PULSE_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some("64".to_string()),
        }],
    })?;
    profile
        .insert(pulse_presentation_kind_definition())
        .map_err(|error| error.to_string())
}
