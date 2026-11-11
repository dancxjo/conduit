//! Rich-Plan native service with one actual affine NOR residence owner.
//! Media publication, kernel acceptance and remote recording remain distinct.
use crate::{
    append::*,
    native_append::{AppendCustody, AppendNativeIdentity, AppendOutcome},
    nor::NorStorage,
    read_result::{GapReason, ReadResult},
};
use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, RequestId};
mod assigned;
pub use assigned::PreparedAssignedRecordService;
mod execution;
mod installation;
mod offer;
pub use offer::capability_offer;
mod selection;
pub use installation::ReadyRecordService;
pub use selection::PreparedRecordService;

pub const SERVICE_CAPABILITY: &str = "conduit/resource-record-service@1";
pub const SERVICE_PROFILE: &str = "conduit/resource-record-service-cooperative@1";
pub const READ_OPERATION: &str = "conduit.host/resource-read-record@1";
pub const READ_AUTHORITY: &str = "authority/resource-read-record@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceLimits {
    pub append_operations: u32,
    pub read_operations: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ServiceRefusal {
    Possession,
    WrongBinding,
    InvalidInput,
    OutputPressure,
    StaleRequest,
    CompletionPending,
    Terminal,
    Capability(c::BaseCapabilityRefusal),
    Media(RecordResidenceRefusal),
    Codec(c::StructuredInfoRefusal),
}
#[derive(Clone, Copy)]
struct Cached {
    request: RequestId,
    operation: usize,
    input_len: usize,
    committed: bool,
}
pub struct RecordServiceHostCall<S> {
    ready: ReadyRecordService<S>,
    operations: [c::BoundBaseOperation; 2],
    routes: [conduit_plan_lowering::selected_operation_set::SelectedServiceRoute; 2],
    node: NodeId,
    next_request: u32,
    cached: Option<Cached>,
    input: [u8; 200],
    custody: Option<AppendCustody>,
    append_failed: bool,
    terminal: bool,
}
impl<S: NorStorage> RecordServiceHostCall<S> {
    /// Retained heap payload only; inline state and physical storage backing
    /// remain separate charges. This observation is not a Root lease.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.operations
            .iter()
            .try_fold(self.ready.retained_backing_bytes()?, |total, op| {
                total.checked_add(op.retained_backing_bytes()?)
            })
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn routes(
        &self,
    ) -> &[conduit_plan_lowering::selected_operation_set::SelectedServiceRoute; 2] {
        &self.routes
    }
    pub fn custody(&self) -> Option<AppendCustody> {
        self.custody
    }
    pub fn inspection(&self, operation: usize) -> Option<&c::CapabilityInspection> {
        self.operations.get(operation).map(|op| op.inspection())
    }
    pub fn revoke(&mut self) -> Result<(), ServiceRefusal> {
        self.terminal = true;
        // Attempt both revocations even if the first one refuses.
        let append = self.operations[0].revoke();
        let read = self.operations[1].revoke();
        append.and(read).map_err(ServiceRefusal::Capability)
    }
    /// Revoked idle retirement preserves uncertain/unaccepted append evidence.
    /// No old-appender migration, renewed grant or durable ACK is provided here.
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<(ReadyRecordService<S>, Option<AppendCustody>), ServiceRefusal> {
        let current = owner.as_ref().ok_or(ServiceRefusal::Possession)?;
        if current.operations.iter().any(|op| {
            op.inspection().lifecycle != c::CapabilityLifecycle::Revoked
                || op.inspection().in_flight_operations != 0
        }) {
            return Err(ServiceRefusal::Terminal);
        }
        let current = owner.take().expect("checked sole service owner");
        Ok((current.ready, current.custody))
    }
}
#[cfg(test)]
#[path = "native_service/tests.rs"]
mod tests;
