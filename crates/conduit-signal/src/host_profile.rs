//! Hosted compatibility implementations for the portable Signal profile.
//!
//! Semantic values and contracts remain in the crate root; this module owns only
//! hosted implementation state, registry installation, and profile catalog wiring.

use super::{
    decode_signal, encode_signal, parse_pulse_configuration, pulse_contract_revision,
    pulse_execution_profile, pulse_host_operation_requirements, pulse_kind,
    pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_kind, show_resource_requirements, signal_value_kind,
    PulseConfiguration, Signal, SIGNAL_ENCODED_LEN, SIGNAL_PORT, SIGNAL_PRESENTATION_KIND,
};
use alloc::boxed::Box;
use conduit_core::{
    kind_id, port_id, ArtifactId, FailureReason, ImplementationId, KindId, PlannedGear,
};
use conduit_runtime::{
    ImplementationFailure, ImplementationRegistry, OperationAction, OperationCompletion,
    OperationImplementation, OperationOutput, OperationState,
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

    fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
        pulse_contract_revision()
    }

    fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
        pulse_execution_profile()
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        pulse_host_operation_requirements()
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        pulse_resource_requirements()
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        let configuration = parse_pulse_configuration(&placement.configuration).map_err(|err| {
            ImplementationFailure::new(FailureReason::InvalidGearConfiguration, err.to_string())
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
            OperationAction::Emit(vec![OperationOutput {
                port: port_id(SIGNAL_PORT),
                value: encode_signal(&Signal {
                    sequence: self.next_sequence,
                    level: if self.next_sequence.is_multiple_of(2) {
                        self.configuration.initial_level
                    } else {
                        !self.configuration.initial_level
                    },
                }),
            }])
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

    fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
        show_contract_revision()
    }

    fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
        show_execution_profile()
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        show_host_operation_requirements()
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        show_resource_requirements()
    }

    fn prepare(
        &self,
        _placement: &PlannedGear,
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
            OperationCompletion::Value { port, value } if port.as_str() == SIGNAL_PORT => {
                match decode_signal(&value) {
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
                }
            }
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
