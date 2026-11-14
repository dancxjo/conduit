//! One cooperative poll of the original admitted deadline owner. No timer loop,
//! second scheduler, retry, fabricated time or transport acknowledgment.
use super::*;
use conduit_monotonic_wait::clock_at::{
    contract::CLOCK_MAXIMUM_BYTES,
    owner::{ClockCallRefusal, MonotonicClockHostCall, MonotonicDeadlineProvider},
};

/// Root must lend this service only the original Started kernel matching its
/// bound owner. All fields are inline; the codec/ticket were funded before Start.
/// After an error, retire this service and kernel; never replay a started call.
pub struct Service<'a, A> {
    owner: &'a mut MonotonicClockHostCall<A>,
    pending: Option<HostCallRequest>,
}
impl<'a, A: MonotonicDeadlineProvider> Service<'a, A> {
    pub fn new(owner: &'a mut MonotonicClockHostCall<A>) -> Self {
        Self {
            owner,
            pending: None,
        }
    }
    pub fn node(&self) -> NodeId {
        self.owner.node()
    }
    pub fn bound_owner(&self) -> &MonotonicClockHostCall<A> {
        self.owner
    }
    pub fn revoke(&mut self) -> Result<(), ClockCallRefusal> {
        self.owner.revoke(self.owner.node(), HostCallId(0))?;
        self.pending = None;
        Ok(())
    }
    /// Preflight completion storage and Signs before starting or polling. Each
    /// call polls the physical provider at most once; pressure leaves the same
    /// request/lease untouched. Borrowing the kernel spans only this bounded step.
    pub fn dispatch<
        D: StepBack<P>,
        L: SignSink,
        V: ValueStorage,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
    >(
        &mut self,
        kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    ) -> Result<Dispatch, SchedulerError> {
        if self.pending.is_none() {
            let node = self.owner.node();
            let mut available = false;
            kernel.next_host_request_matching(|r| {
                available |= r.node == node && r.call == HostCallId(0);
                false
            });
            if !available {
                return Ok(Dispatch::Idle);
            }
        }
        if let Err(error) = kernel.preflight_host_result_storage(CLOCK_MAXIMUM_BYTES) {
            return match error {
                SchedulerError::Storage(
                    StorageError::ItemCapacityExceeded | StorageError::ByteCapacityExceeded,
                )
                | SchedulerError::Sign(
                    SignError::ItemCapacityExceeded | SignError::ByteCapacityExceeded,
                ) => Ok(Dispatch::Pressure),
                error => Err(error),
            };
        }
        if self.pending.is_none() {
            let Some(request) = kernel.next_host_request_matching(|r| {
                r.node == self.owner.node() && r.call == HostCallId(0)
            }) else {
                return Ok(Dispatch::Idle);
            };
            let input = kernel.host_value(request.input.value)?;
            if let Err(error) = self
                .owner
                .start(request.node, request.call, request.request, input)
            {
                return complete(kernel, request, failed(error));
            }
            self.pending = Some(request);
        }
        let request = self.pending.expect("original dispatched deadline");
        let result = self.owner.poll(request.node, request.call, request.request);
        let completion = match result {
            Ok(None) => return Ok(Dispatch::Idle),
            Ok(Some(bytes)) => {
                let value = kernel.store_host_value(bytes)?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, CLOCK_MAXIMUM_BYTES)
                            .expect("native clock codec extent"),
                    ),
                    failure: None,
                }
            }
            Err(error) => failed(error),
        };
        let dispatch = complete(kernel, request, completion)?;
        self.pending = None;
        Ok(dispatch)
    }
}
fn failed(error: ClockCallRefusal) -> HostCallOutcome {
    match error {
        ClockCallRefusal::Capability(_)
        | ClockCallRefusal::Possession
        | ClockCallRefusal::WrongBinding => denied(),
        ClockCallRefusal::Cancelled => HostCallOutcome {
            disposition: HostCallDisposition::Cancelled,
            output: None,
            failure: Some(Failure {
                code: FailureCode::Cancelled,
                detail: 0,
            }),
        },
        _ => outcome(false),
    }
}
fn complete<
    D: StepBack<P>,
    L: SignSink,
    V: ValueStorage,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize,
>(
    kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    request: HostCallRequest,
    mut outcome: HostCallOutcome,
) -> Result<Dispatch, SchedulerError> {
    if let Err(error) = kernel.complete_host_call(request.node, request.request, outcome) {
        if let Some(output) = outcome.output.take() {
            let _ = kernel.discard_host_value(output.value);
        }
        return Err(error);
    }
    Ok(Dispatch::Completed { request, outcome })
}
