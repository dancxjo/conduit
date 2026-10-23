//! Source observation and Body-owned Face projection for the exact resident.
//! Prepared canonical publication is a separate record with its own Show lifecycle.
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
                if crate::installed_browser::patchbay_owner_installation::is_owner(placement)? {
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
                if crate::installed_browser::patchbay_owner_installation::is_owner(placement)? {
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
        let projected = context.project_fragment(source.fragment, source.revision)?;
        let interaction_ready = self.patchbay_publication.as_ref().is_some_and(|prepared| {
            prepared.publication.can_submit()
                && prepared.publication.observation().face.revision == u64::from(source.revision)
                && source.pending_request.is_none()
        });
        Ok(serde_json::json!({
            "schema": "conduit.browser/patchbay-observation@1",
            "body_plan_id": context.plan.plan_id,
            "active_play_id": self.active_play_id,
            "face": projected,
            "owner_checked_plot_id": context.resident.checked_plot_id,
            "pending_owner_request": source.pending_request,
            "last_owner_disposition": source.last_disposition,
            "last_input_refusal": source.input_refusal,
            "mask_show": null,
            "interaction_admitted": false,
            "reason_code": if self.patchbay_publication.is_some() {
                "patchbay-canonical-publication-observed-separately"
            } else { "patchbay-canonical-ingress-not-admitted" },
            "publication": self.patchbay_publication.as_ref().map(|prepared| serde_json::json!({
                "effect": prepared.effect, "observation": prepared.publication.observation(),
                "view": prepared.publication.face_view(interaction_ready),
                "interaction_admitted": interaction_ready,
                "dispatch": prepared.dispatch_observation(),
            })),
        }))
    }
}

#[cfg(test)]
#[path = "patchbay_face_tests.rs"]
mod tests;

impl PatchbayObservation {
    pub(super) fn has_composed_mask(&self) -> Result<bool, String> {
        let selected = self
            .plan
            .mask_topologies
            .iter()
            .find(|topology| topology.face.plot.as_ref() == Some(&self.resident));
        if selected.is_some_and(|topology| topology.face.source_placement_id.is_some()) {
            return Err(
                "resident Patchbay canonical Face requires a Body-composed Mask selector".into(),
            );
        }
        Ok(selected.is_some())
    }

    pub(super) fn project_fragment(
        &self,
        fragment: conduit_face::FaceFragment,
        revision: u32,
    ) -> Result<conduit_face::Face, String> {
        let names = [FaceResidentPlotName {
            source_document_id: &self.resident.source_document_id,
            checked_plot_id: &self.resident.checked_plot_id,
            name: "Patchbay",
        }];
        let projected = BodyFace::project_with_names(
            &self.evidence.body,
            Some(&self.wake),
            u64::from(revision),
            FaceContext::ResidentPlot(self.resident.checked_plot_id.clone()),
            FaceFocus::Contribution {
                role: FaceContributionRole::Foreground,
                node_key: None,
            },
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&self.evidence.friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(|error| format!("Patchbay Body Face refused: {error:?}"))?;
        Ok(projected.face)
    }

    pub(super) fn prepare_mask(
        &self,
        face: conduit_face::Face,
        host: conduit_core::HostId,
        boot: conduit_core::BootId,
        sequence: u64,
    ) -> Result<
        (
            crate::workspace_mask::BrowserMaskRuntime,
            crate::workspace_mask::BrowserMaskEffect,
        ),
        String,
    > {
        crate::workspace_mask::BrowserMaskRuntime::prepare_in_body_plan(
            self.plan.body_id.clone(),
            host,
            boot,
            face,
            self.wake.clone(),
            self.plan.clone(),
            sequence,
        )
    }
}
