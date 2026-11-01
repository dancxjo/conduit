//! Exact request correlation around file effects. Kernel completion and current
//! Body/Play custody remain with Root; effect success is not a committed Step.
use crate::{FileSnapshotRefusal, PreparedFileSnapshot, SnapshotDirectory};
use conduit_core::{AuthorityBinding, PlannedGear};
use conduit_kernel::{scheduler::HostCallRequest, Failure, FailureCode, NodeId};
use conduit_resource_snapshot::{snapshot_reference, PreparedSnapshotOperation};

#[derive(Debug)]
pub enum FileOperationRefusal {
    Preparation(String),
    Operation(Failure),
    Storage(FileSnapshotRefusal),
}
impl From<Failure> for FileOperationRefusal {
    fn from(value: Failure) -> Self {
        Self::Operation(value)
    }
}
impl From<FileSnapshotRefusal> for FileOperationRefusal {
    fn from(value: FileSnapshotRefusal) -> Self {
        Self::Storage(value)
    }
}
pub struct PreparedFileSnapshotOperation<'a> {
    operation: PreparedSnapshotOperation,
    files: PreparedFileSnapshot<'a>,
    authority: AuthorityBinding,
    effect_completed: Option<HostCallRequest>,
}
impl<'a> PreparedFileSnapshotOperation<'a> {
    /// Root independently supplies actual directory custody and the selected
    /// lowered node. This does not admit a Host/Boot/Play or offer storage.
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        placement: &PlannedGear,
        node: NodeId,
    ) -> Result<Self, FileOperationRefusal> {
        let reference = snapshot_reference(placement).map_err(FileOperationRefusal::Preparation)?;
        let operation = PreparedSnapshotOperation::prepare(placement, node)
            .map_err(FileOperationRefusal::Preparation)?;
        let files = PreparedFileSnapshot::prepare(directory, placement, &reference)?;
        Ok(Self {
            operation,
            files,
            authority: placement.authority[0].clone(),
            effect_completed: None,
        })
    }
    pub fn output_bound(&self) -> u32 {
        self.operation.output_bound()
    }
    /// Validate before file work and perform at most once per pending request.
    /// Synchronous OS effects execute outside the allocation-sealed kernel Step.
    pub fn perform(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
    ) -> Result<&[u8], FileOperationRefusal> {
        self.operation.begin(request, input)?;
        self.effect_completed = None;
        if self.operation.is_publish() {
            self.files.publish(&self.authority, input)?;
        } else {
            self.files.load_record(&self.authority)?;
        }
        self.effect_completed = Some(request);
        self.completion(request)
    }
    /// Output pressure may revisit these retained bytes without repeating I/O.
    /// Caller stores them and completes the exact original kernel request.
    pub fn completion(&self, request: HostCallRequest) -> Result<&[u8], FileOperationRefusal> {
        if self.effect_completed != Some(request) {
            return Err(FileOperationRefusal::Operation(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 215,
            }));
        }
        let record = if self.operation.is_publish() {
            None
        } else {
            Some(self.files.cached_record())
        };
        self.operation
            .completion(request, Ok(record))
            .map_err(Into::into)
    }
    /// Call after the kernel completion attempt or cancellation. This retires
    /// correlation, never retries an uncertain effect or acknowledges a Step.
    pub fn finish(&mut self, request: HostCallRequest) -> Result<(), FileOperationRefusal> {
        self.operation.finish(request)?;
        self.effect_completed = None;
        Ok(())
    }
}
