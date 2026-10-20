//! Parent-kernel bridge for finite pure Value activation through an ordinary Each.
use crate::{
    BoundedActivationAdmission, BoundedActivationHost, BoundedActivationState, ScanChildSignReceipt,
};
use alloc::{format, string::String, vec::Vec};
use conduit_core::{ActivePlayId, PlannedActivation, ValuePayload};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct KernelEachBack {
    activation: BoundedActivationHost,
    input: ValuePayload,
    output: ValuePayload,
    staged: Option<u64>,
    parent_bound: bool,
    input_closed: bool,
    child_steps: u32,
    maximum_child_steps: u32,
    output_completion_failed: bool,
    cancellation_failed: bool,
}
impl KernelEachBack {
    pub fn prepare(
        activation: BoundedActivationHost,
        planned: &PlannedActivation,
        maximum_items: u16,
        maximum_child_steps: u32,
    ) -> Result<Self, String> {
        if planned.input.abnormal_kind.is_some()
            || planned.output.abnormal_kind.is_some()
            || planned.limits.maximum_items == 0
            || planned.limits.maximum_items > maximum_items
            || maximum_child_steps == 0
            || !activation.has_admitted_children()
        {
            return Err("pure Each requires exact finite admitted Value children".into());
        }
        let front = |id: &conduit_core::PortId, direction| {
            planned
                .selected_plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.fore_ports)
                .find(|port| &port.front_port_id == id && port.direction == direction)
                .map(|port| port.byte_capacity)
                .ok_or("Each child Fore is absent")
        };
        let input = front(
            &planned.input.front_port_id,
            conduit_core::PortDirection::Input,
        )?;
        let output = front(
            &planned.output.front_port_id,
            conduit_core::PortDirection::Output,
        )?;
        if input == 0 || output == 0 || output > planned.limits.maximum_queue_bytes {
            return Err("Each value storage was not admitted".into());
        }
        Ok(Self {
            activation,
            input: ValuePayload {
                value_kind: planned.input.value_kind.clone(),
                encoded: Vec::with_capacity(input as usize),
            },
            output: ValuePayload {
                value_kind: planned.output.value_kind.clone(),
                encoded: Vec::with_capacity(output as usize),
            },
            staged: None,
            parent_bound: false,
            input_closed: false,
            child_steps: 0,
            maximum_child_steps,
            output_completion_failed: false,
            cancellation_failed: false,
        })
    }
    pub fn bind_parent_play(&mut self, parent: &ActivePlayId) -> Result<(), String> {
        self.activation
            .bind_parent_play(parent)
            .map_err(|e| format!("Each parent binding: {e:?}"))?;
        self.parent_bound = true;
        Ok(())
    }
    pub fn allocation_capacity(&self) -> usize {
        self.input.encoded.capacity() + self.output.encoded.capacity()
    }
    pub fn signs(
        &self,
    ) -> alloc::collections::BTreeMap<conduit_core::HostId, Vec<conduit_kernel::KernelEvent>> {
        self.activation.signs()
    }
    pub fn child_sign_receipts(&self) -> Result<Vec<ScanChildSignReceipt>, String> {
        self.activation
            .child_sign_receipts()
            .map_err(|e| format!("Each child evidence: {e:?}"))
    }
    pub fn cancellation_failed(&self) -> bool {
        self.cancellation_failed
    }
    pub fn output_completion_failed(&self) -> bool {
        self.output_completion_failed
    }
    fn stage_output<const P: usize>(&mut self, io: &mut StepIo<P>) -> Option<StepOutcome> {
        let value = match self.activation.output() {
            Ok(Some((sequence, value))) => (sequence, value),
            Ok(None) => return None,
            Err(_) => return Some(fail(FailureCode::InvalidLifecycle, 20)),
        };
        if !io.output_ready(PortId(0)) {
            return Some(StepOutcome::Await);
        }
        if value.1.value_kind != self.output.value_kind
            || value.1.encoded.len() > self.output.encoded.capacity()
        {
            return Some(fail(FailureCode::StorageExhausted, 21));
        }
        self.output.encoded.clear();
        self.output.encoded.extend_from_slice(&value.1.encoded);
        if io
            .send_prepared(PortId(0), self.output.encoded.len() as u32)
            .is_err()
        {
            return Some(fail(FailureCode::InvalidPort, 22));
        }
        self.staged = Some(value.0);
        Some(StepOutcome::Progress)
    }
}
impl<const P: usize> StepBack<P> for KernelEachBack {
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        if !self.parent_bound
            || self.output_completion_failed
            || self.cancellation_failed
            || self.staged.is_some()
        {
            return fail(FailureCode::InvalidLifecycle, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 2);
        }
        let phase = self.activation.state().clone();
        if matches!(
            phase,
            BoundedActivationState::Idle | BoundedActivationState::Succeeded { .. }
        ) {
            if let Some(value) = io.input(PortId(0)) {
                let sequence = match phase {
                    BoundedActivationState::Idle => 0,
                    BoundedActivationState::Succeeded { sequence } => sequence + 1,
                    _ => unreachable!(),
                };
                if value.byte_len as usize > self.input.encoded.capacity() {
                    return fail(FailureCode::StorageExhausted, 3);
                }
                let Some(bytes) = inputs.input(PortId(0)) else {
                    return fail(FailureCode::InvalidInput, 4);
                };
                self.input.encoded.clear();
                self.input.encoded.extend_from_slice(bytes);
                match self.activation.activate(sequence, &self.input) {
                    Ok(BoundedActivationAdmission::Accepted { .. }) => {}
                    Ok(BoundedActivationAdmission::Full { .. }) => return StepOutcome::Await,
                    Ok(BoundedActivationAdmission::MaximumItemsExceeded { .. }) => {
                        return fail(FailureCode::WorkBudgetExhausted, 5)
                    }
                    Err(_) => return fail(FailureCode::InvalidLifecycle, 6),
                }
                if io.consume(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidPort, 7);
                }
                return StepOutcome::Progress;
            }
        }
        if !self.input_closed && io.input(PortId(0)).is_none() && io.input_closed(PortId(0)) {
            if self.activation.close_input().is_err() || io.consume_closed(PortId(0)).is_err() {
                return fail(FailureCode::InvalidLifecycle, 8);
            }
            self.input_closed = true;
            return StepOutcome::Progress;
        }
        if matches!(phase, BoundedActivationState::Active { .. }) {
            if let Some(outcome) = self.stage_output(io) {
                return outcome;
            }
        } else if !self.input_closed {
            return StepOutcome::Await;
        }
        if self.child_steps == self.maximum_child_steps {
            return fail(FailureCode::WorkBudgetExhausted, 9);
        }
        self.child_steps += 1;
        let state = match self.activation.step() {
            Ok(state) => state.clone(),
            Err(_) => return fail(FailureCode::InvalidLifecycle, 10),
        };
        if self.activation.next_host_request().is_some() {
            return fail(FailureCode::HostCallDenied, 11);
        }
        match state {
            BoundedActivationState::Active { .. } => {
                if let Some(outcome) = self.stage_output(io) {
                    outcome
                } else {
                    io.exhaust_fuel();
                    StepOutcome::Yield
                }
            }
            BoundedActivationState::Succeeded { .. } => StepOutcome::Progress,
            BoundedActivationState::Drained if self.input_closed => StepOutcome::Complete,
            BoundedActivationState::Cancelled { .. } => fail(FailureCode::Cancelled, 12),
            _ => fail(FailureCode::InvalidLifecycle, 13),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged.is_some()).then_some(self.output.encoded.as_slice())
    }
    fn step_committed(&mut self) {
        if let Some(sequence) = self.staged.take() {
            if self.activation.complete_output(sequence).is_err() {
                self.output_completion_failed = true;
            }
        }
    }
    fn cancel(&mut self) {
        self.staged = None;
        if self.activation.cancel().is_err() {
            self.cancellation_failed = true;
        }
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
