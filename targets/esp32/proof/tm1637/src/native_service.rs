//! Bounded Host Call bridge. Only the ordinary kernel selects work/correlates it.
//! Invocation futures borrow providers, never the kernel, so waits cannot stall
//! independent kernel Steps. Drop pending invocations before quiescing owners.
use crate::{
    native_authority::NativeBindings,
    native_kernel_storage::{Kernel, PreparedKernel},
};
use conduit_kernel::{self as k, scheduler::HostCallRequest};
use conduit_monotonic_wait::{
    MonotonicWaitHostCall, MonotonicWaitProvider, WaitCallRefusal, WaitProviderFailure,
};
use conduit_pulse_capture::owner::{PulseHostCall, PulseReadProvider};
const PULSE_BYTES: usize = conduit_pulse_capture::PULSE_READ_CANONICAL_REQUEST_BYTES;
const REPLY_BYTES: usize = conduit_pulse_capture::PULSE_READ_CANONICAL_RESULT_BYTES;
const PULSE_MAXIMUM_BYTES: usize = conduit_pulse_capture::PULSE_READ_MAXIMUM_BYTES as usize;
const WAIT_BYTES: usize = conduit_monotonic_wait::PARAMETER_BYTES as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServicedCall {
    pub request: HostCallRequest,
    pub failure: Option<k::Failure>,
    pub disposition: k::HostCallDisposition,
}
/// Owned finite parameters from a kernel-selected request, not incoming JSON.
/// No Clone/Deserialize entrance can manufacture a second pending invocation.
pub struct Invocation<const N: usize> {
    request: HostCallRequest,
    bytes: [u8; N],
    length: usize,
}
#[allow(
    clippy::large_enum_variant,
    reason = "fixed native input storage is admitted before Play; boxing here would allocate during dispatch"
)]
pub enum Dispatch {
    None,
    Software(ServicedCall),
    Pulse(Invocation<PULSE_BYTES>),
    Wait(Invocation<WAIT_BYTES>),
}
pub struct Reply {
    request: HostCallRequest,
    bytes: [u8; REPLY_BYTES],
    length: Option<usize>,
    failure: Option<k::Failure>,
}
fn parameters<const N: usize>(
    kernel: &Kernel,
    request: HostCallRequest,
    maximum_bytes: usize,
) -> Result<Invocation<N>, k::Failure> {
    let input = kernel
        .host_value(request.input.value)
        .map_err(|_| fail(k::FailureCode::InvalidInput, 1))?;
    copy_parameters(request, input, maximum_bytes)
}
fn copy_parameters<const N: usize>(
    request: HostCallRequest,
    input: &[u8],
    maximum_bytes: usize,
) -> Result<Invocation<N>, k::Failure> {
    if input.len() > maximum_bytes {
        return Err(fail(k::FailureCode::InvalidInput, 2));
    }
    let mut bytes = [0; N];
    // Over-Type-bound input preserves its original refusal without retaining
    // unused public-envelope padding in each invocation queue and future.
    if input.len() <= N {
        bytes[..input.len()].copy_from_slice(input);
    }
    Ok(Invocation {
        request,
        bytes,
        length: input.len(),
    })
}
#[cfg(test)]
pub(crate) fn test_pulse_input(
    request: HostCallRequest,
    input: &[u8],
) -> Result<Invocation<PULSE_BYTES>, k::Failure> {
    copy_parameters(request, input, PULSE_MAXIMUM_BYTES)
}
#[cfg(test)]
pub(crate) fn test_reply_failure(reply: &Reply) -> Option<k::Failure> {
    reply.failure
}
/// Copies only a native call's finitely admitted input. The kernel keeps its
/// own pending input pinned until correlated completion or cancellation.
pub fn next(storage: &mut PreparedKernel) -> Result<Dispatch, k::Failure> {
    let Some(request) = storage.kernel.next_host_request() else {
        return Ok(Dispatch::None);
    };
    if request.node == storage.roles.pulse {
        return match parameters(&storage.kernel, request, PULSE_MAXIMUM_BYTES) {
            Ok(input) => Ok(Dispatch::Pulse(input)),
            Err(reason) => {
                complete(&mut storage.kernel, request, Err(reason)).map(Dispatch::Software)
            }
        };
    }
    if request.node == storage.roles.tick {
        return match parameters(&storage.kernel, request, WAIT_BYTES) {
            Ok(input) => Ok(Dispatch::Wait(input)),
            Err(reason) => {
                complete(&mut storage.kernel, request, Err(reason)).map(Dispatch::Software)
            }
        };
    }
    let result = storage
        .kernel
        .host_value(request.input.value)
        .map_err(|_| fail(k::FailureCode::InvalidInput, 3))
        .and_then(|input| {
            storage
                .selectors
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or(fail(k::FailureCode::HostCallDenied, 4))?
                .select(input)
                .map_err(|_| fail(k::FailureCode::InvalidInput, 5))
        });
    complete(&mut storage.kernel, request, result).map(Dispatch::Software)
}
fn reply(request: HostCallRequest, result: Result<Option<&[u8]>, k::Failure>) -> Reply {
    let mut reply = Reply {
        request,
        bytes: [0; REPLY_BYTES],
        length: None,
        failure: None,
    };
    match result {
        Ok(Some(bytes)) if bytes.len() <= REPLY_BYTES => {
            reply.bytes[..bytes.len()].copy_from_slice(bytes);
            reply.length = Some(bytes.len());
        }
        Ok(Some(_)) => reply.failure = Some(fail(k::FailureCode::StorageExhausted, 6)),
        Ok(None) => (),
        Err(reason) => reply.failure = Some(reason),
    }
    reply
}
pub async fn pulse<P: PulseReadProvider>(
    owner: &mut PulseHostCall<P>,
    input: Invocation<PULSE_BYTES>,
) -> Reply {
    let request = input.request;
    if input.length > PULSE_BYTES {
        // The original canonical decoder rejects before authorizing a lease,
        // incrementing request sequence, or touching the electrical provider.
        return reply(request, Err(fail(k::FailureCode::HostCallDenied, 7)));
    }
    let result = owner
        .invoke(
            request.node,
            request.call,
            request.request,
            &input.bytes[..input.length],
        )
        .await
        .map(Some)
        .map_err(|_| fail(k::FailureCode::HostCallDenied, 7));
    reply(request, result)
}
pub async fn wait<W: MonotonicWaitProvider>(
    owner: &mut MonotonicWaitHostCall<W>,
    input: Invocation<WAIT_BYTES>,
) -> Reply {
    let request = input.request;
    let result = owner
        .invoke(
            request.node,
            request.call,
            request.request,
            &input.bytes[..input.length],
        )
        .await
        .map(|()| None)
        .map_err(|reason| match reason {
            WaitCallRefusal::Provider(WaitProviderFailure::Pressure) => {
                fail(k::FailureCode::StorageExhausted, 8)
            }
            WaitCallRefusal::Provider(_) => fail(k::FailureCode::HostCallFailed, 9),
            _ => fail(k::FailureCode::HostCallDenied, 10),
        });
    reply(request, result)
}
pub fn finish(storage: &mut PreparedKernel, reply: Reply) -> Result<ServicedCall, k::Failure> {
    let result = match reply.failure {
        Some(reason) => Err(reason),
        None => Ok(reply.length.map(|length| &reply.bytes[..length])),
    };
    complete(&mut storage.kernel, reply.request, result)
}
fn complete(
    kernel: &mut Kernel,
    request: HostCallRequest,
    result: Result<Option<&[u8]>, k::Failure>,
) -> Result<ServicedCall, k::Failure> {
    let result = result.and_then(|bytes| {
        bytes
            .map(|bytes| {
                let value = kernel
                    .store_host_value(bytes)
                    .map_err(|_| fail(k::FailureCode::StorageExhausted, 11))?;
                k::BoundedValueRef::new(
                    value,
                    u32::try_from(bytes.len().max(1))
                        .map_err(|_| fail(k::FailureCode::StorageExhausted, 12))?,
                )
                .map_err(|_| fail(k::FailureCode::StorageExhausted, 12))
            })
            .transpose()
    });
    let failure = result.as_ref().err().copied();
    let disposition = match failure {
        None => k::HostCallDisposition::Completed,
        Some(reason) if reason.code == k::FailureCode::HostCallDenied => {
            k::HostCallDisposition::Denied
        }
        Some(_) => k::HostCallDisposition::Failed,
    };
    let output = result.as_ref().ok().copied().flatten();
    if kernel
        .complete_host_call(
            request.node,
            request.request,
            k::HostCallOutcome {
                disposition,
                output,
                failure,
            },
        )
        .is_err()
    {
        if let Some(output) = output {
            kernel
                .discard_host_value(output.value)
                .map_err(|_| fail(k::FailureCode::InvalidLifecycle, 17))?;
        }
        return Err(fail(k::FailureCode::InvalidLifecycle, 13));
    }
    Ok(ServicedCall {
        request,
        failure,
        disposition,
    })
}
/// Regaining mutable owners requires pending invocation futures to have dropped.
/// Attempt every revocation and kernel cancellation; report any refusal honestly.
pub fn quiesce<P: PulseReadProvider, W: MonotonicWaitProvider>(
    storage: &mut PreparedKernel,
    native: &mut NativeBindings<P, W>,
) -> Result<(), k::Failure> {
    let pulse = native.pulse.revoke(storage.roles.pulse, k::HostCallId(0));
    let wait = native.wait.revoke(storage.roles.tick, k::HostCallId(0));
    let cancel = storage.kernel.cancel();
    pulse.map_err(|_| fail(k::FailureCode::HostCallDenied, 14))?;
    wait.map_err(|_| fail(k::FailureCode::HostCallDenied, 15))?;
    cancel.map_err(|_| fail(k::FailureCode::InvalidLifecycle, 16))
}
const fn fail(code: k::FailureCode, detail: u16) -> k::Failure {
    k::Failure { code, detail }
}
