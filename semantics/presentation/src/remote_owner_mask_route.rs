//! An owner-issued presentation route for a remote, single-Host Mask Plot.
//!
//! The two directional Lines bind the Mask Fore to the owner while the
//! workload may be lulled or have its own current Wake. A carrier must supply
//! live Line offers; matching identifiers or browser membership alone do not.

use alloc::{format, string::String};
use conduit_body::{BodyId, BodyLifecycleSession, BodyState};
use conduit_core::{
    AdmittedLine, BootId, HostAdvertisement, HostId, LineAvailability, LineOffer, LineOrdering,
    LineReliability, LineScope, LineTrafficShape, LinkAuthorityReference, LinkCredentialReference,
    OfferGeneration, PlanId, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    owner_mask_route::validate_single_host_mask, LocalOwnerMaskRouteError, ManifestationLifecycle,
    MaskShow, PlannedMaskPlot, Presentation, PresentationBasis, PresentationContentId,
};

const MAX_REMOTE_ROUTE_SEAL_BYTES: usize = 32 * 1024;
const MAX_HOST_OFFER_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteMaskHostBasis {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub offer_digest: String,
}

impl RemoteMaskHostBasis {
    fn bind(host: &HostAdvertisement) -> Result<Self, RemoteOwnerMaskRouteError> {
        if host.protocol_version != PROTOCOL_VERSION
            || host.host_id.as_str().is_empty()
            || host.boot_id.as_str().is_empty()
            || host.offer_generation.0 == 0
        {
            return Err(RemoteOwnerMaskRouteError::InvalidHost);
        }
        let bytes = serde_json::to_vec(host).map_err(|_| RemoteOwnerMaskRouteError::InvalidHost)?;
        if bytes.len() > MAX_HOST_OFFER_BYTES {
            return Err(RemoteOwnerMaskRouteError::SealCapacityExceeded);
        }
        Ok(Self {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            offer_digest: hex(&Sha256::digest(&bytes)),
        })
    }

    fn valid(&self) -> bool {
        !self.host_id.as_str().is_empty()
            && !self.boot_id.as_str().is_empty()
            && self.offer_generation.0 != 0
            && self.offer_digest.len() == 64
            && self
                .offer_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteOwnerMaskRouteSeal {
    pub route_plan_id: PlanId,
    pub body_id: BodyId,
    pub workload_revision: u64,
    pub face_id: PresentationContentId,
    pub face_revision: u64,
    pub face_basis: PresentationBasis,
    pub owner_host: RemoteMaskHostBasis,
    pub mask_host: RemoteMaskHostBasis,
    pub planned_mask: PlannedMaskPlot,
    pub face_line: AdmittedLine,
    pub return_line: AdmittedLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteOwnerMaskRouteError {
    WorkloadNotLulled,
    WorkloadRealizationPresent,
    InvalidFace,
    WrongFaceBasis,
    WrongOrMissingPart,
    InvalidMaskPlan(LocalOwnerMaskRouteError),
    InvalidHost,
    MissingOrInvalidLine,
    LineUnavailable,
    FaceExceedsLine,
    ReturnExceedsLine,
    SealCapacityExceeded,
    InvalidSeal,
    StaleBody,
    StaleFace,
    StaleHost,
    StaleLine,
    InvalidShow,
    ShowUnavailable,
}

impl RemoteOwnerMaskRouteSeal {
    /// This immutable admission binds an ordinary remote Mask Plan to two
    /// current directional Lines. The caller must own those actual Lines; the
    /// seal never discovers connectivity from membership or Host names.
    pub fn seal_lulled(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<Self, RemoteOwnerMaskRouteError> {
        if session.evidence().body.state != BodyState::Lulled {
            return Err(RemoteOwnerMaskRouteError::WorkloadNotLulled);
        }
        Self::seal_current(
            session,
            face,
            owner_offer,
            mask_host_offer,
            planned_mask,
            face_line,
            return_line,
        )
    }

    /// A presentation Play can continue across a workload Wake without
    /// becoming that workload's Play. Each changed Face gets a fresh seal.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_current(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<Self, RemoteOwnerMaskRouteError> {
        let body = &session.evidence().body;
        face.validate()
            .map_err(|_| RemoteOwnerMaskRouteError::InvalidFace)?;
        if face.basis.body_id.as_ref() != Some(&body.body_id)
            || face.basis.plan_id.is_some()
            || face.basis.active_play_id.is_some()
        {
            return Err(RemoteOwnerMaskRouteError::WrongFaceBasis);
        }
        validate_workload_basis(session, face)?;
        validate_hosts(session, owner_offer, mask_host_offer)?;
        let owner_host = RemoteMaskHostBasis::bind(owner_offer)?;
        let mask_host = RemoteMaskHostBasis::bind(mask_host_offer)?;
        validate_single_host_mask(mask_host_offer, planned_mask)
            .map_err(RemoteOwnerMaskRouteError::InvalidMaskPlan)?;
        validate_line(face_line, &owner_host, &mask_host)?;
        validate_line(return_line, &mask_host, &owner_host)?;
        if face_line.binding.binding_id == return_line.binding.binding_id
            || face_line.line_id == return_line.line_id
        {
            return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
        }
        let face_bytes =
            serde_json::to_vec(face).map_err(|_| RemoteOwnerMaskRouteError::InvalidFace)?;
        if face_bytes.len() > face_line.binding.limits.maximum_payload_bytes as usize {
            return Err(RemoteOwnerMaskRouteError::FaceExceedsLine);
        }
        let mut seal = Self {
            route_plan_id: PlanId::from("unsealed"),
            body_id: body.body_id.clone(),
            workload_revision: body.workload_revision,
            face_id: face.identity.clone(),
            face_revision: face.revision,
            face_basis: face.basis.clone(),
            owner_host,
            mask_host,
            planned_mask: planned_mask.clone(),
            face_line: face_line.admitted_line(),
            return_line: return_line.admitted_line(),
        };
        seal.route_plan_id = seal.bind_identity()?;
        Ok(seal)
    }

    pub fn verify_seal(&self) -> Result<(), RemoteOwnerMaskRouteError> {
        if self.face_basis.body_id.as_ref() != Some(&self.body_id)
            || self.face_basis.plan_id.is_some()
            || self.face_basis.active_play_id.is_some()
            || !self.owner_host.valid()
            || !self.mask_host.valid()
            || self.owner_host.host_id == self.mask_host.host_id
        {
            return Err(RemoteOwnerMaskRouteError::InvalidSeal);
        }
        validate_mask_basis(&self.mask_host, &self.planned_mask)?;
        validate_admitted_line(&self.face_line, &self.owner_host, &self.mask_host)?;
        validate_admitted_line(&self.return_line, &self.mask_host, &self.owner_host)?;
        if self.face_line.line_id == self.return_line.line_id
            || self.face_line.binding.binding_id == self.return_line.binding.binding_id
            || self.bind_identity()? != self.route_plan_id
        {
            return Err(RemoteOwnerMaskRouteError::InvalidSeal);
        }
        Ok(())
    }

    /// The selected Mask Host checks the owner-issued basis against its own
    /// current offer before preparing the supplied Plan.
    pub fn validate_mask_host_offer(
        &self,
        host: &HostAdvertisement,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        self.verify_seal()?;
        if RemoteMaskHostBasis::bind(host)? != self.mask_host {
            return Err(RemoteOwnerMaskRouteError::StaleHost);
        }
        validate_single_host_mask(host, &self.planned_mask)
            .map_err(RemoteOwnerMaskRouteError::InvalidMaskPlan)
    }

    /// Recheck mutable Body, Face, Host, Part, and Line availability before
    /// each render, Show, and typed action. A changed Face needs a fresh seal.
    pub fn validate_current(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        self.verify_seal()?;
        let body = &session.evidence().body;
        if body.body_id != self.body_id || body.workload_revision != self.workload_revision {
            return Err(RemoteOwnerMaskRouteError::StaleBody);
        }
        face.validate()
            .map_err(|_| RemoteOwnerMaskRouteError::InvalidFace)?;
        if face.identity != self.face_id
            || face.revision != self.face_revision
            || face.basis != self.face_basis
        {
            return Err(RemoteOwnerMaskRouteError::StaleFace);
        }
        validate_workload_basis(session, face).map_err(|_| RemoteOwnerMaskRouteError::StaleBody)?;
        if RemoteMaskHostBasis::bind(owner_offer)? != self.owner_host
            || RemoteMaskHostBasis::bind(mask_host_offer)? != self.mask_host
        {
            return Err(RemoteOwnerMaskRouteError::StaleHost);
        }
        validate_hosts(session, owner_offer, mask_host_offer)?;
        validate_single_host_mask(mask_host_offer, &self.planned_mask)
            .map_err(RemoteOwnerMaskRouteError::InvalidMaskPlan)?;
        if face_line.admitted_line() != self.face_line
            || return_line.admitted_line() != self.return_line
        {
            return Err(RemoteOwnerMaskRouteError::StaleLine);
        }
        validate_line(face_line, &self.owner_host, &self.mask_host)?;
        validate_line(return_line, &self.mask_host, &self.owner_host)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_available_show(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        face_line: &LineOffer,
        return_line: &LineOffer,
        show: &MaskShow,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        self.validate_current(
            session,
            face,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
        )?;
        if show.planned_mask != self.planned_mask
            || show.show.host_id != self.mask_host.host_id
            || show.show.boot_id != self.mask_host.boot_id
            || show.show.offer_generation != self.mask_host.offer_generation
            || show.validate(face).is_err()
        {
            return Err(RemoteOwnerMaskRouteError::InvalidShow);
        }
        if show.show.lifecycle != ManifestationLifecycle::Available || show.show.failure.is_some() {
            return Err(RemoteOwnerMaskRouteError::ShowUnavailable);
        }
        let bytes = serde_json::to_vec(show).map_err(|_| RemoteOwnerMaskRouteError::InvalidShow)?;
        self.validate_return_payload(bytes.len())
    }

    /// Apply this bound to every Show and interaction frame before emission.
    pub fn validate_return_payload(&self, bytes: usize) -> Result<(), RemoteOwnerMaskRouteError> {
        if bytes == 0 || bytes > self.return_line.binding.limits.maximum_payload_bytes as usize {
            return Err(RemoteOwnerMaskRouteError::ReturnExceedsLine);
        }
        Ok(())
    }

    fn bind_identity(&self) -> Result<PlanId, RemoteOwnerMaskRouteError> {
        let bytes = serde_json::to_vec(&(
            "conduit.presentation/remote-owner-mask-route@1",
            &self.body_id,
            self.workload_revision,
            &self.face_id,
            self.face_revision,
            &self.face_basis,
            &self.owner_host,
            &self.mask_host,
            &self.planned_mask,
            &self.face_line,
            &self.return_line,
        ))
        .map_err(|_| RemoteOwnerMaskRouteError::InvalidSeal)?;
        if bytes.len() > MAX_REMOTE_ROUTE_SEAL_BYTES {
            return Err(RemoteOwnerMaskRouteError::SealCapacityExceeded);
        }
        let digest = Sha256::digest(&bytes);
        Ok(PlanId::from(format!(
            "plan/remote-owner-mask/{}",
            hex(&digest)
        )))
    }
}

fn validate_hosts(
    session: &BodyLifecycleSession,
    owner: &HostAdvertisement,
    remote: &HostAdvertisement,
) -> Result<(), RemoteOwnerMaskRouteError> {
    validate_host_pair(owner, remote)?;
    let present = session.evidence().membership.parts.iter().any(|part| {
        part.current.as_ref().is_some_and(|current| {
            current.host_id == remote.host_id
                && current.boot_id == remote.boot_id
                && current.offer_generation == remote.offer_generation
        })
    });
    if !present {
        return Err(RemoteOwnerMaskRouteError::WrongOrMissingPart);
    }
    Ok(())
}

fn validate_workload_basis(
    session: &BodyLifecycleSession,
    face: &Presentation,
) -> Result<(), RemoteOwnerMaskRouteError> {
    match (
        &session.evidence().body.state,
        session.realization(),
        face.basis.wake_id.as_ref(),
    ) {
        (BodyState::Lulled, None, None) => Ok(()),
        (BodyState::Lulled, Some(_), _) => {
            Err(RemoteOwnerMaskRouteError::WorkloadRealizationPresent)
        }
        (BodyState::Awake { wake_id }, Some(realization), Some(face_wake))
            if wake_id == face_wake && realization.wake.wake_id == *wake_id =>
        {
            Ok(())
        }
        _ => Err(RemoteOwnerMaskRouteError::WrongFaceBasis),
    }
}

fn validate_host_pair(
    owner: &HostAdvertisement,
    remote: &HostAdvertisement,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if owner.protocol_version != PROTOCOL_VERSION
        || remote.protocol_version != PROTOCOL_VERSION
        || owner.host_id == remote.host_id
        || owner.host_id.as_str().is_empty()
        || owner.boot_id.as_str().is_empty()
        || remote.host_id.as_str().is_empty()
        || remote.boot_id.as_str().is_empty()
        || owner.offer_generation.0 == 0
        || remote.offer_generation.0 == 0
    {
        return Err(RemoteOwnerMaskRouteError::InvalidHost);
    }
    Ok(())
}

fn validate_mask_basis(
    host: &RemoteMaskHostBasis,
    planned: &PlannedMaskPlot,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if !conduit_core::verify_plan(&planned.plan)
        || PlannedMaskPlot::admit(&planned.mask, &planned.plan).is_err()
        || planned.plan.fragments.is_empty()
        || planned.plan.fragments.iter().any(|fragment| {
            fragment.host_id != host.host_id
                || fragment.boot_id != host.boot_id
                || fragment.offer_generation != host.offer_generation
        })
    {
        return Err(RemoteOwnerMaskRouteError::InvalidSeal);
    }
    Ok(())
}

fn validate_line(
    line: &LineOffer,
    source: &RemoteMaskHostBasis,
    sink: &RemoteMaskHostBasis,
) -> Result<(), RemoteOwnerMaskRouteError> {
    if !line.validate_sign_identity() {
        return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
    }
    if line.availability.availability != LineAvailability::Ready {
        return Err(RemoteOwnerMaskRouteError::LineUnavailable);
    }
    validate_admitted_line(&line.admitted_line(), source, sink)
}

fn validate_admitted_line(
    line: &AdmittedLine,
    source: &RemoteMaskHostBasis,
    sink: &RemoteMaskHostBasis,
) -> Result<(), RemoteOwnerMaskRouteError> {
    let binding = &line.binding;
    if line.line_id.as_str().is_empty()
        || binding.binding_id.as_str().is_empty()
        || binding.base.as_str().is_empty()
        || binding.base.as_str() == conduit_core::LOCAL_BASE_IMPLEMENTATION_ID
        || binding.base_instance_id.as_str().is_empty()
        || matches!(binding.authority, LinkAuthorityReference::ProcessOwned)
        || matches!(binding.credential, LinkCredentialReference::None)
        || binding.source.endpoint_id.as_str().is_empty()
        || binding.sink.endpoint_id.as_str().is_empty()
        || binding.source.host_id != source.host_id
        || binding.source.boot_id != source.boot_id
        || binding.sink.host_id != sink.host_id
        || binding.sink.boot_id != sink.boot_id
        || binding.limits.maximum_in_flight_items == 0
        || binding.limits.maximum_payload_bytes == 0
        || binding.limits.maximum_frame_bytes == 0
        || binding.limits.maximum_buffered_bytes < binding.limits.maximum_payload_bytes
        || line.contract.traffic_shape != LineTrafficShape::Message
        || line.contract.scope == LineScope::Process
        || line.contract.ordering != LineOrdering::Ordered
        || line.contract.reliability != LineReliability::Reliable
    {
        return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
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
