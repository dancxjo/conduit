use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ConfigurationEntry, ConfigurationValue, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};

pub const TICK_KIND: &str = "time/tick";
pub const TIME_EVERY_KIND: &str = "time/every";
pub const TICK_PORT: &str = "tick";
pub const TICK_VALUE_KIND: &str = "value/tick@1";
pub const TICK_ENCODED_LEN: u32 = 8;
pub const TICK_CONTRACT_REVISION: &str = "conduit.std/time-tick@2";
pub const TIME_EVERY_CONTRACT_REVISION: &str = "conduit.std/time-every@1";
pub const MAX_TICK_COUNT: u64 = 4_096;
pub const TIME_EVERY_COUNT: u64 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickConfiguration {
    pub count: u64,
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
