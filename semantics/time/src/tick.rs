use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationEntry, ConfigurationValue,
    FrontStartupParameter, Kind, KindConfigurationField, KindConfigurationRule, KindId,
    KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection, PortTemporal,
};

pub const TICK_KIND: &str = "time/tick";
pub const TIME_EVERY_KIND: &str = "time/every";
pub const TICK_PORT: &str = "tick";
pub const TICK_VALUE_KIND: &str = "value/tick@1";
pub const TICK_ENCODED_LEN: u32 = 8;
pub const TICK_CONTRACT_REVISION: &str = "conduit.std/time-tick@2";
pub const TIME_EVERY_CONTRACT_REVISION: &str = "conduit.std/time-every@2";
pub const MAX_TICK_COUNT: u64 = 4_096;

pub fn tick_semantic_contract() -> Kind {
    let configuration = vec![
        KindConfigurationField {
            key: "count".into(),
            default_value: ConfigurationValue::U64(4),
            rule: KindConfigurationRule::U64Range {
                minimum: 0,
                maximum: MAX_TICK_COUNT,
            },
        },
        KindConfigurationField {
            key: "period-ms".into(),
            default_value: ConfigurationValue::U64(1_000),
            rule: KindConfigurationRule::U64Range {
                minimum: 0,
                maximum: u64::MAX,
            },
        },
    ];
    Kind {
        startup_parameters: configuration
            .iter()
            .map(|field| FrontStartupParameter {
                name: field.key.clone(),
                value_type: kind_id("value/count"),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        kind_id: kind_id(TICK_KIND),
        kind_contract_revision: TICK_CONTRACT_REVISION.into(),
        inputs: Vec::new(),
        outputs: tick_outputs(),
        configuration,
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::CompletesAfterConfiguredCount,
        )],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

pub fn time_every_semantic_contract() -> Kind {
    let configuration = vec![KindConfigurationField {
        key: "freq".into(),
        default_value: ConfigurationValue::Quantity(conduit_core::Quantity::new(
            1_000,
            conduit_core::QuantityUnit::Millisecond,
        )),
        rule: KindConfigurationRule::QuantityRange {
            minimum: 0,
            maximum: i64::MAX,
            canonical_unit: conduit_core::QuantityUnit::Millisecond,
        },
    }];
    Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "freq".into(),
            value_type: kind_id(conduit_core::DURATION_INFO_ID),
            has_default: false,
        }],
        shorthand: None,
        kind_id: kind_id(TIME_EVERY_KIND),
        kind_contract_revision: TIME_EVERY_CONTRACT_REVISION.into(),
        inputs: Vec::new(),
        outputs: time_every_outputs(),
        configuration,
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::HostObservationEndsOrFailsSource,
        )],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickConfiguration {
    pub count: u64,
    pub period_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EveryConfiguration {
    pub period_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickContractError {
    MissingConfiguration(&'static str),
    InvalidConfiguration(&'static str),
    WrongEncodedLength(usize),
}

impl core::fmt::Display for TickContractError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingConfiguration(key) => write!(formatter, "missing configuration '{key}'"),
            Self::InvalidConfiguration(key) => write!(formatter, "invalid configuration '{key}'"),
            Self::WrongEncodedLength(length) => {
                write!(formatter, "wrong encoded tick length {length}")
            }
        }
    }
}

pub fn parse_tick_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, TickContractError> {
    let mut count = None;
    let mut period_ms = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("period-ms", ConfigurationValue::U64(value)) => period_ms = Some(*value),
            ("count", _) => return Err(TickContractError::InvalidConfiguration("count")),
            ("period-ms", _) => return Err(TickContractError::InvalidConfiguration("period-ms")),
            _ => {}
        }
    }
    let count = count.ok_or(TickContractError::MissingConfiguration("count"))?;
    if count > MAX_TICK_COUNT {
        return Err(TickContractError::InvalidConfiguration("count"));
    }
    Ok(TickConfiguration {
        count,
        period_ms: period_ms.ok_or(TickContractError::MissingConfiguration("period-ms"))?,
    })
}

pub fn encode_tick(sequence: u64) -> [u8; TICK_ENCODED_LEN as usize] {
    sequence.to_le_bytes()
}

pub fn decode_tick(encoded: &[u8]) -> Result<u64, TickContractError> {
    let bytes: [u8; TICK_ENCODED_LEN as usize] = encoded
        .try_into()
        .map_err(|_| TickContractError::WrongEncodedLength(encoded.len()))?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn tick_value_kind() -> KindId {
    kind_id(TICK_VALUE_KIND)
}

pub fn tick_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(TICK_PORT),
        value_kind: tick_value_kind(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    }]
}

/// The recurring interval source has the same typed value as `time/tick`, but
/// remains open until its play is explicitly terminated or its host input is
/// lost. `time/tick` retains the separate closing, configured-count contract.
pub fn time_every_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(TICK_PORT),
        value_kind: tick_value_kind(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: false },
        abnormal_kind: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn codec_and_configuration_bounds_are_exact() {
        assert_eq!(decode_tick(&encode_tick(42)), Ok(42));
        assert_eq!(
            decode_tick(&[0; 7]),
            Err(TickContractError::WrongEncodedLength(7))
        );
        let parsed = parse_tick_configuration(&[
            ConfigurationEntry {
                key: "count".to_string(),
                value: ConfigurationValue::U64(0),
            },
            ConfigurationEntry {
                key: "period-ms".to_string(),
                value: ConfigurationValue::U64(7),
            },
        ])
        .unwrap();
        assert_eq!(
            parsed,
            TickConfiguration {
                count: 0,
                period_ms: 7
            }
        );
    }
}
