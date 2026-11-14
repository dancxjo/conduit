//! Authenticated local control for a finite signed member return.
use super::*;
use conduit_body::PartReturnChallenge;
use conduit_core::HostAdvertisement;
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_member_return_begin(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        presentation: Option<conduit_body::MemberReturnImagePresentation>,
        images: &[Vec<u8>],
    ) -> Result<PartReturnChallenge, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        if let Some(claim) = &presentation {
            let resolved = conduit_std_host::pico_spawn::resolve_member_offer_images(
                &claim.projection,
                images,
            )
            .map_err(|e| format!("member return images: {e:?}"))?;
            if &resolved != advertisement {
                return Err("member return image resolution differs from advertisement".into());
            }
        } else if !images.is_empty() {
            return Err("member return images have no signed presentation".into());
        }
        match presentation {
            Some(claim) => owner.begin_member_image_return(credential, advertisement, Some(claim)),
            None => owner.begin_member_return(credential, advertisement),
        }
    }
    pub(in crate::durable_host_control) fn owned_body_recovered_member_return_begin(
        &mut self,
        wire_credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        presentation: conduit_body::MemberReturnImagePresentation,
        images: &[Vec<u8>],
    ) -> Result<PartReturnChallenge, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        let resolved = conduit_std_host::pico_spawn::resolve_member_offer_images(
            &presentation.projection,
            images,
        )
        .map_err(|e| format!("member recovery images: {e:?}"))?;
        if &resolved != advertisement {
            return Err("member recovery image resolution differs from advertisement".into());
        }
        owner.begin_recovered_member_image_return(wire_credential, advertisement, presentation)
    }

    pub(in crate::durable_host_control) fn owned_body_member_return_complete(
        &mut self,
        proof: crate::durable_host::owner::MemberReturnProofDocument,
    ) -> Result<MembershipCredential, String> {
        let HostSource::Body { owner, root, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        owner.complete_member_return(root, &proof.decode()?)
    }
}
#[cfg(unix)]
pub(crate) fn begin_member_return(
    state_dir: &Path,
    credential: MembershipCredential,
    advertisement: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    match call(
        state_dir,
        Request::BodyMemberReturnBegin {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            credential,
            advertisement: Box::new(advertisement),
            presentation: None,
            images: Vec::new(),
        },
    )? {
        Response::BodyMemberReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        } => Ok(*challenge),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong member return challenge response".into()),
    }
}
#[cfg(unix)]
pub(crate) fn begin_image_member_return(
    state_dir: &Path,
    presentation: conduit_body::MemberReturnImagePresentation,
    images: Vec<Vec<u8>>,
    advertisement: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    match call(
        state_dir,
        Request::BodyMemberReturnBegin {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            credential: presentation.credential.clone(),
            advertisement: Box::new(advertisement),
            presentation: Some(Box::new(presentation)),
            images,
        },
    )? {
        Response::BodyMemberReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        } => Ok(*challenge),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong member return challenge response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn begin_image_member_return(
    _: &Path,
    _: conduit_body::MemberReturnImagePresentation,
    _: Vec<Vec<u8>>,
    _: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    Err("member return requires local owner control".into())
}
#[cfg(unix)]
pub(crate) fn complete_member_return(
    state_dir: &Path,
    proof: crate::durable_host::owner::MemberReturnProofDocument,
) -> Result<MembershipCredential, String> {
    match call(
        state_dir,
        Request::BodyMemberReturnComplete {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            proof: Box::new(proof),
        },
    )? {
        Response::BodyMemberReturned {
            protocol: PROTOCOL,
            credential,
        } => Ok(credential),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong member return completion response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn begin_member_return(
    _: &Path,
    _: MembershipCredential,
    _: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    Err("member return requires local owner control".into())
}
#[cfg(not(unix))]
pub(crate) fn complete_member_return(
    _: &Path,
    _: crate::durable_host::owner::MemberReturnProofDocument,
) -> Result<MembershipCredential, String> {
    Err("member return requires local owner control".into())
}

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn return_member(
        &mut self,
        encounter: crate::durable_host::MemberEncounter,
    ) -> Result<MembershipCredential, String> {
        let HostSource::Bare(host) = &self.host else {
            return Err("member return requires a quiescent non-owner Host".into());
        };
        if self.remote_fragment.is_some()
            || self.pending_worker_text.is_some()
            || self.pool_member.is_some()
            || self.speech_worker.is_some()
            || self.owner_spoken_worker.is_some()
            || self.spoken_template_session.is_some()
            || self.selected_speech_equipment.is_some()
        {
            return Err(
                "member return refused while assigned work or physical custody is active".into(),
            );
        }
        #[cfg(unix)]
        if self.terminal_route.is_some() {
            return Err("member return refused while a terminal carrier is attached".into());
        }
        let state = self
            .birth_root
            .as_deref()
            .ok_or("member return requires installed service custody")?;
        crate::durable_host::return_on_actual_host(state, host.advertisement(), &encounter)
    }
}
#[cfg(unix)]
pub(crate) fn return_member(
    state: &Path,
    encounter: crate::durable_host::MemberEncounter,
) -> Result<MembershipCredential, String> {
    match call(
        state,
        Request::BodyReturn {
            protocol: PROTOCOL,
            token: token(state)?,
            encounter: Box::new(encounter),
        },
    )? {
        Response::BodyMemberReturned {
            protocol: PROTOCOL,
            credential,
        } => Ok(credential),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong member return completion response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn return_member(
    _: &Path,
    _: crate::durable_host::MemberEncounter,
) -> Result<MembershipCredential, String> {
    Err("member return requires authenticated local owner control".into())
}

/// Explicit local recovery selects the transition credential from durable owner
/// custody. The unchanged wire proof must still authenticate the active Part key.
#[cfg(unix)]
pub(crate) fn begin_recovered_image_member_return(
    state_dir: &Path,
    presentation: conduit_body::MemberReturnImagePresentation,
    images: Vec<Vec<u8>>,
    advertisement: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    match call(
        state_dir,
        Request::BodyMemberRecoverReturnBegin {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            credential: presentation.credential.clone(),
            advertisement: Box::new(advertisement),
            presentation: Box::new(presentation),
            images,
        },
    )? {
        Response::BodyMemberReturnChallenge {
            protocol: PROTOCOL,
            challenge,
        } => Ok(*challenge),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong recovered member return challenge response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn begin_recovered_image_member_return(
    _: &Path,
    _: conduit_body::MemberReturnImagePresentation,
    _: Vec<Vec<u8>>,
    _: HostAdvertisement,
) -> Result<PartReturnChallenge, String> {
    Err("member recovery requires local owner control".into())
}
