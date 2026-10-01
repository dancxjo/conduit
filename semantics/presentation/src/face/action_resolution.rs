//! Exact stale-safe routing from current surface actions to semantic owners.

use crate::PresentationActionRefusal;

use super::{Face, FaceOperatorAction, FaceRefusal};

impl Face {
    /// Resolves a current lifecycle or context action without applying it.
    pub fn resolve_operator_action(
        &self,
        presentation_revision: u64,
        action_id: &str,
    ) -> Result<&FaceOperatorAction, FaceRefusal> {
        self.presentation
            .resolve_action(presentation_revision, action_id)
            .map_err(map_action_refusal)?;
        self.operator_actions
            .iter()
            .find(|action| action.surface_action_id == action_id)
            .ok_or(FaceRefusal::UnknownAction)
    }
}

fn map_action_refusal(refusal: PresentationActionRefusal) -> FaceRefusal {
    match refusal {
        PresentationActionRefusal::StaleRevision => FaceRefusal::StaleAction,
        PresentationActionRefusal::UnknownAction => FaceRefusal::UnknownAction,
        PresentationActionRefusal::Unavailable { .. }
        | PresentationActionRefusal::Refused { .. } => FaceRefusal::UnavailableAction,
    }
}
