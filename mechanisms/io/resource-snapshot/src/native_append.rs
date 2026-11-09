//! Ordinary finite native append Host Call. Media commit, kernel commit and
//! remote acknowledgment are three separate boundaries. No erasure or catalog.
#[path = "native_append/assigned.rs"]
mod assigned;
#[path = "native_append/installation.rs"]
mod installation;
use crate::{append::*, nor::NorStorage};
pub use assigned::PreparedAssignedAppender;
use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, RequestId};
pub use installation::{AppendAttachment, AppendNativeIdentity, ReadyRecordAppender};

pub const APPEND_KIND: &str = "resource/try-append-record";
pub const APPEND_REVISION: &str = "conduit.resource/try-append-record@1";
pub const APPEND_OPERATION: &str = "conduit.host/resource-try-append-record@1";
pub const APPEND_AUTHORITY: &str = "authority/resource-try-append-record@1";
pub const APPEND_CAPABILITY: &str = "conduit/resource-try-append-record@1";
pub const APPEND_PROFILE: &str = "conduit/resource-try-append-record-cooperative@1";
pub const APPEND_MECHANISM: &str = "resource/nor-record-residence";
/// One unit is affine possession of the entire independently checked finite
/// residence. Ready's exact bytes/slots/work require separate Root funding.
pub const APPEND_CLASS: &str = "machine/nor-record-residence@1";
pub use crate::append_result::APPEND_RESULT_BYTES;
#[path = "native_append/result.rs"]
mod result;
pub use result::{AppendOutcome, APPEND_ENCODER_MAXIMUM_BACKING_BYTES};

#[derive(Debug, PartialEq, Eq)]
pub enum AppendCallRefusal {
    InvalidIdentity,
    WrongBinding,
    Possession,
    StaleRequest,
    CompletionPending,
    OutputPressure,
    InvalidInput,
    Terminal,
    Capability(c::BaseCapabilityRefusal),
    Media(RecordResidenceRefusal),
    Codec(c::StructuredInfoRefusal),
}
/// Preserved even after revocation. `None` means the media attempt was uncertain;
/// it is not proof of absence. Recovery must expose any occupied/corrupt slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppendCustody {
    pub request: RequestId,
    pub committed_index: Option<DurableRecordIndex>,
    pub kernel_committed: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Available,
    Failed,
    Revoked,
}
pub struct RecordAppendHostCall<S> {
    operation: c::BoundBaseOperation,
    ready: ReadyRecordAppender<S>,
    node: NodeId,
    next_request: u32,
    state: State,
    custody: Option<AppendCustody>,
    input: [u8; MAXIMUM_RECORD_PAYLOAD_BYTES],
    input_len: usize,
    cached: Option<Cached>,
    write_failed: bool,
}
#[derive(Clone, Copy)]
struct Cached {
    request: RequestId,
    outcome: AppendOutcome,
    kernel_committed: bool,
}
impl<S: NorStorage> RecordAppendHostCall<S> {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.operation.inspection()
    }
    pub fn custody(&self) -> Option<AppendCustody> {
        self.custody
    }
    pub fn remaining_slots(&self) -> Option<u32> {
        self.ready.provider.remaining_slots()
    }
    /// Exact retry of the latest request returns its cached declared outcome without authorizing/programming again. A new request waits for the
    /// actual kernel-completion acknowledgment, not merely this media result.
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, AppendCallRefusal> {
        use AppendCallRefusal as E;
        self.check_binding(node, call)?;
        if self.state != State::Available
            || self.inspection().lifecycle == c::CapabilityLifecycle::Revoked
        {
            return Err(E::Terminal);
        }
        if output.len() < APPEND_RESULT_BYTES as usize {
            return Err(E::OutputPressure);
        }
        if let Some(previous) = self.cached {
            if request == previous.request {
                if input != &self.input[..self.input_len] {
                    return Err(E::StaleRequest);
                }
                return Ok(self.copy_result(output));
            }
            if !previous.kernel_committed {
                return Err(E::CompletionPending);
            }
        }
        if request != RequestId(self.next_request) || self.next_request == u32::MAX {
            return Err(E::StaleRequest);
        }
        if input.len() > MAXIMUM_RECORD_PAYLOAD_BYTES {
            return Err(E::InvalidInput);
        }
        let lease = self.operation.authorize().map_err(E::Capability)?;
        self.state = State::Failed;
        self.input[..input.len()].copy_from_slice(input);
        self.input_len = input.len();
        let outcome = if self.write_failed {
            AppendOutcome::RecoveryRequired
        } else if self.remaining_slots() == Some(0) {
            AppendOutcome::Capacity
        } else {
            self.custody = Some(AppendCustody {
                request,
                committed_index: None,
                kernel_committed: false,
            });
            match self.ready.provider.append(input) {
                Ok(index) => {
                    self.custody
                        .as_mut()
                        .expect("attempt retained")
                        .committed_index = Some(index);
                    AppendOutcome::Recorded(index)
                }
                Err(_) => {
                    self.write_failed = true;
                    AppendOutcome::Uncertain
                }
            }
        };
        let encoded = self.ready.encoder.encode(outcome);
        let length = encoded.as_ref().map_or(0, |v| v.len() as u32);
        if let Err(error) = self.operation.complete(lease, length) {
            let _ = self.operation.revoke();
            return Err(E::Capability(error));
        }
        encoded.map_err(E::Codec)?;
        self.cached = Some(Cached {
            request,
            outcome,
            kernel_committed: false,
        });
        self.state = State::Available;
        Ok(self.copy_result(output))
    }
    /// Root calls only after the original kernel accepted the exact request and
    /// declared outcome. This is not a remote recording or delivery ACK.
    pub fn acknowledge_kernel_completion(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        outcome: AppendOutcome,
    ) -> Result<(), AppendCallRefusal> {
        self.check_binding(node, call)?;
        if self.state != State::Available {
            return Err(AppendCallRefusal::Terminal);
        }
        let cached = self
            .cached
            .as_mut()
            .ok_or(AppendCallRefusal::StaleRequest)?;
        if cached.request != request || cached.outcome != outcome {
            return Err(AppendCallRefusal::StaleRequest);
        }
        if !cached.kernel_committed {
            self.next_request = request
                .0
                .checked_add(1)
                .ok_or(AppendCallRefusal::StaleRequest)?;
            cached.kernel_committed = true;
            if let Some(custody) = self.custody.as_mut().filter(|c| c.request == request) {
                custody.kernel_committed = true;
            }
        }
        Ok(())
    }
    pub fn pending_outcome(&self) -> Option<(RequestId, AppendOutcome, bool)> {
        self.cached
            .map(|c| (c.request, c.outcome, c.kernel_committed))
    }
    fn copy_result(&self, output: &mut [u8]) -> usize {
        let encoded = self.ready.encoder.encoded();
        output[..encoded.len()].copy_from_slice(encoded);
        encoded.len()
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), AppendCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(AppendCallRefusal::WrongBinding);
        }
        Ok(())
    }
    pub fn revoke(&mut self) -> Result<(), AppendCallRefusal> {
        self.operation
            .revoke()
            .map_err(AppendCallRefusal::Capability)?;
        self.state = State::Revoked;
        Ok(())
    }
    /// Exact revoked idle custody only. No erase, renewed grant or history ACK.
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<(ReadyRecordAppender<S>, Option<AppendCustody>), AppendCallRefusal> {
        let current = owner.as_ref().ok_or(AppendCallRefusal::Possession)?;
        if current.inspection().lifecycle != c::CapabilityLifecycle::Revoked
            || current.inspection().in_flight_operations != 0
        {
            return Err(AppendCallRefusal::Terminal);
        }
        let current = owner.take().expect("checked owner");
        Ok((current.ready, current.custody))
    }
}
