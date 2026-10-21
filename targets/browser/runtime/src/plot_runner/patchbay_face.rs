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
    pub(super) plan: BodyPlan,
    wake: Wake,
    pub(super) resident: ResidentPlot,
    pub(super) owner_node: conduit_kernel::NodeId,
}

impl PatchbayObservation {
    pub(super) fn prepare(
        plan: &BodyPlan,
        wake: &Wake,
        evidence: Option<&BodyBiographyEvidence>,
        foreground: &str,
        local: &[conduit_plan_lowering::lowering::LoweredPlanFragment],
    ) -> Result<Option<Self>, String> {
        let mut candidates = Vec::new();
        for part in &plan.plots {
            let mut has_patchbay = false;
            for placement in part
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
            {
                if placement.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND
                    && crate::installed_browser::application::application_id(placement)?
                        == "patchbay"
                {
                    has_patchbay = true;
                }
            }
            if has_patchbay {
                candidates.push(part);
            }
        }
        let part = if let Some(part) = candidates
            .iter()
            .find(|part| part.plot.checked_plot_id.as_str() == foreground)
        {
            *part
        } else if let [part] = candidates.as_slice() {
            *part
        } else {
            return Ok(None);
        };
        let mut owners = Vec::new();
        for fragment in &part.plan.fragments {
            for placement in &fragment.placements {
                if placement.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND
                    && crate::installed_browser::application::application_id(placement)?
                        == "patchbay"
                {
                    owners.push((fragment, placement));
                }
            }
        }
        // Optional during migration: ambiguous or externally managed owners
        // cannot borrow another resident's local application instance.
        let [(fragment, placement)] = owners.as_slice() else {
            return Ok(None);
        };
        let mut nodes = local
            .iter()
            .filter(|lowered| {
                lowered.identity.plan_id == part.plan.plan_id
                    && lowered.identity.fragment_id == fragment.fragment_id
            })
            .flat_map(|lowered| &lowered.nodes)
            .filter(|node| node.placement_id == placement.placement_id);
        let Some(owner) = nodes.next() else {
            return Ok(None);
        };
        if nodes.next().is_some() {
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
            owner_node: owner.node,
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
        let source = super::engine::application::observe_patchbay(
            &self.scheduler,
            context.owner_node,
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
            u64::from(source.revision),
            FaceContext::ResidentPlot(context.resident.checked_plot_id.clone()),
            FaceFocus::Contribution {
                role: FaceContributionRole::Foreground,
                node_key: None,
            },
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                source.fragment,
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
            "owner_checked_plot_id": context.resident.checked_plot_id,
            "pending_owner_request": source.pending_request,
            "last_owner_disposition": source.last_disposition,
            "last_input_refusal": source.input_refusal,
            "mask_show": null,
            "interaction_admitted": false,
            "reason_code": "patchbay-canonical-ingress-not-admitted",
        }))
    }
}

#[cfg(test)]
#[path = "patchbay_face_tests.rs"]
mod tests;
