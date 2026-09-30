//! Kernel-backed execution of one exact planned bounded scan.

use crate::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost,
    KernelCompositeHostRequest, KernelCompositeStatus, KernelCompositeTerminal,
    KernelOperationRegistry,
};
use conduit_core::{
    verify_plan, PlannedActivationEffectMultiplicity, PlannedActivationFront,
    PlannedScanAbnormalPolicy, PlannedScanActivation, PlannedScanCancellationPolicy,
    PlannedScanTerminalPolicy, PortDirection, PortTemporal, ValuePayload,
};
use conduit_kernel::HostCallOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedScanState {
    Idle,
    Active,
    OutputReady,
    Abnormal(ValuePayload),
    Cancelled,
    Complete,
}

#[derive(Debug)]
pub enum BoundedScanError {
    PlannedContractMismatch,
    Refused(KernelCompositeError),
    InvalidLifecycle,
    MissingOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedScanAdmission {
    Accepted,
    Full,
    MaximumItemsExceeded { maximum_items: u16 },
}

pub struct BoundedScanActivationHost {
    planned: PlannedScanActivation,
    ready: Vec<KernelCompositeHost>,
    receipts: Vec<KernelCompositeHost>,
    accumulator: ValuePayload,
    queued: ValuePayload,
    queued_ready: bool,
    admission: ValuePayload,
    active: Option<KernelCompositeHost>,
    candidate: ValuePayload,
    candidate_ready: bool,
    output_ready: bool,
    closing: bool,
    state: BoundedScanState,
}

impl BoundedScanActivationHost {
    pub fn prepare(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, BoundedScanError> {
        if planned.selected_plan_id != planned.selected_plan.plan_id
            || planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || planned.accumulator_input.value_kind != planned.output.value_kind
            || planned.limits.maximum_active != 1
            || planned.limits.maximum_queue_items != 1
            || planned.initial_accumulator.len() > planned.retained_accumulator_bytes as usize
            || planned.terminal_policy
                != PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission
            || planned.abnormal_policy
                != PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact
            || planned.cancellation_policy
                != PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission
            || planned.effect_multiplicity
                != PlannedActivationEffectMultiplicity::OncePerAcceptedInput
            || !front_matches(
                &definition,
                &planned.accumulator_input,
                PortDirection::Input,
            )
            || !front_matches(&definition, &planned.item_input, PortDirection::Input)
            || !front_matches(&definition, &planned.output, PortDirection::Output)
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        let mut accumulator_bytes = Vec::with_capacity(planned.retained_accumulator_bytes as usize);
        accumulator_bytes.extend_from_slice(&planned.initial_accumulator);
        let accumulator = ValuePayload {
            value_kind: planned.accumulator_input.value_kind.clone(),
            encoded: accumulator_bytes,
        };
        let candidate = ValuePayload {
            value_kind: planned.output.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
        };
        let queued = ValuePayload {
            value_kind: planned.item_input.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_item_bytes as usize),
        };
        let admission = ValuePayload {
            value_kind: planned.item_input.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_item_bytes as usize),
        };
        let maximum_items = usize::from(planned.limits.maximum_items);
        if maximum_items == 0 {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(BoundedScanError::Refused)?,
            );
        }
        Ok(Self {
            planned: planned.clone(),
            ready,
            receipts: Vec::with_capacity(maximum_items),
            accumulator,
            queued,
            queued_ready: false,
            admission,
            active: None,
            candidate,
            candidate_ready: false,
            output_ready: false,
            closing: false,
            state: BoundedScanState::Idle,
        })
    }

    pub fn admit(&mut self, item: &ValuePayload) -> Result<BoundedScanAdmission, BoundedScanError> {
        if self.closing
            || matches!(
                self.state,
                BoundedScanState::Abnormal(_)
                    | BoundedScanState::Cancelled
                    | BoundedScanState::Complete
            )
        {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        if item.value_kind != self.planned.item_input.value_kind
            || item.encoded.len() > self.planned.retained_item_bytes as usize
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        if self.active.is_some() || self.output_ready {
            if self.ready.is_empty() {
                return Ok(BoundedScanAdmission::MaximumItemsExceeded {
                    maximum_items: self.planned.limits.maximum_items,
                });
            }
            if self.queued_ready {
                return Ok(BoundedScanAdmission::Full);
            }
            self.queued.encoded.clear();
            self.queued.encoded.extend_from_slice(&item.encoded);
            self.queued_ready = true;
            return Ok(BoundedScanAdmission::Accepted);
        }
        if self.ready.is_empty() {
            return Ok(BoundedScanAdmission::MaximumItemsExceeded {
                maximum_items: self.planned.limits.maximum_items,
            });
        }
        self.admission.encoded.clear();
        self.admission.encoded.extend_from_slice(&item.encoded);
        self.start()?;
        Ok(BoundedScanAdmission::Accepted)
    }

    fn start(&mut self) -> Result<(), BoundedScanError> {
        let mut child = self.ready.pop().ok_or(BoundedScanError::InvalidLifecycle)?;
        child.start().map_err(BoundedScanError::Refused)?;
        for (port, value) in [
            (
                &self.planned.accumulator_input.front_port_id,
                &self.accumulator,
            ),
            (&self.planned.item_input.front_port_id, &self.admission),
        ] {
            if !matches!(
                child
                    .admit_input(port, 0, value)
                    .map_err(BoundedScanError::Refused)?,
                conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
            ) {
                return Err(BoundedScanError::InvalidLifecycle);
            }
            child.close_input(port).map_err(BoundedScanError::Refused)?;
        }
        self.active = Some(child);
        self.candidate_ready = false;
        self.state = BoundedScanState::Active;
        Ok(())
    }

    pub fn close_input(&mut self) -> Result<(), BoundedScanError> {
        if self.closing {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        self.closing = true;
        if self.active.is_none() {
            self.state = if self.output_ready {
                BoundedScanState::OutputReady
            } else {
                BoundedScanState::Complete
            };
        }
        Ok(())
    }

    pub fn terminate_input(&mut self, terminal: ValuePayload) -> Result<(), BoundedScanError> {
        if self.planned.item_input.abnormal_kind.as_ref() != Some(&terminal.value_kind) {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        if let Some(active) = &mut self.active {
            active.cancel().map_err(BoundedScanError::Refused)?;
        }
        if let Some(terminated) = self.active.take() {
            self.receipts.push(terminated);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.output_ready = false;
        self.state = BoundedScanState::Abnormal(terminal);
        Ok(())
    }

    pub fn step(&mut self) -> Result<&BoundedScanState, BoundedScanError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(&self.state);
        };
        if !self.candidate_ready
            && active
                .output_into(&self.planned.output.front_port_id, &mut self.candidate)
                .map_err(BoundedScanError::Refused)?
                .is_some()
        {
            active
                .complete_output(&self.planned.output.front_port_id, 0)
                .map_err(BoundedScanError::Refused)?;
            self.candidate_ready = true;
        }
        match active.step().map_err(BoundedScanError::Refused)? {
            KernelCompositeStatus::Active => {}
            KernelCompositeStatus::Complete => {
                let terminal = active
                    .output_terminal(&self.planned.output.front_port_id)
                    .map_err(BoundedScanError::Refused)?;
                if let Some(KernelCompositeTerminal::Abnormal(terminal)) = terminal {
                    if self.planned.output.abnormal_kind.as_ref() != Some(&terminal.value_kind) {
                        return Err(BoundedScanError::PlannedContractMismatch);
                    }
                    let completed = self.active.take().expect("active scan child was borrowed");
                    self.receipts.push(completed);
                    self.queued_ready = false;
                    self.candidate_ready = false;
                    self.state = BoundedScanState::Abnormal(terminal);
                    return Ok(&self.state);
                }
                if terminal != Some(KernelCompositeTerminal::Normal) {
                    return Err(BoundedScanError::MissingOutput);
                }
                if !self.candidate_ready {
                    return Err(BoundedScanError::MissingOutput);
                }
                if self.candidate.value_kind != self.planned.output.value_kind
                    || self.candidate.encoded.len()
                        > self.planned.retained_accumulator_bytes as usize
                {
                    return Err(BoundedScanError::PlannedContractMismatch);
                }
                core::mem::swap(&mut self.accumulator, &mut self.candidate);
                self.candidate.encoded.clear();
                self.candidate_ready = false;
                let completed = self.active.take().expect("active scan child was borrowed");
                self.receipts.push(completed);
                self.output_ready = true;
                self.state = BoundedScanState::OutputReady;
            }
            KernelCompositeStatus::Cancelled => {
                let completed = self.active.take().expect("active scan child was borrowed");
                self.receipts.push(completed);
                self.queued_ready = false;
                self.state = BoundedScanState::Cancelled;
            }
        }
        Ok(&self.state)
    }

    pub fn output_into(&self, destination: &mut ValuePayload) -> Result<bool, BoundedScanError> {
        if !self.output_ready {
            return Ok(false);
        }
        if destination.value_kind != self.accumulator.value_kind
            || destination.encoded.capacity() < self.accumulator.encoded.len()
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        destination.encoded.clear();
        destination
            .encoded
            .extend_from_slice(&self.accumulator.encoded);
        Ok(true)
    }

    pub fn complete_output(&mut self) -> Result<(), BoundedScanError> {
        if !self.output_ready {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        self.output_ready = false;
        if self.queued_ready {
            core::mem::swap(&mut self.admission.encoded, &mut self.queued.encoded);
            self.queued_ready = false;
            self.start()?;
        } else if self.closing {
            self.state = BoundedScanState::Complete;
        } else {
            self.state = BoundedScanState::Idle;
        }
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), BoundedScanError> {
        if let Some(active) = &mut self.active {
            active.cancel().map_err(BoundedScanError::Refused)?;
        }
        if let Some(cancelled) = self.active.take() {
            self.receipts.push(cancelled);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.output_ready = false;
        self.state = BoundedScanState::Cancelled;
        Ok(())
    }
    pub fn allocation_capacities(&self) -> (usize, usize) {
        (self.ready.capacity(), self.receipts.capacity())
    }
    pub fn storage_capacities(&self) -> (usize, usize, usize, usize) {
        (
            self.accumulator.encoded.capacity(),
            self.candidate.encoded.capacity(),
            self.queued.encoded.capacity(),
            self.admission.encoded.capacity(),
        )
    }
    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }
    pub fn complete_host_call(
        &mut self,
        request: &KernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedScanError> {
        self.active
            .as_mut()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedScanError::Refused)
    }
}

fn front_matches(
    definition: &KernelCompositeDefinition,
    expected: &PlannedActivationFront,
    direction: PortDirection,
) -> bool {
    definition
        .boundary
        .input_fronts
        .iter()
        .chain(&definition.boundary.output_fronts)
        .any(|front| {
            front.external_port.port_id == expected.front_port_id
                && front.external_port.direction == direction
                && front.external_port.temporal == PortTemporal::Value
                && front.external_port.value_kind == expected.value_kind
                && front.external_port.abnormal_kind == expected.abnormal_kind
        })
}
