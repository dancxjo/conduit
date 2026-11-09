//! Bounded synchronous actor beneath the original kernel Host Calls. The shared
//! timing exclusion is neither admission nor authority to access a provider.
use super::{Dispatch, NativeActivity};
use crate::execution::StoredExecutionKernel;
use conduit_kernel::{scheduler::*, *};
#[path = "native_sync_append.rs"]
mod append;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallFailure {
    Denied,
    Failed,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    MissingActivity,
    InvalidBounds,
    ConflictingNode,
    ProviderResultBound,
    Acknowledgment(CallFailure),
    Kernel(SchedulerError),
}
/// Original initialized affine, independently bound native possession.
/// # Safety
/// Bounds/node/call remain fixed. Invocation performs only the exact authorized
/// operation, terminates within admitted work, and allocates nothing. Uncertain
/// effects are never blindly retried. `acknowledge` means kernel acceptance only;
/// `revoke` quiesces work and preserves committed/uncertain media custody. Root
/// separately admits every buffer/provider/window and the actual Start.
pub unsafe trait SynchronousHostCall {
    fn node(&self) -> NodeId;
    fn call(&self) -> HostCallId;
    fn maximum_input_bytes(&self) -> u32;
    fn maximum_output_bytes(&self) -> u32;
    fn invoke(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CallFailure>;
    fn acknowledge(&mut self, request: HostCallRequest) -> Result<(), CallFailure>;
    fn revoke(&mut self) -> Result<(), CallFailure>;
}
/// Original owner loan plus independently funded output storage. Input is
/// borrowed synchronously from the original bounded kernel value owner.
pub struct Service<'a, P> {
    owner: &'a mut P,
    output: &'a mut [u8],
    node: NodeId,
    call: HostCallId,
    input_bytes: u32,
    output_bytes: u32,
}
impl<'a, P: SynchronousHostCall> Service<'a, P> {
    pub fn new(owner: &'a mut P, output: &'a mut [u8]) -> Result<Self, Refusal> {
        let output_bytes = owner.maximum_output_bytes();
        let required = usize::try_from(output_bytes).map_err(|_| Refusal::InvalidBounds)?;
        if output.len() < required {
            return Err(Refusal::InvalidBounds);
        }
        Ok(Self {
            node: owner.node(),
            call: owner.call(),
            input_bytes: owner.maximum_input_bytes(),
            output_bytes,
            owner,
            output,
        })
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    /// Inspect original custody without releasing or mutating the affine loan.
    pub fn bound_owner(&self) -> &P {
        self.owner
    }
    pub fn revoke(&mut self) -> Result<(), CallFailure> {
        self.owner.revoke()
    }
    /// Acquire blocking exclusion before taking a request. Timing activity and
    /// result/Sign pressure leave that exact pending request untouched.
    pub fn dispatch<
        D: StepBack<PORTS>,
        L: SignSink,
        const N: usize,
        const C: usize,
        const PORTS: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        &mut self,
        kernel: &mut StoredExecutionKernel<D, L, N, C, PORTS, R, T, H>,
        activity: &NativeActivity,
    ) -> Result<Dispatch, Refusal> {
        let Some(_window) = activity.try_blocking() else {
            return Ok(Dispatch::Idle);
        };
        let room = kernel.preflight_host_result_storage(self.output_bytes);
        let mut pending = false;
        let Some(request) = kernel.next_host_request_matching(|r| {
            if r.node != self.node || r.call != self.call {
                return false;
            }
            pending = true;
            room.is_ok()
        }) else {
            return if pending {
                match room.unwrap_err() {
                    SchedulerError::Storage(
                        StorageError::ItemCapacityExceeded | StorageError::ByteCapacityExceeded,
                    )
                    | SchedulerError::Sign(
                        SignError::ItemCapacityExceeded | SignError::ByteCapacityExceeded,
                    ) => Ok(Dispatch::Pressure),
                    error => Err(Refusal::Kernel(error)),
                }
            } else {
                Ok(Dispatch::Idle)
            };
        };
        let result = {
            let input = kernel
                .host_value(request.input.value)
                .map_err(Refusal::Kernel)?;
            if input.len() > self.input_bytes as usize {
                Err(CallFailure::Denied)
            } else {
                self.owner.invoke(
                    request,
                    input,
                    &mut self.output[..self.output_bytes as usize],
                )
            }
        };
        let mut completion = match result {
            Ok(length) => {
                if length > self.output_bytes as usize {
                    return Err(Refusal::ProviderResultBound);
                }
                let value = kernel
                    .store_host_value(&self.output[..length])
                    .map_err(Refusal::Kernel)?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, self.output_bytes)
                            .expect("checked result bound"),
                    ),
                    failure: None,
                }
            }
            Err(failure) => HostCallOutcome {
                disposition: match failure {
                    CallFailure::Denied => HostCallDisposition::Denied,
                    CallFailure::Failed => HostCallDisposition::Failed,
                },
                output: None,
                failure: Some(Failure {
                    code: match failure {
                        CallFailure::Denied => FailureCode::HostCallDenied,
                        CallFailure::Failed => FailureCode::HostCallFailed,
                    },
                    detail: 0,
                }),
            },
        };
        if let Err(error) = kernel.complete_host_call(request.node, request.request, completion) {
            if let Some(output) = completion.output.take() {
                let _ = kernel.discard_host_value(output.value);
            }
            return Err(Refusal::Kernel(error));
        }
        // Only this actual accepted result boundary permits the owner's ACK.
        // It is not downstream publication, remote commit, or reclamation proof.
        if result.is_ok() {
            self.owner
                .acknowledge(request)
                .map_err(Refusal::Acknowledgment)?;
        }
        Ok(Dispatch::Completed {
            request,
            outcome: completion,
        })
    }
}
pub(super) struct NoService;
// SAFETY: no default Runner dispatches an absent additional owner.
unsafe impl SynchronousHostCall for NoService {
    fn node(&self) -> NodeId {
        NodeId(0)
    }
    fn call(&self) -> HostCallId {
        HostCallId(0)
    }
    fn maximum_input_bytes(&self) -> u32 {
        0
    }
    fn maximum_output_bytes(&self) -> u32 {
        0
    }
    fn invoke(&mut self, _: HostCallRequest, _: &[u8], _: &mut [u8]) -> Result<usize, CallFailure> {
        Err(CallFailure::Denied)
    }
    fn acknowledge(&mut self, _: HostCallRequest) -> Result<(), CallFailure> {
        Err(CallFailure::Denied)
    }
    fn revoke(&mut self) -> Result<(), CallFailure> {
        Ok(())
    }
}
