use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    TICK_VALUE_KIND,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement,
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, ImplementationId, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, BOOL_INFO_ID,
};

pub const TIME_DEBOUNCE_KIND: &str = "time/debounce";
pub const TIME_DEBOUNCE_CONTRACT_REVISION: &str = "conduit.std/time-debounce-bool@1";
pub const TIME_DEBOUNCE_EXECUTION_PROFILE: &str = "conduit.std/time-debounce-bool-kernel-hosted@1";
pub const TIME_DEBOUNCE_IMPLEMENTATION: &str = "std/kernel-time-debounce-bool@1";
pub const TIME_DEBOUNCE_ARTIFACT: &str = "conduit-std-host/time-debounce-bool@1";

pub const TIME_TIMEOUT_KIND: &str = "time/timeout";
pub const TIME_TIMEOUT_CONTRACT_REVISION: &str = "conduit.std/time-timeout-tick-bool@1";
pub const TIME_TIMEOUT_EXECUTION_PROFILE: &str =
    "conduit.std/time-timeout-tick-bool-kernel-hosted@1";
pub const TIME_TIMEOUT_IMPLEMENTATION: &str = "std/kernel-time-timeout-tick-bool@1";
pub const TIME_TIMEOUT_ARTIFACT: &str = "conduit-std-host/time-timeout-tick-bool@1";

pub const TIME_DELAY_KIND: &str = "time/delay";
pub const TIME_DELAY_CONTRACT_REVISION: &str = "conduit.std/time-delay-bool@1";
pub const TIME_DELAY_EXECUTION_PROFILE: &str = "conduit.std/time-delay-bool-kernel-hosted@1";
pub const TIME_DELAY_IMPLEMENTATION: &str = "std/kernel-time-delay-bool@1";
pub const TIME_DELAY_ARTIFACT: &str = "conduit-std-host/time-delay-bool@1";

pub const TIME_THROTTLE_KIND: &str = "time/throttle";
pub const TIME_THROTTLE_CONTRACT_REVISION: &str = "conduit.std/time-throttle-bool-leading@1";
pub const TIME_THROTTLE_EXECUTION_PROFILE: &str =
    "conduit.std/time-throttle-bool-leading-kernel-hosted@1";
pub const TIME_THROTTLE_IMPLEMENTATION: &str = "std/kernel-time-throttle-bool-leading@1";
pub const TIME_THROTTLE_ARTIFACT: &str = "conduit-std-host/time-throttle-bool-leading@1";

pub const TIME_POLICY_TRAILING: &str = "trailing";
pub const TIME_POLICY_LEADING: &str = "leading";
pub const TIME_MAXIMUM_DURATION_MS: u64 = 86_400_000;
pub const TIME_MAXIMUM_VALUES: u64 = 8;
pub const TIME_TIMEOUT_MAXIMUM_VALUES: u64 = 2;

pub fn time_debounce_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_DEBOUNCE_KIND),
        plain_name: "Trailing Boolean debounce".to_string(),
        summary: "Emit the last exact Boolean after one stable admitted duration; flush it on input closure."
            .to_string(),
        inputs: vec![port(
            "in",
            BOOL_INFO_ID,
            PortDirection::Input,
            PortTemporal::Current,
        )],
        outputs: vec![port(
            "out",
            BOOL_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: vec![
            duration_field(),
            StandardConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_TRAILING.to_string()),
                rule: StandardConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_TRAILING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "stable: time/debounce(duration-ms = 50, policy = \"trailing\")".to_string(),
    }
}

pub fn time_timeout_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_TIMEOUT_KIND),
        plain_name: "Tick inactivity timeout".to_string(),
        summary: "Emit false initially, true once after inactivity, and false again when exact tick activity recovers."
            .to_string(),
        inputs: vec![port(
            "activity",
            TICK_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "timed-out",
            BOOL_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: vec![
            duration_field(),
            maximum_values_field(TIME_TIMEOUT_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior: TerminalBehavior::InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "stale: time/timeout(duration-ms = 500)".to_string(),
    }
}

pub fn time_delay_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_DELAY_KIND),
        plain_name: "Ordered Boolean delay".to_string(),
        summary: "Emit every admitted Boolean in input order after one exact duration; drain admitted values on input closure.".to_string(),
        inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input, PortTemporal::Current)],
        outputs: vec![port("out", BOOL_INFO_ID, PortDirection::Output, PortTemporal::Current)],
        configuration: vec![duration_field(), maximum_values_field(TIME_MAXIMUM_VALUES)],
        limits: limits(),
        terminal_behavior: TerminalBehavior::DelaysEachValueInOrderAndDrainsOnInputClosure,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "paced: time/delay(duration-ms = 16ms, maximum-values = 8)".to_string(),
    }
}

pub fn time_throttle_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_THROTTLE_KIND),
        plain_name: "Leading Boolean throttle".to_string(),
        summary: "Emit the first admitted Boolean immediately, then drop values until the exact interval elapses.".to_string(),
        inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input, PortTemporal::Current)],
        outputs: vec![port("out", BOOL_INFO_ID, PortDirection::Output, PortTemporal::Current)],
        configuration: vec![
            duration_field(),
            StandardConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_LEADING.to_string()),
                rule: StandardConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_LEADING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            TerminalBehavior::LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "paced: time/throttle(duration-ms = 16ms, policy = \"leading\", maximum-values = 8)".to_string(),
    }
}

pub fn time_debounce_offer() -> CapabilityOffer {
    offer(
        time_debounce_contract(),
        "time-debounce-bool-v1",
        TIME_DEBOUNCE_CONTRACT_REVISION,
        TIME_DEBOUNCE_EXECUTION_PROFILE,
        TIME_DEBOUNCE_IMPLEMENTATION,
        TIME_DEBOUNCE_ARTIFACT,
    )
}

pub fn time_timeout_offer() -> CapabilityOffer {
    offer(
        time_timeout_contract(),
        "time-timeout-tick-bool-v1",
        TIME_TIMEOUT_CONTRACT_REVISION,
        TIME_TIMEOUT_EXECUTION_PROFILE,
        TIME_TIMEOUT_IMPLEMENTATION,
        TIME_TIMEOUT_ARTIFACT,
    )
}

pub fn time_delay_offer() -> CapabilityOffer {
    offer(
        time_delay_contract(),
        "time-delay-bool-v1",
        TIME_DELAY_CONTRACT_REVISION,
        TIME_DELAY_EXECUTION_PROFILE,
        TIME_DELAY_IMPLEMENTATION,
        TIME_DELAY_ARTIFACT,
    )
}

pub fn time_throttle_offer() -> CapabilityOffer {
    offer(
        time_throttle_contract(),
        "time-throttle-bool-leading-v1",
        TIME_THROTTLE_CONTRACT_REVISION,
        TIME_THROTTLE_EXECUTION_PROFILE,
        TIME_THROTTLE_IMPLEMENTATION,
        TIME_THROTTLE_ARTIFACT,
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_timing_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    for contract in [
        time_debounce_contract(),
        time_timeout_contract(),
        time_delay_contract(),
        time_throttle_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.key.as_str() {
                        "duration-ms" => "Duration",
                        "policy" => "Text",
                        _ => "Count",
                    }
                    .to_string(),
                    default: Some(configuration_source(field)),
                })
                .collect(),
        })?;
        let revision = match contract.kind_id.as_str() {
            TIME_DEBOUNCE_KIND => TIME_DEBOUNCE_CONTRACT_REVISION,
            TIME_TIMEOUT_KIND => TIME_TIMEOUT_CONTRACT_REVISION,
            TIME_DELAY_KIND => TIME_DELAY_CONTRACT_REVISION,
            TIME_THROTTLE_KIND => TIME_THROTTLE_CONTRACT_REVISION,
            _ => unreachable!("timing catalog loop contains only timing contracts"),
        };
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| {
                let validation = match field.rule {
                    StandardConfigurationRule::DurationMillis { minimum, maximum } => {
                        ConfigurationRule::DurationMillis { minimum, maximum }
                    }
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range { minimum, maximum }
                    }
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf { values }
                    }
                    _ => return Err("unsupported timing configuration rule".to_string()),
                };
                Ok(ConfigurationField {
                    key: field.key,
                    default_value: field.default_value,
                    validation,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn offer(
    contract: StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.key.as_str() {
                    "duration-ms" => "Duration",
                    "policy" => "Text",
                    _ => "Count",
                }
                .to_string(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![monotonic_timer_host_operation_requirement()],
        resource_requirements: vec![monotonic_timer_resource_requirement()],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn port(
    name: &str,
    info: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal,
    }
}

fn duration_field() -> StandardConfigurationField {
    StandardConfigurationField {
        key: "duration-ms".to_string(),
        default_value: ConfigurationValue::U64(100),
        rule: StandardConfigurationRule::DurationMillis {
            minimum: 0,
            maximum: TIME_MAXIMUM_DURATION_MS,
        },
    }
}

fn maximum_values_field(maximum: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: "maximum-values".to_string(),
        default_value: ConfigurationValue::U64(maximum),
        rule: StandardConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: 8,
    }
}

#[cfg(feature = "form-catalog")]
fn configuration_source(field: &StandardConfigurationField) -> alloc::string::String {
    match (&*field.key, &field.default_value) {
        ("duration-ms", ConfigurationValue::U64(value)) => alloc::format!("{value}ms"),
        (_, ConfigurationValue::U64(value)) => value.to_string(),
        (_, ConfigurationValue::Text(value)) => alloc::format!("\"{value}\""),
        _ => unreachable!("timing contracts use only bounded unsigned and text values"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_contracts_are_typed_bounded_and_share_one_exact_deadline_requirement() {
        for contract in [
            time_debounce_contract(),
            time_timeout_contract(),
            time_delay_contract(),
            time_throttle_contract(),
        ] {
            assert!(contract
                .inputs
                .iter()
                .chain(&contract.outputs)
                .all(|port| port.value_kind.as_str() != super::super::GENERIC_VALUE_KIND));
            assert_eq!(contract.limits.max_queue_items, 1);
        }
        for offer in [
            time_debounce_offer(),
            time_timeout_offer(),
            time_delay_offer(),
            time_throttle_offer(),
        ] {
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(
                offer.host_operations[0],
                monotonic_timer_host_operation_requirement()
            );
            assert_eq!(offer.resource_requirements.len(), 1);
            assert_eq!(
                offer.resource_requirements[0],
                monotonic_timer_resource_requirement()
            );
        }
        for contract in [
            time_debounce_contract(),
            time_timeout_contract(),
            time_delay_contract(),
            time_throttle_contract(),
        ] {
            assert!(matches!(
                contract.configuration[0].rule,
                StandardConfigurationRule::DurationMillis {
                    minimum: 0,
                    maximum: TIME_MAXIMUM_DURATION_MS
                }
            ));
        }
    }
}
