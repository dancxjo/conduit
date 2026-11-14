//! One admitted synchronous Host Call; no loop, transport or Start authority.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub enum NativeDispatchRefusal {
    Call(I2cCallRefusal),
    Kernel(SchedulerError),
    OutputBounds,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NativeDispatch {
    Idle,
    Pressure,
    Completed,
    Refused(NativeDispatchRefusal),
}

impl<P: I2cProvider> BoundNativeKernel<P> {
    /// Root calls this only after independently authorizing Start. It services
    /// at most one already pending request and never advances the scheduler.
    /// Once dispatched, adapter refusal becomes an exact failed completion, so polling
    /// again cannot retry a physical transaction or strand a dispatched request.
    /// Physical bus outcomes remain ordinary typed result values from the Back.
    /// Result/Sign preflight and synchronous invocation have no intervening
    /// await or kernel mutation. Pressure leaves the original request undispatched.
    pub fn dispatch_one(&mut self) -> Result<NativeDispatch, SchedulerError> {
        dispatch_one(&mut self.kernel, &mut self.call)
    }
}

type DispatchKernel<D, const N: usize, const C: usize, S, const H: usize> =
    FixedScheduler<D, S, FixedSignLog<SIGNS>, N, C, 1, 16, N, C, H, 1>;

/// One synchronous adapter transaction on the same production kernel profile.
pub(crate) fn dispatch_one<
    P: I2cProvider,
    D: StepBack<1>,
    const N: usize,
    const C: usize,
    S: ValueStorage,
    const H: usize,
>(
    kernel: &mut DispatchKernel<D, N, C, S, H>,
    call: &mut I2cHostCall<P>,
) -> Result<NativeDispatch, SchedulerError> {
    let room = kernel.preflight_host_result_storage(
        conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES as u32,
    );
    let mut pending = false;
    let Some(request) = kernel.next_host_request_matching(|_| {
        pending = true;
        room.is_ok()
    }) else {
        if !pending {
            return Ok(NativeDispatch::Idle);
        }
        return match room {
            Err(SchedulerError::Storage(
                StorageError::ItemCapacityExceeded | StorageError::ByteCapacityExceeded,
            )) => Ok(NativeDispatch::Pressure),
            Err(error) => Err(error),
            Ok(()) => unreachable!("matching pending request was not dispatched"),
        };
    };
    let result = (|| {
        let input = kernel
            .host_value(request.input.value)
            .map_err(NativeDispatchRefusal::Kernel)?;
        let output = call
            .invoke(request.node, request.call, request.request, input)
            .map_err(NativeDispatchRefusal::Call)?;
        if output.len() > I2C_MAXIMUM_BYTES as usize {
            return Err(NativeDispatchRefusal::OutputBounds);
        }
        let value = kernel
            .store_host_value(output)
            .map_err(NativeDispatchRefusal::Kernel)?;
        // The preceding finite extent check guarantees this conversion.
        Ok(BoundedValueRef::new(value, I2C_MAXIMUM_BYTES).expect("checked native result extent"))
    })();
    let (outcome, report) = match result {
        Ok(output) => (
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(output),
                failure: None,
            },
            NativeDispatch::Completed,
        ),
        Err(reason) => {
            let code = match &reason {
                NativeDispatchRefusal::Call(
                    I2cCallRefusal::Capability(_)
                    | I2cCallRefusal::Possession
                    | I2cCallRefusal::WrongBinding,
                ) => FailureCode::HostCallDenied,
                NativeDispatchRefusal::Kernel(SchedulerError::Storage(_)) => {
                    FailureCode::StorageExhausted
                }
                _ => FailureCode::HostCallFailed,
            };
            (
                HostCallOutcome {
                    disposition: if code == FailureCode::HostCallDenied {
                        HostCallDisposition::Denied
                    } else {
                        HostCallDisposition::Failed
                    },
                    output: None,
                    failure: Some(Failure { code, detail: 0 }),
                },
                NativeDispatch::Refused(reason),
            )
        }
    };
    if let Err(error) = kernel.complete_host_call(request.node, request.request, outcome) {
        if let Some(output) = outcome.output {
            // The scheduler rejected ownership; do not leak its unqueued value.
            let _ = kernel.discard_host_value(output.value);
        }
        return Err(error);
    }
    Ok(report)
}
