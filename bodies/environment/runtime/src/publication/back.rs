//! One retained canonical value. A staged copy becomes visible only after the
//! production kernel commits the input consumption. Readers borrow that value.
use super::{PublicationChannel, PublicationContract, PublicationSequence};
use alloc::{vec, vec::Vec};
use conduit_core::{PreparedStructuredValueValidator, StructuredInfoRefusal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct PublicationSnapshot<'a> {
    pub channel: PublicationChannel,
    pub sequence: u64,
    pub canonical: &'a [u8],
}
pub struct PublicationBack {
    channel: PublicationChannel,
    validator: PreparedStructuredValueValidator,
    current: Vec<u8>,
    pending: Vec<u8>,
    maximum_events: u32,
    sequence: u64,
    events: u32,
    order: PublicationSequence,
    length: usize,
    staged: bool,
    cancelled: bool,
}
impl PublicationBack {
    pub fn prepare(
        contract: &PublicationContract,
        maximum_events: u32,
        order: PublicationSequence,
    ) -> Result<Self, StructuredInfoRefusal> {
        // Every valid canonical value fits this Type-derived bound. Retain the
        // channel ceiling too; tightening storage does not narrow its domain.
        let bytes = crate::canonical_storage::bytes(
            contract.value_type(),
            contract.channel().maximum_bytes(),
        )?;
        Ok(Self {
            channel: contract.channel(),
            validator: PreparedStructuredValueValidator::new(contract.value_type(), bytes)?,
            current: Vec::with_capacity(bytes),
            pending: vec![0; bytes],
            maximum_events,
            sequence: 0,
            events: 0,
            order,
            length: 0,
            staged: false,
            cancelled: false,
        })
    }
    pub fn snapshot(&self) -> Option<PublicationSnapshot<'_>> {
        (self.sequence != 0).then_some(PublicationSnapshot {
            channel: self.channel,
            sequence: self.sequence,
            canonical: &self.current,
        })
    }
    fn discard_staged(&mut self) {
        if self.staged {
            self.order.release();
            self.staged = false;
        }
    }
    pub fn maximum_events(&self) -> u32 {
        self.maximum_events
    }
}
impl<const P: usize> StepBack<P> for PublicationBack {
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
        self.length = bytes.len();
        if io.consume(PortId(0)).is_err() {
            self.order.release();
            return fail(FailureCode::InvalidLifecycle, 3);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged && !self.cancelled {
            self.current.clear();
            self.current.extend_from_slice(&self.pending[..self.length]);
            self.sequence = self.order.commit();
            self.events += 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.discard_staged();
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl Drop for PublicationBack {
    fn drop(&mut self) {
        self.discard_staged();
    }
}
