//! Bounded activation lifecycle and terminal propagation.
mod contract;
mod preparation;
#[cfg_attr(not(feature = "fixture-registry-preparation"), allow(unused_imports))]
use crate::{
    AdmittedKernelCompositeHostRequest, KernelCompositeError, KernelCompositeHost,
    KernelCompositeHostRequest, KernelCompositeStatus, KernelCompositeTerminal,
};
#[cfg_attr(not(feature = "fixture-registry-preparation"), allow(unused_imports))]
use conduit_core::{KindId, PlannedActivation, PortId, ValuePayload};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_kernel::{HostCallOutcome, KernelEvent};
pub use contract::BoundedActivationContract;
use std::collections::BTreeMap;

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
    abnormal_buffer: Option<ValuePayload>,
    pending_input_terminal: Option<PendingInputTerminal>,
    state: BoundedActivationState,
}

enum PendingInputTerminal {
    Normal,
    Abnormal(ValuePayload),
}

impl BoundedActivationHost {
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
                let terminal_buffer = self
                    .abnormal_buffer
                    .as_mut()
                    .unwrap_or(&mut self.output_buffer);
                let terminal = activation
                    .output_terminal_into(&self.contract.output_port, terminal_buffer)
                    .map_err(BoundedActivationError::Refused)?;
                let abnormal =
                    matches!(terminal, Some(KernelCompositeTerminal::Abnormal)).then(|| {
                        self.abnormal_buffer
                            .take()
                            .expect("planned abnormal buffer")
                    });
                let completed = self.active.take().expect("active child was borrowed");
                self.receipts.push(completed);
                self.input_close_pending = false;
                self.state =
                    activation_terminal_state(sequence, terminal, abnormal, self.output_completed);
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
        self.state = BoundedActivationState::Cancelled { sequence };
        let cancellation = self
            .active
            .as_mut()
            .map(KernelCompositeHost::cancel)
            .transpose();
        if let Some(cancelled) = self.active.take() {
            self.receipts.push(cancelled);
        }
        self.input_close_pending = false;
        self.output_completed = false;
        self.output_sequence = None;
        self.pending_input_terminal = None;
        cancellation
            .map(|_| ())
            .map_err(BoundedActivationError::Refused)
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

    pub fn host_request_view(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<crate::KernelCompositeHostRequestView<'_>, BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_view(request)
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

    pub fn host_request_input(
        &self,
        request: &AdmittedKernelCompositeHostRequest,
    ) -> Result<&[u8], BoundedActivationError> {
        self.active
            .as_ref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .host_request_input(request)
            .map_err(BoundedActivationError::Refused)
    }

    pub fn complete_host_call_bytes(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), BoundedActivationError> {
        self.active
            .as_mut()
            .ok_or(BoundedActivationError::InvalidLifecycle)?
            .complete_host_call_bytes(request, bytes)
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

    pub fn last_cancellation_failures(&self) -> &[(conduit_core::HostId, String)] {
        self.receipts
            .last()
            .map_or(&[], KernelCompositeHost::cancellation_failures)
    }

    pub fn remaining_items(&self) -> usize {
        self.ready.len()
    }

    /// Admit normal closure of the lifted input flow. If one activation is
    /// active, closure remains one finite owed terminal until that activation
    /// succeeds; no later value can be admitted.
    pub fn close_input(&mut self) -> Result<(), BoundedActivationError> {
        self.admit_input_terminal(PendingInputTerminal::Normal)
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
        self.admit_input_terminal(PendingInputTerminal::Abnormal(terminal))
    }

    fn admit_input_terminal(
        &mut self,
        terminal: PendingInputTerminal,
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
            PendingInputTerminal::Normal => Ok(BoundedActivationState::Drained),
            PendingInputTerminal::Abnormal(terminal) => {
                Ok(BoundedActivationState::InputAbnormal { terminal })
            }
        }
    }
}

fn activation_terminal_state(
    sequence: u64,
    terminal: Option<KernelCompositeTerminal>,
    abnormal: Option<ValuePayload>,
    output_completed: bool,
) -> BoundedActivationState {
    match terminal {
        Some(KernelCompositeTerminal::Abnormal) if abnormal.is_some() => {
            let terminal = abnormal.expect("matched prepared abnormal terminal");
            BoundedActivationState::Abnormal { sequence, terminal }
        }
        Some(KernelCompositeTerminal::Abnormal) => BoundedActivationState::Faulted {
            sequence,
            fault: BoundedActivationFault::MissingOutput,
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
mod tests;
