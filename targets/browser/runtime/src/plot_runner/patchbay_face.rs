//! Read-only canonical observation of the existing resident Patchbay instance.
//! No application clone, extra scheduler, Mask Show or effect ingress lives here.
use super::TourSession;
use conduit_body::{BodyBiographyEvidence, BodyPlan, ResidentPlot, Wake};
use conduit_face::{
    BodyFace, FaceContext, FaceContribution, FaceContributionBasis, FaceContributionRole,
    FaceFocus, FaceNames, FaceResidentPlotName,
};

pub(super) struct PatchbayObservation {
    evidence: BodyBiographyEvidence,
    plan: BodyPlan,
    wake: Wake,
    resident: ResidentPlot,
}

impl PatchbayObservation {
    pub(super) fn prepare(
        plan: &BodyPlan,
        wake: &Wake,
        evidence: Option<&BodyBiographyEvidence>,
        foreground: &str,
    ) -> Result<Option<Self>, String> {
        let Some(part) = plan
            .plots
            .iter()
            .find(|part| part.plot.checked_plot_id.as_str() == foreground)
        else {
            return Ok(None);
        };
        let mut count = 0;
        for placement in part
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
        {
            if placement.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND
                && crate::installed_browser::application::application_id(placement)? == "patchbay"
            {
                count += 1;
            }
        }
        if count == 0 {
            return Ok(None);
        }
        // Observation is optional during migration; absence or ambiguous scope
        // must not reject a previously supported legacy application startup.
        if count != 1 {
            return Ok(None);
        }
        let Some(evidence) = evidence else {
            return Ok(None);
        };
        if evidence.body.body_id != plan.body_id || wake.body_id != plan.body_id {
            return Err("Patchbay observation Body basis differs from current Plan".into());
        }
        Ok(Some(Self {
            evidence: evidence.clone(),
            plan: plan.clone(),
            wake: wake.clone(),
            resident: part.plot.clone(),
        }))
    }
}

impl TourSession {
    pub(super) fn patchbay_observation(&self) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("Patchbay Play is retiring".into());
        }
        let context = self
            .patchbay_face
            .as_ref()
            .ok_or("current Body Play has no admitted Patchbay observation context")?;
        let basis = FaceContributionBasis {
            checked_plot_id: context.resident.checked_plot_id.clone(),
            plan_id: context.plan.plan_id.clone(),
            active_play_id: self.active_play_id.clone(),
            required_interaction_context: None,
        };
        let (revision, fragment) = super::engine::application::observe_patchbay(
            &self.scheduler,
            &context.plan.body_id,
            basis,
        )?;
        let names = [FaceResidentPlotName {
            source_document_id: &context.resident.source_document_id,
            checked_plot_id: &context.resident.checked_plot_id,
            name: "Patchbay",
        }];
        let projected = BodyFace::project_with_names(
            &context.evidence.body,
            Some(&context.wake),
            u64::from(revision),
            FaceContext::ResidentPlot(context.resident.checked_plot_id.clone()),
            FaceFocus::Contribution {
                role: FaceContributionRole::Foreground,
                node_key: None,
            },
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&context.evidence.friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(|error| format!("Patchbay Body Face refused: {error:?}"))?;
        Ok(serde_json::json!({
            "schema": "conduit.browser/patchbay-observation@1",
            "body_plan_id": context.plan.plan_id,
            "active_play_id": self.active_play_id,
            "face": projected.face,
            "mask_show": null,
            "interaction_admitted": false,
            "reason_code": "patchbay-canonical-ingress-not-admitted",
        }))
    }
}

#[cfg(test)]
#[path = "patchbay_face_tests.rs"]
mod tests;
