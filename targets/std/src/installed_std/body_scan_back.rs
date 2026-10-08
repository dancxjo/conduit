//! Bounded parent-kernel Step bridge for an exact prepared pure Todo scan.
//! The selected child kernels are prepared through receipt transfer before
//! Play; this Back never reconstructs a Todo state or dispatches a Host Call.

use conduit_composite::{
    BoundedScanActivationHost, BoundedScanAdmission, BoundedScanError, BoundedScanState,
    ScanChildSignReceipt,
};
use conduit_core::{ActivePlayId, PlannedScanActivation, ValuePayload};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub(super) struct BodyScanBack {
    scan: BoundedScanActivationHost,
    item: ValuePayload,
    output: ValuePayload,
    staged_output: bool,
    parent_bound: bool,
    child_steps: u32,
    maximum_child_steps: u32,
    output_completion_failed: bool,
    cancellation_failed: bool,
}

impl BodyScanBack {
    pub(super) fn prepare(
        scan: BoundedScanActivationHost,
        planned: &PlannedScanActivation,
    ) -> Result<Self, String> {
        if planned.item_input.abnormal_kind.is_some()
            || planned.output.abnormal_kind.is_some()
            || planned.accumulator_input.abnormal_kind.is_some()
            || planned.limits.maximum_items == 0
            || planned.limits.maximum_items > 64
            || planned.retained_item_bytes == 0
            || planned.retained_accumulator_bytes == 0
        {
            return Err(
                "installed pure Todo scan requires value-only bounded Flow terminals".into(),
            );
        }
        let expected_children = usize::from(planned.limits.maximum_items);
        let (ready, receipts) = scan.allocation_capacities();
        let (accumulator, candidate, queued, admission) = scan.storage_capacities();
        if ready != expected_children
            || receipts != expected_children
            || accumulator < planned.retained_accumulator_bytes as usize
            || candidate < planned.retained_accumulator_bytes as usize
            || queued < planned.retained_item_bytes as usize
            || admission < planned.retained_item_bytes as usize
        {
            return Err(
                "installed Todo scan child receipts or value scratch were not admitted".into(),
            );
        }
        let maximum_child_steps = u32::from(planned.limits.maximum_items)
            .checked_mul(128)
            .ok_or("installed scan child step budget overflow")?;
        Ok(Self {
            scan,
            item: ValuePayload {
                value_kind: planned.item_input.value_kind.clone(),
                encoded: Vec::with_capacity(planned.retained_item_bytes as usize),
            },
            output: ValuePayload {
                value_kind: planned.output.value_kind.clone(),
                encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
            },
            staged_output: false,
            parent_bound: false,
            child_steps: 0,
            maximum_child_steps,
            output_completion_failed: false,
            cancellation_failed: false,
        })
    }

    pub(super) fn bind_parent_play(&mut self, parent: &ActivePlayId) -> Result<(), String> {
        if self.parent_bound {
            return Err("installed scan was already bound to a Body Play".into());
        }
        self.scan
            .bind_parent_play(parent)
            .map_err(|error| format!("bind exact scan child Plays: {error:?}"))?;
        self.parent_bound = true;
        Ok(())
    }

    /// Snapshot after the sealed Play; every row retains its exact activation,
    /// child Plan, invocation, Host, and parent/child Play identities.
    pub(super) fn child_sign_receipts(
        &self,
    ) -> Result<Vec<ScanChildSignReceipt>, BoundedScanError> {
        self.scan.child_sign_receipts()
    }

    pub(super) fn cancellation_failed(&self) -> bool {
        self.cancellation_failed
    }

    pub(super) fn output_completion_failed(&self) -> bool {
        self.output_completion_failed
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        let (ready, receipts) = self.scan.allocation_capacities();
        let (accumulator, candidate, queued, admission) = self.scan.storage_capacities();
        ready
            + receipts
            + accumulator
            + candidate
            + queued
            + admission
            + self.item.encoded.capacity()
            + self.output.encoded.capacity()
    }

    fn stage_output<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        match self.scan.output_into(&mut self.output) {
            Ok(true) => {
                if io
                    .send_prepared(PortId(0), self.output.encoded.len() as u32)
                    .is_err()
                {
                    return fail(FailureCode::StorageExhausted, 1);
                }
                self.staged_output = true;
                StepOutcome::Progress
            }
            _ => fail(FailureCode::InvalidLifecycle, 2),
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for BodyScanBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.parent_bound
            || self.staged_output
            || self.output_completion_failed
            || self.cancellation_failed
        {
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        if self.scan.next_host_request().is_some() {
            return fail(FailureCode::HostCallDenied, 4);
        }
        if let Some(value) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(FailureCode::InvalidInput, 5);
            };
            if bytes.len() != value.byte_len as usize || bytes.len() > self.item.encoded.capacity()
            {
                return fail(FailureCode::InvalidInput, 6);
            }
            self.item.encoded.clear();
            self.item.encoded.extend_from_slice(bytes);
            match self.scan.admit(&self.item) {
                Ok(BoundedScanAdmission::Accepted) => {
                    if io.consume(PortId(0)).is_err() {
                        return fail(FailureCode::InvalidPort, 7);
                    }
                    return StepOutcome::Progress;
                }
                Ok(BoundedScanAdmission::Full) => {}
                Ok(BoundedScanAdmission::MaximumItemsExceeded { .. }) => {
                    return fail(FailureCode::WorkBudgetExhausted, 8)
                }
                Err(error) => return scan_failure(error),
            }
        } else if io.input_closed(PortId(0)) {
            match self.scan.close_input() {
                Ok(()) => {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(FailureCode::InvalidPort, 9);
                    }
                    return StepOutcome::Progress;
                }
                Err(error) => return scan_failure(error),
            }
        }
        if self.child_steps == self.maximum_child_steps {
            return fail(FailureCode::WorkBudgetExhausted, 10);
        }
        self.child_steps += 1;
        let state = match self.scan.step() {
            Ok(state) => state.clone(),
            Err(error) => return scan_failure(error),
        };
        if self.scan.next_host_request().is_some() {
            return fail(FailureCode::HostCallDenied, 11);
        }
        match state {
            BoundedScanState::OutputReady => self.stage_output(io),
            BoundedScanState::Complete => StepOutcome::Complete,
            BoundedScanState::Cancelled => fail(FailureCode::Cancelled, 12),
            BoundedScanState::Abnormal(_) => fail(FailureCode::InvalidLifecycle, 13),
            BoundedScanState::Idle => StepOutcome::Await,
            BoundedScanState::Active => {
                io.exhaust_fuel();
                StepOutcome::Yield
            }
        }
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged_output).then_some(&self.output.encoded)
    }

    fn step_committed(&mut self) {
        if self.staged_output {
            self.staged_output = false;
            if self.scan.complete_output().is_err() {
                self.output_completion_failed = true;
            }
        }
    }

    fn cancel(&mut self) {
        self.staged_output = false;
        if self.scan.cancel().is_err() {
            self.cancellation_failed = true;
        }
    }
}

fn scan_failure(error: BoundedScanError) -> StepOutcome {
    match error {
        BoundedScanError::PlannedContractMismatch => fail(FailureCode::InvalidInput, 14),
        BoundedScanError::InvalidLifecycle => fail(FailureCode::InvalidLifecycle, 15),
        BoundedScanError::MissingOutput => fail(FailureCode::InvalidLifecycle, 16),
        BoundedScanError::Refused(_) => fail(FailureCode::HostCallFailed, 17),
    }
}

fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
