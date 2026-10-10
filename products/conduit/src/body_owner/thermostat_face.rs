//! Project only the configured initial Form or output from the current scan.
use super::Owner;
use conduit_presentation::{
    Face, FaceContext, FaceContribution, FaceContributionRole, FaceFocus, FaceInteraction,
    FaceNames, FaceResidentPlotName, MaskShow, Presentation, PresentationContributionBasis,
};
use conduit_thermostat_plot::{Command, ThermostatState};

pub(super) struct ThermostatLive {
    pub(super) play: conduit_core::ActivePlayId,
    pub(super) state: ThermostatState,
    pub(super) revision: u64,
    pub(super) actions_admitted: bool,
}

impl Owner {
    pub(super) fn thermostat_basis(&self) -> Result<PresentationContributionBasis, String> {
        if self.resident_name.as_deref() != Some("thermostat/main") {
            return Err("current resident Plot is not Thermostat".into());
        }
        let resident = self
            .resident
            .as_ref()
            .ok_or("Thermostat has no resident Plot")?;
        let realization = self
            .session
            .realization()
            .ok_or("Thermostat has no Body Plan")?;
        let play = realization
            .play
            .as_ref()
            .ok_or("Thermostat has no admitted Play")?;
        if !realization
            .plan
            .plots
            .iter()
            .any(|entry| entry.plot == *resident)
        {
            return Err("Thermostat resident is absent from the current Plan".into());
        }
        Ok(PresentationContributionBasis {
            checked_plot_id: resident.checked_plot_id.clone(),
            plan_id: realization.plan.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            required_interaction_context: None,
        })
    }

    pub(super) fn project_thermostat_face(
        &self,
        live: &ThermostatLive,
    ) -> Result<Presentation, String> {
        if self.current_play_id() != Some(&live.play) {
            return Err("Thermostat Face cache differs from the current Play".into());
        }
        let resident = self
            .resident
            .as_ref()
            .ok_or("Thermostat has no resident Plot")?;
        let fragment = conduit_thermostat_face::fragment(
            &live.state,
            self.thermostat_basis()?,
            live.actions_admitted,
        )
        .map_err(str::to_owned)?;
        let names = [FaceResidentPlotName {
            source_document_id: &resident.source_document_id,
            checked_plot_id: &resident.checked_plot_id,
            name: "thermostat/main",
        }];
        let face = Face::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            live.revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_presentation(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(|error| format!("owner-thermostat-face-refused:{error:?}"))?;
        face.presentation
            .validate()
            .map_err(|error| format!("owner-thermostat-face-invalid:{error:?}"))?;
        Ok(face.presentation)
    }

    pub(crate) fn resolve_thermostat_interaction(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<Command, String> {
        let live = self
            .thermostat_live
            .as_ref()
            .ok_or("Thermostat has no current scan state")?;
        let face = self.project_thermostat_face(live)?;
        conduit_thermostat_face::thermostat_command_from_contributed_interaction(
            &live.state,
            &face,
            show,
            interaction,
            self.thermostat_basis()?,
        )
        .map_err(|error| format!("current Thermostat action refused:{error:?}"))
    }
}
