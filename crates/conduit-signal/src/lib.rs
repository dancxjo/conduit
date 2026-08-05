#![cfg_attr(not(feature = "host-profile"), no_std)]

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
pub const SHOW_KIND: &str = "display/show";
pub const SIGNAL_PORT: &str = "signal";
pub const SIGNAL_ENCODED_LEN: u32 = 9;
pub const SIGNAL_PRESENTATION_KIND: &str = "presentation/signal";
pub const MAX_SIGNAL_COUNT: u64 = 4_096;

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
    let count = count.ok_or(SignalProfileError::MissingConfiguration("count"))?;
    if count > MAX_SIGNAL_COUNT {
        return Err(SignalProfileError::InvalidConfiguration(
            "count".to_string(),
        ));
    }
    Ok(PulseConfiguration {
        count,
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

#[cfg(feature = "host-profile")]
mod host_profile {
    use super::{
        decode_signal, encode_signal, parse_pulse_configuration, pulse_kind, pulse_outputs,
        show_inputs, show_kind, signal_value_kind, PulseConfiguration, Signal, MAX_SIGNAL_COUNT,
        SIGNAL_ENCODED_LEN, SIGNAL_PRESENTATION_KIND,
    };
    use alloc::boxed::Box;
    use conduit_core::{
        kind_id, ArtifactId, ConfigurationValue, FailureReason, ImplementationId, KindId,
        PlannedOperation,
    };
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
    use conduit_runtime::{
        ImplementationFailure, ImplementationRegistry, OperationAction, OperationCompletion,
        OperationImplementation, OperationState,
    };

    pub struct PulseImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl PulseImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: pulse_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for PulseImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn prepare(
            &self,
            placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            let configuration =
                parse_pulse_configuration(&placement.configuration).map_err(|err| {
                    ImplementationFailure::new(
                        FailureReason::InvalidOperationConfiguration,
                        err.to_string(),
                    )
                })?;
            Ok(Box::new(PulseState {
                configuration,
                next_sequence: 0,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            (value_kind == &signal_value_kind()).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct PulseState {
        configuration: PulseConfiguration,
        next_sequence: u64,
    }

    impl PulseState {
        fn next_emit_or_complete(&self) -> OperationAction {
            if self.next_sequence >= self.configuration.count {
                OperationAction::Complete
            } else {
                OperationAction::Emit(encode_signal(&Signal {
                    sequence: self.next_sequence,
                    level: if self.next_sequence.is_multiple_of(2) {
                        self.configuration.initial_level
                    } else {
                        !self.configuration.initial_level
                    },
                }))
            }
        }
    }

    impl OperationState for PulseState {
        fn start(&mut self) -> OperationAction {
            self.next_emit_or_complete()
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Emitted => {
                    self.next_sequence += 1;
                    if self.next_sequence >= self.configuration.count {
                        OperationAction::Complete
                    } else if self.configuration.period_ms > 0 {
                        OperationAction::Wait {
                            duration_ms: self.configuration.period_ms,
                        }
                    } else {
                        self.next_emit_or_complete()
                    }
                }
                OperationCompletion::TimerElapsed => self.next_emit_or_complete(),
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "pulse received an incompatible runtime completion",
                )),
            }
        }
    }

    pub struct ShowImplementation {
        kind_id: KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl ShowImplementation {
        pub fn new(implementation_id: ImplementationId) -> Self {
            Self {
                kind_id: show_kind(),
                implementation_id,
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            }
        }
    }

    impl OperationImplementation for ShowImplementation {
        fn kind_id(&self) -> &KindId {
            &self.kind_id
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn prepare(
            &self,
            _placement: &PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            Ok(Box::new(ShowState {
                expected_sequence: 0,
                pending: None,
            }))
        }

        fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
            (value_kind == &signal_value_kind()).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct ShowState {
        expected_sequence: u64,
        pending: Option<Signal>,
    }

    impl OperationState for ShowState {
        fn start(&mut self) -> OperationAction {
            OperationAction::Idle
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Value(value) => match decode_signal(&value) {
                    Ok(signal) if signal.sequence == self.expected_sequence => {
                        self.pending = Some(signal);
                        OperationAction::Present {
                            presentation_kind: kind_id(SIGNAL_PRESENTATION_KIND),
                            value,
                        }
                    }
                    Ok(signal) => OperationAction::Fail(ImplementationFailure::new(
                        FailureReason::MalformedConnectionEnvelope,
                        format!(
                            "expected signal sequence {}, received {}",
                            self.expected_sequence, signal.sequence
                        ),
                    )),
                    Err(err) => OperationAction::Fail(ImplementationFailure::new(
                        FailureReason::UnsupportedValueKind,
                        err.to_string(),
                    )),
                },
                OperationCompletion::PresentationCompleted { success: true, .. } => {
                    self.pending = None;
                    self.expected_sequence += 1;
                    OperationAction::Idle
                }
                OperationCompletion::PresentationCompleted {
                    success: false,
                    message,
                } => OperationAction::Fail(ImplementationFailure {
                    reason: FailureReason::ManifestationFailed,
                    message,
                }),
                OperationCompletion::InputsClosed if self.pending.is_none() => {
                    OperationAction::Complete
                }
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "show received an incompatible runtime completion",
                )),
            }
        }
    }

    pub fn install_signal_profile(
        registry: &mut ImplementationRegistry,
        pulse_implementation_id: ImplementationId,
        show_implementation_id: ImplementationId,
    ) -> Result<(), ImplementationFailure> {
        registry.install(PulseImplementation::new(pulse_implementation_id))?;
        registry.install(ShowImplementation::new(show_implementation_id))?;
        Ok(())
    }

    pub fn signal_registry(
        pulse_implementation_id: ImplementationId,
        show_implementation_id: ImplementationId,
    ) -> Result<ImplementationRegistry, ImplementationFailure> {
        let mut registry = ImplementationRegistry::new();
        install_signal_profile(
            &mut registry,
            pulse_implementation_id,
            show_implementation_id,
        )?;
        Ok(registry)
    }

    pub fn signal_profile_catalog() -> ProfileCatalog {
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: pulse_kind(),
                inputs: Vec::new(),
                outputs: pulse_outputs(),
                configuration: vec![
                    ConfigurationField {
                        key: "count".to_string(),
                        default_value: ConfigurationValue::U64(16),
                        validation: ConfigurationRule::U64Range {
                            minimum: 0,
                            maximum: MAX_SIGNAL_COUNT,
                        },
                    },
                    ConfigurationField {
                        key: "period-ms".to_string(),
                        default_value: ConfigurationValue::U64(250),
                        validation: ConfigurationRule::U64Range {
                            minimum: 0,
                            maximum: u64::MAX,
                        },
                    },
                    ConfigurationField {
                        key: "initial".to_string(),
                        default_value: ConfigurationValue::Bool(false),
                        validation: ConfigurationRule::Any,
                    },
                ],
            })
            .expect("signal profile kinds are unique");
        catalog
            .insert(KindDefinition {
                kind_id: show_kind(),
                inputs: show_inputs(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .expect("signal profile kinds are unique");
        catalog
    }
}

#[cfg(feature = "host-profile")]
pub use host_profile::{
    install_signal_profile, signal_profile_catalog, signal_registry, PulseImplementation,
    ShowImplementation,
};

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
