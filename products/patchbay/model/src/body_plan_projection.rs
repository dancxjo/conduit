//! Renderer-neutral Patchbay projection of one exact body-wide Plan.
//!
//! This is a read-only view over `BodyPlan`; it neither selects placements nor
//! keeps a second mutable copy of planning truth.

use conduit_body::{BodyId, BodyPlan, WakeId};
use conduit_core::{
    BootId, CapabilityId, CheckedFormId, GearId, HostId, ImplementationId, PlanId, SourceDocumentId,
};
use serde::Serialize;

pub const BODY_PLAN_PROJECTION_SCHEMA: &str = "conduit.patchbay/body-plan-projection@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyPlanProjection {
    pub schema: &'static str,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub plan_id: PlanId,
    pub workload_revision: u64,
    pub active_forms: Vec<BodyPlanFormProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyPlanFormProjection {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub form_plan_id: PlanId,
    pub placements: Vec<BodyPlanPlacementProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyPlanPlacementProjection {
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
}

pub fn project_body_plan(plan: &BodyPlan) -> BodyPlanProjection {
    BodyPlanProjection {
        schema: BODY_PLAN_PROJECTION_SCHEMA,
        body_id: plan.body_id.clone(),
        wake_id: plan.wake_id.clone(),
        plan_id: plan.plan_id.clone(),
        workload_revision: plan.workload_revision,
        active_forms: plan
            .forms
            .iter()
            .map(|form| BodyPlanFormProjection {
                source_document_id: form.form.source_document_id.clone(),
                checked_form_id: form.form.checked_form_id.clone(),
                form_plan_id: form.plan.plan_id.clone(),
                placements: form
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.placements)
                    .map(|placement| BodyPlanPlacementProjection {
                        gear_id: placement.gear_id.clone(),
                        host_id: placement.host_id.clone(),
                        boot_id: placement.boot_id.clone(),
                        capability_id: placement.capability_id.clone(),
                        implementation_id: placement.implementation_id.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
