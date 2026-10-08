//! Exact Todo contribution to the installed owner's Face. The caller must
//! supply state recovered through the admitted Play; this module stores none.

use super::{clock_interval, Owner};
use conduit_presentation::{
    Face, FaceContext, FaceContribution, FaceContributionRole, FaceFocus, FaceInteraction,
    FaceNames, FaceResidentPlotName, MaskShow, Presentation, PresentationContributionBasis,
};
use conduit_todo_face::{todo_command_from_contributed_interaction, todo_fragment};
use conduit_todo_plot::{TodoCommand, TodoState};

impl Owner {
    /// Resolve only an action offered by this exact current Owner Face and
    /// acknowledged Show. The caller retains the state from the admitted Fore
    /// and submits the returned command to that same waiting Play.
    #[allow(dead_code)] // Installed return uses this after the waiting Todo Play is wired.
    pub(crate) fn resolve_todo_interaction(
        &self,
        state: &TodoState,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<TodoCommand, String> {
        let face = self.project_face(Some((state, true)))?;
        let basis = self.todo_basis()?;
        todo_command_from_contributed_interaction(state, &face, show, interaction, basis)
            .map_err(|error| format!("current Todo action refused: {error:?}"))
    }

    fn todo_basis(&self) -> Result<PresentationContributionBasis, String> {
        if !matches!(
            self.resident_name.as_deref(),
            Some("todo/main" | "todo/checkpoint-once" | "todo/checkpoint-restore")
        ) {
            return Err("current resident Plot is not Todo".into());
        }
        let resident = self.resident.as_ref().ok_or("Todo has no resident Plot")?;
        let realization = self
            .session
            .realization()
            .ok_or("Todo has no current Body Plan")?;
        let play = realization
            .play
            .as_ref()
            .ok_or("Todo has no current admitted Play")?;
        let selected = realization
            .plan
            .plots
            .iter()
            .find(|entry| entry.plot == *resident)
            .ok_or("Todo resident Plot is not in the current Plan")?;
        Ok(PresentationContributionBasis {
            checked_plot_id: resident.checked_plot_id.clone(),
            plan_id: selected.plan.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            required_interaction_context: None,
        })
    }

    /// The ordinary owner Face, optionally including current Todo Play truth.
    /// A Plot contribution cannot be projected from an idle or retired Play.
    pub(super) fn project_face(
        &self,
        todo: Option<(&TodoState, bool)>,
    ) -> Result<Presentation, String> {
        let plot_name = self
            .resident
            .as_ref()
            .zip(self.resident_name.as_deref())
            .map(|(resident, name)| FaceResidentPlotName {
                source_document_id: &resident.source_document_id,
                checked_plot_id: &resident.checked_plot_id,
                name,
            });
        let plot_names: Vec<_> = plot_name.into_iter().collect();
        let contributions = todo
            .map(|(state, actions_admitted)| {
                let fragment = todo_fragment(state, self.todo_basis()?, actions_admitted)
                    .map_err(|error| format!("Todo Face contribution refused: {error:?}"))?;
                Ok::<_, String>(vec![FaceContribution::from_presentation(
                    FaceContributionRole::Foreground,
                    fragment,
                )])
            })
            .transpose()?
            .unwrap_or_default();
        let face = Face::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            self.session.evidence().last_sequence(),
            FaceContext::Overview,
            FaceFocus::Body,
            contributions,
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &plot_names,
            },
        )
        .map_err(|error| format!("owner-face-projection-refused:{error:?}"))?;
        face.presentation
            .validate()
            .map_err(|error| format!("owner-face-invalid:{error:?}"))?;
        clock_interval::with_clock_action(self, face.presentation)
    }
}
