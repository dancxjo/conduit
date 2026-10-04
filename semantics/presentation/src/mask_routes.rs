//! Fail-closed admission of Mask Plot routes already sealed by an ordinary Plan.

use alloc::vec::Vec;
use conduit_body::{BodyId, BodyLifecycleSession, BodyPlan};
use conduit_core::{verify_plan, HostAdvertisement, LineOffer, PlanId};

use crate::{
    LocalOwnerMaskRouteError, LocalOwnerMaskRouteSeal, MaskShow, PlannedMaskPlot, Presentation,
    RemoteOwnerMaskRouteError, RemoteOwnerMaskRouteSeal, SealedMaskPlotRoute,
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
    RemoteOwnerRoute(RemoteOwnerMaskRouteError),
}

impl AdmittedMaskPlotRoutes {
    pub(crate) fn from_verified_owner_presentation_routes(
        body_id: BodyId,
        plan_id: PlanId,
        routes: Vec<SealedMaskPlotRoute>,
    ) -> Self {
        // Only OwnerPresentationPlan::admit_current_routes calls this after
        // verifying every retained child seal and current witness.
        Self {
            body_id,
            plan_id,
            routes,
        }
    }

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
        Self::single_owner_route(&seal.body_id, &seal.route_plan_id, &seal.planned_mask)
    }

    fn single_owner_route(
        body_id: &BodyId,
        plan_id: &PlanId,
        planned: &PlannedMaskPlot,
    ) -> Result<Self, MaskRouteAdmissionError> {
        let placement_ids = planned
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
            body_id: body_id.clone(),
            plan_id: plan_id.clone(),
            routes: alloc::vec![SealedMaskPlotRoute {
                route_id: alloc::format!("route/{}", plan_id.as_str()),
                mask_plot: planned.mask.plot_identity.clone(),
                plan_id: plan_id.clone(),
                child_mask_plan_id: None,
                owner_route_seal_id: None,
                placement_ids,
                currently_available: true,
            }],
        })
    }

    /// Admit an acknowledged Show on an owner-sealed remote presentation
    /// route. The route Plan remains separate from the Body workload Wake.
    #[allow(clippy::too_many_arguments)]
    pub fn from_remote_owner_show(
        seal: &RemoteOwnerMaskRouteSeal,
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        face_line: &LineOffer,
        return_line: &LineOffer,
        show: &MaskShow,
    ) -> Result<Self, MaskRouteAdmissionError> {
        seal.validate_available_show(
            session,
            face,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
            show,
        )
        .map_err(MaskRouteAdmissionError::RemoteOwnerRoute)?;
        Self::single_owner_route(&seal.body_id, &seal.route_plan_id, &seal.planned_mask)
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
