use crate::{
    BoundedActivationAdmission, BoundedActivationError, BoundedActivationHost,
    BoundedActivationState, KernelCompositeDefinition, KernelCompositeHostRequest,
    KernelOperationRegistry,
};
use conduit_core::{InfoBool, KindId, PlannedActivation, ValuePayload, BOOL_INFO_ID};
use conduit_kernel::HostCallOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowSelectAdmission {
    Accepted { sequence: u64 },
    Full { sequence: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowSelectState {
    Idle,
    Active {
        sequence: u64,
    },
    OutputReady {
        sequence: u64,
    },
    Drained,
    InputAbnormal {
        terminal: ValuePayload,
    },
    PredicateAbnormal {
        sequence: u64,
        terminal: ValuePayload,
    },
    Faulted {
        sequence: u64,
    },
    Cancelled {
        sequence: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowSelectError {
    Activation(BoundedActivationError),
    InputKindMismatch {
        expected: KindId,
        actual: KindId,
    },
    InputExceedsBound {
        maximum: u32,
        actual: usize,
    },
    InputTerminalKindMismatch {
        expected: Option<KindId>,
        actual: KindId,
    },
    OutputCannotPropagateInputTerminal {
        input: KindId,
        output: Option<KindId>,
    },
    PredicateOutputIsNotCanonicalBoolean {
        actual: KindId,
    },
    MalformedBoolean,
    MissingPredicateDecision,
    InvalidLifecycle,
}

impl From<BoundedActivationError> for FlowSelectError {
    fn from(value: BoundedActivationError) -> Self {
        Self::Activation(value)
    }
}

#[derive(Debug, Clone)]
enum PendingTerminal {
    Normal,
    Abnormal(ValuePayload),
}

/// Production coordinator for the planned `flow/select` activation law.
///
/// The child is always the exact sealed predicate plan prepared by
/// [`BoundedActivationHost`]. This coordinator retains the original item; it
/// never evaluates or looks up predicate meaning itself.
pub struct FlowSelectCoordinator {
    activation: BoundedActivationHost,
    active_item: Option<(u64, ValuePayload)>,
    queued_item: Option<(u64, ValuePayload)>,
    predicate_decision: Option<bool>,
    output: Option<(u64, ValuePayload)>,
    pending_terminal: Option<PendingTerminal>,
    state: FlowSelectState,
}

impl FlowSelectCoordinator {
    pub fn prepare_planned(
        planned: &PlannedActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, FlowSelectError> {
        if planned.output.value_kind.as_str() != BOOL_INFO_ID {
            return Err(FlowSelectError::PredicateOutputIsNotCanonicalBoolean {
                actual: planned.output.value_kind.clone(),
            });
        }
        Ok(Self {
            activation: BoundedActivationHost::prepare_planned(planned, definition, registry)?,
            active_item: None,
            queued_item: None,
            predicate_decision: None,
            output: None,
            pending_terminal: None,
            state: FlowSelectState::Idle,
        })
    }

    pub fn state(&self) -> &FlowSelectState {
        &self.state
    }

    pub fn admit(
        &mut self,
        sequence: u64,
        item: &ValuePayload,
    ) -> Result<FlowSelectAdmission, FlowSelectError> {
        let contract = self.activation.contract();
        if item.value_kind != contract.input_value_kind {
            return Err(FlowSelectError::InputKindMismatch {
                expected: contract.input_value_kind.clone(),
                actual: item.value_kind.clone(),
            });
        }
        if item.encoded.len() > contract.maximum_queue_bytes as usize {
            return Err(FlowSelectError::InputExceedsBound {
                maximum: contract.maximum_queue_bytes,
                actual: item.encoded.len(),
            });
        }
        if self.pending_terminal.is_some()
            || self.output.is_some()
            || matches!(
                self.state,
                FlowSelectState::Drained
                    | FlowSelectState::InputAbnormal { .. }
                    | FlowSelectState::PredicateAbnormal { .. }
                    | FlowSelectState::Faulted { .. }
                    | FlowSelectState::Cancelled { .. }
            )
        {
            return Err(FlowSelectError::InvalidLifecycle);
        }
        if self.active_item.is_none() {
            return self.start(sequence, item.clone());
        }
        if self.queued_item.is_some() {
            return Ok(FlowSelectAdmission::Full { sequence });
        }
        self.queued_item = Some((sequence, item.clone()));
        Ok(FlowSelectAdmission::Accepted { sequence })
    }

    fn start(
        &mut self,
        sequence: u64,
        item: ValuePayload,
    ) -> Result<FlowSelectAdmission, FlowSelectError> {
        match self.activation.activate(sequence, &item)? {
            BoundedActivationAdmission::Accepted { .. } => {
                self.active_item = Some((sequence, item));
                self.predicate_decision = None;
                self.state = FlowSelectState::Active { sequence };
                Ok(FlowSelectAdmission::Accepted { sequence })
            }
            BoundedActivationAdmission::Full { .. } => Ok(FlowSelectAdmission::Full { sequence }),
        }
    }

    pub fn output(&self) -> Option<(u64, &ValuePayload)> {
        self.output
            .as_ref()
            .map(|(sequence, value)| (*sequence, value))
    }

    pub fn complete_output(&mut self, sequence: u64) -> Result<(), FlowSelectError> {
        if !matches!(self.output, Some((actual, _)) if actual == sequence) {
            return Err(FlowSelectError::InvalidLifecycle);
        }
        self.output = None;
        self.advance_after_item()
    }

    pub fn step(&mut self) -> Result<&FlowSelectState, FlowSelectError> {
        if self.output.is_some() {
            return Ok(&self.state);
        }
        if self.active_item.is_none() {
            self.advance_after_item()?;
            if self.active_item.is_none() {
                return Ok(&self.state);
            }
        }

        if self.predicate_decision.is_none() {
            if let Some((sequence, value)) = self.activation.output()? {
                if value.value_kind.as_str() != BOOL_INFO_ID {
                    return Err(FlowSelectError::PredicateOutputIsNotCanonicalBoolean {
                        actual: value.value_kind,
                    });
                }
                self.predicate_decision = Some(
                    InfoBool::decode(&value.encoded)
                        .map_err(|_| FlowSelectError::MalformedBoolean)?
                        .get(),
                );
                self.activation.complete_output(sequence)?;
            }
        }

        match self.activation.step()?.clone() {
            BoundedActivationState::Active { sequence } => {
                self.state = FlowSelectState::Active { sequence };
            }
            BoundedActivationState::Succeeded { sequence } => {
                let selected = self
                    .predicate_decision
                    .take()
                    .ok_or(FlowSelectError::MissingPredicateDecision)?;
                let (item_sequence, item) = self
                    .active_item
                    .take()
                    .ok_or(FlowSelectError::InvalidLifecycle)?;
                if sequence != item_sequence {
                    return Err(FlowSelectError::InvalidLifecycle);
                }
                if selected {
                    self.output = Some((sequence, item));
                    self.state = FlowSelectState::OutputReady { sequence };
                } else {
                    self.advance_after_item()?;
                }
            }
            BoundedActivationState::Abnormal { sequence, terminal } => {
                self.active_item = None;
                self.queued_item = None;
                self.pending_terminal = None;
                self.state = FlowSelectState::PredicateAbnormal { sequence, terminal };
            }
            BoundedActivationState::Faulted { sequence, .. } => {
                self.active_item = None;
                self.queued_item = None;
                self.pending_terminal = None;
                self.state = FlowSelectState::Faulted { sequence };
            }
            BoundedActivationState::Cancelled { sequence } => {
                self.clear_retained();
                self.state = FlowSelectState::Cancelled { sequence };
            }
            BoundedActivationState::Drained => self.state = FlowSelectState::Drained,
            BoundedActivationState::InputAbnormal { terminal } => {
                self.state = FlowSelectState::InputAbnormal { terminal };
            }
            BoundedActivationState::Idle => return Err(FlowSelectError::InvalidLifecycle),
        }
        Ok(&self.state)
    }

    fn advance_after_item(&mut self) -> Result<(), FlowSelectError> {
        if let Some((sequence, item)) = self.queued_item.take() {
            self.start(sequence, item)?;
        } else if let Some(terminal) = self.pending_terminal.take() {
            match terminal {
                PendingTerminal::Normal => self.activation.close_input()?,
                PendingTerminal::Abnormal(value) => self.activation.terminate_input(value)?,
            }
            match self.activation.step()?.clone() {
                BoundedActivationState::Drained => self.state = FlowSelectState::Drained,
                BoundedActivationState::InputAbnormal { terminal } => {
                    self.state = FlowSelectState::InputAbnormal { terminal };
                }
                _ => return Err(FlowSelectError::InvalidLifecycle),
            }
        } else {
            self.state = FlowSelectState::Idle;
        }
        Ok(())
    }

    pub fn close_input(&mut self) -> Result<(), FlowSelectError> {
        self.admit_terminal(PendingTerminal::Normal)
    }

    pub fn terminate_input(&mut self, terminal: ValuePayload) -> Result<(), FlowSelectError> {
        let contract = self.activation.contract();
        if contract.input_abnormal_kind.as_ref() != Some(&terminal.value_kind) {
            return Err(FlowSelectError::InputTerminalKindMismatch {
                expected: contract.input_abnormal_kind.clone(),
                actual: terminal.value_kind,
            });
        }
        if contract.output_abnormal_kind.as_ref() != Some(&terminal.value_kind) {
            return Err(FlowSelectError::OutputCannotPropagateInputTerminal {
                input: terminal.value_kind,
                output: contract.output_abnormal_kind.clone(),
            });
        }
        self.admit_terminal(PendingTerminal::Abnormal(terminal))
    }

    fn admit_terminal(&mut self, terminal: PendingTerminal) -> Result<(), FlowSelectError> {
        if self.pending_terminal.is_some()
            || matches!(
                self.state,
                FlowSelectState::Drained
                    | FlowSelectState::InputAbnormal { .. }
                    | FlowSelectState::PredicateAbnormal { .. }
                    | FlowSelectState::Faulted { .. }
                    | FlowSelectState::Cancelled { .. }
            )
        {
            return Err(FlowSelectError::InvalidLifecycle);
        }
        self.pending_terminal = Some(terminal);
        if self.active_item.is_none() && self.output.is_none() {
            self.advance_after_item()?;
        }
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), FlowSelectError> {
        let sequence = self.active_item.as_ref().map(|(sequence, _)| *sequence);
        if matches!(
            self.activation.state(),
            BoundedActivationState::Idle | BoundedActivationState::Active { .. }
        ) {
            self.activation.cancel()?;
        } else if !matches!(
            self.activation.state(),
            BoundedActivationState::Succeeded { .. }
        ) {
            return Err(FlowSelectError::InvalidLifecycle);
        }
        self.clear_retained();
        self.state = FlowSelectState::Cancelled { sequence };
        Ok(())
    }

    fn clear_retained(&mut self) {
        self.active_item = None;
        self.queued_item = None;
        self.predicate_decision = None;
        self.output = None;
        self.pending_terminal = None;
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.activation.next_host_request()
    }

    pub fn complete_host_call(
        &mut self,
        request: &KernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), FlowSelectError> {
        self.activation.complete_host_call(request, outcome)?;
        Ok(())
    }
}
