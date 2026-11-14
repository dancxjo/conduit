//! Ordinary BodyFace context selection for the current single owner view.
//! This is private presentation state, never Body biography, Source or Play.
use super::{debug, Owner};
use conduit_face::{FaceContext, FaceInteraction, FaceOperatorActionKind, MaskShow};

#[derive(Default)]
pub(super) struct OwnerNavigation {
    library: bool,
    revision: u64,
}
impl OwnerNavigation {
    pub(super) fn context(&self) -> FaceContext {
        if self.library {
            FaceContext::Library
        } else {
            FaceContext::Overview
        }
    }
    pub(super) fn face_revision(&self, basis: u64) -> Result<u64, String> {
        basis
            .checked_add(self.revision)
            .ok_or("owner navigation Face revision exhausted".into())
    }
}
impl Owner {
    /// Called only after the carrier's original Line/current acknowledged Show
    /// checks. Repeat semantic correlation here before changing the single
    /// selected owner context. Other actions retain their existing dispatcher.
    pub(crate) fn navigate_owner_context(
        &mut self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<Option<serde_json::Value>, String> {
        let face = self.local_face_snapshot()?;
        show.validate(&face).map_err(debug)?;
        interaction.validate_against(&face, show).map_err(debug)?;
        let offered = face
            .resolve_action(interaction.face_revision, &interaction.action_id)
            .map_err(debug)?;
        if !matches!(
            offered.intent.as_str(),
            "conduit.intent/open-body-overview@1" | "conduit.intent/open-plot-library@1"
        ) {
            return Ok(None);
        }
        // Preserve the notebook's existing admitted contribution and context
        // instead of projecting an empty replacement over that application.
        if self.has_open_notebook() && self.current_play_id().is_some() {
            return self.navigate_notebook(show, interaction).map(Some);
        }
        // These application projections own additional exact contribution
        // custody. They must expose their own BodyFace routing rather than be
        // erased by an empty generic projection. No Play is stopped to navigate.
        if self.has_open_notebook()
            || self.runtime_face.is_some()
            || self.creche_live.is_some()
            || self.thermostat_live.is_some()
            || self.todo_live.is_some()
            || self.todo_verified.is_some()
        {
            return Err(
                "owner navigation requires this contribution's current BodyFace routing".into(),
            );
        }
        let current = self.project_body_face(None)?;
        let action = current
            .resolve_operator_action(interaction.face_revision, &interaction.action_id)
            .map_err(debug)?;
        // The semantic operator routing and current surface target must agree;
        // descriptive intent strings alone cannot create an available action.
        if offered.target != format!("body/{}", self.session.evidence().body_id.as_str()) {
            return Err("owner navigation target differs from current Body".into());
        }
        let library = match &action.kind {
            FaceOperatorActionKind::OpenOverview => false,
            FaceOperatorActionKind::OpenLibrary => true,
            _ => return Err("owner navigation action is not a context operation".into()),
        };
        let revision = self
            .owner_navigation
            .revision
            .checked_add(1)
            .filter(|next| {
                self.session
                    .evidence()
                    .last_sequence()
                    .checked_add(*next)
                    .is_some()
            })
            .ok_or("owner navigation Face revision exhausted")?;
        let previous = std::mem::replace(
            &mut self.owner_navigation,
            OwnerNavigation { library, revision },
        );
        // Projection is pure and owner dispatch is exclusive: no new context
        // escapes if the resulting complete Face cannot be formed.
        if let Err(error) = self.local_face_snapshot() {
            self.owner_navigation = previous;
            return Err(error);
        }
        Ok(Some(serde_json::json!({
            "schema":"conduit.face/owner-navigation@1",
            "body_id":self.session.evidence().body_id,
            "prior_face_id":interaction.face_id,
            "prior_show_id":interaction.show_id,
            "interaction_id":interaction.identity.as_str(),
            "context":if library { "library" } else { "overview" },
            "face_refreshed":true,
        })))
    }
}
