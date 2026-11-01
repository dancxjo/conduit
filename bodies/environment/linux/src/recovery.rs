use conduit_core as c;
use conduit_environment_runtime::recording::{
    PreparedRecordingKernel, ReadyRecordingGenerations, ReadyRecordingRecovery,
    ReadyRecordingSingleton, RecordingBack, RecordingKernel, RecordingKernelRefusal,
    RecoveredHistory,
};
use conduit_kernel::{
    self as k,
    scheduler::{SchedulerError, SchedulerStatus},
    ValueStorage,
};
use conduit_linux_snapshot_store::{
    FileOperationRefusal, PreparedFileSnapshotOperation, SnapshotDirectory,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

#[derive(Debug)]
pub enum FileRecoveryRefusal {
    Scope,
    Shape,
    Preparation(RecordingKernelRefusal),
    File(FileOperationRefusal),
    Kernel(SchedulerError),
}
pub struct FileRecoveryCompletion {
    pub disposition: k::HostCallDisposition,
    pub refusal: Option<FileOperationRefusal>,
}
/// Owns the one production kernel and its actual prepared file executor. Root
/// still admits capacity/live capabilities and borrows each current Play from
/// its retained Body binding; incoming identity data is never such authority.
pub struct PreparedFileRecovery<'a> {
    active: c::ActivePlayIdentity,
    kernel: Box<RecordingKernel>,
    read: k::NodeId,
    recovery: k::NodeId,
    files: PreparedFileSnapshotOperation<'a>,
}
impl<'a> PreparedFileRecovery<'a> {
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &c::ActivePlayIdentity,
        generations: &mut ReadyRecordingGenerations,
        singleton: &mut ReadyRecordingSingleton,
        recovery: &mut ReadyRecordingRecovery,
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
        let [call] = lowered.host_calls.as_slice() else {
            return Err(E::Shape);
        };
        if call.contract_id.as_str() != conduit_resource_snapshot::READ_OPERATION {
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
        // File preparation precedes transfer of any retained software owner.
        let files = PreparedFileSnapshotOperation::prepare(directory, placement, node.node)
            .map_err(E::File)?;
        let prepared =
            PreparedRecordingKernel::prepare(fragment, lowered, generations, singleton, recovery)
                .map_err(E::Preparation)?;
        Ok(Self {
            active: active.clone(),
            kernel: Box::new(prepared.kernel),
            read: prepared.roles.read,
            recovery: prepared.roles.recovery,
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
        if request.node != self.read {
            return Err(E::Shape);
        }
        let input = self
            .kernel
            .values()
            .get(request.input.value)
            .map_err(|_| E::Shape)?;
        let mut refusal = None;
        let outcome = match self.files.perform(request, input) {
            Ok(bytes) => match self.kernel.store_host_value(bytes) {
                Ok(value) => k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(
                        k::BoundedValueRef::new(value, self.files.output_bound())
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
        self.files.finish(request).map_err(E::File)?;
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
    pub fn kernel(&self) -> &RecordingKernel {
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
