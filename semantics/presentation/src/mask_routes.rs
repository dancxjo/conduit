//! Fail-closed admission of Mask Plot routes already sealed by an ordinary Plan.

use alloc::vec::Vec;
use conduit_body::{BodyId, BodyLifecycleSession, BodyPlan};
use conduit_core::{verify_plan, HostAdvertisement, PlanId};

use crate::{
    LocalOwnerMaskRouteError, LocalOwnerMaskRouteSeal, MaskShow, PlannedMaskPlot, Presentation,
    SealedMaskPlotRoute,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedMaskPlotRoutes {
    body_id: BodyId,
    plan_id: PlanId,
    routes: Vec<SealedMaskPlotRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskRouteAdmissionError {
    InvalidPlan,
    WrongPlan,
    UnknownMaskPlot,
    UnsealedMaskPlot,
    MissingPlacement,
    MissingBoundary,
    OwnerRoute(LocalOwnerMaskRouteError),
}

impl AdmittedMaskPlotRoutes {
    pub fn new(
        body_plan: &BodyPlan,
        masks: &[PlannedMaskPlot],
        routes: Vec<SealedMaskPlotRoute>,
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
                .filter(|planned| planned.mask.plot_identity == route.mask_plot)
                .collect::<Vec<_>>();
            if matching_masks.is_empty() {
                return Err(MaskRouteAdmissionError::UnknownMaskPlot);
            }
            let planned = matching_masks
                .into_iter()
                .find(|planned| route_matches_plan(route, planned))
                .ok_or(MaskRouteAdmissionError::MissingPlacement)?;
            if !verify_plan(&planned.plan)
                || PlannedMaskPlot::admit(&planned.mask, &planned.plan).is_err()
            {
                return Err(MaskRouteAdmissionError::InvalidPlan);
            }
            let sealed = body_plan
                .mask_topologies
                .iter()
                .flat_map(|topology| &topology.chains)
                .any(|chain| chain.plan == planned.plan);
            if !sealed {
                return Err(MaskRouteAdmissionError::UnsealedMaskPlot);
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
            body_id: body_plan.body_id.clone(),
            plan_id: body_plan.plan_id.clone(),
            routes,
        })
    }

    /// Admit a real owner-issued Mask Show while its workload Body is lulled.
    /// The owner seal supplies the ordinary Mask Plan; no workload Wake or
    /// BodyPlan is synthesized to make wardrobe selection possible.
    pub fn from_local_owner_show(
        seal: &LocalOwnerMaskRouteSeal,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current_owner_offer: &HostAdvertisement,
        show: &MaskShow,
    ) -> Result<Self, MaskRouteAdmissionError> {
        seal.validate_available_show(session, face, current_owner_offer, show)
            .map_err(MaskRouteAdmissionError::OwnerRoute)?;
        let placement_ids = seal
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect::<Vec<_>>();
        if placement_ids.is_empty() {
            return Err(MaskRouteAdmissionError::MissingPlacement);
        }
        Ok(Self {
            body_id: seal.body_id.clone(),
            plan_id: seal.route_plan_id.clone(),
            routes: alloc::vec![SealedMaskPlotRoute {
                route_id: alloc::format!("route/{}", seal.route_plan_id.as_str()),
                mask_plot: seal.planned_mask.mask.plot_identity.clone(),
                plan_id: seal.route_plan_id.clone(),
                placement_ids,
                currently_available: true,
            }],
        })
    }

    pub fn body_id(&self) -> &BodyId {
        &self.body_id
    }

    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn routes(&self) -> &[SealedMaskPlotRoute] {
        &self.routes
    }
}

fn route_matches_plan(route: &SealedMaskPlotRoute, planned: &PlannedMaskPlot) -> bool {
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
