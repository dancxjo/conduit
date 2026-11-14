//! One-shot native publication beneath the existing kernel Host Call boundary.
//! Root separately admits resources, provider work/memory and actual Start.
#[path = "native/assigned.rs"]
mod assigned;
#[path = "native/installation.rs"]
mod installation;
use crate::{
    nor::{NorRefusal, NorStorage},
    SnapshotRefusal,
};
pub use assigned::PreparedAssignedSnapshot;
use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, RequestId};
pub use installation::{ReadySnapshotPublisher, SnapshotNativeIdentity};
pub const SNAPSHOT_CAPABILITY: &str = "conduit/resource-snapshot-nor-publish@1";
pub const SNAPSHOT_PROFILE: &str = "conduit/resource-snapshot-nor-cooperative@1";
pub const SNAPSHOT_MECHANISM: &str = "resource/snapshot-nor";
pub const SNAPSHOT_CLASS: &str = "resource/snapshot@1";
pub const SNAPSHOT_KIND: &str = "resource/publish-json-snapshot";
pub const SNAPSHOT_REVISION: &str = "conduit.resource/json-snapshot@1";
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotCallRefusal {
    InvalidIdentity,
    WrongBinding,
    Possession,
    StaleRequest,
    OutputPressure,
    InvalidInput,
    Terminal,
    Capability(c::BaseCapabilityRefusal),
    Record(SnapshotRefusal),
    Media(NorRefusal),
}
#[derive(Clone, Copy)]
enum State {
    Ready,
    Published(usize),
    Failed,
    Cancelled,
}
pub struct SnapshotPublishHostCall<S> {
    operation: c::BoundBaseOperation,
    ready: ReadySnapshotPublisher<S>,
    node: NodeId,
    state: State,
}
impl<S: NorStorage> SnapshotPublishHostCall<S> {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.operation.inspection()
    }
    /// One exact request/generation. Output pressure is checked before media or
    /// authority consumption. A lost kernel result may be requested again only
    /// with the same bytes/request; cached completion never repeats publication.
    /// This return is not a kernel commit, delivery ACK or reclamation grant.
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, SnapshotCallRefusal> {
        use SnapshotCallRefusal as E;
        if node != self.node || call != HostCallId(0) {
            return Err(E::WrongBinding);
        }
        if request != RequestId(0) {
            return Err(E::StaleRequest);
        }
        if matches!(self.state, State::Cancelled)
            || self.operation.inspection().lifecycle == c::CapabilityLifecycle::Revoked
        {
            return Err(E::Terminal);
        }
        if output.len() < self.ready.reference.len() {
            return Err(E::OutputPressure);
        }
        match self.state {
            State::Published(length)
                if length == input.len()
                    && self.ready.record.candidate_record().is_some_and(|record| {
                        record.get(record.len() - length..) == Some(input)
                    }) =>
            {
                return Ok(self.copy_result(output))
            }
            State::Published(_) => return Err(E::StaleRequest),
            State::Failed | State::Cancelled => return Err(E::Terminal),
            State::Ready => {}
        }
        if input.len() > crate::MAXIMUM_SNAPSHOT_BYTES
            || conduit_web::validate_json_text(input).is_err()
        {
            return Err(E::InvalidInput);
        }
        // Validate exact extent/authority before taking a finite operation lease.
        self.ready
            .record
            .publication(&self.ready.authority, input)
            .map_err(E::Record)?;
        let lease = self.operation.authorize().map_err(E::Capability)?;
        self.state = State::Failed; // Any uncertain side effect is terminal.
        let published =
            self.ready
                .provider
                .publish(&mut self.ready.record, &self.ready.authority, input);
        let result_bytes = if published.is_ok() {
            self.ready.reference.len() as u32
        } else {
            0
        };
        if let Err(error) = self.operation.complete(lease, result_bytes) {
            let _ = self.operation.revoke();
            return Err(E::Capability(error));
        }
        published.map_err(E::Media)?;
        self.state = State::Published(input.len());
        Ok(self.copy_result(output))
    }
    fn copy_result(&self, out: &mut [u8]) -> usize {
        let n = self.ready.reference.len();
        out[..n].copy_from_slice(&self.ready.reference);
        n
    }
    pub fn revoke(&mut self) -> Result<(), SnapshotCallRefusal> {
        self.operation
            .revoke()
            .map_err(SnapshotCallRefusal::Capability)?;
        self.state = State::Cancelled;
        Ok(())
    }
    /// Revoked idle ownership returns the exact provider, never an erase grant.
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<ReadySnapshotPublisher<S>, SnapshotCallRefusal> {
        let current = owner.as_ref().ok_or(SnapshotCallRefusal::Possession)?;
        if current.inspection().lifecycle != c::CapabilityLifecycle::Revoked
            || current.inspection().in_flight_operations != 0
        {
            return Err(SnapshotCallRefusal::Terminal);
        }
        Ok(owner.take().expect("checked owner").ready)
    }
}
