//! Checked configuration and Kind boundary for bounded replay control.

use conduit_core::{ConfigurationEntry, ConfigurationValue};

use crate::{
    ReplayPolicy, MAXIMUM_REPLAY_DURATION_SECONDS, MAXIMUM_REPLAY_RATE_TERM,
    REPLAY_MODE_ORIGINAL_TIMING, REPLAY_MODE_RATE, REPLAY_MODE_STEP,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayPolicyConfigurationRefusal {
    Missing(&'static str),
    WrongType(&'static str),
    UnknownMode,
    RateOutOfBounds,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayDurationConfigurationRefusal {
    Missing,
    WrongType,
    OutOfBounds,
}

pub fn replay_maximum_duration_from_configuration(
    entries: &[ConfigurationEntry],
) -> Result<u64, ReplayDurationConfigurationRefusal> {
    let value = entries
        .iter()
        .find(|entry| entry.key == "maximum-duration-seconds")
        .ok_or(ReplayDurationConfigurationRefusal::Missing)?;
    let ConfigurationValue::U64(seconds) = &value.value else {
        return Err(ReplayDurationConfigurationRefusal::WrongType);
    };
    if *seconds == 0 || *seconds > MAXIMUM_REPLAY_DURATION_SECONDS {
        return Err(ReplayDurationConfigurationRefusal::OutOfBounds);
    }
    Ok(*seconds)
}

pub fn replay_policy_from_configuration(
    entries: &[ConfigurationEntry],
) -> Result<ReplayPolicy, ReplayPolicyConfigurationRefusal> {
    let mut mode = None;
    let mut numerator = None;
    let mut denominator = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("mode", ConfigurationValue::Text(value)) => mode = Some(value.as_str()),
            ("rate-numerator", ConfigurationValue::U64(value)) => numerator = Some(*value),
            ("rate-denominator", ConfigurationValue::U64(value)) => denominator = Some(*value),
            ("mode", _) => return Err(ReplayPolicyConfigurationRefusal::WrongType("mode")),
            ("rate-numerator", _) => {
                return Err(ReplayPolicyConfigurationRefusal::WrongType(
                    "rate-numerator",
                ));
            }
            ("rate-denominator", _) => {
                return Err(ReplayPolicyConfigurationRefusal::WrongType(
                    "rate-denominator",
                ));
            }
            _ => {}
        }
    }
    let mode = mode.ok_or(ReplayPolicyConfigurationRefusal::Missing("mode"))?;
    let numerator = numerator.ok_or(ReplayPolicyConfigurationRefusal::Missing("rate-numerator"))?;
    let denominator = denominator.ok_or(ReplayPolicyConfigurationRefusal::Missing(
        "rate-denominator",
    ))?;
    if numerator == 0
        || denominator == 0
        || numerator > u64::from(MAXIMUM_REPLAY_RATE_TERM)
        || denominator > u64::from(MAXIMUM_REPLAY_RATE_TERM)
    {
        return Err(ReplayPolicyConfigurationRefusal::RateOutOfBounds);
    }
    match mode {
        REPLAY_MODE_STEP => Ok(ReplayPolicy::Step),
        REPLAY_MODE_ORIGINAL_TIMING => Ok(ReplayPolicy::OriginalTiming),
        REPLAY_MODE_RATE => Ok(ReplayPolicy::Rate {
            numerator: numerator as u32,
            denominator: denominator as u32,
        }),
        _ => Err(ReplayPolicyConfigurationRefusal::UnknownMode),
    }
}

#[cfg(feature = "form-catalog")]
pub fn replay_control_kind_definition() -> conduit_form::KindDefinition {
    use alloc::string::ToString;
    use conduit_core::{
        kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
        StructuredInfoType,
    };
    use conduit_form::{ConfigurationField, ConfigurationRule};
    let port = |name, value_kind, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: StructuredInfoType::leaf(kind_id(value_kind))
            .expect("reviewed replay value identity")
            .profile()
            .expect("reviewed replay value profile")
            .value_kind()
            .clone(),
        direction,
        temporal,
    };
    conduit_form::KindDefinition {
        kind_id: kind_id(crate::REPLAY_CONTROL_KIND),
        kind_contract_revision: KindContractRevision::from(crate::REPLAY_CONTROL_CONTRACT_REVISION),
        inputs: alloc::vec![
            port(
                "timeline",
                "history/replay-timeline@1",
                PortDirection::Input,
                PortTemporal::Value
            ),
            port(
                "control",
                "history/replay-control@1",
                PortDirection::Input,
                PortTemporal::Flow { closes: true }
            ),
            port(
                "clock",
                "time/playback-tick@1",
                PortDirection::Input,
                PortTemporal::Flow { closes: true }
            ),
        ],
        outputs: alloc::vec![
            port(
                "event",
                "history/replay-event@1",
                PortDirection::Output,
                PortTemporal::Flow { closes: true }
            ),
            port(
                "state",
                "history/replay-state@1",
                PortDirection::Output,
                PortTemporal::Flow { closes: true }
            ),
        ],
        configuration: alloc::vec![
            ConfigurationField {
                key: "mode".to_string(),
                default_value: ConfigurationValue::Text(REPLAY_MODE_ORIGINAL_TIMING.to_string()),
                validation: ConfigurationRule::TextOneOf {
                    values: alloc::vec![
                        REPLAY_MODE_STEP.to_string(),
                        REPLAY_MODE_ORIGINAL_TIMING.to_string(),
                        REPLAY_MODE_RATE.to_string()
                    ],
                },
            },
            ConfigurationField {
                key: "rate-numerator".to_string(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u64::from(MAXIMUM_REPLAY_RATE_TERM)
                },
            },
            ConfigurationField {
                key: "rate-denominator".to_string(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u64::from(MAXIMUM_REPLAY_RATE_TERM)
                },
            },
            ConfigurationField {
                key: "maximum-duration-seconds".to_string(),
                default_value: ConfigurationValue::U64(MAXIMUM_REPLAY_DURATION_SECONDS),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_REPLAY_DURATION_SECONDS
                },
            },
        ],
    }
}
