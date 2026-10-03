//! One owner-issued local Mask route while the Body workload remains lulled.
//!
//! This seal is independent of a Wake-scoped BodyPlan. The ordinary Mask Plot
//! keeps its own Plan and Play. A later cross-Host route must admit its actual
//! external Lines; this local entrance refuses remote fragments explicitly.

use alloc::{format, string::String};
use conduit_body::{BodyId, BodyLifecycleSession, BodyState};
use conduit_core::{verify_plan, HostAdvertisement, PlanId, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ManifestationLifecycle, MaskShow, PlannedMaskPlot, Presentation, PresentationBasis,
    PresentationContentId,
};

const MAX_LOCAL_ROUTE_SEAL_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOwnerMaskRouteSeal {
    pub route_plan_id: PlanId,
    pub body_id: BodyId,
    pub workload_revision: u64,
    pub face_id: PresentationContentId,
    pub face_revision: u64,
    pub face_basis: PresentationBasis,
    pub owner_offer: HostAdvertisement,
    pub planned_mask: PlannedMaskPlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalOwnerMaskRouteError {
    WorkloadNotLulled,
    WorkloadRealizationPresent,
    InvalidFace,
    WrongFaceBasis,
    InvalidMaskPlan,
    RemoteLineRequired,
    StaleOrMissingOffer,
    UnsupportedAuthority,
    SealCapacityExceeded,
    InvalidSeal,
    StaleBody,
    StaleFace,
    StaleHost,
    InvalidShow,
    ShowUnavailable,
}

impl LocalOwnerMaskRouteSeal {
    /// The caller must supply the owner's actual current Host advertisement.
    /// Preparing this immutable route neither wakes the Body nor performs a
    /// Host effect; an attached provider must be checked again before Show.
    pub fn seal_lulled(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
    ) -> Result<Self, LocalOwnerMaskRouteError> {
        let body = &session.evidence().body;
        if body.state != BodyState::Lulled {
            return Err(LocalOwnerMaskRouteError::WorkloadNotLulled);
        }
        if session.realization().is_some() {
            return Err(LocalOwnerMaskRouteError::WorkloadRealizationPresent);
        }
        face.validate()
            .map_err(|_| LocalOwnerMaskRouteError::InvalidFace)?;
        if face.basis.body_id.as_ref() != Some(&body.body_id)
            || face.basis.wake_id.is_some()
            || face.basis.plan_id.is_some()
            || face.basis.active_play_id.is_some()
        {
            return Err(LocalOwnerMaskRouteError::WrongFaceBasis);
        }
        validate_local_mask(owner_offer, planned_mask)?;
        let mut route = Self {
            route_plan_id: PlanId::from("unsealed"),
            body_id: body.body_id.clone(),
            workload_revision: body.workload_revision,
            face_id: face.identity.clone(),
            face_revision: face.revision,
            face_basis: face.basis.clone(),
            owner_offer: owner_offer.clone(),
            planned_mask: planned_mask.clone(),
        };
        route.route_plan_id = route.bind_identity()?;
        Ok(route)
    }

    pub fn verify_seal(&self) -> Result<(), LocalOwnerMaskRouteError> {
        validate_local_mask(&self.owner_offer, &self.planned_mask)?;
        if self.face_basis.body_id.as_ref() != Some(&self.body_id)
            || self.face_basis.wake_id.is_some()
            || self.face_basis.plan_id.is_some()
            || self.face_basis.active_play_id.is_some()
            || self.bind_identity()? != self.route_plan_id
        {
            return Err(LocalOwnerMaskRouteError::InvalidSeal);
        }
        Ok(())
    }

    /// Recheck all mutable truth immediately before render, action, or Show.
    /// A changed Face must be freshly sealed; an old Show cannot be refreshed
    /// by substituting new content under this identity.
    pub fn validate_current(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current_owner_offer: &HostAdvertisement,
    ) -> Result<(), LocalOwnerMaskRouteError> {
        self.verify_seal()?;
        let body = &session.evidence().body;
        if body.body_id != self.body_id
            || body.workload_revision != self.workload_revision
            || body.state != BodyState::Lulled
            || session.realization().is_some()
        {
            return Err(LocalOwnerMaskRouteError::StaleBody);
        }
        face.validate()
            .map_err(|_| LocalOwnerMaskRouteError::InvalidFace)?;
        if face.identity != self.face_id
            || face.revision != self.face_revision
            || face.basis != self.face_basis
        {
            return Err(LocalOwnerMaskRouteError::StaleFace);
        }
        if current_owner_offer != &self.owner_offer {
            return Err(LocalOwnerMaskRouteError::StaleHost);
        }
        Ok(())
    }

    pub fn validate_available_show(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current_owner_offer: &HostAdvertisement,
        show: &MaskShow,
    ) -> Result<(), LocalOwnerMaskRouteError> {
        self.validate_current(session, face, current_owner_offer)?;
        if show.planned_mask != self.planned_mask
            || show.show.host_id != self.owner_offer.host_id
            || show.show.boot_id != self.owner_offer.boot_id
            || show.show.offer_generation != self.owner_offer.offer_generation
            || show.validate(face).is_err()
        {
            return Err(LocalOwnerMaskRouteError::InvalidShow);
        }
        if show.show.lifecycle != ManifestationLifecycle::Available || show.show.failure.is_some() {
            return Err(LocalOwnerMaskRouteError::ShowUnavailable);
        }
        Ok(())
    }

    fn bind_identity(&self) -> Result<PlanId, LocalOwnerMaskRouteError> {
        let bytes = postcard::to_allocvec(&(
            "conduit.presentation/local-owner-mask-route@1",
            &self.body_id,
            self.workload_revision,
            &self.face_id,
            self.face_revision,
            &self.face_basis,
            &self.owner_offer,
            &self.planned_mask,
        ))
        .map_err(|_| LocalOwnerMaskRouteError::InvalidSeal)?;
        if bytes.len() > MAX_LOCAL_ROUTE_SEAL_BYTES {
            return Err(LocalOwnerMaskRouteError::SealCapacityExceeded);
        }
        let digest = Sha256::digest(&bytes);
        Ok(PlanId::from(format!(
            "plan/local-owner-mask/{}",
            hex(&digest)
        )))
    }
}

fn validate_local_mask(
    owner: &HostAdvertisement,
    planned: &PlannedMaskPlot,
) -> Result<(), LocalOwnerMaskRouteError> {
    if owner.protocol_version != PROTOCOL_VERSION
        || owner.host_id.as_str().is_empty()
        || owner.boot_id.as_str().is_empty()
        || owner.offer_generation.0 == 0
        || !verify_plan(&planned.plan)
        || PlannedMaskPlot::admit(&planned.mask, &planned.plan).is_err()
    {
        return Err(LocalOwnerMaskRouteError::InvalidMaskPlan);
    }
    if planned.plan.fragments.is_empty() {
        return Err(LocalOwnerMaskRouteError::InvalidMaskPlan);
    }
    for fragment in &planned.plan.fragments {
        if fragment.host_id != owner.host_id || fragment.boot_id != owner.boot_id {
            return Err(LocalOwnerMaskRouteError::RemoteLineRequired);
        }
        if fragment.offer_generation != owner.offer_generation {
            return Err(LocalOwnerMaskRouteError::StaleOrMissingOffer);
        }
        for placement in &fragment.placements {
            if placement.host_id != owner.host_id
                || placement.boot_id != owner.boot_id
                || placement.offer_generation != owner.offer_generation
            {
                return Err(LocalOwnerMaskRouteError::StaleOrMissingOffer);
            }
            if placement.base.is_some()
                || !placement.authority.is_empty()
                || placement
                    .resources
                    .iter()
                    .any(|resource| resource.protected.is_some())
            {
                return Err(LocalOwnerMaskRouteError::UnsupportedAuthority);
            }
            let offered = owner.capabilities.iter().any(|offer| {
                offer.capability_id == placement.capability_id
                    && offer.kind_id == placement.kind_id
                    && offer.kind_contract_revision == placement.kind_contract_revision
                    && offer.implementation.execution_profile_id == placement.execution_profile_id
                    && offer.implementation.implementation_id == placement.implementation_id
                    && offer.implementation.artifact_id == placement.artifact_id
                    && offer.inputs == placement.inputs
                    && offer.outputs == placement.outputs
                    && offer.semantic_contract == placement.semantic_contract
                    && offer.host_calls == placement.host_calls
                    && offer.limits == placement.limits
            });
            if !offered
                || placement.resources.iter().any(|binding| {
                    !owner.resources.iter().any(|resource| {
                        resource.pool_id == binding.pool_id
                            && resource.class_id == binding.class_id
                            && resource.capacity_units >= binding.units
                            && resource.content == binding.content
                    })
                })
            {
                return Err(LocalOwnerMaskRouteError::StaleOrMissingOffer);
            }
        }
        if fragment
            .connections
            .iter()
            .any(|cord| cord.selected_line.is_some() || !cord.admitted_lines.is_empty())
        {
            return Err(LocalOwnerMaskRouteError::RemoteLineRequired);
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
