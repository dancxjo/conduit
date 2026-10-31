//! Target custody around the same prepared kernel; no second scheduler.
#[cfg(not(feature = "display-physical-check"))]
use alloc::boxed::Box;
use conduit_environment_runtime::{display_output::*, PreparedCaptureSelector};
use conduit_kernel::{self as k, scheduler::*};
#[cfg(feature = "display-physical-check")]
use k::scheduler::prepared_session::{OwnedValueBoundary, PreparedKernelSession};
#[cfg(feature = "display-physical-check")]
pub type Log = crate::native_display_evidence::Log;
#[cfg(not(feature = "display-physical-check"))]
pub type Log = k::HostedSignLog;
pub type Raw = DisplayKernel<k::HostedValueStore, Log>;
#[cfg(feature = "display-physical-check")]
type Session = PreparedKernelSession<DisplayBack, k::HostedValueStore, Log, 4, 5, 1, 5, 5, 5, 4, 4>;
pub struct Kernel {
    #[cfg(feature = "display-physical-check")]
    inner: Session,
    #[cfg(not(feature = "display-physical-check"))]
    inner: Box<Raw>,
}
pub struct Prepared {
    pub identity: conduit_core::AssignedPlanView,
    pub kernel: Kernel,
    pub selectors: [Option<PreparedCaptureSelector>; 4],
    pub write: k::NodeId,
    pub input: conduit_core::AssignedForeRequirement,
    pub refusal: conduit_core::AssignedForeRequirement,
    maximum_frames: u32,
}
impl Prepared {
    pub fn maximum_frames(&self) -> u32 {
        self.maximum_frames
    }
    pub fn from_assigned(
        assigned: PreparedAssignedDisplayKernel<k::HostedValueStore, Log>,
    ) -> Result<Self, &'static str> {
        let maximum_frames = assigned.maximum_frames();
        #[cfg(feature = "display-physical-check")]
        let kernel = {
            let input = OwnedValueBoundary {
                endpoint: k::RemoteEndpointId(assigned.input.endpoint()),
                cord: k::CordId(assigned.input.cord()),
            };
            let refusal = OwnedValueBoundary {
                endpoint: k::RemoteEndpointId(assigned.refusal.endpoint()),
                cord: k::CordId(assigned.refusal.cord()),
            };
            Kernel {
                inner: Session::new_with_owned_optional_value_outputs(
                    assigned.kernel,
                    &[input, refusal],
                    &[refusal],
                    6,
                )
                .map_err(|_| "display-session")?,
            }
        };
        #[cfg(not(feature = "display-physical-check"))]
        let kernel = Kernel {
            inner: assigned.kernel,
        };
        Ok(Self {
            identity: assigned.identity,
            kernel,
            selectors: assigned.selectors,
            write: assigned.write,
            input: assigned.input,
            refusal: assigned.refusal,
            maximum_frames,
        })
    }
}
impl Kernel {
    pub fn raw(&self) -> &Raw {
        #[cfg(feature = "display-physical-check")]
        {
            self.inner.kernel()
        }
        #[cfg(not(feature = "display-physical-check"))]
        {
            &self.inner
        }
    }
    pub fn decisions(&self) -> u32 {
        self.raw().decisions()
    }
    pub fn logical_invocations(&self) -> u64 {
        self.raw().logical_invocations()
    }
    pub fn pending_host_call_count(&self) -> usize {
        self.raw().pending_host_call_count()
    }
    pub fn values(&self) -> &k::HostedValueStore {
        self.raw().values()
    }
    pub fn signs(&self) -> &Log {
        self.raw().signs()
    }
    pub fn host_value(&self, value: k::ValueRef) -> Result<&[u8], SchedulerError> {
        self.raw().host_value(value)
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.inner.step()
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.inner.cancel()
    }
    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.inner.next_host_request()
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<k::ValueRef, SchedulerError> {
        self.inner.store_host_value(bytes)
    }
    pub fn discard_host_value(
        &mut self,
        value: k::ValueRef,
    ) -> Result<(), k::scheduler::HostValueDiscardRefusal> {
        self.inner.discard_host_value(value)
    }
    pub fn complete_host_call(
        &mut self,
        node: k::NodeId,
        request: k::RequestId,
        outcome: k::HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        self.inner.complete_host_call(node, request, outcome)
    }
    pub fn admit_remote_input(
        &mut self,
        endpoint: k::RemoteEndpointId,
        cord: k::CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        #[cfg(feature = "display-physical-check")]
        {
            self.inner
                .admit_owned_input(endpoint, cord, sequence, bytes)
        }
        #[cfg(not(feature = "display-physical-check"))]
        {
            self.inner
                .admit_remote_input(endpoint, cord, sequence, bytes)
        }
    }
    pub fn close_remote_input(
        &mut self,
        endpoint: k::RemoteEndpointId,
        cord: k::CordId,
    ) -> Result<(), SchedulerError> {
        #[cfg(feature = "display-physical-check")]
        {
            self.inner.close_owned_input(endpoint, cord)
        }
        #[cfg(not(feature = "display-physical-check"))]
        {
            self.inner.close_remote_input(endpoint, cord)
        }
    }
    pub fn remote_egress_offer(
        &mut self,
        endpoint: k::RemoteEndpointId,
        cord: k::CordId,
    ) -> Result<Option<RemoteValueOffer>, SchedulerError> {
        #[cfg(feature = "display-physical-check")]
        {
            self.inner.owned_output(endpoint, cord)
        }
        #[cfg(not(feature = "display-physical-check"))]
        {
            self.inner.remote_egress_offer(endpoint, cord)
        }
    }
    pub fn remote_egress_accept(
        &mut self,
        endpoint: k::RemoteEndpointId,
        cord: k::CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        #[cfg(feature = "display-physical-check")]
        {
            self.inner.accept_owned_output(endpoint, cord, sequence)
        }
        #[cfg(not(feature = "display-physical-check"))]
        {
            self.inner.remote_egress_accept(endpoint, cord, sequence)
        }
    }
    #[cfg(not(feature = "display-physical-check"))]
    pub fn remote_egress_delivered(
        &mut self,
        endpoint: k::RemoteEndpointId,
        cord: k::CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        self.inner.remote_egress_delivered(endpoint, cord, sequence)
    }
    #[cfg(feature = "display-physical-check")]
    pub fn begin_next(&mut self) -> Result<(), &'static str> {
        let old = self
            .inner
            .begin_next(
                [(); 4],
                crate::native_display_evidence::fresh_log().map_err(|_| "display-session-log")?,
            )
            .map_err(|_| "display-session-rearm")?;
        // The caller has already transferred this exact page to its owner.
        drop(old);
        Ok(())
    }
}
