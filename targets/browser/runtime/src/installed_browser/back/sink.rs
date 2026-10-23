//! Finite Host Call sink; completion, refusal and cancellation stay distinct.
use super::{fail, identity_exhausted};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) struct FaceBack {
    pub(super) maximum_input_bytes: u32,
    pub(super) next_request: u32,
    pub(super) pending: Option<RequestId>,
}

impl<const PORTS: usize> StepBack<PORTS> for FaceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return fail(3);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, None, None) => {
                    let Some(next) = self.next_request.checked_add(1) else {
                        return identity_exhausted(3);
                    };
                    io.consume_host_completion()
                        .expect("observed face completion");
                    self.pending = None;
                    self.next_request = next;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, None, None) => {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 3,
                    });
                }
                (HostCallDisposition::Failed | HostCallDisposition::Denied, None, Some(reason)) => {
                    return StepOutcome::Fail(reason)
                }
                _ => return fail(3),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return fail(3);
            }
            let input = match BoundedValueRef::new(value, self.maximum_input_bytes) {
                Ok(input) => input,
                Err(_) => return fail(3),
            };
            let request = RequestId(self.next_request);
            io.consume(PortId(0)).expect("present face input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("face Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed face input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}
