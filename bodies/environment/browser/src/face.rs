//! Project only the selected recovery Back's committed state into the existing
//! Face. Browser and HTTP Masks consume this same Face, never parallel counters.
use conduit_core as c;
use conduit_environment_face::{
    PreparedRecordingFace, RecordingFaceRefusal, RecordingRecoveryFailure, RecoveredRecordingRange,
};
use conduit_environment_runtime::recording::{RecordingBack, RecordingBatchKernel};
use conduit_face::Face;
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingBrowserFaceRefusal {
    Binding,
    State,
    Face(RecordingFaceRefusal),
}
pub struct PreparedRecordingBrowserFace {
    recovery: NodeId,
    face: PreparedRecordingFace,
}
impl PreparedRecordingBrowserFace {
    /// Root supplies a fresh prepared Face with its actual contribution basis.
    /// This validates provenance but admits neither execution nor disclosure.
    /// Root must retain this exact kernel/Face pair and current Body custody.
    pub fn prepare(
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        recovery: NodeId,
        active: &c::ActivePlayIdentity,
        face: PreparedRecordingFace,
    ) -> Result<Self, RecordingBrowserFaceRefusal> {
        use RecordingBrowserFaceRefusal as E;
        let basis = &face.current().basis;
        if !c::verify_plan_fragment(fragment)
            || lower_plan_fragment(fragment).map_err(|_| E::Binding)? != *lowered
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || *active
                != c::bind_active_play(
                    &fragment.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    active.play_sequence,
                )
            || basis.source_document_id.as_ref() != Some(&fragment.source_document_id)
            || basis.checked_plot_id.as_ref() != Some(&fragment.checked_plot_id)
            || basis.expanded_plot_id.as_ref() != Some(&fragment.expanded_plot_id)
            || basis.plan_id.as_ref() != Some(&fragment.plan_id)
            || basis.active_play_id.as_ref() != Some(&active.active_play_id)
            || face.current().revision != 0
        {
            return Err(E::Binding);
        }
        let node = lowered
            .nodes
            .iter()
            .find(|node| node.node == recovery)
            .ok_or(E::Binding)?;
        if !fragment.placements.iter().any(|placement| {
            placement.placement_id == node.placement_id
                && placement.kind_id.as_str() == "environment/recover-recording"
        }) {
            return Err(E::Binding);
        }
        Ok(Self { recovery, face })
    }
    pub fn current(&self) -> &Face {
        self.face.current()
    }
    /// Root supplies an observed refusal; the Mask does not infer one from
    /// absent progress. The existing Face retains its last committed range.
    pub fn publish_refusal(
        &mut self,
        failure: RecordingRecoveryFailure,
    ) -> Result<(), RecordingBrowserFaceRefusal> {
        self.face
            .publish_refusal(failure)
            .map_err(RecordingBrowserFaceRefusal::Face)
    }
    /// Pending I/O and validated but unconsumed completions have no effect.
    /// Successful repeated projection is idempotent. No recovery state is copied
    /// outside the existing Face projection and no runtime storage grows here.
    pub fn refresh<const N: usize, const C: usize>(
        &mut self,
        kernel: &RecordingBatchKernel<N, C>,
    ) -> Result<(), RecordingBrowserFaceRefusal> {
        use RecordingBrowserFaceRefusal as E;
        let Some(RecordingBack::Recovery(back)) =
            kernel.drivers().get(usize::from(self.recovery.0))
        else {
            return Err(E::Binding);
        };
        let recovered = back.recovered();
        if recovered.chunks == 0 {
            if recovered.samples != 0
                || recovered.first_sequence.is_some()
                || recovered.last_sequence.is_some()
            {
                return Err(E::State);
            }
            return Ok(());
        }
        self.face
            .publish_recovered(RecoveredRecordingRange {
                chunks: recovered.chunks,
                samples: recovered.samples,
                first_sequence: recovered.first_sequence.ok_or(E::State)?,
                last_sequence: recovered.last_sequence.ok_or(E::State)?,
            })
            .map_err(E::Face)
    }
}
