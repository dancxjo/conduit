use alloc::string::String;
use conduit_core::{ActivePlayId, CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
use conduit_presentation::{ApplicationEvent, ApplicationViewRefusal};

use crate::{
    CANONICAL_RESULT, CANONICAL_SPECIMEN_ID, OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID,
    TourWorkspacePhase, TourWorkspaceState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceRequest {
    Run,
    OpenPatchbay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourRunProof {
    pub specimen_id: String,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub result: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceRefusal {
    Presentation,
    Event(ApplicationViewRefusal),
    RunAlreadyPending,
    RunNotPending,
    WrongSpecimen,
    WrongResult,
    MissingIdentity,
    RevisionExhausted,
}

pub struct TourWorkspaceController {
    state: TourWorkspaceState,
    last_run: Option<TourRunProof>,
    last_pointer_sequence: Option<u64>,
}

impl TourWorkspaceController {
    pub fn canonical(revision: u32) -> Self {
        Self {
            state: TourWorkspaceState::canonical(revision, TourWorkspacePhase::LessonReady),
            last_run: None,
            last_pointer_sequence: None,
        }
    }

    pub const fn state(&self) -> &TourWorkspaceState {
        &self.state
    }

    pub fn last_run(&self) -> Option<&TourRunProof> {
        self.last_run.as_ref()
    }

    pub(crate) const fn last_pointer_sequence(&self) -> Option<u64> {
        self.last_pointer_sequence
    }

    pub(crate) fn commit_pointer(
        &mut self,
        sequence: u64,
        hovered: String,
        selected: Option<String>,
        revision: u32,
    ) {
        self.last_pointer_sequence = Some(sequence);
        self.state.hovered_patchbay_subject = Some(hovered);
        if let Some(selected) = selected {
            self.state.selected_patchbay_subject = Some(selected);
            self.state.focused_key = "patchbay".into();
        }
        self.state.revision = revision;
    }

    pub fn request(
        &mut self,
        event: &ApplicationEvent,
    ) -> Result<TourWorkspaceRequest, TourWorkspaceRefusal> {
        let view = self
            .state
            .presentation()
            .and_then(|presentation| presentation.lower())
            .map_err(|_| TourWorkspaceRefusal::Presentation)?;
        event.validate(&view).map_err(TourWorkspaceRefusal::Event)?;
        let revision = self.next_revision()?;
        match event.action.as_str() {
            RUN_ACTION_ID => {
                if self.state.run_pending {
                    return Err(TourWorkspaceRefusal::RunAlreadyPending);
                }
                self.state.run_pending = true;
                self.state.revision = revision;
                Ok(TourWorkspaceRequest::Run)
            }
            OPEN_PATCHBAY_ACTION_ID => {
                if self.state.run_pending {
                    return Err(TourWorkspaceRefusal::RunAlreadyPending);
                }
                self.state.phase = TourWorkspacePhase::PatchbayOpen;
                self.state.focused_key = "patchbay".into();
                self.state.revision = revision;
                Ok(TourWorkspaceRequest::OpenPatchbay)
            }
            _ => Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::UnknownAction,
            )),
        }
    }

    pub fn complete_run(&mut self, proof: TourRunProof) -> Result<(), TourWorkspaceRefusal> {
        if !self.state.run_pending {
            return Err(TourWorkspaceRefusal::RunNotPending);
        }
        if proof.specimen_id != CANONICAL_SPECIMEN_ID {
            return Err(TourWorkspaceRefusal::WrongSpecimen);
        }
        if proof.result != CANONICAL_RESULT {
            return Err(TourWorkspaceRefusal::WrongResult);
        }
        if [
            proof.source_document_id.as_str(),
            proof.checked_form_id.as_str(),
            proof.expanded_form_id.as_str(),
            proof.plan_id.as_str(),
            proof.active_play_id.as_str(),
        ]
        .iter()
        .any(|identity| identity.is_empty())
        {
            return Err(TourWorkspaceRefusal::MissingIdentity);
        }
        let revision = self.next_revision()?;
        self.state.run_pending = false;
        self.state.phase = TourWorkspacePhase::ResultVisible;
        self.state.focused_key = "result".into();
        self.state.result = Some(proof.result.clone());
        self.last_run = Some(proof);
        self.state.revision = revision;
        Ok(())
    }

    pub(crate) fn next_revision(&self) -> Result<u32, TourWorkspaceRefusal> {
        self.state
            .revision
            .checked_add(1)
            .ok_or(TourWorkspaceRefusal::RevisionExhausted)
    }
}

#[cfg(test)]
mod tests {
    use alloc::{string::ToString, vec};
    use conduit_presentation::{ApplicationEventKind, ApplicationViewRefusal};

    use super::*;

    fn event(revision: u32, action: &str) -> ApplicationEvent {
        ApplicationEvent {
            revision,
            action: action.into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
    }

    fn proof() -> TourRunProof {
        TourRunProof {
            specimen_id: CANONICAL_SPECIMEN_ID.into(),
            source_document_id: SourceDocumentId::from("source"),
            checked_form_id: CheckedFormId::from("checked"),
            expanded_form_id: ExpandedFormId::from("expanded"),
            plan_id: PlanId::from("plan"),
            active_play_id: ActivePlayId::from("play"),
            result: CANONICAL_RESULT.to_string(),
        }
    }

    #[test]
    fn result_requires_a_current_run_event_and_exact_proof() {
        let mut controller = TourWorkspaceController::canonical(4);
        assert_eq!(
            controller.complete_run(proof()),
            Err(TourWorkspaceRefusal::RunNotPending)
        );
        assert_eq!(
            controller.request(&event(3, RUN_ACTION_ID)),
            Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::StaleRevision
            ))
        );
        assert_eq!(
            controller.request(&event(4, RUN_ACTION_ID)),
            Ok(TourWorkspaceRequest::Run)
        );
        assert!(controller.state().run_pending);
        let mut wrong = proof();
        wrong.result = "painted success".into();
        assert_eq!(
            controller.complete_run(wrong),
            Err(TourWorkspaceRefusal::WrongResult)
        );
        assert_eq!(controller.complete_run(proof()), Ok(()));
        assert_eq!(controller.state().phase, TourWorkspacePhase::ResultVisible);
        assert_eq!(controller.state().focused_key, "result");
        assert_eq!(controller.last_run().unwrap().plan_id.as_str(), "plan");
    }

    #[test]
    fn patchbay_navigation_uses_the_current_portable_action() {
        let mut controller = TourWorkspaceController::canonical(8);
        assert_eq!(
            controller.request(&event(8, OPEN_PATCHBAY_ACTION_ID)),
            Ok(TourWorkspaceRequest::OpenPatchbay)
        );
        assert_eq!(controller.state().phase, TourWorkspacePhase::PatchbayOpen);
        assert_eq!(controller.state().focused_key, "patchbay");
    }

    #[test]
    fn revision_exhaustion_and_pending_run_leave_state_unchanged() {
        let mut exhausted = TourWorkspaceController::canonical(u32::MAX);
        let before = exhausted.state().clone();
        assert_eq!(
            exhausted.request(&event(u32::MAX, RUN_ACTION_ID)),
            Err(TourWorkspaceRefusal::RevisionExhausted)
        );
        assert_eq!(exhausted.state(), &before);

        let mut pending = TourWorkspaceController::canonical(1);
        assert_eq!(
            pending.request(&event(1, RUN_ACTION_ID)),
            Ok(TourWorkspaceRequest::Run)
        );
        let before = pending.state().clone();
        assert_eq!(
            pending.request(&event(2, OPEN_PATCHBAY_ACTION_ID)),
            Err(TourWorkspaceRefusal::RunAlreadyPending)
        );
        assert_eq!(pending.state(), &before);
    }
}
