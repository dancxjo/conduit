//! Staging owns no publication truth until the kernel commits consumption.
use super::CommitOrder;
use alloc::{vec, vec::Vec};
use conduit_core::{BorrowedStructuredValueValidator, StructuredInfoRefusal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct RetainedSnapshot<'a> {
    pub sequence: u64,
    pub canonical: &'a [u8],
}

pub struct PreparedTypedRetention<'schema> {
    validator: BorrowedStructuredValueValidator<'schema>,
    current: Vec<u8>,
    pending: Vec<u8>,
    maximum_events: u32,
    events: u32,
    sequence: u64,
    order: CommitOrder,
    staged_length: usize,
    staged: bool,
    cancelled: bool,
}
impl<'schema> PreparedTypedRetention<'schema> {
    /// Borrows the exact immutable canonical schema (including flash-resident
    /// bytes). All value storage is allocated here; this grants no admission.
    /// Primitive representation validation does not discharge native-law witnesses.
    pub fn prepare(
        schema: &'schema [u8],
        maximum_value_bytes: usize,
        maximum_events: u32,
        order: CommitOrder,
    ) -> Result<Self, StructuredInfoRefusal> {
        let validator = BorrowedStructuredValueValidator::new(schema, maximum_value_bytes)?;
        Ok(Self {
            validator,
            current: Vec::with_capacity(maximum_value_bytes),
            pending: vec![0; maximum_value_bytes],
            maximum_events,
            events: 0,
            sequence: 0,
            order,
            staged_length: 0,
            staged: false,
            cancelled: false,
        })
    }
    pub fn snapshot(&self) -> Option<RetainedSnapshot<'_>> {
        (self.sequence != 0).then_some(RetainedSnapshot {
            sequence: self.sequence,
            canonical: &self.current,
        })
    }
    pub fn maximum_events(&self) -> u32 {
        self.maximum_events
    }
    /// Actual backing capacity of the two value buffers. The caller separately
    /// funds inline Self, shared CommitOrder, schema residence and allocator costs.
    pub fn value_storage_bytes(&self) -> Option<usize> {
        self.current.capacity().checked_add(self.pending.capacity())
    }
    fn discard_staged(&mut self) {
        if self.staged {
            self.staged = false;
            self.order.release();
        }
    }
}
impl<const P: usize> StepBack<P> for PreparedTypedRetention<'_> {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.discard_staged();
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 1);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    fail(FailureCode::InvalidLifecycle, 1)
                } else {
                    StepOutcome::Complete
                }
            } else {
                StepOutcome::Await
            };
        };
        if self.events >= self.maximum_events {
            return fail(FailureCode::InvalidInput, 2);
        }
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 3);
        };
        if bytes.len() != reference.byte_len as usize || self.validator.validate(bytes).is_err() {
            return fail(FailureCode::InvalidInput, 4);
        }
        if io.consume_fuel(1).is_err() {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        if !self.order.reserve() {
            return fail(FailureCode::InvalidInput, 5);
        }
        self.pending[..bytes.len()].copy_from_slice(bytes);
        if io.consume(PortId(0)).is_err() {
            self.order.release();
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        self.staged_length = bytes.len();
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if !self.cancelled && self.staged {
            self.staged = false;
            let length = self.staged_length;
            self.current.clear();
            self.current.extend_from_slice(&self.pending[..length]);
            self.sequence = self.order.commit();
            self.events += 1;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.discard_staged();
    }
}
impl Drop for PreparedTypedRetention<'_> {
    fn drop(&mut self) {
        self.discard_staged();
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
