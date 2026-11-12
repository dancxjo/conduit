//! Affine backing for repeated read-only Shows through the existing kernel owner.
use super::*;
use conduit_kernel::scheduler::prepared_session::{
    DrainedInvocation, OwnedValueBoundary, PreparedKernelSession,
};
use conduit_kernel::{RequestId, SignSink};
use core::ops::Deref;
pub(super) type Session = PreparedKernelSession<
    MaskBack,
    HostedValueStore,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    HOST_BINDINGS,
    NODES,
>;

/// Managed scheduler inline backing plus actual value-store capacities only.
/// Excludes Plan/Face/lowered metadata heaps, compositor, serialization, allocator
/// overhead and preparation peak. This is not a whole-Root memory receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMaskBackingReceipt {
    pub scheduler_inline_bytes: usize,
    pub value_storage_bytes: usize,
    pub optional_output_storage_bytes: usize,
    pub managed_store_bytes: usize,
}
/// Transferred local child-Play evidence; taking it is NOT durable-recording ACK.
pub struct RetiredNativeMaskEvidence {
    pub receipt: NativeMaskPlayReceipt,
    pub signs: FixedSignLog<SIGNS>,
    pub decisions: u32,
    pub logical_invocations: u64,
}
/// Finite affine kernel/value/Sign backing for the exact immutable Mask Plan.
/// This does not retain Face serialization/scene metadata and does not itself
/// reserve a Host ledger pool or establish renderer/Show authority.
pub struct RetainedNativeMaskStorage {
    session: Session,
    plan_id: conduit_core::PlanId,
    profile: NativeMaskStorageProfile,
    sequence: u64,
    request_base: u32,
    backing: NativeMaskBackingReceipt,
    invocation: super::prepared::InvocationBinding,
    metadata_bytes: usize,
}
impl RetainedNativeMaskStorage {
    pub fn required_store_bytes(
        profile: NativeMaskStorageProfile,
    ) -> Result<usize, NativeMaskPlayError> {
        let values = HostedValueStore::canonical_storage_bytes(core::iter::repeat_n(
            profile.bytes(),
            VALUES,
        ))
        .map_err(|_| NativeMaskPlayError::Pressure)?;
        core::mem::size_of::<Scheduler>()
            .checked_add(values)
            .and_then(|bytes| bytes.checked_add(Session::optional_output_storage_requirement()))
            .ok_or(NativeMaskPlayError::Pressure)
    }
    /// Refuse the immutable store requirement before allocating its kernel/store.
    /// Checking/planning metadata remains preparation and is separately accounted.
    pub fn prepare(
        planned: &PlannedMaskPlot,
        profile: NativeMaskStorageProfile,
        maximum_bytes: usize,
    ) -> Result<Self, NativeMaskPlayError> {
        storage::validate_profile(planned, profile)?;
        UnboundNativeMaskStorage::prepare(profile, maximum_bytes)?.bind(planned)
    }
    pub(super) fn from_prepared_kernel(
        planned: &PlannedMaskPlot,
        lowered: &LoweredPlanFragment,
        profile: NativeMaskStorageProfile,
        scheduler: alloc::boxed::Box<Scheduler>,
        optional_storage: conduit_kernel::scheduler::prepared_session::OptionalOutputStorage<CORDS>,
        backing: NativeMaskBackingReceipt,
    ) -> Result<Self, NativeMaskPlayError> {
        let invocation = super::prepared::InvocationBinding::prepare(planned, lowered)?;
        let metadata_bytes = invocation
            .backing_bytes()
            .and_then(|bytes| bytes.checked_add(planned.plan.plan_id.0.len()))
            .ok_or(NativeMaskPlayError::Pressure)?;
        let boundaries = lowered
            .fore_ports
            .iter()
            .map(|port| OwnedValueBoundary {
                cord: port.cord,
                endpoint: port.endpoint,
            })
            .collect::<Vec<_>>();
        let optional = lowered
            .fore_ports
            .iter()
            .filter(|port| {
                port.front_port_id.as_str() == "interaction"
                    && port.direction == conduit_core::PortDirection::Output
            })
            .map(|port| OwnedValueBoundary {
                cord: port.cord,
                endpoint: port.endpoint,
            })
            .collect::<Vec<_>>();
        if optional.len() != 1 {
            return Err(NativeMaskPlayError::Shape);
        }
        let session = Session::new_with_preallocated_optional_outputs(
            scheduler,
            &boundaries,
            &optional,
            32,
            optional_storage,
        )
        .map_err(|_| NativeMaskPlayError::Shape)?;
        if session.optional_output_storage_bytes() != backing.optional_output_storage_bytes {
            return Err(NativeMaskPlayError::Pressure);
        }
        Ok(Self {
            session,
            plan_id: planned.plan.plan_id.clone(),
            profile,
            sequence: 0,
            request_base: 0,
            backing,
            invocation,
            metadata_bytes,
        })
    }
    #[cfg(test)]
    pub(super) fn backing_identity(&self) -> usize {
        (self.session.kernel() as *const Scheduler) as usize
    }

    /// Owned Plan identity payload, separate from the fixed scheduler stores.
    pub fn retained_metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(super) fn invocation_binding(&self) -> &super::prepared::InvocationBinding {
        &self.invocation
    }
    pub fn backing(&self) -> NativeMaskBackingReceipt {
        self.backing
    }
    pub(super) fn rearm(
        &mut self,
    ) -> Result<DrainedInvocation<FixedSignLog<SIGNS>>, NativeMaskPlayError> {
        let next = self
            .request_base
            .checked_add(2)
            .ok_or(NativeMaskPlayError::Pressure)?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(NativeMaskPlayError::Pressure)?;
        let bytes = self.session.kernel().signs().byte_capacity();
        let signs = FixedSignLog::new_with_remote_storage(
            bytes,
            32,
            conduit_kernel::remote_sign_storage_bytes(32).ok_or(NativeMaskPlayError::Kernel)?,
        )
        .map_err(|_| NativeMaskPlayError::Kernel)?;
        let retired = self
            .session
            .begin_next([(); NODES], signs)
            .map_err(|_| NativeMaskPlayError::Kernel)?;
        self.request_base = next;
        self.sequence = sequence;
        Ok(retired)
    }
}

/// One-shot compatibility and retained invocations share the same kernel. This
/// adapter exposes no mutable scheduler escape from PreparedKernelSession.
// Transfer the already admitted affine owners inline; boxing here would add
// an allocation at invocation/input rather than reuse the original stores.
#[allow(clippy::large_enum_variant)]
pub(super) enum MaskScheduler {
    Ephemeral(alloc::boxed::Box<Scheduler>),
    Retained(RetainedNativeMaskStorage),
    Retired,
}
impl Deref for MaskScheduler {
    type Target = Scheduler;
    fn deref(&self) -> &Scheduler {
        match self {
            Self::Ephemeral(value) => value,
            Self::Retained(value) => value.session.kernel(),
            Self::Retired => panic!("retired native Mask has no scheduler"),
        }
    }
}
impl MaskScheduler {
    pub(super) fn retained_metadata_bytes(&self) -> Option<usize> {
        match self {
            Self::Retained(storage) => Some(storage.plan_id.0.capacity()),
            Self::Ephemeral(_) | Self::Retired => None,
        }
    }

    pub(super) fn retained(
        storage: RetainedNativeMaskStorage,
        planned: &PlannedMaskPlot,
        profile: NativeMaskStorageProfile,
    ) -> Result<Self, NativeMaskPlayError> {
        if storage.plan_id != planned.plan.plan_id || storage.profile != profile {
            return Err(NativeMaskPlayError::Plan);
        }
        Ok(Self::Retained(storage))
    }
    pub(super) fn input_sequence(&self) -> u64 {
        match self {
            Self::Retained(value) => value.sequence,
            _ => 0,
        }
    }
    pub(super) fn renderer_request(&self) -> RequestId {
        RequestId(match self {
            Self::Retained(value) => value.request_base,
            _ => 0,
        })
    }
    pub(super) fn interaction_request(&self) -> RequestId {
        RequestId(self.renderer_request().0 + 1)
    }
    pub(super) fn take_storage(
        &mut self,
    ) -> Result<RetainedNativeMaskStorage, NativeMaskPlayError> {
        match core::mem::replace(self, Self::Retired) {
            Self::Retained(storage) => Ok(storage),
            other => {
                *self = other;
                Err(NativeMaskPlayError::Shape)
            }
        }
    }
    pub(super) fn step(
        &mut self,
    ) -> Result<conduit_kernel::scheduler::SchedulerStatus, conduit_kernel::scheduler::SchedulerError>
    {
        match self {
            Self::Ephemeral(value) => value.step(),
            Self::Retained(value) => value.session.step(),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn cancel(&mut self) -> Result<(), conduit_kernel::scheduler::SchedulerError> {
        match self {
            Self::Ephemeral(value) => value.cancel(),
            Self::Retained(value) => value.session.cancel(),
            Self::Retired => Ok(()),
        }
    }
    pub(super) fn next_host_request(
        &mut self,
    ) -> Option<conduit_kernel::scheduler::HostCallRequest> {
        match self {
            Self::Ephemeral(value) => value.next_host_request(),
            Self::Retained(value) => value.session.next_host_request(),
            Self::Retired => None,
        }
    }
    pub(super) fn store_host_value(
        &mut self,
        bytes: &[u8],
    ) -> Result<conduit_kernel::ValueRef, conduit_kernel::scheduler::SchedulerError> {
        match self {
            Self::Ephemeral(value) => value.store_host_value(bytes),
            Self::Retained(value) => value.session.store_host_value(bytes),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn complete_host_call(
        &mut self,
        node: conduit_kernel::NodeId,
        request: RequestId,
        outcome: conduit_kernel::HostCallOutcome,
    ) -> Result<(), conduit_kernel::scheduler::SchedulerError> {
        match self {
            Self::Ephemeral(value) => value.complete_host_call(node, request, outcome),
            Self::Retained(value) => value.session.complete_host_call(node, request, outcome),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn admit_remote_input(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        cord: conduit_kernel::CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<
        conduit_kernel::scheduler::RemoteIngressOutcome,
        conduit_kernel::scheduler::SchedulerError,
    > {
        match self {
            Self::Ephemeral(value) => value.admit_remote_input(endpoint, cord, sequence, bytes),
            Self::Retained(value) => value
                .session
                .admit_owned_input(endpoint, cord, sequence, bytes),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn close_remote_input(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        cord: conduit_kernel::CordId,
    ) -> Result<(), conduit_kernel::scheduler::SchedulerError> {
        match self {
            Self::Ephemeral(value) => value.close_remote_input(endpoint, cord),
            Self::Retained(value) => value.session.close_owned_input(endpoint, cord),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn remote_egress_offer(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        cord: conduit_kernel::CordId,
    ) -> Result<
        Option<conduit_kernel::scheduler::RemoteValueOffer>,
        conduit_kernel::scheduler::SchedulerError,
    > {
        match self {
            Self::Ephemeral(value) => value.remote_egress_offer(endpoint, cord),
            Self::Retained(value) => value.session.owned_output(endpoint, cord),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
    pub(super) fn accept_output(
        &mut self,
        endpoint: conduit_kernel::RemoteEndpointId,
        cord: conduit_kernel::CordId,
        sequence: u64,
    ) -> Result<(), conduit_kernel::scheduler::SchedulerError> {
        match self {
            Self::Ephemeral(value) => value
                .remote_egress_accept(endpoint, cord, sequence)
                .and_then(|()| value.remote_egress_delivered(endpoint, cord, sequence)),
            Self::Retained(value) => value.session.accept_owned_output(endpoint, cord, sequence),
            Self::Retired => Err(conduit_kernel::scheduler::SchedulerError::Cancelled),
        }
    }
}
