//! One kernel owner; reusable mode admits only exact local pure-Value adapters.
use super::*;
use conduit_kernel::scheduler::prepared_session::{
    DrainedInvocation, OwnedValueBoundary, PreparedKernelSession, SessionRefusal,
};
use conduit_kernel::{SignSink, ValueRef};
type Session = PreparedKernelSession<
    BoxedKernelBack,
    HostedValueStore,
    HostedSignLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;
pub(super) enum ChildScheduler {
    Single(Box<RawChildScheduler>),
    Reusable { session: Session, remote_items: u16 },
}
impl core::ops::Deref for ChildScheduler {
    type Target = RawChildScheduler;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Single(kernel) => kernel,
            Self::Reusable { session, .. } => session.kernel(),
        }
    }
}
impl ChildScheduler {
    pub(super) fn new(
        kernel: Box<RawChildScheduler>,
        boundaries: &[BoundaryEndpoint],
        remote_items: u16,
        reusable: bool,
    ) -> Result<Self, String> {
        if !reusable {
            return Ok(Self::Single(kernel));
        }
        let owned = boundaries
            .iter()
            .map(|b| OwnedValueBoundary {
                cord: b.cord,
                endpoint: b.endpoint,
            })
            .collect::<Vec<_>>();
        let session = Session::new_with_owned_value_boundaries(kernel, &owned, remote_items)
            .map_err(|(reason, _)| format!("owned Value session: {reason:?}"))?;
        Ok(Self::Reusable {
            session,
            remote_items,
        })
    }
    pub(super) fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        match self {
            Self::Single(k) => k.step(),
            Self::Reusable { session, .. } => session.step(),
        }
    }
    pub(super) fn cancel(&mut self) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.cancel(),
            Self::Reusable { session, .. } => session.cancel(),
        }
    }
    pub(super) fn next_host_request(&mut self) -> Option<HostCallRequest> {
        match self {
            Self::Single(k) => k.next_host_request(),
            Self::Reusable { .. } => None,
        }
    }
    pub(super) fn complete_host_call(
        &mut self,
        node: NodeId,
        request: conduit_kernel::RequestId,
        outcome: conduit_kernel::HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.complete_host_call(node, request, outcome),
            Self::Reusable { .. } => Err(SchedulerError::InvalidHostCallAccess),
        }
    }
    pub(super) fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        match self {
            Self::Single(k) => k.store_host_value(bytes),
            Self::Reusable { .. } => Err(SchedulerError::InvalidHostCallAccess),
        }
    }
    pub(super) fn discard_host_value(
        &mut self,
        value: ValueRef,
    ) -> Result<(), conduit_kernel::scheduler::HostValueDiscardRefusal> {
        match self {
            Self::Single(k) => k.discard_host_value(value),
            Self::Reusable { .. } => {
                Err(conduit_kernel::scheduler::HostValueDiscardRefusal::ValueOwned)
            }
        }
    }
    pub(super) fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        match self {
            Self::Single(k) => k.admit_remote_input(endpoint, cord, sequence, bytes),
            Self::Reusable { session, .. } => {
                session.admit_owned_input(endpoint, cord, sequence, bytes)
            }
        }
    }
    pub(super) fn admit_remote_input_fanout(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        match self {
            Self::Single(k) => k.admit_remote_input_fanout(targets, sequence, bytes),
            Self::Reusable { session, .. } => match targets {
                [(endpoint, cord)] => session.admit_owned_input(*endpoint, *cord, sequence, bytes),
                _ => Err(SchedulerError::InvalidRemoteCordAccess),
            },
        }
    }
    pub(super) fn close_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.close_remote_input(endpoint, cord),
            Self::Reusable { session, .. } => session.close_owned_input(endpoint, cord),
        }
    }
    pub(super) fn close_remote_input_fanout(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.close_remote_input_fanout(targets),
            Self::Reusable { session, .. } => match targets {
                [(endpoint, cord)] => session.close_owned_input(*endpoint, *cord),
                _ => Err(SchedulerError::InvalidRemoteCordAccess),
            },
        }
    }
    pub(super) fn close_remote_input_fanout_abnormal(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        terminal: CanonicalValue,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.close_remote_input_fanout_abnormal(targets, terminal),
            Self::Reusable { .. } => Err(SchedulerError::InvalidRemoteCordAccess),
        }
    }
    pub(super) fn close_remote_input_abnormal(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        terminal: CanonicalValue,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k.close_remote_input_abnormal(endpoint, cord, terminal),
            Self::Reusable { .. } => Err(SchedulerError::InvalidRemoteCordAccess),
        }
    }
    pub(super) fn remote_egress_offer(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<conduit_kernel::scheduler::RemoteValueOffer>, SchedulerError> {
        match self {
            Self::Single(k) => k.remote_egress_offer(endpoint, cord),
            Self::Reusable { session, .. } => session.owned_output(endpoint, cord),
        }
    }
    pub(super) fn deliver_value(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        match self {
            Self::Single(k) => k
                .remote_egress_accept(endpoint, cord, sequence)
                .and_then(|()| k.remote_egress_delivered(endpoint, cord, sequence)),
            Self::Reusable { session, .. } => session.accept_owned_output(endpoint, cord, sequence),
        }
    }
    pub(super) fn empty_evidence(&self) -> Result<HostedSignLog, String> {
        let Self::Reusable { remote_items, .. } = self else {
            return Err("child was not admitted for reusable Values".into());
        };
        HostedSignLog::new_with_remote_storage(
            self.signs().item_capacity(),
            self.signs().byte_capacity(),
            *remote_items,
            conduit_kernel::remote_sign_storage_bytes(*remote_items)
                .ok_or("remote Sign reservation overflow")?,
        )
        .map_err(|e| format!("Value evidence: {e:?}"))
    }
    pub(super) fn rearm(
        &mut self,
        signs: HostedSignLog,
    ) -> Result<DrainedInvocation<HostedSignLog>, SessionRefusal> {
        match self {
            Self::Reusable { session, .. } => session
                .begin_next([(); MAX_NODES], signs)
                .map_err(|refusal| refusal.reason),
            Self::Single(_) => Err(SessionRefusal::NotFresh),
        }
    }
}
