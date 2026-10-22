//! Whole-Body publication reuses its admitted None selector and actual Mask kernel.
use super::TourSession;
use crate::patchbay_ingress::PublicationValueLease;
use crate::workspace_mask::{
    BrowserMaskAcknowledgement, BrowserMaskEffect, BrowserMaskInteraction, BrowserMaskRuntime,
};
use conduit_body::{BodyBiographyEvidence, BodyPlan, Wake};
use conduit_body_make::{BodyEvidenceAttachment, BodyEvidenceEntrance, ReviewedBodyPlot};
use conduit_core::{ActivePlayId, PlanId};
use conduit_face::Face;

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BodyFaceRequest {
    pub schema: String,
    pub body_plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub revision: u64,
    pub body_evidence: BodyBiographyEvidence,
    pub reviewed_plots: Vec<ReviewedBodyPlot>,
    pub face: Face,
}

pub(super) struct BodyFaceContext {
    plan: BodyPlan,
    wake: Wake,
    evidence: BodyBiographyEvidence,
    reviewed: Vec<ReviewedBodyPlot>,
    expected_first_revision: u64,
    revision: Option<u64>,
    sequence: u64,
    publication: Option<PreparedBodyFace>,
}

struct PreparedBodyFace {
    mask: BrowserMaskRuntime,
    effect: BrowserMaskEffect,
    // Release accounting after the actual scheduler and its buffers are dropped.
    lease: PublicationValueLease,
}

impl BodyFaceContext {
    pub(super) fn prepare(
        plan: &BodyPlan,
        wake: &Wake,
        evidence: Option<&BodyBiographyEvidence>,
        source: &str,
        owner_face_revision: Option<&str>,
    ) -> Result<Option<Self>, String> {
        let mut topologies = plan
            .mask_topologies
            .iter()
            .filter(|topology| topology.face.plot.is_none());
        let Some(topology) = topologies.next() else {
            return Ok(None);
        };
        if topologies.next().is_some()
            || topology.face.source_placement_id.is_some()
            || topology.chains.len() != 1
        {
            return Err("whole Body Mask requires one exact admitted None selector".into());
        }
        let baseline =
            owner_face_revision.ok_or("whole Body Mask requires an owner revision basis")?;
        let revision = baseline
            .parse::<u64>()
            .map_err(|_| "whole Body owner revision is not a u64")?;
        if revision.to_string() != baseline {
            return Err("whole Body owner revision is not canonical decimal".into());
        }
        // The actual local startup projection appends the Started Wake once;
        // coordinator history::retain advances its evidence revision once.
        let expected_first_revision = revision
            .checked_add(1)
            .ok_or("whole Body owner revision exhausted")?;
        let evidence = evidence.ok_or("whole Body Mask requires started owner evidence")?;
        evidence
            .validate()
            .map_err(|error| format!("whole Body evidence: {error:?}"))?;
        if evidence.body_id != plan.body_id || !evidence.wakes.iter().any(|current| current == wake)
        {
            return Err("whole Body Mask has no current started Wake".into());
        }
        let reviewed = crate::creche::body_face_inventory(source)?;
        if !plan.plots.iter().all(|part| {
            reviewed.iter().any(|plot| {
                part.plot.source_document_id.as_str() == plot.source_document_id
                    && part.plot.checked_plot_id.as_str() == plot.checked_plot_id
            })
        }) {
            return Err("whole Body Source inventory omits its admitted resident".into());
        }
        Ok(Some(Self {
            reviewed,
            plan: plan.clone(),
            wake: wake.clone(),
            evidence: evidence.clone(),
            expected_first_revision,
            revision: None,
            sequence: 0,
            publication: None,
        }))
    }

    fn validate(&self, request: &BodyFaceRequest, play: &ActivePlayId) -> Result<(), String> {
        if request.schema != "conduit.browser/body-face-publication@1"
            || request.body_plan_id != self.plan.plan_id
            || &request.active_play_id != play
            || request.body_evidence != self.evidence
            || self.revision.map_or(
                request.revision != self.expected_first_revision,
                |revision| request.revision <= revision,
            )
            || request.face.revision != request.revision
        {
            return Err("whole Body publication is stale or has a foreign owner basis".into());
        }
        request
            .face
            .validate()
            .map_err(|error| format!("whole Body Face: {error:?}"))?;
        if request.reviewed_plots != self.reviewed {
            return Err(
                "whole Body reviewed Source differs from the checked owner inventory".into(),
            );
        }
        let encoded =
            serde_json::to_vec(&request.body_evidence).map_err(|error| error.to_string())?;
        let attachment =
            BodyEvidenceAttachment::open_serialized(&encoded, BodyEvidenceEntrance::ExternalReader)
                .map_err(|error| format!("whole Body evidence attachment: {error:?}"))?;
        let expected = conduit_body_make::project_body_workbench_face(
            request.revision,
            &attachment,
            &request.reviewed_plots,
        )?;
        if request.face != expected {
            return Err("whole Body Face differs from its canonical owner projection".into());
        }
        Ok(())
    }
}

impl PreparedBodyFace {
    fn observe(&self, plan: &PlanId, play: &ActivePlayId) -> Result<serde_json::Value, String> {
        if !self.lease.is_live() {
            return Err("whole Body publication owner is retired".into());
        }
        let actual = self.mask.observation();
        let admitted = actual.mask_show.show.lifecycle
            == conduit_face::ManifestationLifecycle::Available
            && actual.interaction.is_none();
        Ok(serde_json::json!({
            "schema":"conduit.browser/body-face-observation@1", "body_plan_id":plan, "active_play_id":play,
            "effect":self.effect, "view":self.mask.face_view(admitted), "face":actual.face,
            "mask_show":actual.mask_show, "execution":actual.execution, "interaction":actual.interaction, "alternate":actual.alternate,
        }))
    }
}

impl TourSession {
    pub(super) fn retire_body_face(&mut self) -> Result<(), String> {
        if let Some(context) = self.body_face_publication.as_mut() {
            if let Some(mut publication) = context.publication.take() {
                publication.mask.retire()?;
                // Drop actual scheduler/value backing before its accounting lease.
                drop(publication);
            }
        }
        Ok(())
    }

    pub(super) fn body_face_prepare(
        &mut self,
        request: BodyFaceRequest,
    ) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("whole Body Play is retiring".into());
        }
        let context = self
            .body_face_publication
            .as_mut()
            .ok_or("no admitted whole Body Mask selector")?;
        context.validate(&request, &self.active_play_id)?;
        let budget = self
            .scheduler
            ._prepared_storage
            .publication_value_budget
            .as_ref()
            .ok_or("whole Body publication has no prepared parent budget")?;
        // Reuse one admitted Mask slot: retire and drop old work before allocating its successor.
        if let Some(mut old) = context.publication.take() {
            old.mask.retire()?;
        }
        let sequence = context
            .sequence
            .checked_add(1)
            .ok_or("whole Body Mask sequence exhausted")?;
        let (mask, effect) = BrowserMaskRuntime::prepare_selected_in_body_plan(
            context.plan.body_id.clone(),
            self.host_id.clone(),
            self.boot_id.clone(),
            request.face,
            context.wake.clone(),
            context.plan.clone(),
            sequence,
        )?;
        let lease = budget.admit(
            &context.plan.plan_id,
            &self.active_play_id,
            mask.reserved_value_storage_bytes(),
        )?;
        let publication = PreparedBodyFace {
            mask,
            effect,
            lease,
        };
        let result = publication.observe(&context.plan.plan_id, &self.active_play_id)?;
        if serde_json::to_vec(&result)
            .map_err(|error| error.to_string())?
            .len()
            > super::abi::OUTPUT_BYTES
        {
            return Err("whole Body publication exceeds the existing output bound".into());
        }
        context.revision = Some(request.revision);
        context.sequence = sequence;
        context.publication = Some(publication);
        Ok(result)
    }

    pub(super) fn body_face_observation(&self) -> Result<serde_json::Value, String> {
        if self.cancellation.is_some() {
            return Err("whole Body Play is retiring".into());
        }
        let context = self
            .body_face_publication
            .as_ref()
            .ok_or("no admitted whole Body Mask selector")?;
        context
            .publication
            .as_ref()
            .ok_or("no prepared whole Body publication")?
            .observe(&context.plan.plan_id, &self.active_play_id)
    }

    pub(super) fn body_face_acknowledge(
        &mut self,
        request: BrowserMaskAcknowledgement,
    ) -> Result<serde_json::Value, String> {
        let context = self
            .body_face_publication
            .as_mut()
            .ok_or("no admitted whole Body Mask selector")?;
        let publication = context
            .publication
            .as_mut()
            .ok_or("no prepared whole Body publication")?;
        if self.cancellation.is_some() || !publication.lease.is_live() {
            return Err("whole Body Play is retired".into());
        }
        publication.mask.acknowledge(&request)?;
        self.body_face_observation()
    }

    pub(super) fn body_face_interact(
        &mut self,
        request: BrowserMaskInteraction,
    ) -> Result<serde_json::Value, String> {
        let context = self
            .body_face_publication
            .as_mut()
            .ok_or("no admitted whole Body Mask selector")?;
        let publication = context
            .publication
            .as_mut()
            .ok_or("no prepared whole Body publication")?;
        if self.cancellation.is_some() || !publication.lease.is_live() {
            return Err("whole Body Play is retired".into());
        }
        publication.mask.interact(&request)?;
        self.body_face_observation()
    }
}

#[cfg(test)]
#[path = "body_face_publication_tests.rs"]
mod tests;
