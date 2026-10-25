//! Finite protocol Backs under the production scheduler. No credential store,
//! radio policy, retry or attachment identity is implemented here.
use conduit_kernel::{
    scheduler::{HostCallBack, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
#[derive(Clone, Copy)]
enum Phase {
    Ready,
    Pending,
    Done,
    Cancelled,
}
struct Endpoint {
    phase: Phase,
    staged: Option<Phase>,
    empty: Option<ValueRef>,
}
/// One credentials source, one join transformer or one attachment-sign sink.
/// Root must still select and admit exact contracts, routes and Host Calls.
pub struct NetworkControlBack(Back);
enum Back {
    Credentials(Endpoint),
    Join(HostCallBack),
    AttachmentSign(Endpoint),
}
impl NetworkControlBack {
    /// Root reserves an exactly empty input value before Play. The callback
    /// supplies the private request only after its independent admission.
    pub fn credentials(empty: ValueRef) -> Result<Self, Failure> {
        if empty.byte_len != 0 {
            return Err(failure(1));
        }
        Ok(Self(Back::Credentials(Endpoint {
            phase: Phase::Ready,
            staged: None,
            empty: Some(empty),
        })))
    }
    pub fn join() -> Self {
        Self(Back::Join(HostCallBack::new(
            conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
        )))
    }
    pub fn attachment_sign() -> Self {
        Self(Back::AttachmentSign(Endpoint {
            phase: Phase::Ready,
            staged: None,
            empty: None,
        }))
    }
}
impl<const P: usize> StepBack<P> for NetworkControlBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match &mut self.0 {
            Back::Join(back) => back.step(io, input),
            Back::Credentials(endpoint) => endpoint.source(io),
            Back::AttachmentSign(endpoint) => endpoint.sink(io),
        }
    }
    fn step_committed(&mut self) {
        match &mut self.0 {
            Back::Join(back) => <HostCallBack as StepBack<P>>::step_committed(back),
            Back::Credentials(endpoint) | Back::AttachmentSign(endpoint) => {
                if let Some(phase) = endpoint.staged.take() {
                    endpoint.phase = phase;
                }
            }
        }
    }
    fn cancel(&mut self) {
        match &mut self.0 {
            Back::Join(back) => <HostCallBack as StepBack<P>>::cancel(back),
            Back::Credentials(endpoint) | Back::AttachmentSign(endpoint) => {
                endpoint.staged = None;
                endpoint.phase = Phase::Cancelled;
            }
        }
    }
}
impl Endpoint {
    fn source<const P: usize>(&mut self, io: &mut StepIo<P>) -> StepOutcome {
        self.staged = None;
        match self.phase {
            Phase::Cancelled => cancelled(),
            Phase::Done => StepOutcome::Complete,
            Phase::Ready => {
                let input = BoundedValueRef::new(self.empty.expect("prepared empty input"), 0)
                    .expect("exactly empty");
                if io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
                {
                    return fail(2);
                }
                self.staged = Some(Phase::Pending);
                StepOutcome::Progress
            }
            Phase::Pending => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != RequestId(0) {
                    return fail(3);
                }
                if let Some(result) = failed(outcome) {
                    return result;
                }
                let Some(output) = outcome.output else {
                    return fail(4);
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err()
                    || io.send(PortId(0), output.value).is_err()
                {
                    return fail(5);
                }
                self.staged = Some(Phase::Done);
                StepOutcome::Progress
            }
        }
    }
    fn sink<const P: usize>(&mut self, io: &mut StepIo<P>) -> StepOutcome {
        self.staged = None;
        match self.phase {
            Phase::Cancelled => cancelled(),
            Phase::Done => {
                if io.input(PortId(0)).is_some() {
                    return fail(6);
                }
                close(io)
            }
            Phase::Ready => {
                let Some(value) = io.input(PortId(0)) else {
                    return close(io);
                };
                let Ok(input) = BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES)
                else {
                    return fail(7);
                };
                if io.consume(PortId(0)).is_err()
                    || io
                        .request_host_call(RequestId(0), HostCallId(0), input)
                        .is_err()
                {
                    return fail(8);
                }
                self.staged = Some(Phase::Pending);
                StepOutcome::Progress
            }
            Phase::Pending => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != RequestId(0) {
                    return fail(9);
                }
                if let Some(result) = failed(outcome) {
                    return result;
                }
                if outcome.output.is_some() || io.consume_host_completion().is_err() {
                    return fail(10);
                }
                self.staged = Some(Phase::Done);
                StepOutcome::Progress
            }
        }
    }
}
fn failed(outcome: conduit_kernel::HostCallOutcome) -> Option<StepOutcome> {
    match (outcome.disposition, outcome.output, outcome.failure) {
        (HostCallDisposition::Completed, _, None) => None,
        (HostCallDisposition::Failed | HostCallDisposition::Denied, None, Some(error)) => {
            Some(StepOutcome::Fail(error))
        }
        (HostCallDisposition::Cancelled, None, None) => Some(cancelled()),
        _ => Some(fail(11)),
    }
}
fn close<const P: usize>(io: &mut StepIo<P>) -> StepOutcome {
    if io.input_abnormal(PortId(0)).is_some() {
        return fail(12);
    }
    if !io.input_closed(PortId(0)) {
        return StepOutcome::Await;
    }
    if io.consume_closed(PortId(0)).is_err() {
        fail(13)
    } else {
        StepOutcome::Complete
    }
}
const fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 1900 + detail,
    }
}
const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(failure(detail))
}
const fn cancelled() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::Cancelled,
        detail: 1900,
    })
}
