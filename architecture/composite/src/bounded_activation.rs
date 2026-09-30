use crate::{
    AdmittedKernelCompositeHostRequest, KernelCompositeDefinition, KernelCompositeError,
    KernelCompositeHost, KernelCompositeHostRequest, KernelCompositeStatus,
    KernelCompositeTerminal, KernelOperationRegistry,
};
use conduit_core::{
    semantic_digest, verify_plan, KindId, PlanId, PlannedActivation, PortDirection, PortId,
    PortTemporal, ValuePayload,
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
    pub maximum_items: u16,
    pub identity: [u8; 32],
}

impl BoundedActivationContract {
    fn for_definition(
        definition: &KernelCompositeDefinition,
        input_port: PortId,
        output_port: PortId,
        maximum_items: u16,
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
        if maximum_items == 0 {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
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
        encoded.extend_from_slice(&maximum_items.to_le_bytes());
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
            maximum_items,
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
    InputTerminalKindMismatch {
        expected: Option<KindId>,
        actual: KindId,
    },
    OutputCannotPropagateInputTerminal {
        input: KindId,
        output: Option<KindId>,
    },
    PlannedContractMismatch,
    InvalidLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedActivationAdmission {
    Accepted { sequence: u64 },
    Full { sequence: u64 },
    MaximumItemsExceeded { sequence: u64, maximum_items: u16 },
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
    Abnormal {
        sequence: u64,
        terminal: ValuePayload,
    },
    Drained,
    InputAbnormal {
        terminal: ValuePayload,
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
    MalformedAbnormalTerminal {
        expected: Option<KindId>,
        actual: KindId,
    },
    MissingOutput,
}

/// Owns at most one activation of an exact planned subgraph. Every accepted
/// value receives a fresh ordinary kernel execution; no callback, Kind lookup,
/// iterator, retry, or hidden scheduler participates.
pub struct BoundedActivationHost {
    contract: BoundedActivationContract,
    ready: Vec<KernelCompositeHost>,
    receipts: Vec<KernelCompositeHost>,
    active: Option<KernelCompositeHost>,
    input_close_pending: bool,
    output_completed: bool,
    output_sequence: Option<u64>,
    output_buffer: ValuePayload,
    pending_input_terminal: Option<KernelCompositeTerminal>,
    state: BoundedActivationState,
}

impl BoundedActivationHost {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        input_port: PortId,
        output_port: PortId,
        maximum_items: u16,
    ) -> Result<Self, BoundedActivationError> {
        let contract = BoundedActivationContract::for_definition(
            &definition,
            input_port,
            output_port,
            maximum_items,
        )?;
        Self::prepare_with_contract(definition, registry, contract)
    }

    /// Prepare the production adapter from exact Plan truth. The selected
    /// subplan, Value fronts, and finite activation limits must be identical
    /// to the executable composite definition; no runtime lookup or widening
    /// may repair a disagreement.
    pub fn prepare_planned(
        planned: &PlannedActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, BoundedActivationError> {
        if planned.selected_plan_id != planned.selected_plan.plan_id
            || planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        let contract = BoundedActivationContract::for_definition(
            &definition,
            planned.input.front_port_id.clone(),
            planned.output.front_port_id.clone(),
            planned.limits.maximum_items,
        )?;
        if contract.selected_plan_id != planned.selected_plan_id
            || contract.input_value_kind != planned.input.value_kind
            || contract.input_abnormal_kind != planned.input.abnormal_kind
            || contract.output_value_kind != planned.output.value_kind
            || contract.output_abnormal_kind != planned.output.abnormal_kind
            || contract.maximum_active != planned.limits.maximum_active
            || contract.maximum_queue_items != planned.limits.maximum_queue_items
            || contract.maximum_queue_bytes != planned.limits.maximum_queue_bytes
            || contract.maximum_items != planned.limits.maximum_items
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        Self::prepare_with_contract(definition, registry, contract)
    }

    fn prepare_with_contract(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        contract: BoundedActivationContract,
    ) -> Result<Self, BoundedActivationError> {
        // Refuse an unavailable or over-budget exact subgraph before any input
        // can become owed work. Each activation is prepared afresh below.
        let maximum_items = usize::from(contract.maximum_items);
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(BoundedActivationError::Refused)?,
            );
        }
        Self::prepare_with_ready(contract, ready)
    }

    pub(crate) fn prepare_planned_with_ready(
        planned: &PlannedActivation,
        definition: &KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedActivationError> {
        let contract = BoundedActivationContract::for_definition(
            definition,
            planned.input.front_port_id.clone(),
            planned.output.front_port_id.clone(),
            planned.limits.maximum_items,
        )?;
        if planned.selected_plan.as_ref() != &definition.internal_plan
            || contract.selected_plan_id != planned.selected_plan_id
            || contract.input_value_kind != planned.input.value_kind
            || contract.output_value_kind != planned.output.value_kind
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        Self::prepare_with_ready(contract, ready)
    }

    fn prepare_with_ready(
        contract: BoundedActivationContract,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedActivationError> {
        let maximum_items = usize::from(contract.maximum_items);
        if ready.len() != maximum_items || ready.capacity() != maximum_items {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        let output_buffer = ValuePayload {
            value_kind: contract.output_value_kind.clone(),
            encoded: Vec::with_capacity(contract.maximum_queue_bytes as usize),
        };
        Ok(Self {
            contract,
            ready,
            receipts: Vec::with_capacity(maximum_items),
            active: None,
            input_close_pending: false,
            output_completed: false,
            output_sequence: None,
            output_buffer,
            pending_input_terminal: None,
            state: BoundedActivationState::Idle,
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
        if self.pending_input_terminal.is_some()
            || matches!(
                self.state,
                BoundedActivationState::Cancelled { .. }
                    | BoundedActivationState::Drained
                    | BoundedActivationState::InputAbnormal { .. }
            )
        {
            return Err(BoundedActivationError::InvalidLifecycle);
        }
        if self.active.is_some() {
            return Ok(BoundedActivationAdmission::Full { sequence });
        }
        let Some(mut activation) = self.ready.pop() else {
            return Ok(BoundedActivationAdmission::MaximumItemsExceeded {
                sequence,
                maximum_items: self.contract.maximum_items,
            });
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

    pub fn output(&mut self) -> Result<Option<(u64, &ValuePayload)>, BoundedActivationError> {
        let sequence = match self.state {
            BoundedActivationState::Active { sequence } => sequence,
            _ => return Err(BoundedActivationError::InvalidLifecycle),
        };
        if self.output_sequence.is_none() {
            self.output_sequence = self
                .active
                .as_mut()
                .ok_or(BoundedActivationError::InvalidLifecycle)?
                .output_into(&self.contract.output_port, &mut self.output_buffer)
                .map_err(BoundedActivationError::Refused)?;
        }
        Ok(self
            .output_sequence
            .map(|_| (sequence, &self.output_buffer)))
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
        self.output_sequence = None;
        Ok(())
    }

    pub fn step(&mut self) -> Result<&BoundedActivationState, BoundedActivationError> {
        if self.active.is_none() && self.pending_input_terminal.is_some() {
            self.state = self.settle_pending_input_terminal()?;
            return Ok(&self.state);
        }
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
                        let completed = self.active.take().expect("active child was borrowed");
                        self.receipts.push(completed);
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
                let completed = self.active.take().expect("active child was borrowed");
                self.receipts.push(completed);
                self.input_close_pending = false;
                self.state = activation_terminal_state(
                    sequence,
                    terminal,
                    self.contract.output_abnormal_kind.as_ref(),
                    self.output_completed,
                );
                if !matches!(self.state, BoundedActivationState::Succeeded { .. }) {
                    self.pending_input_terminal = None;
                }
            }
            Ok(KernelCompositeStatus::Cancelled) => {
                let completed = self.active.take().expect("active child was borrowed");
                self.receipts.push(completed);
                self.input_close_pending = false;
                self.state = BoundedActivationState::Cancelled {
                    sequence: Some(sequence),
                };
            }
            Err(fault) => {
                let completed = self.active.take().expect("active child was borrowed");
                self.receipts.push(completed);
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
        }
        if let Some(cancelled) = self.active.take() {
            self.receipts.push(cancelled);
        }
        self.input_close_pending = false;
        self.output_completed = false;
        self.output_sequence = None;
        self.pending_input_terminal = None;
        self.state = BoundedActivationState::Cancelled { sequence };
        Ok(())
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }

    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&crate::KernelCompositeHostCallObligation, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_obligation(request)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &conduit_core::PreparationHostIdentity,
        resources: &[conduit_core::ResourceBinding],
        authorities: &[conduit_core::AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .admit_host_request(request, host, resources, authorities)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn complete_host_call(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedActivationError> {
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn signs(&self) -> BTreeMap<conduit_core::HostId, Vec<KernelEvent>> {
        let mut signs = BTreeMap::new();
        for receipt in self.receipts.iter().chain(self.active.iter()) {
            for (host, events) in receipt.signs() {
                signs.entry(host).or_insert_with(Vec::new).extend(events);
            }
        }
        signs
    }

    pub fn allocation_capacities(&self) -> (usize, usize) {
        (self.ready.capacity(), self.receipts.capacity())
    }

    pub fn remaining_items(&self) -> usize {
        self.ready.len()
    }

    /// Admit normal closure of the lifted input flow. If one activation is
    /// active, closure remains one finite owed terminal until that activation
    /// succeeds; no later value can be admitted.
    pub fn close_input(&mut self) -> Result<(), BoundedActivationError> {
        self.admit_input_terminal(KernelCompositeTerminal::Normal)
    }

    /// Admit one exact typed abnormal input terminal for propagation after the
    /// current activation drains. The input and output promises must name the
    /// same semantic terminal kind; mapping belongs to a reviewed combinator,
    /// not this generic activation primitive.
    pub fn terminate_input(
        &mut self,
        terminal: ValuePayload,
    ) -> Result<(), BoundedActivationError> {
        validate_propagated_input_terminal(
            self.contract.input_abnormal_kind.as_ref(),
            self.contract.output_abnormal_kind.as_ref(),
            &terminal.value_kind,
        )?;
        self.admit_input_terminal(KernelCompositeTerminal::Abnormal(terminal))
    }

    fn admit_input_terminal(
        &mut self,
        terminal: KernelCompositeTerminal,
    ) -> Result<(), BoundedActivationError> {
        if self.pending_input_terminal.is_some()
            || matches!(
                self.state,
                BoundedActivationState::Faulted { .. }
                    | BoundedActivationState::Abnormal { .. }
                    | BoundedActivationState::Cancelled { .. }
                    | BoundedActivationState::Drained
                    | BoundedActivationState::InputAbnormal { .. }
            )
        {
            return Err(BoundedActivationError::InvalidLifecycle);
        }
        self.pending_input_terminal = Some(terminal);
        Ok(())
    }

    fn settle_pending_input_terminal(
        &mut self,
    ) -> Result<BoundedActivationState, BoundedActivationError> {
        match self
            .pending_input_terminal
            .take()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
        {
            KernelCompositeTerminal::Normal => Ok(BoundedActivationState::Drained),
            KernelCompositeTerminal::Abnormal(terminal) => {
                Ok(BoundedActivationState::InputAbnormal { terminal })
            }
        }
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

fn activation_terminal_state(
    sequence: u64,
    terminal: Option<KernelCompositeTerminal>,
    expected_abnormal_kind: Option<&KindId>,
    output_completed: bool,
) -> BoundedActivationState {
    match terminal {
        Some(KernelCompositeTerminal::Abnormal(terminal))
            if expected_abnormal_kind == Some(&terminal.value_kind) =>
        {
            BoundedActivationState::Abnormal { sequence, terminal }
        }
        Some(KernelCompositeTerminal::Abnormal(terminal)) => BoundedActivationState::Faulted {
            sequence,
            fault: BoundedActivationFault::MalformedAbnormalTerminal {
                expected: expected_abnormal_kind.cloned(),
                actual: terminal.value_kind,
            },
        },
        Some(KernelCompositeTerminal::Normal) if output_completed => {
            BoundedActivationState::Succeeded { sequence }
        }
        _ => BoundedActivationState::Faulted {
            sequence,
            fault: BoundedActivationFault::MissingOutput,
        },
    }
}

fn validate_propagated_input_terminal(
    expected_input: Option<&KindId>,
    promised_output: Option<&KindId>,
    actual: &KindId,
) -> Result<(), BoundedActivationError> {
    if expected_input != Some(actual) {
        return Err(BoundedActivationError::InputTerminalKindMismatch {
            expected: expected_input.cloned(),
            actual: actual.clone(),
        });
    }
    if promised_output != Some(actual) {
        return Err(BoundedActivationError::OutputCannotPropagateInputTerminal {
            input: actual.clone(),
            output: promised_output.cloned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::kind_id;

    #[test]
    fn exact_abnormal_terminal_is_not_execution_failure_or_success() {
        let terminal = ValuePayload {
            value_kind: kind_id("test/transform-terminal"),
            encoded: vec![7],
        };
        assert_eq!(
            activation_terminal_state(
                4,
                Some(KernelCompositeTerminal::Abnormal(terminal.clone())),
                Some(&kind_id("test/transform-terminal")),
                false,
            ),
            BoundedActivationState::Abnormal {
                sequence: 4,
                terminal,
            }
        );
    }

    #[test]
    fn wrong_or_unpromised_abnormal_kind_is_a_malformed_execution_terminal() {
        for expected in [None, Some(kind_id("test/expected-terminal"))] {
            assert_eq!(
                activation_terminal_state(
                    5,
                    Some(KernelCompositeTerminal::Abnormal(ValuePayload {
                        value_kind: kind_id("test/wrong-terminal"),
                        encoded: vec![9],
                    })),
                    expected.as_ref(),
                    false,
                ),
                BoundedActivationState::Faulted {
                    sequence: 5,
                    fault: BoundedActivationFault::MalformedAbnormalTerminal {
                        expected,
                        actual: kind_id("test/wrong-terminal"),
                    },
                }
            );
        }
    }

    #[test]
    fn input_abnormal_propagation_requires_exact_matching_promises() {
        let terminal = kind_id("test/input-terminal");
        assert_eq!(
            validate_propagated_input_terminal(Some(&terminal), Some(&terminal), &terminal),
            Ok(())
        );
        assert_eq!(
            validate_propagated_input_terminal(None, Some(&terminal), &terminal),
            Err(BoundedActivationError::InputTerminalKindMismatch {
                expected: None,
                actual: terminal.clone(),
            })
        );
        assert_eq!(
            validate_propagated_input_terminal(Some(&terminal), None, &terminal),
            Err(BoundedActivationError::OutputCannotPropagateInputTerminal {
                input: terminal,
                output: None,
            })
        );
    }
}
