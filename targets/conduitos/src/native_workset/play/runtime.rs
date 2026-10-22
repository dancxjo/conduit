//! Exactly one prepared scheduler, with an explicit legacy or canonical profile.
use alloc::boxed::Box;
use conduit_kernel::{
    CordId, HostCallOutcome, NodeId, RemoteEndpointId, RequestId, SignRetentionGap, ValueRef,
    scheduler::{HostCallRequest, RemoteIngressOutcome, SchedulerError, SchedulerStatus},
};
use conduitos_runtime_core::{FACE_VALUE_BYTES, VALUE_BYTES, admission::Admission};

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
type Platform<const BYTES: usize> = crate::protected_runtime::Broker<BYTES>;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
type Platform<const BYTES: usize> = conduitos_runtime_core::NativeScheduler<BYTES>;

pub(super) enum Runtime {
    Legacy(Box<Platform<VALUE_BYTES>>),
    Face(Box<Platform<FACE_VALUE_BYTES>>),
}

macro_rules! dispatch {
    ($owner:expr, $runtime:ident => $body:expr) => {
        match $owner {
            Runtime::Legacy($runtime) => $body,
            Runtime::Face($runtime) => $body,
        }
    };
}

impl Runtime {
    pub fn prepare(admission: Admission, canonical: bool) -> Result<Self, SchedulerError> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            if canonical {
                return Platform::<FACE_VALUE_BYTES>::new_face(admission)
                    .map(|runtime| Self::Face(Box::new(runtime)));
            }
            Platform::<VALUE_BYTES>::new(admission).map(|runtime| Self::Legacy(Box::new(runtime)))
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            if canonical {
                return admission
                    .build_face()
                    .map(|runtime| Self::Face(Box::new(runtime)));
            }
            admission
                .build()
                .map(|runtime| Self::Legacy(Box::new(runtime)))
        }
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        dispatch!(self, runtime => runtime.step())
    }
    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        dispatch!(self, runtime => runtime.next_host_request())
    }
    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        dispatch!(self, runtime => runtime.host_value(value))
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        dispatch!(self, runtime => runtime.store_host_value(bytes))
    }
    pub fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        dispatch!(self, runtime => runtime.complete_host_call(node, request, outcome))
    }
    pub fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        match self {
            Self::Face(runtime) => runtime.admit_remote_input(endpoint, cord, sequence, bytes),
            Self::Legacy(_) => Err(SchedulerError::InvalidPlan),
        }
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        dispatch!(self, runtime => runtime.cancel())
    }
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub fn abort(&mut self) {
        dispatch!(self, runtime => runtime.abort())
    }
    pub fn sign_retention_gap(&self) -> Option<SignRetentionGap> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            dispatch!(self, runtime => runtime.sign_retention_gap())
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            dispatch!(self, runtime => conduit_kernel::SignQuery::retention_gap(runtime.signs()))
        }
    }
    #[cfg(test)]
    pub fn pending_host_call_count(&self) -> usize {
        dispatch!(self, runtime => runtime.pending_host_call_count())
    }
    #[cfg(test)]
    pub fn values(&self) -> &dyn conduit_kernel::ValueStorage {
        dispatch!(self, runtime => runtime.values())
    }
}
