//! Local command arguments require explicit submission; field edits retain blur commit.
use super::*;

impl NativeFaceScene {
    // Source text-entry fields retain their established blur-commit behavior.
    // A command's argument draft is not an action: navigation cancels it.
    pub(super) fn commit_on_navigation(&self) -> bool {
        self.input
            .action
            .and_then(|index| self.face.actions.get(index))
            .is_some_and(|action| {
                self.face.subjects.iter().any(|subject| {
                    subject.identity == action.target
                        && subject.role == conduit_face::FaceRole::TextEntry
                })
            })
    }
    pub(super) fn discard_command_draft_for(&mut self, next: Option<FaceControl>) -> bool {
        let discard = self.input.action.is_some()
            && !self.commit_on_navigation()
            && next.map(|control| control.action) != self.input.action;
        if discard {
            self.input = InputState::default();
        }
        discard
    }

}
