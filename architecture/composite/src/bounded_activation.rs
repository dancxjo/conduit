use crate::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost,
    KernelCompositeHostRequest, KernelCompositeStatus, KernelCompositeTerminal,
    KernelOperationRegistry,
};
use conduit_core::{
    semantic_digest, KindId, PlanId, PortDirection, PortId, PortTemporal, ValuePayload,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_kernel::{HostCallOutcome, KernelEvent};
use std::collections::BTreeMap;

const ACTIVATION_CONTRACT_INFO_ID: &str = "conduit.execution.bounded-activation-contract.v1";

/// The immutable checked and planned contract for repeatedly activating one
/// exact Value behavior. This is temporal lifting machinery; it does not alter
/// the selected behavior's own port modality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedActivationContract {
    pub selected_plan_id: PlanId,
    pub input_port: PortId,
    pub input_value_kind: KindId,
    pub input_abnormal_kind: Option<KindId>,
    pub output_port: PortId,
    pub output_value_kind: KindId,
    pub output_abnormal_kind: Option<KindId>,
    pub maximum_active: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub identity: [u8; 32],
}

impl BoundedActivationContract {
    fn for_definition(
        definition: &KernelCompositeDefinition,
        input_port: PortId,
        output_port: PortId,
    ) -> Result<Self, BoundedActivationError> {
        let input = definition
            .boundary
            .input_fronts
            .iter()
            .find(|front| front.external_port.port_id == input_port)
            .ok_or_else(|| BoundedActivationError::UnknownInput(input_port.clone()))?;
        let output = definition
            .boundary
            .output_fronts
            .iter()
            .find(|front| front.external_port.port_id == output_port)
            .ok_or_else(|| BoundedActivationError::UnknownOutput(output_port.clone()))?;
        if input.external_port.direction != PortDirection::Input
            || input.external_port.temporal != PortTemporal::Value
        {
            return Err(BoundedActivationError::NotValueInput(input_port));
        }
        if output.external_port.direction != PortDirection::Output
            || output.external_port.temporal != PortTemporal::Value
        {
            return Err(BoundedActivationError::NotValueOutput(output_port));
        }

        let maximum_active = 1u16;
        let maximum_queue_items = 1u16;
        let maximum_queue_bytes = definition.external_capability.limits.max_queue_bytes;
        let mut encoded = Vec::new();
        encode_string(&mut encoded, definition.internal_plan.plan_id.as_str());
        encode_string(&mut encoded, input.external_port.port_id.as_str());
        encode_string(&mut encoded, input.external_port.value_kind.as_str());
        encode_optional_kind(&mut encoded, input.external_port.abnormal_kind.as_ref());
        encode_string(&mut encoded, output.external_port.port_id.as_str());
        encode_string(&mut encoded, output.external_port.value_kind.as_str());
        encode_optional_kind(&mut encoded, output.external_port.abnormal_kind.as_ref());
        encoded.extend_from_slice(&maximum_active.to_le_bytes());
        encoded.extend_from_slice(&maximum_queue_items.to_le_bytes());
        encoded.extend_from_slice(&maximum_queue_bytes.to_le_bytes());
        Ok(Self {
            selected_plan_id: definition.internal_plan.plan_id.clone(),
            input_port: input.external_port.port_id.clone(),
            input_value_kind: input.external_port.value_kind.clone(),
            input_abnormal_kind: input.external_port.abnormal_kind.clone(),
            output_port: output.external_port.port_id.clone(),
            output_value_kind: output.external_port.value_kind.clone(),
            output_abnormal_kind: output.external_port.abnormal_kind.clone(),
            maximum_active,
            maximum_queue_items,
            maximum_queue_bytes,
            identity: semantic_digest(ACTIVATION_CONTRACT_INFO_ID, &encoded),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedActivationError {
    UnknownInput(PortId),
    UnknownOutput(PortId),
    NotValueInput(PortId),
    NotValueOutput(PortId),
    Refused(KernelCompositeError),
    InvalidLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedActivationAdmission {
    Accepted { sequence: u64 },
    Full { sequence: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedActivationState {
    Idle,
    Active {
        sequence: u64,
    },
    Succeeded {
        sequence: u64,
    },
    Faulted {
        sequence: u64,
        fault: BoundedActivationFault,
    },
    Cancelled {
        sequence: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedActivationFault {
    Execution(KernelCompositeError),
    Abnormal(ValuePayload),
    MissingOutput,
}

/// Owns at most one activation of an exact planned subgraph. Every accepted
/// value receives a fresh ordinary kernel execution; no callback, Kind lookup,
/// iterator, retry, or hidden scheduler participates.
pub struct BoundedActivationHost {
    definition: KernelCompositeDefinition,
    registry: KernelOperationRegistry,
    contract: BoundedActivationContract,
    ready: Option<KernelCompositeHost>,
    active: Option<KernelCompositeHost>,
    input_close_pending: bool,
    output_completed: bool,
    state: BoundedActivationState,
    last_signs: BTreeMap<conduit_core::HostId, Vec<KernelEvent>>,
}

impl BoundedActivationHost {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        input_port: PortId,
        output_port: PortId,
    ) -> Result<Self, BoundedActivationError> {
        let contract =
            BoundedActivationContract::for_definition(&definition, input_port, output_port)?;
        // Refuse an unavailable or over-budget exact subgraph before any input
        // can become owed work. Each activation is prepared afresh below.
        let ready = KernelCompositeHost::prepare(definition.clone(), registry)
            .map_err(BoundedActivationError::Refused)?;
        Ok(Self {
            definition,
            registry: registry.clone(),
            contract,
            ready: Some(ready),
            active: None,
            input_close_pending: false,
            output_completed: false,
            state: BoundedActivationState::Idle,
            last_signs: BTreeMap::new(),
        })
    }

    pub fn contract(&self) -> &BoundedActivationContract {
        &self.contract
    }

    pub fn state(&self) -> &BoundedActivationState {
        &self.state
    }

    pub fn activate(
        &mut self,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<BoundedActivationAdmission, BoundedActivationError> {
        if self.active.is_some() {
            return Ok(BoundedActivationAdmission::Full { sequence });
        }
        if matches!(self.state, BoundedActivationState::Cancelled { .. }) {
            return Err(BoundedActivationError::InvalidLifecycle);
        }
        let mut activation = match self.ready.take() {
            Some(ready) => ready,
            None => KernelCompositeHost::prepare(self.definition.clone(), &self.registry)
                .map_err(BoundedActivationError::Refused)?,
        };
        activation
            .start()
            .map_err(BoundedActivationError::Refused)?;
        match activation
            .admit_input(&self.contract.input_port, 0, value)
            .map_err(BoundedActivationError::Refused)?
        {
            RemoteIngressOutcome::Accepted { .. } => {}
            RemoteIngressOutcome::Full { .. } => {
                return Ok(BoundedActivationAdmission::Full { sequence });
            }
        }
        self.active = Some(activation);
        self.input_close_pending = true;
        self.output_completed = false;
        self.state = BoundedActivationState::Active { sequence };
        Ok(BoundedActivationAdmission::Accepted { sequence })
    }

    pub fn output(&mut self) -> Result<Option<(u64, ValuePayload)>, BoundedActivationError> {
        let sequence = match self.state {
            BoundedActivationState::Active { sequence } => sequence,
            _ => return Err(BoundedActivationError::InvalidLifecycle),
        };
        let output = self
            .active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .output(&self.contract.output_port)
            .map_err(BoundedActivationError::Refused)?;
        Ok(output.map(|(_, value)| (sequence, value)))
    }

    pub fn complete_output(&mut self, sequence: u64) -> Result<(), BoundedActivationError> {
        if self.state != (BoundedActivationState::Active { sequence }) {
            return Err(BoundedActivationError::InvalidLifecycle);
        }
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_output(&self.contract.output_port, 0)
            .map_err(BoundedActivationError::Refused)?;
        self.output_completed = true;
        Ok(())
    }

    pub fn step(&mut self) -> Result<&BoundedActivationState, BoundedActivationError> {
        let sequence = match self.state {
            BoundedActivationState::Active { sequence } => sequence,
            _ => return Err(BoundedActivationError::InvalidLifecycle),
        };
        let activation = self
            .active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?;
        match activation.step() {
            Ok(KernelCompositeStatus::Active) => {
                if self.input_close_pending {
                    if let Err(fault) = activation.close_input(&self.contract.input_port) {
                        self.last_signs = activation.signs();
                        self.active = None;
                        self.input_close_pending = false;
                        self.state = BoundedActivationState::Faulted {
                            sequence,
                            fault: BoundedActivationFault::Execution(fault),
                        };
                        return Ok(&self.state);
                    }
                    self.input_close_pending = false;
                }
            }
            Ok(KernelCompositeStatus::Complete) => {
                let terminal = activation
                    .output_terminal(&self.contract.output_port)
                    .map_err(BoundedActivationError::Refused)?;
                self.last_signs = activation.signs();
                self.active = None;
                self.input_close_pending = false;
                self.state = match terminal {
                    Some(KernelCompositeTerminal::Abnormal(terminal)) => {
                        BoundedActivationState::Faulted {
                            sequence,
                            fault: BoundedActivationFault::Abnormal(terminal),
                        }
                    }
                    Some(KernelCompositeTerminal::Normal) if self.output_completed => {
                        BoundedActivationState::Succeeded { sequence }
                    }
                    _ => BoundedActivationState::Faulted {
                        sequence,
                        fault: BoundedActivationFault::MissingOutput,
                    },
                };
            }
            Ok(KernelCompositeStatus::Cancelled) => {
                self.last_signs = activation.signs();
                self.active = None;
                self.input_close_pending = false;
                self.state = BoundedActivationState::Cancelled {
                    sequence: Some(sequence),
                };
            }
            Err(fault) => {
                self.last_signs = activation.signs();
                self.active = None;
                self.input_close_pending = false;
                self.state = BoundedActivationState::Faulted {
                    sequence,
                    fault: BoundedActivationFault::Execution(fault),
                };
            }
        }
        Ok(&self.state)
    }

    pub fn cancel(&mut self) -> Result<(), BoundedActivationError> {
        let sequence = match self.state {
            BoundedActivationState::Active { sequence } => Some(sequence),
            BoundedActivationState::Idle => None,
            _ => return Err(BoundedActivationError::InvalidLifecycle),
        };
        if let Some(active) = &mut self.active {
            active.cancel().map_err(BoundedActivationError::Refused)?;
            self.last_signs = active.signs();
        }
        self.active = None;
        self.input_close_pending = false;
        self.output_completed = false;
        self.state = BoundedActivationState::Cancelled { sequence };
        Ok(())
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }

    pub fn complete_host_call(
        &mut self,
        request: &KernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedActivationError> {
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn signs(&self) -> BTreeMap<conduit_core::HostId, Vec<KernelEvent>> {
        self.active
            .as_ref()
            .map_or_else(|| self.last_signs.clone(), KernelCompositeHost::signs)
    }
}

fn encode_string(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&(value.len() as u32).to_le_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn encode_optional_kind(encoded: &mut Vec<u8>, value: Option<&KindId>) {
    match value {
        Some(kind) => {
            encoded.push(1);
            encode_string(encoded, kind.as_str());
        }
        None => encoded.push(0),
    }
}
