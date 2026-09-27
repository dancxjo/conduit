//! Portable collection of pressed-button instants into one finite timed attempt.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, FrontStartupParameter, Kind,
    KindIdentity, PortDescriptor, PortDirection, PortTemporal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
};

pub const TIMED_BUTTON_ATTEMPT_KIND: &str = "time/pressed-button-attempt";
pub const TIMED_BUTTON_ATTEMPT_REVISION: &str = "conduit.time/pressed-button-attempt@2";
pub const DEFAULT_ATTEMPT_PRESSES: u64 = 4;
pub const DEFAULT_ATTEMPT_TRANSITIONS: u64 = 16;
pub const MAXIMUM_ATTEMPT_TRANSITIONS: u64 = 32;
pub const DEFAULT_ATTEMPT_TIMEOUT_MS: u64 = 3_000;
pub const MAXIMUM_ATTEMPT_TIMEOUT_MS: u64 = 60_000;

pub fn timed_button_attempt_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(TIMED_BUTTON_ATTEMPT_KIND),
        kind_contract_revision: KindIdentity::from(TIMED_BUTTON_ATTEMPT_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("transition"),
            value_kind: crate::input_button_transition_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
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
            KindConfigurationField {
                key: "maximum-presses".into(),
                default_value: ConfigurationValue::U64(DEFAULT_ATTEMPT_PRESSES),
                rule: KindConfigurationRule::U64Range {
                    minimum: 2,
                    maximum: crate::MAXIMUM_TIMED_EVENTS as u64,
                },
            },
            KindConfigurationField {
                key: "maximum-transitions".into(),
                default_value: ConfigurationValue::U64(DEFAULT_ATTEMPT_TRANSITIONS),
                rule: KindConfigurationRule::U64Range {
                    minimum: 2,
                    maximum: MAXIMUM_ATTEMPT_TRANSITIONS,
                },
            },
            KindConfigurationField {
                key: "timeout-ms".into(),
                default_value: ConfigurationValue::U64(DEFAULT_ATTEMPT_TIMEOUT_MS),
                rule: KindConfigurationRule::DurationMillis {
                    minimum: 1,
                    maximum: MAXIMUM_ATTEMPT_TIMEOUT_MS,
                },
            },
        ],
    }
}

pub fn timed_button_attempt_semantic_contract() -> Kind {
    let definition = timed_button_attempt_definition();
    Kind {
        startup_parameters: vec![
            FrontStartupParameter {
                name: "maximum-transitions".into(),
                value_type: kind_id("value/count"),
                has_default: true,
            },
            FrontStartupParameter {
                name: "maximum-presses".into(),
                value_type: kind_id("value/count"),
                has_default: true,
            },
            FrontStartupParameter {
                name: "timeout-ms".into(),
                value_type: kind_id(conduit_core::DURATION_INFO_ID),
                has_default: true,
            },
        ],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: crate::MAXIMUM_TIMED_EVENTS as u16,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                * (crate::MAXIMUM_TIMED_EVENTS as u32 + 1),
        },
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
                    name: "maximum-transitions".into(),
                    value_type: "Count".into(),
                    default: Some(DEFAULT_ATTEMPT_TRANSITIONS.to_string()),
                },
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
            PortTemporal::Flow { closes: false }
        );
        assert_eq!(definition.outputs[0].temporal, PortTemporal::Value);
        assert_eq!(definition.configuration.len(), 3);
        let debug = alloc::format!("{definition:?}");
        for forbidden in ["browser", "dom", "gpio", "socket", "address"] {
            assert!(!debug.contains(forbidden));
        }
    }

    #[test]
    fn semantic_contract_owns_exact_startup_front_and_capacity() {
        let contract = timed_button_attempt_semantic_contract();
        assert_eq!(contract.startup_parameters.len(), 3);
        assert_eq!(contract.startup_parameters[0].name, "maximum-transitions");
        assert_eq!(contract.startup_parameters[1].name, "maximum-presses");
        assert_eq!(
            contract.startup_parameters[2].value_type.as_str(),
            conduit_core::DURATION_INFO_ID
        );
        assert!(contract
            .startup_parameters
            .iter()
            .all(|parameter| parameter.has_default));
        assert_eq!(contract.limits.max_active_instances, 8);
        assert_eq!(
            contract.limits.max_queue_items,
            crate::MAXIMUM_TIMED_EVENTS as u16
        );
    }
}
