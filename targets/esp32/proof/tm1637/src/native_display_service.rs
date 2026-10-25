//! One bounded service of an ordinary-kernel-selected output Host Call.
//! This adapter schedules no Step, selects no Look and creates no Fore input.
use super::runtime::Kernel;
use conduit_environment_runtime::PreparedCaptureSelector;
use conduit_kernel::{self as k, scheduler::HostCallRequest};
use conduit_tm1637_wire::{
    owner::{WriteCallRefusal, WriteHostCall},
    OpenDrain,
};
/// Single-use custody of a request taken from the retained kernel.
pub struct PendingCall {
    request: HostCallRequest,
}
impl PendingCall {
    pub fn request(&self) -> HostCallRequest {
        self.request
    }
}
pub struct DisplayService<'a, P: OpenDrain> {
    pub kernel: &'a mut Kernel,
    pub selectors: &'a mut [Option<PreparedCaptureSelector>; 4],
    pub write: k::NodeId,
    pub owner: &'a mut WriteHostCall<P>,
}
pub struct ServicedCall {
    pub request: HostCallRequest,
    pub failure: Option<k::Failure>,
}
impl<P: OpenDrain> DisplayService<'_, P> {
    pub fn take(&mut self) -> Option<PendingCall> {
        self.kernel
            .next_host_request()
            .map(|request| PendingCall { request })
    }
    pub fn service(
        &mut self,
        pending: PendingCall,
        cancelled: impl FnMut() -> bool,
    ) -> Result<ServicedCall, k::Failure> {
        let request = pending.request;
        let result = self
            .kernel
            .host_value(request.input.value)
            .map_err(|_| fail(k::FailureCode::InvalidInput, 1))
            .and_then(|bytes| {
                if request.node == self.write {
                    self.owner
                        .invoke(
                            request.node,
                            request.call,
                            request.request,
                            bytes,
                            cancelled,
                        )
                        .map(|()| None)
                        .map_err(write_failure)
                } else {
                    self.selectors
                        .get_mut(usize::from(request.node.0))
                        .and_then(Option::as_mut)
                        .ok_or(fail(k::FailureCode::HostCallDenied, 2))?
                        .select(bytes)
                        .map_err(|_| fail(k::FailureCode::InvalidInput, 3))
                }
            });
        let result = result.and_then(|output| {
            output
                .map(|bytes| {
                    let value = self
                        .kernel
                        .store_host_value(bytes)
                        .map_err(|_| fail(k::FailureCode::StorageExhausted, 4))?;
                    k::BoundedValueRef::new(value, 1024)
                        .map_err(|_| fail(k::FailureCode::StorageExhausted, 5))
                })
                .transpose()
        });
        let failure = result.as_ref().err().copied();
        let output = result.ok().flatten();
        let disposition = match failure.map(|f| f.code) {
            None => k::HostCallDisposition::Completed,
            Some(k::FailureCode::HostCallDenied) => k::HostCallDisposition::Denied,
            Some(k::FailureCode::Cancelled) => k::HostCallDisposition::Cancelled,
            Some(_) => k::HostCallDisposition::Failed,
        };
        if self
            .kernel
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
                self.kernel
                    .discard_host_value(output.value)
                    .map_err(|_| fail(k::FailureCode::InvalidLifecycle, 7))?;
            }
            return Err(fail(k::FailureCode::InvalidLifecycle, 6));
        }
        Ok(ServicedCall { request, failure })
    }
}
fn write_failure<E>(reason: WriteCallRefusal<E>) -> k::Failure {
    use conduit_tm1637_wire::Failure as Electrical;
    use k::FailureCode as C;
    let (code, detail) = match reason {
        WriteCallRefusal::WrongBinding
        | WriteCallRefusal::Possession
        | WriteCallRefusal::Capability(_) => (C::HostCallDenied, 10),
        WriteCallRefusal::StaleRequest | WriteCallRefusal::SequenceExhausted => {
            (C::InvalidLifecycle, 11)
        }
        WriteCallRefusal::Packets(_) => (C::InvalidInput, 12),
        WriteCallRefusal::Electrical(Electrical::Cancelled) => (C::Cancelled, 13),
        WriteCallRefusal::Electrical(Electrical::InvalidGeometry) => (C::InvalidInput, 14),
        WriteCallRefusal::Electrical(Electrical::Nack { .. }) => (C::HostCallFailed, 15),
        WriteCallRefusal::Electrical(Electrical::Provider(_)) => (C::HostCallFailed, 16),
    };
    fail(code, detail)
}
const fn fail(code: k::FailureCode, detail: u16) -> k::Failure {
    k::Failure { code, detail }
}
