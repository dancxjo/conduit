//! One current, opaque kernel-committed contribution. Neither labels nor
//! transport delivery authorize a contribution, and terminal Plays are historical.
use super::Owner;
use conduit_face::{
    BodyFace, Face, FaceContext, FaceContribution, FaceContributionRole, FaceFocus, FaceNames,
};
use conduit_std_host::body_execution::{
    BodyFaceContributionObserver, CommittedFaceContribution, FaceContributionRefusal,
    ReturnedFaceContribution,
};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

pub(super) struct RuntimeFaceSender(SyncSender<CommittedFaceContribution>);
pub(super) struct RuntimeFaceReceiver(Receiver<CommittedFaceContribution>);
/// One receiver slot and one owner's latest frame. Immutable frame backing
/// retains its original execution lease; channel/cache metadata belongs to the
/// installed owner's separately bounded service, not the producer's workspace.
pub(super) fn channel() -> (RuntimeFaceSender, RuntimeFaceReceiver) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (RuntimeFaceSender(sender), RuntimeFaceReceiver(receiver))
}
impl BodyFaceContributionObserver for RuntimeFaceSender {
    fn contribute(
        &mut self,
        contribution: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        self.0.try_send(contribution).map_err(|error| match error {
            TrySendError::Full(contribution) => ReturnedFaceContribution {
                contribution,
                refusal: FaceContributionRefusal::Pressure,
            },
            TrySendError::Disconnected(contribution) => ReturnedFaceContribution {
                contribution,
                refusal: FaceContributionRefusal::Revoked,
            },
        })
    }
}
impl RuntimeFaceReceiver {
    pub(super) fn progress(&self, owner: &mut Owner) -> Result<(), String> {
        if owner
            .runtime_face
            .as_ref()
            .is_some_and(|frame| !owner.runtime_face_is_current(frame))
        {
            owner.runtime_face = None;
        }
        // A single bounded slot: never drain an unbounded producer queue.
        if let Ok(frame) = self.0.try_recv() {
            if !owner.runtime_face_scope_matches_current(&frame) {
                return Err(
                    "committed Face differs from current Body/Host/Boot/Plan/Play/workset".into(),
                );
            }
            if let Some(previous) = &owner.runtime_face {
                if previous.scope().body_play() != frame.scope().body_play()
                    || previous.scope().plot() != frame.scope().plot()
                    || previous.scope().placement() != frame.scope().placement()
                    || previous.scope().call() != frame.scope().call()
                    || previous.request() >= frame.request()
                {
                    return Err(
                        "committed Face route or monotonic request changed within current Play"
                            .into(),
                    );
                }
            }
            if frame.scope().is_retired() {
                owner.runtime_face = None;
            } else {
                owner.runtime_face = Some(frame);
            }
        }
        Ok(())
    }
}
impl Owner {
    fn runtime_face_is_current(&self, frame: &CommittedFaceContribution) -> bool {
        !frame.scope().is_retired() && self.runtime_face_scope_matches_current(frame)
    }
    fn runtime_face_scope_matches_current(&self, frame: &CommittedFaceContribution) -> bool {
        if self.workset_recovery_required || self.execution_persistence_pending {
            return false;
        }
        let scope = frame.scope();
        let host = self.host.advertisement();
        let Some(realization) = self.session.realization() else {
            return false;
        };
        let Some(play) = &realization.play else {
            return false;
        };
        if play != scope.body_play()
            || self.session.evidence().body_id != play.body_id
            || realization.wake.body_id != play.body_id
            || realization.wake.wake_id != play.wake_id
            || realization.plan.plan_id != play.plan_id
            || realization.plan.validate_for(&realization.wake).is_err()
            || !play.validate_for(&realization.plan)
            || scope.host() != &host.host_id
            || scope.boot() != &host.boot_id
            || frame.fragment().basis.checked_plot_id != scope.plot().checked_plot_id
            || frame.fragment().basis.plan_id != play.plan_id
            || frame.fragment().basis.active_play_id != play.active_play_id
            || frame
                .fragment()
                .basis
                .required_interaction_context
                .is_some()
        {
            return false;
        }
        realization.plan.plots.iter().any(|entry| {
            entry.plan.plan_id == *scope.partition_plan()
                && entry.plot.source_document_id == scope.plot().source_document_id
                && entry.plot.checked_plot_id == scope.plot().checked_plot_id
                && self
                    .session
                    .evidence()
                    .body
                    .workset
                    .plots()
                    .contains(&entry.plot)
                && entry.plan.fragments.iter().any(|fragment| {
                    fragment.fragment_id == *scope.fragment()
                        && fragment.expanded_plot_id == scope.plot().expanded_plot_id
                        && fragment.host_id == *scope.host()
                        && fragment.boot_id == *scope.boot()
                        && fragment.offer_generation == host.offer_generation
                        && fragment.placements.iter().any(|placement| {
                            placement.placement_id == *scope.placement()
                                && placement.host_id == *scope.host()
                                && placement.boot_id == *scope.boot()
                                && placement.host_calls.get(scope.call().0 as usize).is_some()
                        })
                })
        })
    }
    pub(super) fn project_runtime_face(&self) -> Result<Option<Face>, String> {
        let Some(frame) = &self.runtime_face else {
            return Ok(None);
        };
        if !self.runtime_face_is_current(frame) {
            return Ok(None);
        }
        let revision = self
            .session
            .evidence()
            .last_sequence()
            .checked_add(u64::from(frame.request().0) + 1)
            .ok_or("runtime Face revision exhausted")?;
        // This clone is bounded projection workspace owned by the snapshot
        // service. It neither creates a committed token nor changes its basis.
        let projected = BodyFace::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_face(
                FaceContributionRole::Inspection,
                frame.fragment().clone(),
            )],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &[],
            },
        )
        .map_err(|e| format!("current committed Face projection refused: {e:?}"))?;
        projected
            .face
            .validate()
            .map_err(|e| format!("current committed Face invalid: {e:?}"))?;
        Ok(Some(projected.face))
    }
}

#[cfg(test)]
#[path = "runtime_face/tests.rs"]
mod tests;
