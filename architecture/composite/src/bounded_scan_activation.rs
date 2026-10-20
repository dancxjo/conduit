//! Kernel-backed execution of one exact planned bounded scan.

use crate::prelude::*;
#[cfg_attr(not(feature = "fixture-registry-preparation"), allow(unused_imports))]
use crate::{
    AdmittedKernelCompositeHostRequest, KernelCompositeDefinition, KernelCompositeError,
    KernelCompositeHost, KernelCompositeHostRequest, KernelCompositeStatus,
    KernelCompositeTerminal, KernelOperationRegistry,
};
#[cfg_attr(not(feature = "fixture-registry-preparation"), allow(unused_imports))]
use conduit_core::{
    verify_plan, ActivePlayId, PlannedActivationEffectMultiplicity, PlannedActivationFront,
    PlannedScanAbnormalPolicy, PlannedScanActivation, PlannedScanCancellationPolicy,
    PlannedScanTerminalPolicy, PortDirection, PortTemporal, ValuePayload,
};
use conduit_kernel::HostCallOutcome;
mod parent_play;
mod reuse_pool;
pub use parent_play::ScanChildSignReceipt;
use reuse_pool::ReusableScanPool;

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
    reusable: Option<ReusableScanPool>,
    receipts: Vec<KernelCompositeHost>,
    accumulator: ValuePayload,
    queued: ValuePayload,
    queued_ready: bool,
    admission: ValuePayload,
    active: Option<KernelCompositeHost>,
    candidate: ValuePayload,
    abnormal_buffer: Option<ValuePayload>,
    candidate_ready: bool,
    output_ready: bool,
    closing: bool,
    state: BoundedScanState,
    parent_play: Option<ActivePlayId>,
}

impl BoundedScanActivationHost {
    #[cfg(feature = "fixture-registry-preparation")]
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
        Self::prepare_with_ready(planned, definition, ready)
    }

    pub(crate) fn prepare_with_ready(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedScanError> {
        Self::prepare_pool(planned, definition, ready, false)
    }
    pub(crate) fn prepare_reusable_with_ready(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedScanError> {
        Self::prepare_pool(planned, definition, ready, true)
    }
    fn prepare_pool(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        mut ready: Vec<KernelCompositeHost>,
        reusable: bool,
    ) -> Result<Self, BoundedScanError> {
        let expected = if reusable {
            1
        } else {
            usize::from(planned.limits.maximum_items)
        };
        if planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || ready.len() != expected
            || ready.capacity() != expected
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
        let reusable = if reusable {
            let host = ready.pop().unwrap();
            ready = Vec::new();
            Some(ReusableScanPool::new(host, planned.limits.maximum_items)?)
        } else {
            None
        };
        let abnormal_buffer = planned
            .output
            .abnormal_kind
            .clone()
            .map(|value_kind| ValuePayload {
                value_kind,
                encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
            });
        Ok(Self {
            planned: planned.clone(),
            ready,
            receipts: Vec::with_capacity(if reusable.is_some() { 0 } else { maximum_items }),
            reusable,
            accumulator,
            queued,
            queued_ready: false,
            admission,
            active: None,
            candidate,
            abnormal_buffer,
            candidate_ready: false,
            output_ready: false,
            closing: false,
            state: BoundedScanState::Idle,
            parent_play: None,
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
            if self.remaining_children() == 0 {
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
        if self.remaining_children() == 0 {
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
        let reused = self.reusable.is_some();
        let mut child = if let Some(pool) = &mut self.reusable {
            pool.take()?
        } else {
            self.ready.pop().ok_or(BoundedScanError::InvalidLifecycle)?
        };
        let sequence = if reused {
            u64::from(
                child
                    .parent_play_binding()
                    .ok_or(BoundedScanError::InvalidLifecycle)?
                    .1,
            )
        } else {
            0
        };
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
                    .admit_input(port, sequence, value)
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
        self.state = BoundedScanState::Abnormal(terminal);
        let cancellation = self
            .active
            .as_mut()
            .map(KernelCompositeHost::cancel)
            .transpose();
        if let Some(terminated) = self.active.take() {
            self.retire_child(terminated);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.output_ready = false;
        cancellation.map(|_| ()).map_err(BoundedScanError::Refused)
    }

    pub fn step(&mut self) -> Result<&BoundedScanState, BoundedScanError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(&self.state);
        };
        if !self.candidate_ready {
            if let Some(sequence) = active
                .output_into(&self.planned.output.front_port_id, &mut self.candidate)
                .map_err(BoundedScanError::Refused)?
            {
                active
                    .complete_output(&self.planned.output.front_port_id, sequence)
                    .map_err(BoundedScanError::Refused)?;
                self.candidate_ready = true;
            }
        }
        match active.step().map_err(BoundedScanError::Refused)? {
            KernelCompositeStatus::Active => {}
            KernelCompositeStatus::Complete => {
                let terminal_buffer = self.abnormal_buffer.as_mut().unwrap_or(&mut self.candidate);
                let terminal = active
                    .output_terminal_into(&self.planned.output.front_port_id, terminal_buffer)
                    .map_err(BoundedScanError::Refused)?;
                if let Some(KernelCompositeTerminal::Abnormal) = terminal {
                    let terminal = self
                        .abnormal_buffer
                        .take()
                        .ok_or(BoundedScanError::PlannedContractMismatch)?;
                    let completed = self.active.take().expect("active scan child was borrowed");
                    self.retire_child(completed);
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
                // Keep recurrence staged until the containing output is accepted.
                let completed = self.active.take().expect("active scan child was borrowed");
                self.retire_child(completed);
                self.output_ready = true;
                self.state = BoundedScanState::OutputReady;
            }
            KernelCompositeStatus::Cancelled => {
                let completed = self.active.take().expect("active scan child was borrowed");
                self.retire_child(completed);
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
        if destination.value_kind != self.candidate.value_kind
            || destination.encoded.capacity() < self.candidate.encoded.len()
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        destination.encoded.clear();
        destination
            .encoded
            .extend_from_slice(&self.candidate.encoded);
        Ok(true)
    }

    pub fn complete_output(&mut self) -> Result<(), BoundedScanError> {
        if !self.output_ready {
            return Err(BoundedScanError::InvalidLifecycle);
        }
        core::mem::swap(&mut self.accumulator, &mut self.candidate);
        self.candidate.encoded.clear();
        self.candidate_ready = false;
        self.output_ready = false;
        if let Some(pool) = &mut self.reusable {
            pool.advance()?;
        }
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
        self.state = BoundedScanState::Cancelled;
        let cancellation = self
            .active
            .as_mut()
            .map(KernelCompositeHost::cancel)
            .transpose();
        if let Some(cancelled) = self.active.take() {
            self.retire_child(cancelled);
        }
        self.queued_ready = false;
        self.candidate_ready = false;
        self.output_ready = false;
        cancellation.map(|_| ()).map_err(BoundedScanError::Refused)
    }
    pub fn allocation_capacities(&self) -> (usize, usize) {
        (self.ready.capacity(), self.receipts.capacity())
    }
    /// Read-only scheduling phase for an installed parent Back's admitted
    /// child-Step budget. Output pressure must not consume a child Step.
    pub fn has_active_child(&self) -> bool {
        self.active.is_some()
    }

    pub fn has_pending_output(&self) -> bool {
        self.output_ready
    }
    /// Resolve a failure index against the exact prepared child identities.
    pub fn child_identity(&self, child: usize) -> Option<&conduit_core::HostId> {
        self.observed_child()
            .and_then(|host| host.child_identity(child))
    }

    pub fn last_cancellation_failures(
        &self,
    ) -> &[(usize, conduit_kernel::scheduler::SchedulerError)] {
        self.observed_child()
            .map_or(&[], KernelCompositeHost::cancellation_failures)
    }

    /// Receipt-correlated child Signs for face after the bounded Play.
    /// The selected Plan and prepared child identities remain the source of
    /// provenance; this snapshot never grants a new Host route.
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
    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&crate::KernelCompositeHostCallObligation, BoundedScanError> {
        self.active
            .as_ref()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .host_request_obligation(request)
            .map_err(BoundedScanError::Refused)
    }
    pub fn host_request_view(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<crate::KernelCompositeHostRequestView<'_>, BoundedScanError> {
        self.active
            .as_ref()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .host_request_view(request)
            .map_err(BoundedScanError::Refused)
    }
    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &conduit_core::PreparationHostIdentity,
        resources: &[conduit_core::ResourceBinding],
        authorities: &[conduit_core::AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, BoundedScanError> {
        self.active
            .as_ref()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .admit_host_request(request, host, resources, authorities)
            .map_err(BoundedScanError::Refused)
    }
    pub fn complete_host_call(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), BoundedScanError> {
        self.active
            .as_mut()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .complete_host_call(request, outcome)
            .map_err(BoundedScanError::Refused)
    }
    pub fn host_request_input(
        &self,
        request: &AdmittedKernelCompositeHostRequest,
    ) -> Result<&[u8], BoundedScanError> {
        self.active
            .as_ref()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .host_request_input(request)
            .map_err(BoundedScanError::Refused)
    }
    pub fn complete_host_call_bytes(
        &mut self,
        request: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), BoundedScanError> {
        self.active
            .as_mut()
            .ok_or(BoundedScanError::InvalidLifecycle)?
            .complete_host_call_bytes(request, bytes)
            .map_err(BoundedScanError::Refused)
    }
}

#[cfg(feature = "fixture-registry-preparation")]
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
