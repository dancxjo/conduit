//! An owner-issued presentation route for a remote, single-Host Mask Plot.
//!
//! Directional Lines bind the Mask Fore to the owner while the
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_line: Option<AdmittedLine>,
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
    FaceExceedsFore,
    ReturnExceedsLine,
    ReturnExceedsFore,
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
        Self::seal_current_inner(
            session,
            face,
            owner_offer,
            mask_host_offer,
            planned_mask,
            face_line,
            return_line,
            None,
        )
    }

    /// Bind a distinct typed interaction return to the same owner route.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_current_with_interaction(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: &LineOffer,
    ) -> Result<Self, RemoteOwnerMaskRouteError> {
        Self::seal_current_inner(
            session,
            face,
            owner_offer,
            mask_host_offer,
            planned_mask,
            face_line,
            return_line,
            Some(interaction_line),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn seal_current_inner(
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: Option<&LineOffer>,
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
        validate_single_host_mask(mask_host_offer, planned_mask, true)
            .map_err(RemoteOwnerMaskRouteError::InvalidMaskPlan)?;
        validate_line(face_line, &owner_host, &mask_host)?;
        validate_line(return_line, &mask_host, &owner_host)?;
        if let Some(interaction_line) = interaction_line {
            validate_line(interaction_line, &mask_host, &owner_host)?;
            if interaction_line.line_id == face_line.line_id
                || interaction_line.line_id == return_line.line_id
                || interaction_line.binding.binding_id == face_line.binding.binding_id
                || interaction_line.binding.binding_id == return_line.binding.binding_id
            {
                return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
            }
        }
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
            interaction_line: interaction_line.map(LineOffer::admitted_line),
        };
        seal.verify_selected_fore_lines()?;
        if seal
            .selected_fore_byte_capacity(
                &seal.planned_mask.mask.face_input.front_port_id,
                conduit_core::PortDirection::Input,
            )
            .is_some_and(|limit| face_bytes.len() > limit)
        {
            return Err(RemoteOwnerMaskRouteError::FaceExceedsFore);
        }
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
        if let Some(interaction_line) = &self.interaction_line {
            validate_admitted_line(interaction_line, &self.mask_host, &self.owner_host)?;
            if interaction_line.line_id == self.face_line.line_id
                || interaction_line.line_id == self.return_line.line_id
                || interaction_line.binding.binding_id == self.face_line.binding.binding_id
                || interaction_line.binding.binding_id == self.return_line.binding.binding_id
            {
                return Err(RemoteOwnerMaskRouteError::InvalidSeal);
            }
        }
        self.verify_selected_fore_lines()?;
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
        validate_single_host_mask(host, &self.planned_mask, true)
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
        if self.interaction_line.is_some() {
            return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
        }
        self.validate_current_base(
            session,
            face,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
        )
    }

    fn validate_current_base(
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
        validate_single_host_mask(mask_host_offer, &self.planned_mask, true)
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
    pub fn validate_current_with_interaction(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: &LineOffer,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        if self.interaction_line.is_none() {
            return Err(RemoteOwnerMaskRouteError::MissingOrInvalidLine);
        }
        self.validate_current_base(
            session,
            face,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
        )?;
        if self.interaction_line.as_ref() != Some(&interaction_line.admitted_line()) {
            return Err(RemoteOwnerMaskRouteError::StaleLine);
        }
        validate_line(interaction_line, &self.mask_host, &self.owner_host)
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
        self.validate_show(face, show)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_available_show_with_interaction(
        &self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: &LineOffer,
        show: &MaskShow,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
        self.validate_current_with_interaction(
            session,
            face,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
            interaction_line,
        )?;
        self.validate_show(face, show)
    }

    fn validate_show(
        &self,
        face: &Presentation,
        show: &MaskShow,
    ) -> Result<(), RemoteOwnerMaskRouteError> {
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
}

mod validation;
use validation::{
    hex, validate_admitted_line, validate_hosts, validate_line, validate_mask_basis,
    validate_workload_basis,
};
