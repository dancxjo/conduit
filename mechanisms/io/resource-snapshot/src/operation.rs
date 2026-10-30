//! One prepared snapshot Host Call, independent of any particular kernel driver
//! set. This validates and correlates bytes; it does not execute storage.
use super::*;
use alloc::{format, string::String, vec::Vec};
use conduit_core::{AuthorityBinding, PlannedGear};
use conduit_kernel::{
    scheduler::HostCallRequest, Failure, FailureCode, HostCallId, NodeId, RequestId,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SnapshotStorageRequest<'a> {
    pub effect_kind: &'static str,
    pub key: &'a str,
    pub record: Option<&'a [u8]>,
}
pub struct PreparedSnapshotOperation {
    record: PreparedSnapshotRecord,
    reference: Vec<u8>,
    authority: AuthorityBinding,
    node: NodeId,
    publish: bool,
    pending: Option<HostCallRequest>,
    last: Option<RequestId>,
}
impl PreparedSnapshotOperation {
    /// Root supplies the selected placement and its ordinary lowered node.
    /// Root must retain current Host/Boot/Play custody, actual storage and the
    /// admitted memory/work closure separately; this creates none of them.
    pub fn prepare(placement: &PlannedGear, node: NodeId) -> Result<Self, String> {
        let reference = crate::snapshot_reference(placement)?;
        let record = PreparedSnapshotRecord::prepare(placement, &reference)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            record,
            reference: reference.encode().map_err(|error| format!("{error:?}"))?,
            authority: placement.authority[0].clone(),
            node,
            publish: placement.host_calls[0].contract_id.as_str() == PUBLISH_OPERATION,
            pending: None,
            last: None,
        })
    }
    pub fn is_publish(&self) -> bool {
        self.publish
    }
    pub fn output_bound(&self) -> u32 {
        if self.publish {
            512
        } else {
            4096
        }
    }
    /// Only a current kernel request may begin an effect. Overlap, replay and
    /// foreign node/call input refuse without replacing the pending request.
    pub fn begin(&mut self, request: HostCallRequest, input: &[u8]) -> Result<(), Failure> {
        if self.pending.is_some() || self.last.is_some_and(|last| request.request.0 <= last.0) {
            return Err(lifecycle());
        }
        let bound = if self.publish { 4096 } else { 512 };
        if request.node != self.node
            || request.call != HostCallId(0)
            || request.input.admitted_bytes != bound
            || request.input.value.byte_len > bound
            || input.len() != request.input.value.byte_len as usize
        {
            return Err(Failure {
                code: FailureCode::InvalidInput,
                detail: 213,
            });
        }
        if self.publish {
            conduit_web::validate_json_text(input).map_err(|error| Failure {
                code: FailureCode::InvalidInput,
                detail: error as u16,
            })?;
            self.record
                .publication(&self.authority, input)
                .map_err(snapshot_failure)?;
        } else if input != self.reference {
            return Err(Failure {
                code: FailureCode::InvalidInput,
                detail: 208,
            });
        }
        self.pending = Some(request);
        self.last = Some(request.request);
        Ok(())
    }
    pub fn describe(
        &self,
        request: HostCallRequest,
    ) -> Result<SnapshotStorageRequest<'_>, Failure> {
        self.check_pending(request)?;
        Ok(SnapshotStorageRequest {
            effect_kind: if self.publish {
                "resource-publish"
            } else {
                "resource-read"
            },
            key: self.record.storage_key(),
            record: if self.publish {
                self.record.candidate_record()
            } else {
                None
            },
        })
    }
    /// Validate the storage result for the original pending request. The caller
    /// stores these bytes and completes that exact kernel request separately.
    /// This is not a committed kernel acknowledgement or recovery publication.
    pub fn completion<'a>(
        &'a self,
        request: HostCallRequest,
        result: Result<Option<&'a [u8]>, Failure>,
    ) -> Result<&'a [u8], Failure> {
        self.check_pending(request)?;
        match (self.publish, result?) {
            (true, None) => Ok(&self.reference),
            (false, Some(record)) => {
                let content = self
                    .record
                    .restore(&self.authority, record)
                    .map_err(snapshot_failure)?;
                conduit_web::validate_json_text(content).map_err(|_| Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 214,
                })?;
                Ok(content)
            }
            _ => Err(Failure {
                code: FailureCode::HostCallFailed,
                detail: 209,
            }),
        }
    }
    /// Retire the effect after the caller has attempted kernel completion. A
    /// failed/retired kernel cannot implicitly retry the external operation.
    pub fn finish(&mut self, request: HostCallRequest) -> Result<(), Failure> {
        self.check_pending(request)?;
        self.pending = None;
        Ok(())
    }
    fn check_pending(&self, request: HostCallRequest) -> Result<(), Failure> {
        if self.pending == Some(request) {
            Ok(())
        } else {
            Err(lifecycle())
        }
    }
}
fn lifecycle() -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 212,
    }
}
fn snapshot_failure(error: SnapshotRefusal) -> Failure {
    let detail = match error {
        SnapshotRefusal::InvalidBinding => 200,
        SnapshotRefusal::ForeignHost => 201,
        SnapshotRefusal::StaleBoot => 202,
        SnapshotRefusal::UnsupportedLifetime => 203,
        SnapshotRefusal::AuthorityDenied => 204,
        SnapshotRefusal::Reference(_) => 205,
        SnapshotRefusal::WrongAccess => 206,
        SnapshotRefusal::ContentExtent => 207,
        SnapshotRefusal::CorruptRecord => 208,
        SnapshotRefusal::UnsupportedExpiry => 210,
    };
    Failure {
        code: FailureCode::HostCallFailed,
        detail,
    }
}

#[cfg(test)]
#[path = "operation_tests.rs"]
mod tests;
