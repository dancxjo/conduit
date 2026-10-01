//! Fail-closed admission of Mask Form routes already sealed by an ordinary Plan.

use alloc::vec::Vec;
use conduit_body::BodyPlan;
use conduit_core::{verify_plan, PlanId};

use crate::{PlannedMaskForm, SealedMaskFormRoute};

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
    UnsealedMaskForm,
    MissingPlacement,
    MissingBoundary,
}

impl AdmittedMaskFormRoutes {
    pub fn new(
        body_plan: &BodyPlan,
        masks: &[PlannedMaskForm],
        routes: Vec<SealedMaskFormRoute>,
    ) -> Result<Self, MaskRouteAdmissionError> {
        if body_plan.verify_seal().is_err() {
            return Err(MaskRouteAdmissionError::InvalidPlan);
        }
        for route in &routes {
            if route.plan_id != body_plan.plan_id {
                return Err(MaskRouteAdmissionError::WrongPlan);
            }
            let matching_masks = masks
                .iter()
                .filter(|planned| planned.mask.form_identity == route.mask_form)
                .collect::<Vec<_>>();
            if matching_masks.is_empty() {
                return Err(MaskRouteAdmissionError::UnknownMaskForm);
            }
            let planned = matching_masks
                .into_iter()
                .find(|planned| route_matches_plan(route, planned))
                .ok_or(MaskRouteAdmissionError::MissingPlacement)?;
            if !verify_plan(&planned.plan)
                || PlannedMaskForm::admit(&planned.mask, &planned.plan).is_err()
            {
                return Err(MaskRouteAdmissionError::InvalidPlan);
            }
            let sealed = body_plan
                .mask_topologies
                .iter()
                .flat_map(|topology| &topology.chains)
                .any(|chain| chain.plan == planned.plan);
            if !sealed {
                return Err(MaskRouteAdmissionError::UnsealedMaskForm);
            }
            let placements = planned
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .collect::<Vec<_>>();
            for boundary in [
                &planned.mask.face_input,
                &planned.mask.interaction_output,
                &planned.mask.show_output,
            ] {
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
            plan_id: body_plan.plan_id.clone(),
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

fn route_matches_plan(route: &SealedMaskFormRoute, planned: &PlannedMaskForm) -> bool {
    let placements = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| &placement.placement_id)
        .collect::<Vec<_>>();
    route.placement_ids.len() == placements.len()
        && route
            .placement_ids
            .iter()
            .enumerate()
            .all(|(index, placement_id)| {
                !route.placement_ids[..index].contains(placement_id)
                    && placements.contains(&placement_id)
            })
}
