//! Fail-closed admission of Mask Form routes already sealed by an ordinary Plan.

use alloc::vec::Vec;
use conduit_core::{verify_plan, Plan, PlanId};

use crate::{MaskForm, SealedMaskFormRoute};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedMaskFormRoutes {
    plan_id: PlanId,
    routes: Vec<SealedMaskFormRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskRouteAdmissionError {
    InvalidPlan,
    WrongPlan,
    UnknownMaskForm,
    MissingPlacement,
    MissingBoundary,
}

impl AdmittedMaskFormRoutes {
    pub fn new(
        plan: &Plan,
        masks: &[MaskForm],
        routes: Vec<SealedMaskFormRoute>,
    ) -> Result<Self, MaskRouteAdmissionError> {
        if !verify_plan(plan) {
            return Err(MaskRouteAdmissionError::InvalidPlan);
        }
        let placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .collect::<Vec<_>>();
        for route in &routes {
            if route.plan_id != plan.plan_id {
                return Err(MaskRouteAdmissionError::WrongPlan);
            }
            let mask = masks
                .iter()
                .find(|mask| mask.form_identity == route.mask_form)
                .ok_or(MaskRouteAdmissionError::UnknownMaskForm)?;
            if route.placement_ids.iter().any(|placement_id| {
                !placements
                    .iter()
                    .any(|placement| placement.placement_id == *placement_id)
            }) {
                return Err(MaskRouteAdmissionError::MissingPlacement);
            }
            for boundary in mask
                .presentation_inputs
                .iter()
                .chain([&mask.interaction_output, &mask.show_output])
            {
                let admitted = placements.iter().any(|placement| {
                    placement.gear_id == boundary.gear_id
                        && route.placement_ids.contains(&placement.placement_id)
                });
                if !admitted {
                    return Err(MaskRouteAdmissionError::MissingBoundary);
                }
            }
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            routes,
        })
    }

    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn routes(&self) -> &[SealedMaskFormRoute] {
        &self.routes
    }
}
