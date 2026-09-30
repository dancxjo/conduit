//! Kernel-backed execution of one exact planned bounded fold.

use crate::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost,
    KernelCompositeHostRequest, KernelCompositeStatus, KernelCompositeTerminal,
    KernelOperationRegistry,
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

pub struct BoundedFoldActivationHost {
    planned: PlannedFoldActivation,
    definition: KernelCompositeDefinition,
    registry: KernelOperationRegistry,
    accumulator: ValuePayload,
    queued: Option<ValuePayload>,
    active: Option<KernelCompositeHost>,
    candidate: Option<ValuePayload>,
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
        let accumulator = ValuePayload {
            value_kind: planned.accumulator_input.value_kind.clone(),
            encoded: planned.initial_accumulator.clone(),
        };
        KernelCompositeHost::prepare(definition.clone(), registry)
            .map_err(BoundedFoldError::Refused)?;
        Ok(Self {
            planned: planned.clone(),
            definition,
            registry: registry.clone(),
            accumulator,
            queued: None,
            active: None,
            candidate: None,
            closing: false,
            state: BoundedFoldState::Idle,
        })
    }

    pub fn admit(&mut self, item: &ValuePayload) -> Result<bool, BoundedFoldError> {
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
            if self.queued.is_some() {
                return Ok(false);
            }
            self.queued = Some(item.clone());
            return Ok(true);
        }
        self.start(item.clone())?;
        Ok(true)
    }

    fn start(&mut self, item: ValuePayload) -> Result<(), BoundedFoldError> {
        let mut child = KernelCompositeHost::prepare(self.definition.clone(), &self.registry)
            .map_err(BoundedFoldError::Refused)?;
        child.start().map_err(BoundedFoldError::Refused)?;
        for (port, value) in [
            (
                &self.planned.accumulator_input.front_port_id,
                &self.accumulator,
            ),
            (&self.planned.item_input.front_port_id, &item),
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
        self.candidate = None;
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
        self.active = None;
        self.queued = None;
        self.candidate = None;
        self.state = BoundedFoldState::Abnormal(terminal);
        Ok(())
    }

    pub fn step(&mut self) -> Result<&BoundedFoldState, BoundedFoldError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(&self.state);
        };
        if self.candidate.is_none() {
            if let Some((_, value)) = active
                .output(&self.planned.output.front_port_id)
                .map_err(BoundedFoldError::Refused)?
            {
                active
                    .complete_output(&self.planned.output.front_port_id, 0)
                    .map_err(BoundedFoldError::Refused)?;
                self.candidate = Some(value);
            }
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
                    self.active = None;
                    self.queued = None;
                    self.candidate = None;
                    self.state = BoundedFoldState::Abnormal(terminal);
                    return Ok(&self.state);
                }
                if terminal != Some(KernelCompositeTerminal::Normal) {
                    return Err(BoundedFoldError::MissingOutput);
                }
                let candidate = self
                    .candidate
                    .take()
                    .ok_or(BoundedFoldError::MissingOutput)?;
                if candidate.value_kind != self.planned.output.value_kind
                    || candidate.encoded.len() > self.planned.retained_accumulator_bytes as usize
                {
                    return Err(BoundedFoldError::PlannedContractMismatch);
                }
                self.accumulator = candidate;
                self.active = None;
                if let Some(item) = self.queued.take() {
                    self.start(item)?;
                } else if self.closing {
                    self.state = BoundedFoldState::FinalReady;
                } else {
                    self.state = BoundedFoldState::Idle;
                }
            }
            KernelCompositeStatus::Cancelled => {
                self.active = None;
                self.queued = None;
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
        self.active = None;
        self.queued = None;
        self.candidate = None;
        self.state = BoundedFoldState::Cancelled;
        Ok(())
    }
    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        self.active.as_mut()?.next_host_request()
    }
    pub fn complete_host_call(
        &mut self,
        request: &KernelCompositeHostRequest,
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
