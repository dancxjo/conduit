//! Host-reported owner-request disposition, scoped to the current exact Play.
use super::TourSession;
use conduit_body::BodyId;
use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) enum RequestOutcome {
    Completed,
    Rejected,
    Cancelled,
    Uncertain,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequestDisposition {
    pub body_id: BodyId,
    pub body_plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub owner_checked_plot_id: CheckedPlotId,
    pub application_revision: u32,
    pub outcome: RequestOutcome,
    pub evidence_id: String,
}
impl RequestDisposition {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.evidence_id.trim().is_empty() || self.evidence_id.len() > 256 {
            return Err("Patchbay disposition requires a bounded Host evidence identity".into());
        }
        Ok(())
    }
}
impl TourSession {
    pub(super) fn patchbay_request_disposition(
        &mut self,
        disposition: RequestDisposition,
    ) -> Result<serde_json::Value, String> {
        disposition.validate()?;
        if self.cancellation.is_some() {
            return Err("Patchbay Play is retiring".into());
        }
        let context = self
            .patchbay_face
            .as_ref()
            .ok_or("no current Patchbay owner context")?;
        if disposition.body_id != context.plan.body_id
            || disposition.body_plan_id != context.plan.plan_id
            || disposition.active_play_id != self.active_play_id
            || disposition.owner_checked_plot_id != context.resident.checked_plot_id
        {
            return Err("Patchbay disposition differs from the current owner Play".into());
        }
        super::engine::application::record_patchbay_disposition(
            &mut self.scheduler,
            context.owner_node,
            disposition,
        )?;
        self.patchbay_observation()
    }
}
