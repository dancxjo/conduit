//! Bounded transfer of kernel-committed Home Faces to the installed owner.
use super::Owner;
use conduit_std_host::body_execution::{
    BodyFaceContributionObserver, CommittedFaceContribution, FaceContributionRefusal,
    ReturnedFaceContribution,
};
use std::{
    cell::Cell,
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
};
#[path = "runtime_face/ambient_cadence.rs"]
mod ambient_cadence;

pub(super) struct RuntimeFaceSender(SyncSender<CommittedFaceContribution>);
pub(super) struct RuntimeFaceReceiver {
    incoming: Receiver<CommittedFaceContribution>,
    last_request: Cell<Option<conduit_kernel::RequestId>>,
}

pub(super) fn channel() -> (RuntimeFaceSender, RuntimeFaceReceiver) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (
        RuntimeFaceSender(sender),
        RuntimeFaceReceiver {
            incoming: receiver,
            last_request: Cell::new(None),
        },
    )
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
        if let Ok(frame) = self.incoming.try_recv() {
            if frame.scope().is_retired() {
                return Ok(());
            }
            if !owner.runtime_face_scope_matches_current(&frame) {
                return Err("Home Face differs from the current Body execution".into());
            }
            if owner.runtime_face.as_ref().is_some_and(|previous| {
                !previous.scope().is_retired()
                    && (previous.scope().placement() != frame.scope().placement()
                        || previous.request() >= frame.request())
            }) {
                return Err("Home Face differs from its retained route or request order".into());
            }
            if self
                .last_request
                .get()
                .is_some_and(|previous| previous >= frame.request())
            {
                return Err("Home Face request did not advance".into());
            }
            self.last_request.set(Some(frame.request()));
            if owner.current_home_fragment().is_some_and(|previous| {
                ambient_cadence::same_presented_minute(previous, frame.fragment())
            }) {
                return Ok(());
            }
            // The selected Patchbay Face only exposes the availability of
            // the Home route. Replacing a hidden Home frame leaves that Face
            // and its acknowledged Show current.
            if !owner.patchbay_inspection || owner.current_home_fragment().is_none() {
                owner.advance_runtime_face_revision()?;
            }
            owner.runtime_face = Some(frame);
        }
        Ok(())
    }
}

impl Owner {
    pub(crate) fn has_current_home_navigation(&self, face: &conduit_face::Face) -> bool {
        (self.current_home_fragment().is_some() || self.runtime_patchbay.is_some())
            && face.actions.iter().any(|action| {
                action.availability.is_available()
                    && action.arguments.is_empty()
                    && matches!(
                        action.intent.as_str(),
                        "conduit.intent/open-inspection@1" | "conduit.intent/open-body-overview@1"
                    )
            })
    }

    pub(crate) fn navigate_home_inspection(
        &mut self,
        show: &conduit_face::MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let current = self.project_body_face(None)?;
        show.validate(&current.face)
            .map_err(|error| format!("stale inspection Show: {error:?}"))?;
        interaction
            .validate_against(&current.face, show)
            .map_err(|error| format!("inspection interaction refused: {error:?}"))?;
        let action = current
            .resolve_operator_action(interaction.face_revision, &interaction.action_id)
            .map_err(|error| format!("inspection action refused: {error:?}"))?;
        self.apply_inspection_navigation(&action.kind, show, interaction)
    }

    pub(super) fn apply_inspection_navigation(
        &mut self,
        action: &conduit_face::FaceOperatorActionKind,
        show: &conduit_face::MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        use conduit_face::FaceOperatorActionKind;
        let (inspect, patchbay) = match action {
            FaceOperatorActionKind::OpenInspection(plot)
                if self
                    .runtime_patchbay
                    .as_ref()
                    .is_some_and(|route| route.matches_plot(plot)) =>
            {
                (false, true)
            }
            FaceOperatorActionKind::OpenInspection(plot)
                if self
                    .current_home_fragment()
                    .is_some_and(|home| *plot == home.basis.checked_plot_id) =>
            {
                (true, false)
            }
            FaceOperatorActionKind::OpenOverview => (false, false),
            _ => return Err("action does not target a current inspection".into()),
        };
        if self.runtime_face_inspection != inspect || self.patchbay_inspection != patchbay {
            self.advance_runtime_face_revision()?;
            self.runtime_face_inspection = inspect;
            self.patchbay_inspection = patchbay;
        }
        Ok(serde_json::json!({
            "schema": "conduit.body/inspection-navigation@1",
            "body_id": show.show.body_id,
            "prior_face_id": interaction.face_id,
            "prior_show_id": interaction.show_id,
            "interaction_id": interaction.identity.as_str(),
            "context": if patchbay { "patchbay" } else if inspect { "inspection" } else { "overview" }
        }))
    }

    pub(crate) fn apply_runtime_patchbay_action(
        &mut self,
        show: &conduit_face::MaskShow,
        interaction: &conduit_face::FaceInteraction,
        deadline: std::time::Instant,
    ) -> Result<serde_json::Value, String> {
        let face = self.local_face_snapshot()?;
        if !self.patchbay_inspection {
            return Err("Patchbay inspection is not current".into());
        }
        let request = self
            .runtime_patchbay
            .as_ref()
            .ok_or("Patchbay has no current Play")?
            .apply(face, show, interaction, deadline)?;
        self.advance_runtime_face_revision()?;
        let request = request.map(|request| match request {
            patchbay_application::PatchbayApplicationRequest::EditCurrent { expanded_plot_id, subject_identity } =>
                serde_json::json!({"kind": "edit-current", "plot_id": expanded_plot_id, "subject": subject_identity}),
            patchbay_application::PatchbayApplicationRequest::ChangeMasks { body_plan_id, mode } =>
                serde_json::json!({"kind": "change-masks", "body_plan_id": body_plan_id, "mode": format!("{mode:?}")}),
        });
        Ok(serde_json::json!({
            "schema": "conduit.patchbay/installed-action@1",
            "interaction_id": interaction.identity.as_str(),
            "prior_show_id": show.show_id,
            "active_play_id": self.current_play_id(),
            "request": request,
        }))
    }

    pub(super) fn advance_runtime_face_revision(&mut self) -> Result<(), String> {
        self.runtime_face_updates = self
            .runtime_face_updates
            .checked_add(1)
            .ok_or("Home Face revision exhausted")?;
        Ok(())
    }

    pub(super) fn projected_face_revision(&self) -> Result<u64, String> {
        self.session
            .evidence()
            .last_sequence()
            .checked_add(self.runtime_face_updates)
            .ok_or_else(|| "Body Face revision exhausted".into())
    }

    pub(super) fn current_home_fragment(&self) -> Option<&conduit_face::FaceFragment> {
        self.runtime_face
            .as_ref()
            .filter(|frame| {
                !frame.scope().is_retired() && self.runtime_face_scope_matches_current(frame)
            })
            .map(CommittedFaceContribution::fragment)
    }

    pub(super) fn runtime_face_scope_matches_current(
        &self,
        frame: &CommittedFaceContribution,
    ) -> bool {
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
            || realization
                .plan
                .validate_for_borrowed(&realization.wake)
                .is_err()
            || !play.validate_for_borrowed(&realization.plan)
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
}
