//! Portable collection of pressed-button instants into one finite timed attempt.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const TIMED_BUTTON_ATTEMPT_KIND: &str = "time/pressed-button-attempt";
pub const TIMED_BUTTON_ATTEMPT_REVISION: &str = "conduit.time/pressed-button-attempt@1";
pub const DEFAULT_ATTEMPT_PRESSES: u64 = 4;
pub const DEFAULT_ATTEMPT_TIMEOUT_MS: u64 = 3_000;
pub const MAXIMUM_ATTEMPT_TIMEOUT_MS: u64 = 60_000;

pub fn timed_button_attempt_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TIMED_BUTTON_ATTEMPT_KIND),
        kind_contract_revision: KindContractRevision::from(TIMED_BUTTON_ATTEMPT_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("transition"),
            value_kind: crate::input_button_transition_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("events"),
            value_kind: crate::timed_event_sequence_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![
            ConfigurationField {
                key: "maximum-presses".into(),
                default_value: ConfigurationValue::U64(DEFAULT_ATTEMPT_PRESSES),
                validation: ConfigurationRule::U64Range {
                    minimum: 2,
                    maximum: crate::MAXIMUM_TIMED_EVENTS as u64,
                },
            },
            ConfigurationField {
                key: "timeout-ms".into(),
                default_value: ConfigurationValue::U64(DEFAULT_ATTEMPT_TIMEOUT_MS),
                validation: ConfigurationRule::DurationMillis {
                    minimum: 1,
                    maximum: MAXIMUM_ATTEMPT_TIMEOUT_MS,
                },
            },
        ],
    }
}

pub fn install_timed_button_attempt_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: TIMED_BUTTON_ATTEMPT_KIND.into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "maximum-presses".into(),
                    value_type: "Count".into(),
                    default: Some(DEFAULT_ATTEMPT_PRESSES.to_string()),
                },
                StartupParameterSignature {
                    name: "timeout-ms".into(),
                    value_type: "Duration".into(),
                    default: Some(alloc::format!("{}ms", DEFAULT_ATTEMPT_TIMEOUT_MS)),
                },
            ],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(timed_button_attempt_definition())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_collects_portable_pressed_transitions_under_explicit_bounds() {
        let definition = timed_button_attempt_definition();
        assert_eq!(
            definition.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(definition.outputs[0].temporal, PortTemporal::Value);
        assert_eq!(definition.configuration.len(), 2);
        let debug = alloc::format!("{definition:?}");
        for forbidden in ["browser", "dom", "gpio", "socket", "address"] {
            assert!(!debug.contains(forbidden));
        }
    }
}
