//! Finite Pulse Back used by the production R1 source kernel.

use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub(super) const MAXIMUM_VALUES: usize = 16;
pub(super) const MAXIMUM_WAITS: usize = MAXIMUM_VALUES - 1;

pub(super) struct PulseBack {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl PulseBack {
    pub(super) fn new(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self {
            values,
            waits,
            next: 0,
            pending: None,
        }
    }

    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.values.capacity() + self.waits.capacity()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for PulseBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(expected) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
                || io.consume_host_completion().is_err()
            {
                return Self::fail(1);
            }
            self.pending = None;
        }

        let Some(value) = self.values.get(self.next).copied() else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), value).is_err() {
            return Self::fail(2);
        }
        self.next += 1;
        if self.next >= self.values.len() {
            return StepOutcome::Complete;
        }
        let Some(wait) = self.waits.get(self.next - 1).copied() else {
            return Self::fail(3);
        };
        let request = RequestId(self.next as u32);
        if io
            .request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(wait, 8).expect("planned wait is exactly eight bytes"),
            )
            .is_err()
        {
            return Self::fail(4);
        }
        self.pending = Some(request);
        StepOutcome::Progress
    }
}
