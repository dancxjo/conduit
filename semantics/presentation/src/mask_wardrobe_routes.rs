//! Admission proof that runtime Mask routes already exist in one exact Plan.

use alloc::vec::Vec;
use conduit_core::PlanId;
use serde::{Deserialize, Serialize};

use crate::{
    MaskBoundaryRole, MaskSpecification, MaskWardrobeError, PlannedMask, SealedMaskRoute,
    MAX_SEALED_MASK_ROUTES,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedMaskRoutes {
    plan_id: PlanId,
    routes: Vec<SealedMaskRoute>,
}

impl AdmittedMaskRoutes {
    pub fn new(
        plan_id: PlanId,
        specifications: &[MaskSpecification],
        planned_masks: &[PlannedMask],
        routes: Vec<SealedMaskRoute>,
    ) -> Result<Self, MaskWardrobeError> {
        if routes.len() > MAX_SEALED_MASK_ROUTES {
            return Err(MaskWardrobeError::RouteCapacityExceeded);
        }
        for route in &routes {
            let specification = specifications
                .iter()
                .find(|candidate| candidate.specification_id == route.specification_id)
                .ok_or(MaskWardrobeError::UnsealedRoute)?;
            let planned = planned_masks
                .iter()
                .find(|candidate| {
                    candidate.specification_id == route.specification_id
                        && candidate.plan_id == plan_id
                })
                .ok_or(MaskWardrobeError::UnsealedRoute)?;
            validate_route(&plan_id, specification, planned, route)?;
        }
        Ok(Self { plan_id, routes })
    }

    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn routes(&self) -> &[SealedMaskRoute] {
        &self.routes
    }
}

fn validate_route(
    plan_id: &PlanId,
    specification: &MaskSpecification,
    planned: &PlannedMask,
    route: &SealedMaskRoute,
) -> Result<(), MaskWardrobeError> {
    if route.plan_id != *plan_id
        || planned.plan_id != *plan_id
        || planned.specification_id != specification.specification_id
        || planned.specification_revision != specification.revision
        || route.stage_ids.iter().any(|stage_id| {
            !planned
                .stages
                .iter()
                .any(|stage| stage.stage_id == *stage_id)
        })
        || route.stage_ids.windows(2).any(|pair| {
            !planned
                .cords
                .iter()
                .any(|cord| cord.source_stage_id == pair[0] && cord.sink_stage_id == pair[1])
        })
    {
        return Err(MaskWardrobeError::UnsealedRoute);
    }
    let starts_at_presentation = specification.boundaries.iter().any(|boundary| {
        boundary.role == MaskBoundaryRole::PresentationInput
            && route.stage_ids.first() == Some(&boundary.stage_id)
    });
    let ends_at_show = specification.boundaries.iter().any(|boundary| {
        boundary.role == MaskBoundaryRole::ShowOutput
            && route.stage_ids.last() == Some(&boundary.stage_id)
    });
    if !starts_at_presentation || !ends_at_show {
        return Err(MaskWardrobeError::UnsealedRoute);
    }
    Ok(())
}
