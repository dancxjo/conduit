#![cfg_attr(not(feature = "host-profile"), no_std)]

#[cfg(feature = "host-profile")]
extern crate alloc;

#[cfg(feature = "host-profile")]
mod trigger;
#[cfg(feature = "host-profile")]
pub use trigger::*;
#[cfg(feature = "host-profile")]
mod control;
#[cfg(feature = "host-profile")]
pub use control::*;
#[cfg(feature = "host-profile")]
mod canonical;
#[cfg(feature = "host-profile")]
use alloc::string::ToString;
#[cfg(feature = "host-profile")]
use alloc::vec;
#[cfg(feature = "host-profile")]
use alloc::vec::Vec;
#[cfg(feature = "host-profile")]
pub use canonical::{primary_signal_startup_catalog, signal_startup_catalog};
#[cfg(feature = "host-profile")]
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ConfigurationEntry, ConfigurationValue, ExecutionProfileId,
    HostOperationRequirement, KindContractRevision, KindId, PortDescriptor, PortDirection,
    ResourceOffer, ResourceRequirement, ValuePayload, PRESENTATION_RESOURCE_CLASS,
    TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const SIGNAL_PORT: &str = "signal";

#[cfg(feature = "host-profile")]
pub fn pulse_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![
        conduit_core::FaceStartupParameter {
            name: "count".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "period-ms".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "initial".to_string(),
            value_type: "Boolean".to_string(),
            has_default: true,
        },
    ]
}
pub const SIGNAL_ENCODED_LEN: u32 = 9;
pub const SIGNAL_PRESENTATION_KIND: &str = "presentation/signal";
pub const MAX_SIGNAL_COUNT: u64 = 4_096;
pub const PULSE_CONTRACT_REVISION: &str = "conduit.signal/flow-pulse@1";
pub const SHOW_CONTRACT_REVISION: &str = "conduit.signal/presentation-show@1";
pub const PULSE_EXECUTION_PROFILE: &str = "conduit.signal/pulse-hosted@1";
pub const SHOW_EXECUTION_PROFILE: &str = "conduit.signal/show-hosted@1";
pub const SIGNAL_ENCODED_LEN_USIZE: usize = SIGNAL_ENCODED_LEN as usize;
pub type EncodedSignal = [u8; SIGNAL_ENCODED_LEN_USIZE];

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
    InvalidConfiguration(&'static str),
    WrongValueKind,
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
            SignalProfileError::WrongValueKind => f.write_str("wrong value kind"),
            SignalProfileError::WrongEncodedLength(length) => {
                write!(f, "wrong encoded signal length {length}")
            }
        }
    }
}

#[cfg(feature = "host-profile")]
pub fn pulse_kind() -> KindId {
    kind_id(PULSE_KIND)
}

#[cfg(feature = "host-profile")]
pub fn show_kind() -> KindId {
    kind_id(SHOW_KIND)
}

#[cfg(feature = "host-profile")]
pub fn signal_value_kind() -> KindId {
    kind_id(SIGNAL_VALUE_KIND)
}

#[cfg(feature = "host-profile")]
pub fn pulse_contract_revision() -> KindContractRevision {
    KindContractRevision::from(PULSE_CONTRACT_REVISION)
}

#[cfg(feature = "host-profile")]
pub fn show_contract_revision() -> KindContractRevision {
    KindContractRevision::from(SHOW_CONTRACT_REVISION)
}

#[cfg(feature = "host-profile")]
pub fn pulse_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(PULSE_EXECUTION_PROFILE)
}

#[cfg(feature = "host-profile")]
pub fn show_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(SHOW_EXECUTION_PROFILE)
}

#[cfg(feature = "host-profile")]
pub fn pulse_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![wait_host_operation_requirement()]
}

#[cfg(feature = "host-profile")]
pub fn show_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![present_host_operation_requirement(
        kind_id(SIGNAL_PRESENTATION_KIND),
        SIGNAL_ENCODED_LEN,
    )]
}

#[cfg(feature = "host-profile")]
pub fn pulse_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
}

#[cfg(feature = "host-profile")]
pub fn show_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
}

#[cfg(feature = "host-profile")]
pub fn signal_resource_offers(
    timer_pool_id: &str,
    presentation_pool_id: &str,
    capacity_units: u32,
) -> Vec<ResourceOffer> {
    let mut offers = vec![
        resource_offer(timer_pool_id, TIMER_RESOURCE_CLASS, capacity_units),
        resource_offer(
            presentation_pool_id,
            PRESENTATION_RESOURCE_CLASS,
            capacity_units,
        ),
    ];
    offers.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    offers
}

#[cfg(feature = "host-profile")]
pub fn pulse_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

#[cfg(feature = "host-profile")]
pub fn show_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(SIGNAL_PORT),
        value_kind: signal_value_kind(),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

#[cfg(feature = "host-profile")]
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

#[cfg(feature = "host-profile")]
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
                return Err(SignalProfileError::InvalidConfiguration(
                    match entry.key.as_str() {
                        "count" => "count",
                        "period-ms" => "period-ms",
                        "initial" => "initial",
                        _ => "unknown",
                    },
                ));
            }
            _ => {}
        }
    }
    let count = count.ok_or(SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(SignalProfileError::InvalidConfiguration("count"));
    }
    Ok(PulseConfiguration {
        count,
        period_ms: period_ms.ok_or(SignalProfileError::MissingConfiguration("period-ms"))?,
        initial_level: initial_level.ok_or(SignalProfileError::MissingConfiguration("initial"))?,
    })
}

pub fn signal_level_for_sequence(sequence: u64, initial_level: bool) -> bool {
    if sequence.is_multiple_of(2) {
        initial_level
    } else {
        !initial_level
    }
}

pub fn encode_signal_fixed(signal: &Signal) -> EncodedSignal {
    let mut encoded = [0u8; SIGNAL_ENCODED_LEN_USIZE];
    encoded[..8].copy_from_slice(&signal.sequence.to_le_bytes());
    encoded[8] = u8::from(signal.level);
    encoded
}

pub fn encode_signal_into(signal: &Signal, encoded: &mut [u8]) -> Result<(), SignalProfileError> {
    if encoded.len() != SIGNAL_ENCODED_LEN_USIZE {
        return Err(SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    encoded.copy_from_slice(&encode_signal_fixed(signal));
    Ok(())
}

pub fn decode_signal_fixed(encoded: &EncodedSignal) -> Signal {
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&encoded[..8]);
    Signal {
        sequence: u64::from_le_bytes(sequence),
        level: encoded[8] != 0,
    }
}

#[cfg(feature = "host-profile")]
pub fn encode_signal(signal: &Signal) -> ValuePayload {
    let mut encoded = Vec::with_capacity(SIGNAL_ENCODED_LEN_USIZE);
    encoded.extend_from_slice(&encode_signal_fixed(signal));
    ValuePayload {
        value_kind: signal_value_kind(),
        encoded,
    }
}

#[cfg(feature = "host-profile")]
pub fn decode_signal(payload: &ValuePayload) -> Result<Signal, SignalProfileError> {
    if payload.value_kind.as_str() != SIGNAL_VALUE_KIND {
        return Err(SignalProfileError::WrongValueKind);
    }
    decode_signal_bytes(&payload.encoded)
}

pub fn decode_signal_bytes(encoded: &[u8]) -> Result<Signal, SignalProfileError> {
    if encoded.len() != SIGNAL_ENCODED_LEN_USIZE {
        return Err(SignalProfileError::WrongEncodedLength(encoded.len()));
    }
    let mut sequence = [0u8; 8];
    sequence.copy_from_slice(&encoded[..8]);
    Ok(Signal {
        sequence: u64::from_le_bytes(sequence),
        level: encoded[8] != 0,
    })
}

pub fn signal_payload_size() -> u32 {
    SIGNAL_ENCODED_LEN
}

#[cfg(feature = "host-profile")]
mod profile_catalog;
#[cfg(feature = "host-profile")]
pub fn signal_profile_catalog() -> conduit_form::ProfileCatalog {
    let mut catalog = profile_catalog::signal_profile_catalog();
    trigger::extend_profile_catalog(&mut catalog);
    control::extend_control_profile_catalog(&mut catalog);
    catalog
}

#[cfg(feature = "host-profile")]
pub fn primary_signal_profile_catalog() -> conduit_form::ProfileCatalog {
    profile_catalog::signal_profile_catalog()
}

#[cfg(test)]
mod tests;
