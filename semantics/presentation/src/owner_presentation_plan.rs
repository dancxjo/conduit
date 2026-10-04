//! One finite owner presentation Plan over already sealed ordinary Mask routes.
//! It does not Wake the workload Body or replace any child Mask Plot Plan.
use alloc::{boxed::Box, format, string::String, vec::Vec};
use conduit_body::{BodyId, BodyLifecycleSession};
use conduit_core::{HostAdvertisement, LineOffer, PlanId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AdmittedMaskPlotRoutes, LocalOwnerMaskRouteError, LocalOwnerMaskRouteSeal, Presentation,
    PresentationBasis, PresentationContentId, RemoteOwnerMaskRouteError, RemoteOwnerMaskRouteSeal,
    SealedMaskPlotRoute,
};

pub const MAX_OWNER_PRESENTATION_ROUTES: usize = 8;
const MAX_OWNER_PRESENTATION_PLAN_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnerPresentationChildRoute {
    Local { seal: Box<LocalOwnerMaskRouteSeal> },
    Remote { seal: Box<RemoteOwnerMaskRouteSeal> },
}

#[derive(Clone, Copy)]
pub enum CurrentOwnerPresentationRoute<'a> {
    Local {
        seal: &'a LocalOwnerMaskRouteSeal,
        owner_offer: &'a HostAdvertisement,
    },
    Remote {
        seal: &'a RemoteOwnerMaskRouteSeal,
        owner_offer: &'a HostAdvertisement,
        mask_host_offer: &'a HostAdvertisement,
        face_line: &'a LineOffer,
        return_line: &'a LineOffer,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerPresentationPlan {
    pub plan_id: PlanId,
    pub body_id: BodyId,
    pub workload_revision: u64,
    pub face_id: PresentationContentId,
    pub face_revision: u64,
    pub face_basis: PresentationBasis,
    pub routes: Vec<OwnerPresentationChildRoute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerPresentationPlanError {
    EmptyOrTooManyRoutes,
    DuplicateRoute,
    UnorderedRoutes,
    WrongBodyOrFace,
    StaleBodyOrFace,
    UnknownOrDuplicateWitness,
    InvalidChild,
    InvalidIdentity,
    CapacityExceeded,
    Local(LocalOwnerMaskRouteError),
    Remote(RemoteOwnerMaskRouteError),
}

impl OwnerPresentationChildRoute {
    pub fn route_plan_id(&self) -> &PlanId {
        match self {
            Self::Local { seal } => &seal.route_plan_id,
            Self::Remote { seal } => &seal.route_plan_id,
        }
    }

    fn planned_mask(&self) -> &crate::PlannedMaskPlot {
        match self {
            Self::Local { seal } => &seal.planned_mask,
            Self::Remote { seal } => &seal.planned_mask,
        }
    }

    fn common_basis(
        &self,
    ) -> (
        &BodyId,
        u64,
        &PresentationContentId,
        u64,
        &PresentationBasis,
    ) {
        match self {
            Self::Local { seal } => (
                &seal.body_id,
                seal.workload_revision,
                &seal.face_id,
                seal.face_revision,
                &seal.face_basis,
            ),
            Self::Remote { seal } => (
                &seal.body_id,
                seal.workload_revision,
                &seal.face_id,
                seal.face_revision,
                &seal.face_basis,
            ),
        }
    }

    fn verify_seal(&self) -> Result<(), OwnerPresentationPlanError> {
        match self {
            Self::Local { seal } => seal
                .verify_seal()
                .map_err(OwnerPresentationPlanError::Local),
            Self::Remote { seal } => seal
                .verify_seal()
                .map_err(OwnerPresentationPlanError::Remote),
        }
    }
}

impl CurrentOwnerPresentationRoute<'_> {
    fn child(&self) -> OwnerPresentationChildRoute {
        match self {
            Self::Local { seal, .. } => OwnerPresentationChildRoute::Local {
                seal: Box::new((*seal).clone()),
            },
            Self::Remote { seal, .. } => OwnerPresentationChildRoute::Remote {
                seal: Box::new((*seal).clone()),
            },
        }
    }

    fn validate_current(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
    ) -> Result<(), OwnerPresentationPlanError> {
        match self {
            Self::Local { seal, owner_offer } => seal
                .validate_current(session, face, owner_offer)
                .map_err(OwnerPresentationPlanError::Local),
            Self::Remote {
                seal,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
            } => seal
                .validate_current(
                    session,
                    face,
                    owner_offer,
                    mask_host_offer,
                    face_line,
                    return_line,
                )
                .map_err(OwnerPresentationPlanError::Remote),
        }
    }
}

impl OwnerPresentationPlan {
    pub fn seal_current(
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<Self, OwnerPresentationPlanError> {
        if current.is_empty() || current.len() > MAX_OWNER_PRESENTATION_ROUTES {
            return Err(OwnerPresentationPlanError::EmptyOrTooManyRoutes);
        }
        let mut routes = current
            .iter()
            .map(CurrentOwnerPresentationRoute::child)
            .collect::<Vec<_>>();
        routes.sort_by(|a, b| a.route_plan_id().cmp(b.route_plan_id()));
        let (body_id, workload_revision, face_id, face_revision, face_basis) =
            routes[0].common_basis();
        let mut plan = Self {
            plan_id: PlanId::from("unsealed"),
            body_id: body_id.clone(),
            workload_revision,
            face_id: face_id.clone(),
            face_revision,
            face_basis: face_basis.clone(),
            routes,
        };
        plan.validate_children()?;
        plan.validate_current_basis(session, face)?;
        for witness in current {
            witness.validate_current(session, face)?;
        }
        plan.plan_id = plan.bind_identity()?;
        Ok(plan)
    }

    pub fn verify_seal(&self) -> Result<(), OwnerPresentationPlanError> {
        self.validate_children()?;
        if self.bind_identity()? != self.plan_id {
            return Err(OwnerPresentationPlanError::InvalidIdentity);
        }
        Ok(())
    }

    /// Missing current witnesses make only those previously sealed routes unavailable.
    /// Supplied but stale or unsealed witnesses refuse rather than becoming availability.
    pub fn admit_current_routes(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<AdmittedMaskPlotRoutes, OwnerPresentationPlanError> {
        self.verify_seal()?;
        self.validate_current_basis(session, face)?;
        if current.len() > self.routes.len() {
            return Err(OwnerPresentationPlanError::UnknownOrDuplicateWitness);
        }
        let mut available = Vec::with_capacity(self.routes.len());
        available.resize(self.routes.len(), false);
        for witness in current {
            let child = witness.child();
            let Some(index) = self.routes.iter().position(|route| route == &child) else {
                return Err(OwnerPresentationPlanError::UnknownOrDuplicateWitness);
            };
            if available[index] {
                return Err(OwnerPresentationPlanError::UnknownOrDuplicateWitness);
            }
            witness.validate_current(session, face)?;
            available[index] = true;
        }
        let routes = self
            .routes
            .iter()
            .zip(available)
            .map(|(child, currently_available)| {
                let planned = child.planned_mask();
                SealedMaskPlotRoute {
                    route_id: format!("route/{}", child.route_plan_id().as_str()),
                    mask_plot: planned.mask.plot_identity.clone(),
                    plan_id: self.plan_id.clone(),
                    child_mask_plan_id: Some(planned.plan.plan_id.clone()),
                    owner_route_seal_id: Some(child.route_plan_id().clone()),
                    placement_ids: planned
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| {
                            fragment
                                .placements
                                .iter()
                                .map(|placement| placement.placement_id.clone())
                        })
                        .collect(),
                    currently_available,
                }
            })
            .collect();
        Ok(
            AdmittedMaskPlotRoutes::from_verified_owner_presentation_routes(
                self.body_id.clone(),
                self.plan_id.clone(),
                routes,
            ),
        )
    }

    fn validate_children(&self) -> Result<(), OwnerPresentationPlanError> {
        if self.routes.is_empty() || self.routes.len() > MAX_OWNER_PRESENTATION_ROUTES {
            return Err(OwnerPresentationPlanError::EmptyOrTooManyRoutes);
        }
        if self.face_basis.body_id.as_ref() != Some(&self.body_id) {
            return Err(OwnerPresentationPlanError::WrongBodyOrFace);
        }
        for (index, child) in self.routes.iter().enumerate() {
            child.verify_seal()?;
            if child.route_plan_id().as_str().is_empty()
                || child.planned_mask().plan.plan_id.as_str().is_empty()
                || child
                    .planned_mask()
                    .plan
                    .fragments
                    .iter()
                    .all(|fragment| fragment.placements.is_empty())
            {
                return Err(OwnerPresentationPlanError::InvalidChild);
            }
            let (body, revision, face_id, face_revision, basis) = child.common_basis();
            if body != &self.body_id
                || revision != self.workload_revision
                || face_id != &self.face_id
                || face_revision != self.face_revision
                || basis != &self.face_basis
            {
                return Err(OwnerPresentationPlanError::WrongBodyOrFace);
            }
            if index > 0 && self.routes[index - 1].route_plan_id() >= child.route_plan_id() {
                return Err(
                    if self.routes[index - 1].route_plan_id() == child.route_plan_id() {
                        OwnerPresentationPlanError::DuplicateRoute
                    } else {
                        OwnerPresentationPlanError::UnorderedRoutes
                    },
                );
            }
        }
        Ok(())
    }

    fn validate_current_basis(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
    ) -> Result<(), OwnerPresentationPlanError> {
        let body = &session.evidence().body;
        if body.body_id != self.body_id
            || body.workload_revision != self.workload_revision
            || face.validate().is_err()
            || face.identity != self.face_id
            || face.revision != self.face_revision
            || face.basis != self.face_basis
        {
            return Err(OwnerPresentationPlanError::StaleBodyOrFace);
        }
        Ok(())
    }

    fn bind_identity(&self) -> Result<PlanId, OwnerPresentationPlanError> {
        let bytes = serde_json::to_vec(&(
            "conduit.presentation/owner-presentation-plan@1",
            &self.body_id,
            self.workload_revision,
            &self.face_id,
            self.face_revision,
            &self.face_basis,
            &self.routes,
        ))
        .map_err(|_| OwnerPresentationPlanError::InvalidIdentity)?;
        if bytes.len() > MAX_OWNER_PRESENTATION_PLAN_BYTES {
            return Err(OwnerPresentationPlanError::CapacityExceeded);
        }
        let digest = Sha256::digest(&bytes);
        let mut hex = String::with_capacity(64);
        for byte in digest {
            use core::fmt::Write;
            write!(&mut hex, "{byte:02x}")
                .map_err(|_| OwnerPresentationPlanError::InvalidIdentity)?;
        }
        Ok(PlanId::from(format!("plan/owner-presentation/{hex}")))
    }
}
