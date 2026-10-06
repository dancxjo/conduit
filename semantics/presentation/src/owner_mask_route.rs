//! One owner-issued local Mask route while the Body workload remains lulled.
//!
//! This seal is independent of a Wake-scoped BodyPlan. The ordinary Mask Plot
//! keeps its own Plan and Play. A later cross-Host route must admit its actual
//! external Lines; this local entrance refuses remote fragments explicitly.

use alloc::{format, string::String, vec::Vec};
use conduit_body::{BodyId, BodyLifecycleSession, BodyState};
use conduit_core::{verify_plan, AuthorityGrant, HostAdvertisement, PlanId, PROTOCOL_VERSION};
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
    /// Exact grants supplied by the owning Host for this local Mask Plan.
    /// The owner must withdraw the current witness when a provider is lost.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authority_grants: Vec<AuthorityGrant>,
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
    InvalidAuthorityGrant,
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
        Self::seal_lulled_with_grants(session, face, owner_offer, planned_mask, &[])
    }

    /// Seal a local Mask with explicit Host/Boot-scoped authority. Grant
    /// possession is checked again by the owner before supplying a current
    /// witness; retaining the sealed Plan never keeps a lost provider alive.
    pub fn seal_lulled_with_grants(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        grants: &[AuthorityGrant],
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
        validate_local_grants(owner_offer, planned_mask, grants)?;
        let mut route = Self {
            route_plan_id: PlanId::from("unsealed"),
            body_id: body.body_id.clone(),
            workload_revision: body.workload_revision,
            face_id: face.identity.clone(),
            face_revision: face.revision,
            face_basis: face.basis.clone(),
            owner_offer: owner_offer.clone(),
            planned_mask: planned_mask.clone(),
            authority_grants: grants.to_vec(),
        };
        route.route_plan_id = route.bind_identity()?;
        Ok(route)
    }

    pub fn verify_seal(&self) -> Result<(), LocalOwnerMaskRouteError> {
        validate_local_grants(
            &self.owner_offer,
            &self.planned_mask,
            &self.authority_grants,
        )?;
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

    /// The installed owner calls this before publishing a current local
    /// speech witness. A Plan's retained grant list is not evidence that the
    /// provider still possesses those grants after loss or reattachment.
    pub fn validate_current_with_grants(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current_owner_offer: &HostAdvertisement,
        current_grants: &[AuthorityGrant],
    ) -> Result<(), LocalOwnerMaskRouteError> {
        self.validate_current(session, face, current_owner_offer)?;
        if self.authority_grants.len() != current_grants.len()
            || self
                .authority_grants
                .iter()
                .any(|grant| !current_grants.contains(grant))
        {
            return Err(LocalOwnerMaskRouteError::InvalidAuthorityGrant);
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
        // CapabilityOffer is a named-field contract with flattened fields and
        // deliberately refuses positional serializers such as postcard.
        let bytes = serde_json::to_vec(&(
            "conduit.presentation/local-owner-mask-route@1",
            &self.body_id,
            self.workload_revision,
            &self.face_id,
            self.face_revision,
            &self.face_basis,
            &self.owner_offer,
            &self.planned_mask,
            &self.authority_grants,
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

pub(crate) fn validate_single_host_mask(
    host: &HostAdvertisement,
    planned: &PlannedMaskPlot,
    allow_offered_authority: bool,
) -> Result<(), LocalOwnerMaskRouteError> {
    if host.protocol_version != PROTOCOL_VERSION
        || host.host_id.as_str().is_empty()
        || host.boot_id.as_str().is_empty()
        || host.offer_generation.0 == 0
        || !verify_plan(&planned.plan)
        || PlannedMaskPlot::admit(&planned.mask, &planned.plan).is_err()
    {
        return Err(LocalOwnerMaskRouteError::InvalidMaskPlan);
    }
    if planned.plan.fragments.is_empty() {
        return Err(LocalOwnerMaskRouteError::InvalidMaskPlan);
    }
    for fragment in &planned.plan.fragments {
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(LocalOwnerMaskRouteError::RemoteLineRequired);
        }
        if fragment.offer_generation != host.offer_generation {
            return Err(LocalOwnerMaskRouteError::StaleOrMissingOffer);
        }
        for placement in &fragment.placements {
            if placement.host_id != host.host_id
                || placement.boot_id != host.boot_id
                || placement.offer_generation != host.offer_generation
            {
                return Err(LocalOwnerMaskRouteError::StaleOrMissingOffer);
            }
            if (!allow_offered_authority
                && (placement.base.is_some() || !placement.authority.is_empty()))
                || placement
                    .resources
                    .iter()
                    .any(|resource| resource.protected.is_some())
            {
                return Err(LocalOwnerMaskRouteError::UnsupportedAuthority);
            }
            let offered = host.capabilities.iter().any(|offer| {
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
                    && placement.base.as_ref()
                        == host
                            .bases
                            .iter()
                            .find(|base| base.capability_ids.contains(&offer.capability_id))
                            .map(|base| base.binding())
                            .as_ref()
                    && placement.authority.len() == offer.authority_requirements.len()
                    && placement.authority.iter().all(|binding| {
                        !binding.grant_id.as_str().is_empty()
                            && binding.host_id == host.host_id
                            && binding.boot_id == host.boot_id
                            && binding.capability_id == offer.capability_id
                            && offer.authority_requirements.iter().any(|requirement| {
                                binding.contract_id == requirement.contract_id
                                    && binding.host_call_contract_id
                                        == requirement.host_call_contract_id
                                    && binding.subject_kind == requirement.subject_kind
                            })
                    })
            });
            if !offered
                || placement.resources.iter().any(|binding| {
                    !host.resources.iter().any(|resource| {
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

fn validate_local_grants(
    host: &HostAdvertisement,
    planned: &PlannedMaskPlot,
    grants: &[AuthorityGrant],
) -> Result<(), LocalOwnerMaskRouteError> {
    if grants.len() > 16 {
        return Err(LocalOwnerMaskRouteError::InvalidAuthorityGrant);
    }
    validate_single_host_mask(host, planned, !grants.is_empty())?;
    let bindings = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .flat_map(|placement| &placement.authority)
        .collect::<alloc::vec::Vec<_>>();
    if bindings.len() != grants.len() {
        return Err(LocalOwnerMaskRouteError::InvalidAuthorityGrant);
    }
    for (index, grant) in grants.iter().enumerate() {
        if grant.grant_id.as_str().is_empty()
            || grant.host_id != host.host_id
            || grant.boot_id != host.boot_id
            || grants[..index]
                .iter()
                .any(|prior| prior.grant_id == grant.grant_id)
            || bindings
                .iter()
                .filter(|binding| {
                    binding.grant_id == grant.grant_id
                        && binding.contract_id == grant.contract_id
                        && binding.host_call_contract_id == grant.host_call_contract_id
                        && binding.subject_kind == grant.subject_kind
                        && binding.host_id == grant.host_id
                        && binding.boot_id == grant.boot_id
                        && binding.capability_id == grant.capability_id
                })
                .count()
                != 1
        {
            return Err(LocalOwnerMaskRouteError::InvalidAuthorityGrant);
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
