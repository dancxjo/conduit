use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, Kind, PortDescriptor, PortDirection,
    PortTemporal, BOOL_INFO_ID,
};

pub const TIME_DEBOUNCE_KIND: &str = "time/debounce";
pub const TIME_DEBOUNCE_CONTRACT_REVISION: &str = "conduit.std/time-debounce-bool@1";

pub const TIME_TIMEOUT_KIND: &str = "time/timeout";
pub const TIME_TIMEOUT_CONTRACT_REVISION: &str = "conduit.std/time-timeout-tick-bool@1";

pub const TIME_DELAY_KIND: &str = "time/delay";
pub const TIME_DELAY_CONTRACT_REVISION: &str = "conduit.std/time-delay-bool@1";

pub const TIME_THROTTLE_KIND: &str = "time/throttle";
pub const TIME_THROTTLE_CONTRACT_REVISION: &str = "conduit.std/time-throttle-bool-leading@1";

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
            KindConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_TRAILING.to_string()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_TRAILING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
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
            conduit_time::TICK_VALUE_KIND,
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
        terminal_behavior: KindTerminalBehavior::InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
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
        terminal_behavior: KindTerminalBehavior::DelaysEachValueInOrderAndDrainsOnInputClosure,
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
            KindConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_LEADING.to_string()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_LEADING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "paced: time/throttle(duration-ms = 16ms, policy = \"leading\", maximum-values = 8)".to_string(),
    }
}

pub fn time_debounce_semantic_contract() -> Kind {
    semantic_contract(time_debounce_contract(), TIME_DEBOUNCE_CONTRACT_REVISION)
}

pub fn time_timeout_semantic_contract() -> Kind {
    semantic_contract(time_timeout_contract(), TIME_TIMEOUT_CONTRACT_REVISION)
}

pub fn time_delay_semantic_contract() -> Kind {
    semantic_contract(time_delay_contract(), TIME_DELAY_CONTRACT_REVISION)
}

pub fn time_throttle_semantic_contract() -> Kind {
    semantic_contract(time_throttle_contract(), TIME_THROTTLE_CONTRACT_REVISION)
}

fn semantic_contract(contract: StandardKindContract, revision: &str) -> Kind {
    let startup_parameters = super::startup_front(&contract.configuration);
    Kind {
        startup_parameters,
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: revision.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_timing_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindSignature, StartupParameterSignature};

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
        profile
            .insert_kind(semantic_contract(contract, revision))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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

fn duration_field() -> KindConfigurationField {
    KindConfigurationField {
        key: "duration-ms".to_string(),
        default_value: ConfigurationValue::Quantity(conduit_core::Quantity::new(
            100,
            conduit_core::QuantityUnit::Millisecond,
        )),
        rule: KindConfigurationRule::QuantityRange {
            minimum: 0,
            maximum: TIME_MAXIMUM_DURATION_MS as i64,
            canonical_unit: conduit_core::QuantityUnit::Millisecond,
        },
    }
}

fn maximum_values_field(maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: "maximum-values".to_string(),
        default_value: ConfigurationValue::U64(maximum),
        rule: KindConfigurationRule::U64Range {
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
fn configuration_source(field: &KindConfigurationField) -> alloc::string::String {
    match (&*field.key, &field.default_value) {
        (_, ConfigurationValue::Quantity(value)) => {
            alloc::format!("{}{}", value.value(), value.unit().form_suffix())
        }
        (_, ConfigurationValue::U64(value)) => value.to_string(),
        (_, ConfigurationValue::Text(value)) => alloc::format!("\"{value}\""),
        _ => unreachable!("timing contracts use only bounded Quantity, Count, and Text values"),
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
            assert_eq!(contract.limits.max_queue_items, 1);
        }
        for contract in [
            time_debounce_contract(),
            time_timeout_contract(),
            time_delay_contract(),
            time_throttle_contract(),
        ] {
            assert!(matches!(
                contract.configuration[0].rule,
                KindConfigurationRule::QuantityRange {
                    minimum: 0,
                    maximum,
                    canonical_unit: conduit_core::QuantityUnit::Millisecond,
                } if maximum == TIME_MAXIMUM_DURATION_MS as i64
            ));
        }
    }
}
