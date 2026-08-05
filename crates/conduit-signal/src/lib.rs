#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, ConfigurationEntry, ConfigurationValue, KindId, PortDescriptor,
    PortDirection, ValuePayload,
};
use serde::{Deserialize, Serialize};

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const SIGNAL_PORT: &str = "signal";
pub const SIGNAL_ENCODED_LEN: u32 = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub sequence: u64,
    pub level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulseConfiguration {
    pub count: u64,
    pub period_ms: u64,
    pub initial_level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalProfileError {
    MissingConfiguration(&'static str),
    InvalidConfiguration(String),
    WrongValueKind(String),
    WrongEncodedLength(usize),
}

impl core::fmt::Display for SignalProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SignalProfileError::MissingConfiguration(key) => {
                write!(f, "missing configuration '{key}'")
            }
            SignalProfileError::InvalidConfiguration(key) => {
                write!(f, "invalid configuration '{key}'")
            }
            SignalProfileError::WrongValueKind(kind) => {
                write!(f, "wrong value kind '{kind}'")
            }
            SignalProfileError::WrongEncodedLength(length) => {
                write!(f, "wrong encoded signal length {length}")
            }
        }
    }
}

pub fn pulse_kind() -> KindId {
    kind_id(PULSE_KIND)
}

pub fn show_kind() -> KindId {
    kind_id(SHOW_KIND)
}

pub fn signal_value_kind() -> KindId {
    kind_id(SIGNAL_VALUE_KIND)
}

pub fn pulse_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Output,
    }]
}

pub fn show_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Input,
    }]
}

pub fn pulse_configuration_entries(config: &PulseConfiguration) -> Vec<ConfigurationEntry> {
    vec![
        ConfigurationEntry {
            key: "count".to_string(),
            value: ConfigurationValue::U64(config.count),
        },
        ConfigurationEntry {
            key: "period-ms".to_string(),
            value: ConfigurationValue::U64(config.period_ms),
        },
        ConfigurationEntry {
            key: "initial".to_string(),
            value: ConfigurationValue::Bool(config.initial_level),
        },
    ]
}

pub fn parse_pulse_configuration(
    entries: &[ConfigurationEntry],
) -> Result<PulseConfiguration, SignalProfileError> {
    let mut count = None;
    let mut period_ms = None;
    let mut initial_level = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("period-ms", ConfigurationValue::U64(value)) => period_ms = Some(*value),
            ("initial", ConfigurationValue::Bool(value)) => initial_level = Some(*value),
            ("count", _) | ("period-ms", _) | ("initial", _) => {
                return Err(SignalProfileError::InvalidConfiguration(entry.key.clone()));
            }
            _ => {}
        }
    }
    Ok(PulseConfiguration {
        count: count.ok_or(SignalProfileError::MissingConfiguration("count"))?,
        period_ms: period_ms.ok_or(SignalProfileError::MissingConfiguration("period-ms"))?,
        initial_level: initial_level.ok_or(SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn encode_signal(signal: &Signal) -> ValuePayload {
    let mut encoded = Vec::with_capacity(SIGNAL_ENCODED_LEN as usize);
    encoded.extend_from_slice(&signal.sequence.to_le_bytes());
    encoded.push(u8::from(signal.level));
    ValuePayload {
        value_kind: signal_value_kind(),
        encoded,
    }
}

pub fn decode_signal(payload: &ValuePayload) -> Result<Signal, SignalProfileError> {
    if payload.value_kind.as_str() != SIGNAL_VALUE_KIND {
        return Err(SignalProfileError::WrongValueKind(
            payload.value_kind.as_str().to_string(),
        ));
    }
    if payload.encoded.len() != SIGNAL_ENCODED_LEN as usize {
        return Err(SignalProfileError::WrongEncodedLength(
            payload.encoded.len(),
        ));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&payload.encoded[..8]);
    Ok(Signal {
        sequence: u64::from_le_bytes(sequence),
        level: payload.encoded[8] != 0,
    })
}

pub fn signal_payload_size() -> u32 {
    SIGNAL_ENCODED_LEN
}

#[cfg(test)]
mod tests {
    use super::{
        decode_signal, encode_signal, parse_pulse_configuration, pulse_configuration_entries,
        PulseConfiguration, Signal,
    };

    #[test]
    fn round_trips_signal_payload() {
        let payload = encode_signal(&Signal {
            sequence: 7,
            level: true,
        });
        let decoded = decode_signal(&payload).expect("signal payload should decode");
        assert_eq!(decoded.sequence, 7);
        assert!(decoded.level);
    }

    #[test]
    fn round_trips_pulse_configuration_entries() {
        let config = PulseConfiguration {
            count: 3,
            period_ms: 0,
            initial_level: false,
        };
        let parsed = parse_pulse_configuration(&pulse_configuration_entries(&config))
            .expect("pulse configuration should parse");
        assert_eq!(parsed, config);
    }
}
