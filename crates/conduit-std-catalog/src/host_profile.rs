use super::{
    capability_slug, contract_revision, execution_profile, standard_contracts,
    standard_host_operation_requirements, standard_resource_requirements, FILTER_KIND,
    FORMAT_KIND, GENERIC_VALUE_KIND, LATEST_KIND, MAP_KIND, PULSE_KIND, SHOW_KIND,
    SIGNAL_VALUE_KIND, TEE_KIND, TICK_KIND,
};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use conduit_core::{
    kind_id, ArtifactId, ConfigurationValue, FailureReason, ImplementationId, KindId,
    PlannedOperation, PortId, ValuePayload,
};
use conduit_runtime::{
    ImplementationFailure, ImplementationRegistry, OperationAction, OperationCompletion,
    OperationImplementation, OperationOutput, OperationState,
};

pub fn standard_registry(
    implementation_prefix: &str,
) -> Result<ImplementationRegistry, ImplementationFailure> {
    let mut registry = ImplementationRegistry::new();
    install_standard_profile(&mut registry, implementation_prefix)?;
    Ok(registry)
}

pub fn install_standard_profile(
    registry: &mut ImplementationRegistry,
    implementation_prefix: &str,
) -> Result<(), ImplementationFailure> {
    for contract in standard_contracts() {
        registry.install(StandardImplementation {
            kind_id: contract.kind_id.clone(),
            implementation_id: ImplementationId::from(format!(
                "{implementation_prefix}/{}-v1",
                capability_slug(contract.kind_id.as_str())
            )),
            artifact_id: ArtifactId::from(format!(
                "conduit-std-catalog/{}",
                capability_slug(contract.kind_id.as_str())
            )),
        })?;
    }
    Ok(())
}

struct StandardImplementation {
    kind_id: KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
}

impl OperationImplementation for StandardImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    fn kind_contract_revision(&self) -> conduit_core::KindContractRevision {
        contract_revision(&self.kind_id)
    }

    fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId {
        execution_profile(&self.kind_id)
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        let maximum_value_bytes = standard_contracts()
            .into_iter()
            .find(|contract| contract.kind_id == self.kind_id)
            .map_or(0, |contract| contract.limits.max_queue_bytes);
        standard_host_operation_requirements(&self.kind_id, maximum_value_bytes)
    }

    fn resource_requirements(&self) -> Vec<conduit_core::ResourceRequirement> {
        standard_resource_requirements(&self.kind_id)
    }

    fn prepare(
        &self,
        placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        match self.kind_id.as_str() {
            PULSE_KIND => Ok(Box::new(CountedSourceState::new(
                placement,
                GENERIC_VALUE_KIND,
                16,
                250,
            )?)),
            TICK_KIND => Ok(Box::new(CountedSourceState::new(
                placement,
                GENERIC_VALUE_KIND,
                16,
                1_000,
            )?)),
            SHOW_KIND => Ok(Box::new(ShowState::new(placement)?)),
            MAP_KIND => Ok(Box::new(PassState::new(placement)?)),
            FILTER_KIND => Ok(Box::new(FilterState {
                predicate_id: u64_config(placement, "predicate-id", 0)?,
                output_port: only_output(placement)?,
            })),
            TEE_KIND => Ok(Box::new(PassState::new(placement)?)),
            FORMAT_KIND => Ok(Box::new(FormatState {
                output_port: only_output(placement)?,
            })),
            LATEST_KIND => Ok(Box::new(LatestState {
                latest: None,
                output_port: only_output(placement)?,
            })),
            _ => Err(ImplementationFailure::new(
                FailureReason::UnsupportedKind,
                format!("unsupported standard kind '{}'", self.kind_id.as_str()),
            )),
        }
    }

    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        match value_kind.as_str() {
            SIGNAL_VALUE_KIND | GENERIC_VALUE_KIND => Some(8),
            super::TEXT_VALUE_KIND => Some(1),
            _ => None,
        }
    }
}

struct CountedSourceState {
    value_kind: KindId,
    output_port: PortId,
    next: u64,
    count: u64,
    period_ms: u64,
    waiting: bool,
}

impl CountedSourceState {
    fn new(
        placement: &PlannedOperation,
        value_kind: &str,
        default_count: u64,
        default_period_ms: u64,
    ) -> Result<Self, ImplementationFailure> {
        Ok(Self {
            value_kind: kind_id(value_kind),
            output_port: only_output(placement)?,
            next: 0,
            count: u64_config(placement, "count", default_count)?,
            period_ms: u64_config(placement, "period-ms", default_period_ms)?,
            waiting: false,
        })
    }

    fn next_action(&mut self) -> OperationAction {
        if self.next >= self.count {
            OperationAction::Complete
        } else if self.waiting {
            OperationAction::Idle
        } else {
            self.waiting = true;
            OperationAction::Wait {
                duration_ms: self.period_ms,
            }
        }
    }
}

impl OperationState for CountedSourceState {
    fn start(&mut self) -> OperationAction {
        self.next_action()
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::TimerElapsed => {
                self.waiting = false;
                OperationAction::Emit(vec![OperationOutput {
                    port: self.output_port.clone(),
                    value: number_payload(&self.value_kind, self.next),
                }])
            }
            OperationCompletion::Emitted => {
                self.next += 1;
                self.next_action()
            }
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "counted source received invalid completion",
            )),
        }
    }
}

struct PassState {
    output_ports: Vec<PortId>,
}

impl PassState {
    fn new(placement: &PlannedOperation) -> Result<Self, ImplementationFailure> {
        if placement.outputs.is_empty() {
            return Err(ImplementationFailure::new(
                FailureReason::InvalidOperationConfiguration,
                "pass operation requires at least one output",
            ));
        }
        Ok(Self {
            output_ports: placement
                .outputs
                .iter()
                .map(|port| port.port_id.clone())
                .collect(),
        })
    }
}

impl OperationState for PassState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { value, .. } => emit_to(&self.output_ports, value),
            OperationCompletion::Emitted => OperationAction::Complete,
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "pass operation received invalid completion",
            )),
        }
    }
}

struct FilterState {
    predicate_id: u64,
    output_port: PortId,
}

impl OperationState for FilterState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { value, .. } => {
                if self.accepts(&value) {
                    emit_to(std::slice::from_ref(&self.output_port), value)
                } else {
                    OperationAction::Idle
                }
            }
            OperationCompletion::Emitted => OperationAction::Complete,
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "filter operation received invalid completion",
            )),
        }
    }
}

impl FilterState {
    fn accepts(&self, value: &ValuePayload) -> bool {
        self.predicate_id == 0 || decode_number(value).is_none_or(|number| number % 2 == 0)
    }
}

struct FormatState {
    output_port: PortId,
}

impl OperationState for FormatState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { value, .. } => {
                let text = match decode_number(&value) {
                    Some(number) => format!("value:{number}"),
                    None => format!("bytes:{}", value.encoded.len()),
                };
                emit_to(
                    std::slice::from_ref(&self.output_port),
                    ValuePayload {
                        value_kind: kind_id(GENERIC_VALUE_KIND),
                        encoded: text.into_bytes(),
                    },
                )
            }
            OperationCompletion::Emitted => OperationAction::Complete,
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "format operation received invalid completion",
            )),
        }
    }
}

struct LatestState {
    latest: Option<ValuePayload>,
    output_port: PortId,
}

impl OperationState for LatestState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { value, .. } => {
                self.latest = Some(value.clone());
                emit_to(std::slice::from_ref(&self.output_port), value)
            }
            OperationCompletion::Emitted => OperationAction::Complete,
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "latest operation received invalid completion",
            )),
        }
    }

    fn release(&mut self) {
        self.latest = None;
    }
}

struct ShowState {
    input_port: PortId,
}

impl ShowState {
    fn new(placement: &PlannedOperation) -> Result<Self, ImplementationFailure> {
        let input_port = placement
            .inputs
            .first()
            .filter(|_| placement.inputs.len() == 1)
            .map(|port| port.port_id.clone())
            .ok_or_else(|| {
                ImplementationFailure::new(
                    FailureReason::InvalidOperationConfiguration,
                    "show operation requires one exact input",
                )
            })?;
        Ok(Self { input_port })
    }
}

impl OperationState for ShowState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value { port, value } if port == self.input_port => {
                OperationAction::Present {
                    presentation_kind: kind_id("presentation/stdout"),
                    value,
                }
            }
            OperationCompletion::PresentationCompleted { success, message } => {
                if success {
                    OperationAction::Complete
                } else {
                    OperationAction::Fail(ImplementationFailure::new(
                        FailureReason::ManifestationFailed,
                        message.unwrap_or_else(|| "presentation failed".to_string()),
                    ))
                }
            }
            OperationCompletion::InputsClosed => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "show operation received invalid completion",
            )),
        }
    }
}

fn only_output(placement: &PlannedOperation) -> Result<PortId, ImplementationFailure> {
    placement
        .outputs
        .first()
        .filter(|_| placement.outputs.len() == 1)
        .map(|port| port.port_id.clone())
        .ok_or_else(|| {
            ImplementationFailure::new(
                FailureReason::InvalidOperationConfiguration,
                "operation requires one exact output",
            )
        })
}

fn emit_to(ports: &[PortId], value: ValuePayload) -> OperationAction {
    OperationAction::Emit(
        ports
            .iter()
            .map(|port| OperationOutput {
                port: port.clone(),
                value: value.clone(),
            })
            .collect(),
    )
}

fn u64_config(
    placement: &PlannedOperation,
    key: &str,
    default_value: u64,
) -> Result<u64, ImplementationFailure> {
    placement
        .configuration
        .iter()
        .find(|entry| entry.key == key)
        .map_or(Ok(default_value), |entry| match entry.value {
            ConfigurationValue::U64(value) => Ok(value),
            _ => Err(ImplementationFailure::new(
                FailureReason::InvalidOperationConfiguration,
                format!("configuration '{key}' must be u64"),
            )),
        })
}

fn number_payload(value_kind: &KindId, value: u64) -> ValuePayload {
    ValuePayload {
        value_kind: value_kind.clone(),
        encoded: value.to_le_bytes().to_vec(),
    }
}

fn decode_number(value: &ValuePayload) -> Option<u64> {
    let bytes = value.encoded.get(..8)?;
    let mut number = [0; 8];
    number.copy_from_slice(bytes);
    Some(u64::from_le_bytes(number))
}
