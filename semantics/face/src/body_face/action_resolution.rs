//! Exact stale-safe routing from current surface actions to semantic owners.

use crate::FaceActionRefusal;

use super::{BodyFace, FaceOperatorAction, FaceRefusal};

impl BodyFace {
    /// Resolves a current lifecycle or context action without applying it.
    pub fn resolve_operator_action(
        &self,
        face_revision: u64,
        action_id: &str,
    ) -> Result<&FaceOperatorAction, FaceRefusal> {
        self.face
            .resolve_action(face_revision, action_id)
            .map_err(map_action_refusal)?;
        self.operator_actions
            .iter()
            .find(|action| action.surface_action_id == action_id)
            .ok_or(FaceRefusal::UnknownAction)
    }
}

fn map_action_refusal(refusal: FaceActionRefusal) -> FaceRefusal {
    match refusal {
        FaceActionRefusal::StaleRevision => FaceRefusal::StaleAction,
        FaceActionRefusal::UnknownAction => FaceRefusal::UnknownAction,
        FaceActionRefusal::Unavailable { .. } | FaceActionRefusal::Refused { .. } => {
            FaceRefusal::UnavailableAction
        }
    }
}
