//! Kernel-backed execution of one exact planned bounded fold.

use crate::{
    AdmittedKernelCompositeHostRequest, KernelCompositeDefinition, KernelCompositeError,
    KernelCompositeHost, KernelCompositeHostRequest, KernelCompositeStatus,
    KernelCompositeTerminal, KernelOperationRegistry,
};
use conduit_core::{
    verify_plan, PlannedActivationEffectMultiplicity, PlannedActivationFront,
    PlannedFoldAbnormalPolicy, PlannedFoldActivation, PlannedFoldCancellationPolicy,
    PlannedFoldTerminalPolicy, PortDirection, PortTemporal, ValuePayload,
};
use conduit_kernel::HostCallOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedFoldState {
    Idle,
    Active,
    FinalReady,
    Abnormal(ValuePayload),
    Cancelled,
    Complete,
}

#[derive(Debug)]
pub enum BoundedFoldError {
    PlannedContractMismatch,
    Refused(KernelCompositeError),
    InvalidLifecycle,
    MissingOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedFoldAdmission {
    Accepted,
    Full,
    MaximumItemsExceeded { maximum_items: u16 },
}

pub struct BoundedFoldActivationHost {
    planned: PlannedFoldActivation,
    ready: Vec<KernelCompositeHost>,
    receipts: Vec<KernelCompositeHost>,
    accumulator: ValuePayload,
    queued: ValuePayload,
    queued_ready: bool,
    admission: ValuePayload,
    active: Option<KernelCompositeHost>,
    candidate: ValuePayload,
    candidate_ready: bool,
    closing: bool,
    state: BoundedFoldState,
}

impl BoundedFoldActivationHost {
    pub fn prepare(
        planned: &PlannedFoldActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, BoundedFoldError> {
        if planned.selected_plan_id != planned.selected_plan.plan_id
            || planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || planned.accumulator_input.value_kind != planned.output.value_kind
            || planned.limits.maximum_active != 1
            || planned.limits.maximum_queue_items != 1
            || planned.initial_accumulator.len() > planned.retained_accumulator_bytes as usize
            || planned.terminal_policy
                != PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce
            || planned.abnormal_policy
                != PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact
            || planned.cancellation_policy
                != PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission
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
            return Err(BoundedFoldError::PlannedContractMismatch);
        }
        let maximum_items = usize::from(planned.limits.maximum_items);
        if maximum_items == 0 {
            return Err(BoundedFoldError::PlannedContractMismatch);
        }
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(BoundedFoldError::Refused)?,
            );
        }
        Self::prepare_with_ready(planned, definition, ready)
    }

    pub(crate) fn prepare_with_ready(
        planned: &PlannedFoldActivation,
        definition: KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedFoldError> {
        if planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || ready.len() != usize::from(planned.limits.maximum_items)
            || ready.capacity() != usize::from(planned.limits.maximum_items)
        {
            return Err(BoundedFoldError::PlannedContractMismatch);
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
        let maximum_items = ready.len();
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
            closing: false,
            state: BoundedFoldState::Idle,
        })
    }

    pub fn admit(&mut self, item: &ValuePayload) -> Result<BoundedFoldAdmission, BoundedFoldError> {
        if self.closing
            || matches!(
                self.state,
                BoundedFoldState::Abnormal(_)
                    | BoundedFoldState::Cancelled
                    | BoundedFoldState::Complete
            )
        {
            return Err(BoundedFoldError::InvalidLifecycle);
        }
        if item.value_kind != self.planned.item_input.value_kind
            || item.encoded.len() > self.planned.retained_item_bytes as usize
        {
            return Err(BoundedFoldError::PlannedContractMismatch);
        }
        if self.active.is_some() {
            if self.queued_ready {
                return Ok(BoundedFoldAdmission::Full);
            }
            if self.ready.is_empty() {
                return Ok(BoundedFoldAdmission::MaximumItemsExceeded {
                    maximum_items: self.planned.limits.maximum_items,
                });
            }
            self.queued.encoded.clear();
            self.queued.encoded.extend_from_slice(&item.encoded);
            self.queued_ready = true;
            return Ok(BoundedFoldAdmission::Accepted);
        }
        if self.ready.is_empty() {
            return Ok(BoundedFoldAdmission::MaximumItemsExceeded {
                maximum_items: self.planned.limits.maximum_items,
            });
        }
        self.admission.encoded.clear();
        self.admission.encoded.extend_from_slice(&item.encoded);
        self.start()?;
        Ok(BoundedFoldAdmission::Accepted)
    }

    fn start(&mut self) -> Result<(), BoundedFoldError> {
        let mut child = self.ready.pop().ok_or(BoundedFoldError::InvalidLifecycle)?;
        child.start().map_err(BoundedFoldError::Refused)?;
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
                    .map_err(BoundedFoldError::Refused)?,
                conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
            ) {
                return Err(BoundedFoldError::InvalidLifecycle);
            }
            child.close_input(port).map_err(BoundedFoldError::Refused)?;
        }
        self.active = Some(child);
        self.candidate_ready = false;
        self.state = BoundedFoldState::Active;
        Ok(())
    }

    pub fn close_input(&mut self) -> Result<(), BoundedFoldError> {
        if self.closing {
            return Err(BoundedFoldError::InvalidLifecycle);
        }
        self.closing = true;
        if self.active.is_none() {
            self.state = BoundedFoldState::FinalReady;
        }
        Ok(())
    }

    pub fn terminate_input(&mut self, terminal: ValuePayload) -> Result<(), BoundedFoldError> {
        if self.planned.item_input.abnormal_kind.as_ref() != Some(&terminal.value_kind) {
            return Err(BoundedFoldError::PlannedContractMismatch);
        }
        if let Some(active) = &mut self.active {
            active.cancel().map_err(BoundedFoldError::Refused)?;
        }
        if let Some(terminated) = self.active.take() {
            self.receipts.push(terminated);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.state = BoundedFoldState::Abnormal(terminal);
        Ok(())
    }

    pub fn step(&mut self) -> Result<&BoundedFoldState, BoundedFoldError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(&self.state);
        };
        if !self.candidate_ready
            && active
                .output_into(&self.planned.output.front_port_id, &mut self.candidate)
                .map_err(BoundedFoldError::Refused)?
                .is_some()
        {
            active
                .complete_output(&self.planned.output.front_port_id, 0)
                .map_err(BoundedFoldError::Refused)?;
            self.candidate_ready = true;
        }
        match active.step().map_err(BoundedFoldError::Refused)? {
            KernelCompositeStatus::Active => {}
            KernelCompositeStatus::Complete => {
                let terminal = active
                    .output_terminal(&self.planned.output.front_port_id)
                    .map_err(BoundedFoldError::Refused)?;
                if let Some(KernelCompositeTerminal::Abnormal(terminal)) = terminal {
                    if self.planned.output.abnormal_kind.as_ref() != Some(&terminal.value_kind) {
                        return Err(BoundedFoldError::PlannedContractMismatch);
                    }
                    let completed = self.active.take().expect("active fold child was borrowed");
                    self.receipts.push(completed);
                    self.queued_ready = false;
                    self.candidate_ready = false;
                    self.state = BoundedFoldState::Abnormal(terminal);
                    return Ok(&self.state);
                }
                if terminal != Some(KernelCompositeTerminal::Normal) {
                    return Err(BoundedFoldError::MissingOutput);
                }
                if !self.candidate_ready {
                    return Err(BoundedFoldError::MissingOutput);
                }
                if self.candidate.value_kind != self.planned.output.value_kind
                    || self.candidate.encoded.len()
                        > self.planned.retained_accumulator_bytes as usize
                {
                    return Err(BoundedFoldError::PlannedContractMismatch);
                }
                core::mem::swap(&mut self.accumulator, &mut self.candidate);
                self.candidate.encoded.clear();
                self.candidate_ready = false;
                let completed = self.active.take().expect("active fold child was borrowed");
                self.receipts.push(completed);
                if self.queued_ready {
                    core::mem::swap(&mut self.admission.encoded, &mut self.queued.encoded);
                    self.queued_ready = false;
                    self.start()?;
                } else if self.closing {
                    self.state = BoundedFoldState::FinalReady;
                } else {
                    self.state = BoundedFoldState::Idle;
                }
            }
            KernelCompositeStatus::Cancelled => {
                let completed = self.active.take().expect("active fold child was borrowed");
                self.receipts.push(completed);
                self.queued_ready = false;
                self.state = BoundedFoldState::Cancelled;
            }
        }
        Ok(&self.state)
    }

    pub fn final_value(&mut self) -> Result<Option<ValuePayload>, BoundedFoldError> {
        if self.state != BoundedFoldState::FinalReady {
            return Ok(None);
        }
        self.state = BoundedFoldState::Complete;
        Ok(Some(self.accumulator.clone()))
    }

    pub fn cancel(&mut self) -> Result<(), BoundedFoldError> {
        if let Some(active) = &mut self.active {
            active.cancel().map_err(BoundedFoldError::Refused)?;
        }
        if let Some(cancelled) = self.active.take() {
            self.receipts.push(cancelled);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.state = BoundedFoldState::Cancelled;
        Ok(())
    }
    pub fn allocation_capacities(&self) -> (usize, usize) {
        (self.ready.capacity(), self.receipts.capacity())
    }
    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }
    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&crate::KernelCompositeHostCallObligation, BoundedFoldError> {
        self.active
            .as_ref()
            .ok_or(BoundedFoldError::InvalidLifecycle)?
            .host_request_obligation(request)
            .map_err(BoundedFoldError::Refused)
    }
    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &conduit_core::PreparationHostIdentity,
        resources: &[conduit_core::ResourceBinding],
        authorities: &[conduit_core::AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, BoundedFoldError> {
        self.active
            .as_ref()
            .ok_or(BoundedFoldError::InvalidLifecycle)?
            .admit_host_request(request, host, resources, authorities)
            .map_err(BoundedFoldError::Refused)
    }
    pub fn complete_host_call(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedFoldError> {
        self.active
            .as_mut()
            .ok_or(BoundedFoldError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedFoldError::Refused)
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
