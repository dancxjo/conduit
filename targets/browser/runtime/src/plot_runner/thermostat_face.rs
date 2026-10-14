//! Current Body Face from exact Plan seed or validated output of its kernel scan.
use super::TourSession;
use crate::workspace_mask::{
    BrowserMaskAcknowledgement, BrowserMaskEffect, BrowserMaskInteraction, BrowserMaskRuntime,
};
use conduit_body::{BodyBiographyEvidence, BodyPlan, Wake};
use conduit_face::{
    BodyFace, FaceContext, FaceContribution, FaceContributionBasis, FaceContributionRole,
    FaceFocus, FaceNames, FaceResidentPlotName,
};

pub(super) struct ThermostatFace {
    evidence: BodyBiographyEvidence,
    plan: BodyPlan,
    wake: Wake,
    resident: conduit_body::ResidentPlot,
    play_sequence: u64,
    /// Show publication order; settings revision stays solely in the kernel state.
    publication_revision: u64,
    mask: Option<Box<BrowserMaskRuntime>>,
    effect: Option<BrowserMaskEffect>,
}

impl ThermostatFace {
    pub(super) fn prepare(
        plan: &BodyPlan,
        wake: &Wake,
        evidence: Option<&BodyBiographyEvidence>,
        play_sequence: u64,
    ) -> Result<Option<Self>, String> {
        let mut residents = plan.plots.iter().filter(|part| {
            part.plan
                .activations
                .iter()
                .any(|activation| match activation {
                    conduit_core::PlannedActivationEntry::Scan(scan) => scan
                        .selected_plan
                        .fragments
                        .iter()
                        .flat_map(|f| &f.placements)
                        .any(|gear| {
                            gear.kind_id.as_str() == conduit_thermostat_plot::THERMOSTAT_KIND
                        }),
                    _ => false,
                })
        });
        let Some(part) = residents.next() else {
            return Ok(None);
        };
        if residents.next().is_some() {
            return Err("Thermostat Face requires one exact resident scan Plan".into());
        }
        let evidence = evidence.ok_or("Thermostat Face requires current Body evidence")?;
        if evidence.body.body_id != plan.body_id || wake.body_id != plan.body_id {
            return Err("Thermostat Face Body basis differs from current Plan".into());
        }
        Ok(Some(Self {
            evidence: evidence.clone(),
            plan: plan.clone(),
            wake: wake.clone(),
            resident: part.plot.clone(),
            play_sequence,
            publication_revision: 0,
            mask: None,
            effect: None,
        }))
    }
    fn basis(&self, play: &conduit_core::ActivePlayId) -> FaceContributionBasis {
        FaceContributionBasis {
            checked_plot_id: self.resident.checked_plot_id.clone(),
            plan_id: self.plan.plan_id.clone(),
            active_play_id: play.clone(),
            required_interaction_context: None,
        }
    }
}

impl TourSession {
    fn prepare_thermostat_show(&mut self) -> Result<(), String> {
        let state = self
            .scheduler
            .thermostat_state()
            .ok_or("no current Thermostat scan state")?;
        let context = self
            .thermostat_face
            .as_mut()
            .ok_or("current resident is not Thermostat")?;
        let basis = context.basis(&self.active_play_id);
        let fragment = conduit_thermostat_face::fragment(
            &state,
            basis,
            self.scheduler.thermostat_commands_available(),
        )
        .map_err(str::to_owned)?;
        let names = [FaceResidentPlotName {
            source_document_id: &context.resident.source_document_id,
            checked_plot_id: &context.resident.checked_plot_id,
            name: "thermostat/main",
        }];
        let face = BodyFace::project_with_names(
            &context.evidence.body,
            Some(&context.wake),
            context.publication_revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&context.evidence.friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(|error| format!("Thermostat Face refused: {error:?}"))?;
        let mask_sequence = context
            .play_sequence
            .checked_mul(257)
            .and_then(|base| base.checked_add(context.publication_revision + 1))
            .ok_or("Thermostat Mask Play sequence exhausted")?;
        let (mask, effect) = BrowserMaskRuntime::prepare_sequenced(
            context.plan.body_id.clone(),
            self.host_id.clone(),
            self.boot_id.clone(),
            face.face,
            context.wake.clone(),
            context.plan.clone(),
            mask_sequence,
        )?;
        context.mask = Some(Box::new(mask));
        context.effect = Some(effect);
        Ok(())
    }

    pub(super) fn thermostat_snapshot(&mut self) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Thermostat Play is retiring".into());
        }
        if self
            .thermostat_face
            .as_ref()
            .ok_or("current resident is not Thermostat")?
            .mask
            .is_none()
        {
            self.prepare_thermostat_show()?;
        }
        let context = self
            .thermostat_face
            .as_ref()
            .ok_or("current resident is not Thermostat")?;
        let observation = context
            .mask
            .as_ref()
            .ok_or("Thermostat Mask absent")?
            .observation();
        let view = crate::owner_face_mask::view::local_view(
            &context.plan.body_id,
            &observation.face,
            &observation.mask_show,
            &observation.mask_play,
        );
        Ok(
            serde_json::json!({ "schema":"conduit.browser/thermostat-face@1",
            "body_plan_id": context.plan.plan_id, "active_play_id": self.active_play_id,
            "face": observation.face, "effect": context.effect,
            "mask_show": observation.mask_show, "execution": observation.execution, "view": view }),
        )
    }

    pub(super) fn acknowledge_thermostat_show(
        &mut self,
        ack: BrowserMaskAcknowledgement,
    ) -> Result<serde_json::Value, String> {
        let mask = self
            .thermostat_face
            .as_mut()
            .and_then(|context| context.mask.as_mut())
            .ok_or("Thermostat has no pending Show")?;
        let current = mask.observation();
        if current.mask_show.show.lifecycle == conduit_face::ManifestationLifecycle::Available {
            let show = &current.mask_show;
            if ack.show_id != show.show_id.as_str()
                || ack.manifestation_id != show.show.manifestation_id.as_str()
                || ack.mask_plan_id != current.planned_mask.plan.plan_id
                || ack.active_play_id != current.mask_play.active_play_id
                || ack.placement_id != show.show.placement_id
                || ack.face_id != current.face.identity.as_str()
                || ack.face_revision != current.face.revision
            {
                return Err("Thermostat acknowledgement is stale or mismatched".into());
            }
        } else {
            mask.acknowledge(&ack)?;
        }
        self.thermostat_snapshot()
    }

    pub(super) fn interact_thermostat(
        &mut self,
        proposed: BrowserMaskInteraction,
    ) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Thermostat Play is retiring".into());
        }
        let state = self
            .scheduler
            .thermostat_state()
            .ok_or("no current Thermostat scan state")?;
        let context = self
            .thermostat_face
            .as_mut()
            .ok_or("current resident is not Thermostat")?;
        let basis = context.basis(&self.active_play_id);
        let mask = context
            .mask
            .as_mut()
            .ok_or("Thermostat has no acknowledged Show")?;
        let observation = mask.observation();
        // Validate the exact current canonical contract before consuming the Mask Fore.
        let interaction = conduit_face::FaceInteraction::new(
            &observation.face,
            &observation.mask_show,
            &proposed.action_id,
            &proposed.target,
            proposed.arguments.clone(),
            proposed.sequence,
        )
        .map_err(|error| format!("Thermostat interaction refused: {error:?}"))?;
        let command = conduit_thermostat_face::thermostat_command_from_contributed_interaction(
            &state,
            &observation.face,
            &observation.mask_show,
            &interaction,
            basis,
        )
        .map_err(|error| format!("Thermostat command refused: {error:?}"))?;
        self.scheduler.validate_thermostat_command(command)?;
        let receipt = mask.interact(&proposed)?;
        let progress = self.submit_thermostat_command(command)?;
        let context = self
            .thermostat_face
            .as_mut()
            .ok_or("Thermostat Face absent")?;
        context.publication_revision = context
            .publication_revision
            .checked_add(1)
            .filter(|revision| *revision <= 256)
            .ok_or("Thermostat Face revision exhausted")?;
        self.prepare_thermostat_show()?;
        Ok(
            serde_json::json!({ "schema":"conduit.browser/thermostat-action@1",
            "progress":progress, "interaction":receipt, "face":self.thermostat_snapshot()? }),
        )
    }
}

#[cfg(test)]
#[path = "thermostat_face_tests.rs"]
mod tests;
