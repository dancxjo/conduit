//! Owner-projected navigation over the existing resident notebook and Play.
//! Context changes never edit, save, copy, or replace the document or its Plan.
use super::*;
use conduit_face::{FaceContext, FaceInteraction, FaceOperatorActionKind, MaskShow};

#[derive(Clone)]
pub(super) struct NotebookNavigation {
    pub(super) context: FaceContext,
    /// Per-incarnation view advance, distinct from the document revision.
    pub(super) revision: u64,
}
impl Default for NotebookNavigation {
    fn default() -> Self {
        Self {
            context: FaceContext::Overview,
            revision: 0,
        }
    }
}
impl Owner {
    pub(crate) fn navigate_notebook(
        &mut self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let current = self.project_notebook_body_face()?;
        show.validate(&current.face).map_err(debug)?;
        interaction
            .validate_against(&current.face, show)
            .map_err(debug)?;
        let action = current
            .resolve_operator_action(interaction.face_revision, &interaction.action_id)
            .map_err(debug)?;
        let context = match &action.kind {
            FaceOperatorActionKind::OpenOverview => FaceContext::Overview,
            FaceOperatorActionKind::OpenLibrary => FaceContext::Library,
            FaceOperatorActionKind::OpenResidentPlot(plot) => {
                FaceContext::ResidentPlot(plot.clone())
            }
            _ => return Err("notebook navigation requires its offered context action".into()),
        };
        let document = self.notebook.as_mut().ok_or("notebook is not open")?;
        let revision = document
            .navigation
            .revision
            .checked_add(1)
            .filter(|next| {
                self.session
                    .evidence()
                    .last_sequence()
                    .checked_add(*next)
                    .is_some()
            })
            .ok_or("notebook Face revision exhausted")?;
        document.navigation = NotebookNavigation { context, revision };
        Ok(serde_json::json!({
            "schema":"conduit.notebook/navigation@1",
            "body_id":show.show.body_id,
            "prior_face_id":interaction.face_id,
            "prior_show_id":interaction.show_id,
            "interaction_id":interaction.identity.as_str(),
            "face_refreshed":true
        }))
    }
}
