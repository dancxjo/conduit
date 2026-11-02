//! Authenticated local control for a finite signed member return.
use super::*;
use conduit_body::PartReturnChallenge;
use conduit_core::HostAdvertisement;
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_member_return_begin(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
    ) -> Result<PartReturnChallenge, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        owner.begin_member_return(credential, advertisement)
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
