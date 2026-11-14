//! Native request service for the original admitted kernel, never a scheduler.
//!
//! Pulse retains the exclusive kernel loan over its bounded provider future:
//! result/Sign preflight cannot be invalidated by another actor. Dropping that
//! future first drops the provider's pending lease, fencing unfinished work.
//! The supervisor must drop all provider futures before calling kernel.cancel.
//! Clock waits instead retain an exact request and eight owned input bytes;
//! they release the kernel loan, then recheck it when publishing completion.
use crate::execution::StoredExecutionKernel;
#[path = "native_deadline.rs"]
pub mod deadline;
#[path = "native_write_failure.rs"]
pub mod write_failure;
use conduit_kernel::{scheduler::*, *};
use conduit_monotonic_wait::{MonotonicWaitHostCall, MonotonicWaitProvider, WaitCallRefusal};
use conduit_pulse_capture::owner::{PulseCallRefusal, PulseHostCall, PulseReadProvider};
use conduit_tm1637_wire::{
    owner::{WriteCallRefusal, WriteHostCall},
    OpenDrain,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    Idle,
    Pressure,
    Completed {
        request: HostCallRequest,
        outcome: HostCallOutcome,
    },
}
fn outcome(success: bool) -> HostCallOutcome {
    HostCallOutcome {
        disposition: if success {
            HostCallDisposition::Completed
        } else {
            HostCallDisposition::Failed
        },
        output: None,
        failure: if success {
            None
        } else {
            Some(Failure {
                code: FailureCode::HostCallFailed,
                detail: 0,
            })
        },
    }
}
fn denied() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Denied,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostCallDenied,
            detail: 0,
        }),
    }
}
fn pressure(error: SchedulerError) -> Result<Dispatch, SchedulerError> {
    match error {
        SchedulerError::Storage(
            StorageError::ItemCapacityExceeded | StorageError::ByteCapacityExceeded,
        ) => Ok(Dispatch::Pressure),
        other => Err(other),
    }
}
/// Services at most one actual Pulse request; never retries a physical effect.
pub async fn pulse<
    D: StepBack<P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize,
    L: SignSink,
    V: ValueStorage,
    A: PulseReadProvider,
>(
    kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    call: &mut PulseHostCall<A>,
) -> Result<Dispatch, SchedulerError> {
    let room = kernel.preflight_host_result_storage(1024);
    let mut pending = false;
    let Some(request) = kernel.next_host_request_matching(|r| {
        if r.node != call.node() {
            return false;
        }
        pending = true;
        room.is_ok()
    }) else {
        return if pending {
            pressure(room.unwrap_err())
        } else {
            Ok(Dispatch::Idle)
        };
    };
    let mut completion = {
        let input = kernel.host_value(request.input.value)?;
        match call
            .invoke(request.node, request.call, request.request, input)
            .await
        {
            Ok(bytes) => {
                let value = kernel.store_host_value(bytes)?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, 1024).expect("preflighted Pulse extent"),
                    ),
                    failure: None,
                }
            }
            Err(
                PulseCallRefusal::Capability(_)
                | PulseCallRefusal::Possession
                | PulseCallRefusal::WrongBinding,
            ) => denied(),
            Err(_) => outcome(false),
        }
    };
    if let Err(error) = kernel.complete_host_call(request.node, request.request, completion) {
        if let Some(output) = completion.output.take() {
            let _ = kernel.discard_host_value(output.value);
        }
        return Err(error);
    }
    Ok(Dispatch::Completed {
        request,
        outcome: completion,
    })
}
/// Services one synchronous TM transaction. Cancellation belongs to the actual
/// admitted Play; owner transport loss is never a cancellation signal.
pub fn write<
    D: StepBack<P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize,
    L: SignSink,
    V: ValueStorage,
    A: OpenDrain,
>(
    kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    call: &mut WriteHostCall<A>,
    cancelled: impl FnMut() -> bool,
) -> Result<Dispatch, SchedulerError> {
    // No result value is produced, but one exact completion Sign is required.
    let room = kernel.signs().ensure_capacity(1);
    let mut pending = false;
    let Some(request) = kernel.next_host_request_matching(|r| {
        if r.node != call.node() {
            return false;
        }
        pending = true;
        room.is_ok()
    }) else {
        if pending {
            room.map_err(SchedulerError::Sign)?;
        }
        return Ok(Dispatch::Idle);
    };
    let completion = {
        let input = kernel.host_value(request.input.value)?;
        match call.invoke(
            request.node,
            request.call,
            request.request,
            input,
            cancelled,
        ) {
            Ok(()) => outcome(true),
            Err(
                WriteCallRefusal::Capability(_)
                | WriteCallRefusal::Possession
                | WriteCallRefusal::WrongBinding,
            ) => denied(),
            Err(reason) => {
                let mut failed = outcome(false);
                failed.failure.as_mut().expect("failed outcome").detail =
                    write_failure::detail(&reason);
                failed
            }
        }
    };
    kernel.complete_host_call(request.node, request.request, completion)?;
    Ok(Dispatch::Completed {
        request,
        outcome: completion,
    })
}
/// An affine outstanding wait. Its copied input is bounded and contains no
/// authority; the actual Host Call owner still validates duration and custody.
pub struct PendingWait {
    request: HostCallRequest,
    input: [u8; 8],
}
impl PendingWait {
    pub async fn invoke<A: MonotonicWaitProvider>(
        &self,
        call: &mut MonotonicWaitHostCall<A>,
    ) -> HostCallOutcome {
        match call
            .invoke(
                self.request.node,
                self.request.call,
                self.request.request,
                &self.input,
            )
            .await
        {
            Ok(()) => outcome(true),
            Err(
                WaitCallRefusal::Capability(_)
                | WaitCallRefusal::Possession
                | WaitCallRefusal::WrongBinding,
            ) => denied(),
            Err(_) => outcome(false),
        }
    }
}
pub fn begin_wait<
    D: StepBack<P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize,
    L: SignSink,
    V: ValueStorage,
    A: MonotonicWaitProvider,
>(
    kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    call: &MonotonicWaitHostCall<A>,
) -> Result<Option<PendingWait>, SchedulerError> {
    let room = kernel.signs().ensure_capacity(1);
    let mut pending = false;
    let Some(request) = kernel.next_host_request_matching(|r| {
        if r.node != call.node() {
            return false;
        }
        pending = true;
        room.is_ok()
    }) else {
        if pending {
            room.map_err(SchedulerError::Sign)?;
        }
        return Ok(None);
    };
    let input = kernel.host_value(request.input.value)?;
    let Ok(input) = <[u8; 8]>::try_from(input) else {
        kernel.complete_host_call(request.node, request.request, outcome(false))?;
        return Ok(None);
    };
    Ok(Some(PendingWait { request, input }))
}
/// Called only after the provider future is quiescent. Rechecks the original
/// pending request and fresh Sign room after wake; produces no stored value.
pub fn complete_wait<
    D: StepBack<P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize,
    L: SignSink,
    V: ValueStorage,
>(
    kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>,
    pending: &PendingWait,
    completion: HostCallOutcome,
) -> Result<Dispatch, SchedulerError> {
    if !kernel.host_call_is_pending(pending.request) {
        return Ok(Dispatch::Idle);
    }
    kernel
        .signs()
        .ensure_capacity(1)
        .map_err(SchedulerError::Sign)?;
    kernel.complete_host_call(pending.request.node, pending.request.request, completion)?;
    Ok(Dispatch::Completed {
        request: pending.request,
        outcome: completion,
    })
}
