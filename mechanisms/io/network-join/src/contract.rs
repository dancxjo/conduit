//! Finite semantic request/result representation, independent of radio truth.
use conduit_core::{Kind, KindId};
use conduit_net::*;
use conduit_plan_lowering::selected_operation::SelectedOperationContract;
/// Only an initialized native radio provider may establish attachment truth.
/// Join includes the independently admitted withdrawal/association/address work.
/// Both methods must obey Root's finite deadlines and storage bounds. Dropping
/// join must quiesce pending work. Root owes bounded cleanup after failure or
/// cancellation, and when a successful attachment is retired. A successful join
/// may outlive its configuration Play if Root retains this actual owner and its
/// attachment resources. Neither method may grow storage beyond Root's admitted
/// finite native reservation. Any SDK allocator must be confined to that budget;
/// it cannot fall back to unadmitted storage. The Host Call owner itself reserves
/// its recurring buffers before Play.
pub trait NetworkJoinProvider {
    fn join(
        &mut self,
        request: NetworkJoinRequest<'_>,
    ) -> impl core::future::Future<Output = Result<AttachmentRecord, StationAttemptFailure>>;
    fn cleanup(&mut self) -> impl core::future::Future<Output = Result<(), StationFailure>>;
}

/// Fixed-capacity semantic output. Construction validates representation, not
/// physical attachment or authority. The selected owner checks current context.
pub struct AttachmentRecord {
    bytes: [u8; MAXIMUM_JOIN_OUTPUT_BYTES as usize],
    length: usize,
}
impl AttachmentRecord {
    pub fn encode(attachment: NetworkAttachmentInfo<'_>) -> Result<Self, NetworkJoinError> {
        let mut record = Self {
            bytes: [0; MAXIMUM_JOIN_OUTPUT_BYTES as usize],
            length: 0,
        };
        record.length = encode_network_attachment(attachment, &mut record.bytes)?;
        Ok(record)
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

/// Prepared from the actual semantic contract; callers cannot substitute a Kind.
pub struct PreparedJoinContract {
    kind: Kind,
    pub(crate) subject: KindId,
}
impl Default for PreparedJoinContract {
    fn default() -> Self {
        Self::prepare()
    }
}
impl PreparedJoinContract {
    pub fn prepare() -> Self {
        Self {
            kind: network_join_contract(),
            subject: NETWORK_CONFIG_SUBJECT.into(),
        }
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }
    pub(crate) fn definition(&self) -> SelectedOperationContract<'_> {
        SelectedOperationContract {
            kind: &self.kind,
            call: NETWORK_JOIN_HOST_CALL,
            input_bytes: MAXIMUM_JOIN_INPUT_BYTES,
            output_bytes: MAXIMUM_JOIN_OUTPUT_BYTES,
            resource_bytes: 1,
        }
    }
}
