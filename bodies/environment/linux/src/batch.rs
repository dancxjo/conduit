use conduit_core as c;
use conduit_environment_runtime::recording::{
    PreparedRecordingBatchKernel, ReadyRecordingConcat, ReadyRecordingGenerations,
    ReadyRecordingRecovery, ReadyRecordingSingleton, RecordingBack, RecordingBatchKernel,
    RecoveredHistory,
};
use conduit_kernel::{self as k, scheduler::SchedulerStatus, ValueStorage};
use conduit_linux_snapshot_store::{
    FileOperationRefusal, PreparedFileSnapshotOperation, SnapshotDirectory,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

use crate::{FileRecoveryCompletion, FileRecoveryRefusal};
/// Borrowed retained software owners; rejected preparation transfers none.
pub struct FileBatchSoftware<'a> {
    pub generations: &'a mut [ReadyRecordingGenerations],
    pub singletons: &'a mut [ReadyRecordingSingleton],
    pub concats: &'a mut [ReadyRecordingConcat],
    pub recovery: &'a mut ReadyRecordingRecovery,
}
/// Owns one ordinary batch kernel and one exact file executor per selected read.
/// Root separately admits capacity and live authority and retains Body custody.
pub struct PreparedFileBatchRecovery<'a, const N: usize, const C: usize> {
    active: c::ActivePlayIdentity,
    kernel: Box<RecordingBatchKernel<N, C>>,
    recovery: k::NodeId,
    files: Vec<(k::NodeId, PreparedFileSnapshotOperation<'a>)>,
}
impl<'a, const N: usize, const C: usize> PreparedFileBatchRecovery<'a, N, C> {
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &c::ActivePlayIdentity,
        software: FileBatchSoftware<'_>,
    ) -> Result<Self, FileRecoveryRefusal> {
        use FileRecoveryRefusal as E;
        if !c::verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || *active
                != c::bind_active_play(
                    &active.plan_id,
                    &active.host_id,
                    &active.boot_id,
                    active.play_sequence,
                )
        {
            return Err(E::Scope);
        }
        if lower_plan_fragment(fragment).map_err(|_| E::Shape)? != *lowered {
            return Err(E::Shape);
        }
        if lowered.host_calls.is_empty()
            || lowered.host_calls.len()
                > conduit_environment_runtime::recording::MAXIMUM_CATALOG_GENERATIONS
        {
            return Err(E::Shape);
        }
        // Prepare every file executor before transferring any software owner.
        let mut files = Vec::with_capacity(lowered.host_calls.len());
        for call in &lowered.host_calls {
            if call.contract_id.as_str() != conduit_resource_snapshot::READ_OPERATION
                || files.iter().any(|(node, _)| *node == call.node)
            {
                return Err(E::Shape);
            }
            let node = lowered
                .nodes
                .iter()
                .find(|n| n.node == call.node)
                .ok_or(E::Shape)?;
            let placement = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == node.placement_id)
                .ok_or(E::Shape)?;
            files.push((
                node.node,
                PreparedFileSnapshotOperation::prepare(directory, placement, node.node)
                    .map_err(E::File)?,
            ));
        }
        let prepared = PreparedRecordingBatchKernel::<N, C>::prepare(
            fragment,
            lowered,
            software.generations,
            software.singletons,
            software.concats,
            software.recovery,
        )
        .map_err(E::Preparation)?;
        Ok(Self {
            active: active.clone(),
            kernel: prepared.kernel,
            recovery: prepared.recovery_node,
            files,
        })
    }
    fn current(&self, active: &c::ActivePlayIdentity) -> Result<(), FileRecoveryRefusal> {
        if active == &self.active {
            Ok(())
        } else {
            Err(FileRecoveryRefusal::Scope)
        }
    }
    /// Exactly one ordinary kernel Step, not an alternate scheduler. Root must
    /// first obtain this current scope from its actual started Body owner.
    pub fn step(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<SchedulerStatus, FileRecoveryRefusal> {
        self.current(active)?;
        self.kernel.step().map_err(FileRecoveryRefusal::Kernel)
    }
    /// Service at most one original request from this owned kernel. No external
    /// request, completion, counter or kernel object may be substituted.
    pub fn service_next(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<Option<FileRecoveryCompletion>, FileRecoveryRefusal> {
        use FileRecoveryRefusal as E;
        self.current(active)?;
        let Some(request) = self.kernel.next_host_request() else {
            return Ok(None);
        };
        let (_, files) = self
            .files
            .iter_mut()
            .find(|(node, _)| *node == request.node)
            .ok_or(E::Shape)?;
        let input = self
            .kernel
            .values()
            .get(request.input.value)
            .map_err(|_| E::Shape)?;
        let mut refusal = None;
        let outcome = match files.perform(request, input) {
            Ok(bytes) => match self.kernel.store_host_value(bytes) {
                Ok(value) => k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(
                        k::BoundedValueRef::new(value, files.output_bound())
                            .map_err(|_| E::Shape)?,
                    ),
                    failure: None,
                },
                Err(_) => failed(k::Failure {
                    code: k::FailureCode::HostCallFailed,
                    detail: 217,
                }),
            },
            Err(error) => {
                let failure = match &error {
                    FileOperationRefusal::Operation(failure) => *failure,
                    _ => k::Failure {
                        code: k::FailureCode::HostCallFailed,
                        detail: 216,
                    },
                };
                refusal = Some(error);
                failed(failure)
            }
        };
        let disposition = outcome.disposition;
        let output = outcome.output.map(|value| value.value);
        let completed = self
            .kernel
            .complete_host_call(request.node, request.request, outcome);
        files.finish(request).map_err(E::File)?;
        if completed.is_err() {
            if let Some(value) = output {
                self.kernel
                    .discard_host_value(value)
                    .map_err(|_| E::Shape)?;
            }
        }
        completed.map_err(E::Kernel)?;
        Ok(Some(FileRecoveryCompletion {
            disposition,
            refusal,
        }))
    }
    /// Cleanup uses Root's retained exact reservation scope, including when the
    /// ordinary owner can no longer supply a running-Play loan.
    pub fn cancel(&mut self, reserved: &c::ActivePlayIdentity) -> Result<(), FileRecoveryRefusal> {
        self.current(reserved)?;
        self.kernel.cancel().map_err(FileRecoveryRefusal::Kernel)
    }
    /// Read-only inspection of the one retained kernel; no mutable substitute.
    pub fn kernel(&self) -> &RecordingBatchKernel<N, C> {
        &self.kernel
    }
    pub fn history(&self) -> RecoveredHistory {
        let RecordingBack::Recovery(back) = &self.kernel.drivers()[usize::from(self.recovery.0)]
        else {
            unreachable!("prepared recovery role")
        };
        back.recovered()
    }
}
fn failed(failure: k::Failure) -> k::HostCallOutcome {
    k::HostCallOutcome {
        disposition: k::HostCallDisposition::Failed,
        output: None,
        failure: Some(failure),
    }
}
