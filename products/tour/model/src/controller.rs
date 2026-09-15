use alloc::string::String;
use conduit_core::{ActivePlayId, CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
use conduit_presentation::{ApplicationEvent, ApplicationViewRefusal};

use crate::{
    NEXT_CHAPTER_ACTION_ID, NEXT_STAGE_ACTION_ID, OPEN_PATCHBAY_ACTION_ID,
    PREVIOUS_CHAPTER_ACTION_ID, PREVIOUS_STAGE_ACTION_ID, RUN_ACTION_ID, TourApplicationAction,
    TourApplicationRefusal, TourRunState, TourWorkspacePhase, TourWorkspaceState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceRequest {
    Run { chapter: u8, stage: u8 },
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
    pub terminal: TourRunTerminal,
    pub comparison: Option<TourComparisonProof>,
    pub multi_host: Option<TourMultiHostProof>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourComparisonProof {
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourMultiHostProof {
    pub source_fragment_id: String,
    pub sink_fragment_id: String,
    pub source_active_play_id: String,
    pub sink_active_play_id: String,
    pub line_id: String,
    pub transferred_values: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourRunTerminal {
    Completed,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourWorkspaceRefusal {
    Presentation,
    Event(ApplicationViewRefusal),
    RunAlreadyPending,
    RunNotPending,
    WrongSpecimen,
    WrongResult,
    MissingExpectedResult,
    MissingComparison,
    MissingMultiHostProof,
    MissingIdentity,
    RevisionExhausted,
    Chapter(TourApplicationRefusal),
}

impl TourWorkspaceRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Presentation => "presentation-refused",
            Self::Event(ApplicationViewRefusal::StaleRevision) => "stale-revision",
            Self::Event(ApplicationViewRefusal::UnknownAction) => "unknown-action",
            Self::Event(_) => "application-event-refused",
            Self::RunAlreadyPending => "run-already-pending",
            Self::RunNotPending => "run-not-pending",
            Self::WrongSpecimen => "wrong-specimen",
            Self::WrongResult => "wrong-result",
            Self::MissingExpectedResult => "missing-expected-result",
            Self::MissingComparison => "missing-comparison",
            Self::MissingMultiHostProof => "missing-multi-host-proof",
            Self::MissingIdentity => "missing-identity",
            Self::RevisionExhausted => "revision-exhausted",
            Self::Chapter(_) => "chapter-navigation-refused",
        }
    }
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

    /// Select an exact specimen Gear from the current workspace revision.
    pub fn select_gear(&mut self, revision: u32, gear: &str) -> Result<(), TourWorkspaceRefusal> {
        if revision != self.state.revision {
            return Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::StaleRevision,
            ));
        }
        if self.state.phase != TourWorkspacePhase::PatchbayOpen
            || !crate::CANONICAL_PATCHBAY_GEARS.contains(&gear)
        {
            return Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::UnknownAction,
            ));
        }
        let next = self.next_revision()?;
        self.state.selected_patchbay_subject = Some(gear.into());
        self.state.focused_key = "patchbay".into();
        self.state.revision = next;
        Ok(())
    }

    pub fn dismiss_inspector(&mut self) -> Result<bool, TourWorkspaceRefusal> {
        if self.state.selected_patchbay_subject.is_none() {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.state.selected_patchbay_subject = None;
        self.state.hovered_patchbay_subject = None;
        self.state.revision = revision;
        Ok(true)
    }

    pub(crate) const fn last_pointer_sequence(&self) -> Option<u64> {
        self.last_pointer_sequence
    }

    pub(crate) fn commit_pointer_leave(&mut self, sequence: u64, revision: u32) {
        self.last_pointer_sequence = Some(sequence);
        self.state.hovered_patchbay_subject = None;
        self.state.revision = revision;
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
    ) -> Result<Option<TourWorkspaceRequest>, TourWorkspaceRefusal> {
        let view = self
            .state
            .presentation()
            .and_then(|presentation| presentation.lower())
            .map_err(|_| TourWorkspaceRefusal::Presentation)?;
        event.validate(&view).map_err(TourWorkspaceRefusal::Event)?;
        match event.action.as_str() {
            RUN_ACTION_ID => {
                if self.state.progress.run == TourRunState::Running {
                    return Err(TourWorkspaceRefusal::RunAlreadyPending);
                }
                let chapter = self.state.progress.chapter;
                let stage = self.state.progress.stage;
                if self.state.progress.current_stage().is_none() {
                    return Err(TourWorkspaceRefusal::Event(
                        ApplicationViewRefusal::UnknownAction,
                    ));
                }
                self.apply_progress(TourApplicationAction::Run)?;
                Ok(Some(TourWorkspaceRequest::Run { chapter, stage }))
            }
            OPEN_PATCHBAY_ACTION_ID => {
                if self.state.progress.run == TourRunState::Running {
                    return Err(TourWorkspaceRefusal::RunAlreadyPending);
                }
                let revision = self.next_revision()?;
                self.state.phase = TourWorkspacePhase::PatchbayOpen;
                self.state.focused_key = "patchbay".into();
                self.state.revision = revision;
                Ok(Some(TourWorkspaceRequest::OpenPatchbay))
            }
            PREVIOUS_CHAPTER_ACTION_ID => {
                self.apply_progress(TourApplicationAction::PreviousChapter)?;
                self.sync_stage()?;
                Ok(None)
            }
            NEXT_CHAPTER_ACTION_ID => {
                self.apply_progress(TourApplicationAction::NextChapter)?;
                self.sync_stage()?;
                Ok(None)
            }
            PREVIOUS_STAGE_ACTION_ID => {
                self.apply_progress(TourApplicationAction::PreviousStage)?;
                self.sync_stage()?;
                Ok(None)
            }
            NEXT_STAGE_ACTION_ID => {
                self.apply_progress(TourApplicationAction::NextStage)?;
                self.sync_stage()?;
                Ok(None)
            }
            _ => Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::UnknownAction,
            )),
        }
    }

    pub fn complete_run(&mut self, proof: TourRunProof) -> Result<(), TourWorkspaceRefusal> {
        if self.state.progress.run != TourRunState::Running {
            return Err(TourWorkspaceRefusal::RunNotPending);
        }
        let Some(stage) = self.state.progress.current_stage() else {
            return Err(TourWorkspaceRefusal::WrongSpecimen);
        };
        if proof.specimen_id != stage.identity {
            return Err(TourWorkspaceRefusal::WrongSpecimen);
        }
        let Some(expected) = stage.expected_text else {
            return Err(TourWorkspaceRefusal::MissingExpectedResult);
        };
        if proof.result != expected {
            return Err(TourWorkspaceRefusal::WrongResult);
        }
        if stage.mode == crate::TourStageMode::Compare && proof.comparison.is_none() {
            return Err(TourWorkspaceRefusal::MissingComparison);
        }
        if matches!(
            stage.mode,
            crate::TourStageMode::TwoHost | crate::TourStageMode::TwoHostPlan
        ) && proof.multi_host.is_none()
        {
            return Err(TourWorkspaceRefusal::MissingMultiHostProof);
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
            || proof.comparison.as_ref().is_some_and(|comparison| {
                comparison.expanded_form_id.as_str().is_empty()
                    || comparison.plan_id.as_str().is_empty()
            })
        {
            return Err(TourWorkspaceRefusal::MissingIdentity);
        }
        if proof.comparison.as_ref().is_some_and(|comparison| {
            comparison.expanded_form_id == proof.expanded_form_id
                || comparison.plan_id == proof.plan_id
        }) {
            return Err(TourWorkspaceRefusal::MissingComparison);
        }
        if proof.multi_host.as_ref().is_some_and(|multi| {
            multi.source_fragment_id.is_empty()
                || multi.sink_fragment_id.is_empty()
                || multi.source_active_play_id.is_empty()
                || multi.sink_active_play_id.is_empty()
                || multi.line_id.is_empty()
                || multi.source_fragment_id == multi.sink_fragment_id
                || multi.source_active_play_id == multi.sink_active_play_id
                || multi.transferred_values != 1
        }) {
            return Err(TourWorkspaceRefusal::MissingMultiHostProof);
        }
        self.apply_progress(match proof.terminal {
            TourRunTerminal::Completed => TourApplicationAction::Complete,
            TourRunTerminal::Stopped => TourApplicationAction::Stop,
        })?;
        self.state.phase = TourWorkspacePhase::ResultVisible;
        self.state.focused_key = "result".into();
        self.state.result = Some(proof.result.clone());
        self.last_run = Some(proof);
        Ok(())
    }

    fn apply_progress(
        &mut self,
        action: TourApplicationAction,
    ) -> Result<(), TourWorkspaceRefusal> {
        let mut application = crate::TourApplicationState {
            revision: self.state.revision,
            progress: self.state.progress,
        };
        application.apply(action).map_err(|error| match error {
            TourApplicationRefusal::RevisionExhausted => TourWorkspaceRefusal::RevisionExhausted,
            error => TourWorkspaceRefusal::Chapter(error),
        })?;
        self.state.revision = application.revision;
        self.state.progress = application.progress;
        Ok(())
    }

    fn sync_stage(&mut self) -> Result<(), TourWorkspaceRefusal> {
        let Some(stage) = self.state.progress.current_stage() else {
            self.state.specimen_id = crate::TOUR_CHAPTERS[usize::from(self.state.progress.chapter)]
                .companion
                .into();
            self.state.source.clear();
            self.state.result = None;
            self.state.phase = TourWorkspacePhase::LessonReady;
            self.state.focused_key = "lesson".into();
            return Ok(());
        };
        self.state.specimen_id = stage.identity.into();
        self.state.source = crate::lesson::tour_stage_source(
            self.state.progress.chapter,
            self.state.progress.stage,
        )
        .map_err(|_| TourWorkspaceRefusal::Presentation)?;
        self.state.result = None;
        self.state.phase = TourWorkspacePhase::LessonReady;
        self.state.focused_key = "source".into();
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
    use crate::{CANONICAL_RESULT, CANONICAL_SPECIMEN_ID};

    fn event(revision: u32, action: &str) -> ApplicationEvent {
        ApplicationEvent {
            revision,
            action: action.into(),
            kind: ApplicationEventKind::Activate,
            value: vec![],
        }
    }

    #[test]
    fn chooser_selection_refuses_stale_and_unknown_subjects_without_mutation() {
        let mut controller = TourWorkspaceController::canonical(8);
        controller
            .request(&event(8, OPEN_PATCHBAY_ACTION_ID))
            .unwrap();
        let before = controller.state().clone();
        assert!(controller.select_gear(8, "meet-one-gear/words").is_err());
        assert!(controller.select_gear(9, "other/words").is_err());
        assert_eq!(controller.state(), &before);
        controller.select_gear(9, "meet-one-gear/change").unwrap();
        assert_eq!(
            controller.state().selected_patchbay_subject.as_deref(),
            Some("meet-one-gear/change")
        );
    }

    #[test]
    fn refused_close_preserves_the_selection() {
        let mut controller = TourWorkspaceController::canonical(u32::MAX);
        controller.state.selected_patchbay_subject = Some("meet-one-gear/change".into());
        let before = controller.state().clone();
        assert_eq!(
            controller.dismiss_inspector(),
            Err(TourWorkspaceRefusal::RevisionExhausted)
        );
        assert_eq!(controller.state(), &before);
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
            terminal: TourRunTerminal::Completed,
            comparison: None,
            multi_host: None,
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
            Ok(Some(TourWorkspaceRequest::Run {
                chapter: 0,
                stage: 0
            }))
        );
        assert_eq!(controller.state().progress.run, TourRunState::Running);
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
        assert_eq!(
            controller.request(&event(6, RUN_ACTION_ID)),
            Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::UnknownAction
            ))
        );
    }

    #[test]
    fn patchbay_navigation_uses_the_current_portable_action() {
        let mut controller = TourWorkspaceController::canonical(8);
        assert_eq!(
            controller.request(&event(8, OPEN_PATCHBAY_ACTION_ID)),
            Ok(Some(TourWorkspaceRequest::OpenPatchbay))
        );
        assert_eq!(controller.state().phase, TourWorkspacePhase::PatchbayOpen);
        assert_eq!(controller.state().focused_key, "patchbay");
    }

    #[test]
    fn resident_navigation_uses_the_canonical_seven_chapter_state() {
        let mut controller = TourWorkspaceController::canonical(1);
        assert_eq!(
            controller.state().progress.chapter_count,
            crate::TOUR_CHAPTER_COUNT
        );
        assert_eq!(controller.state().progress.chapter, 0);
        assert_eq!(
            controller.request(&event(1, NEXT_CHAPTER_ACTION_ID)),
            Ok(None)
        );
        assert_eq!(controller.state().progress.chapter, 1);
        assert_eq!(controller.state().revision, 2);
        assert_eq!(
            controller.state().specimen_id,
            "canonical-form:same-morse-caller"
        );
        assert!(controller.state().source.contains("form same-morse-caller"));
        let application_view = controller.state().presentation().unwrap().lower().unwrap();
        assert!(
            application_view
                .nodes
                .iter()
                .any(|node| node.text.contains("Faces, Backs, and implementation"))
        );
        let workspace = controller.state().workspace_presentation().unwrap();
        assert!(
            workspace
                .text
                .iter()
                .any(|text| text.text.contains("stable semantics"))
        );
        assert_eq!(
            controller.request(&event(2, PREVIOUS_CHAPTER_ACTION_ID)),
            Ok(None)
        );
        assert_eq!(controller.state().progress.chapter, 0);
        assert_eq!(controller.state().revision, 3);
        assert_eq!(controller.state().specimen_id, CANONICAL_SPECIMEN_ID);
        assert_eq!(controller.state().source, crate::CANONICAL_SOURCE);
        let before = controller.state().clone();
        assert_eq!(
            controller.request(&event(3, PREVIOUS_CHAPTER_ACTION_ID)),
            Err(TourWorkspaceRefusal::Event(
                ApplicationViewRefusal::UnknownAction
            ))
        );
        assert_eq!(controller.state(), &before);
    }

    #[test]
    fn resident_exercise_navigation_updates_exact_source_and_run_request() {
        let mut controller = TourWorkspaceController::canonical(1);
        assert_eq!(
            controller.request(&event(1, NEXT_STAGE_ACTION_ID)),
            Ok(None)
        );
        assert_eq!(controller.state().progress.stage, 1);
        assert_eq!(
            controller.state().specimen_id,
            "canonical-form:edit-one-gear"
        );
        assert!(controller.state().source.contains("form edit-one-gear"));
        assert_eq!(
            controller.request(&event(2, RUN_ACTION_ID)),
            Ok(Some(TourWorkspaceRequest::Run {
                chapter: 0,
                stage: 1
            }))
        );
        let mut evidence = proof();
        evidence.specimen_id = "canonical-form:edit-one-gear".into();
        evidence.result = "MAKE THIS LOUD".into();
        evidence.terminal = TourRunTerminal::Stopped;
        controller.complete_run(evidence).unwrap();
        assert_eq!(controller.state().progress.run, TourRunState::Stopped);
        assert_eq!(controller.state().result.as_deref(), Some("MAKE THIS LOUD"));
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
            Ok(Some(TourWorkspaceRequest::Run {
                chapter: 0,
                stage: 0
            }))
        );
        let before = pending.state().clone();
        assert_eq!(
            pending.request(&event(2, OPEN_PATCHBAY_ACTION_ID)),
            Err(TourWorkspaceRefusal::RunAlreadyPending)
        );
        assert_eq!(pending.state(), &before);
    }
}
