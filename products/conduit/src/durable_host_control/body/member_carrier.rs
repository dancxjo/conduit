//! Protected local control for current-member authentication. Descriptive replies
//! are not Line evidence or effect authority. The transport owner supplies the
//! locally authorized receiver/session context; remote packets cannot call this
//! entrance without the existing local control authentication.
use super::*;
use conduit_body::{FederationChallenge, FederationInspection};
use conduit_core::{HostAdvertisement, LineId, LineSecurity, LinkBindingId};
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_member_carrier_receiver(
        &mut self,
    ) -> Result<conduit_body::FederationPeer, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member carrier requires the live installed owner".into());
        };
        owner.prepare_current_member_carrier_receiver()
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_begin(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        line: LineId,
        security: LineSecurity,
        maximum_millis: u32,
        maximum_frames: u32,
    ) -> Result<(FederationChallenge, [u8; 64]), String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member carrier requires the live installed owner".into());
        };
        owner.begin_current_member_carrier(
            credential,
            advertisement,
            line,
            security,
            maximum_millis,
            maximum_frames,
        )
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_complete(
        &mut self,
        challenge: &FederationChallenge,
        signature: Vec<u8>,
    ) -> Result<FederationInspection, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member carrier requires the live installed owner".into());
        };
        owner.complete_current_member_carrier(
            challenge,
            signature
                .try_into()
                .map_err(|_| "member carrier signature must be exactly64bytes")?,
        )
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_mask_route(
        &mut self,
        session: &LinkBindingId,
        face_line: &conduit_core::LineOffer,
        return_line: &conduit_core::LineOffer,
    ) -> Result<conduit_face::RemoteOwnerMaskRouteSeal, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member presentation requires the live installed owner".into());
        };
        owner.select_member_carrier_mask_route(session, face_line, return_line)
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_show(
        &mut self,
        session: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member presentation requires the live installed owner".into());
        };
        owner.acknowledge_member_carrier_show(session, request, show)
    }
    pub(in crate::durable_host_control) fn owned_body_member_presence_renew(
        &mut self,
        session: &LinkBindingId,
        sequence: u64,
    ) -> Result<conduit_body::HostPresenceLease, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member presence requires the live installed owner".into());
        };
        owner.renew_current_member_presence(session, sequence)
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_inspect(
        &self,
        session: &LinkBindingId,
    ) -> Result<FederationInspection, String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("current-member carrier requires the live installed owner".into());
        };
        owner.inspect_current_member_carrier(session)
    }
    pub(in crate::durable_host_control) fn owned_body_member_carrier_retire(
        &mut self,
        session: &LinkBindingId,
    ) -> Result<(), String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("current-member carrier requires the live installed owner".into());
        };
        owner.retire_current_member_carrier(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable_host_control::{handle, Request, Response, PROTOCOL};
    use conduit_std_host::StdHost;
    fn requests(token: u8) -> Vec<Request> {
        let ad = StdHost::new().advertisement().clone();
        let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
            "credential_id":"fixture/credential", "body_id":"fixture/body", "part_id":"fixture/part",
            "host_id":ad.host_id, "boot_id":ad.boot_id, "issued_at_millis":1
        })).unwrap();
        let peer = conduit_body::FederationPeer {
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
            offer_generation: ad.offer_generation,
            verifying_key: [7; 32],
        };
        let challenge = FederationChallenge {
            session_id: "fixture/session".into(),
            line_id: "fixture/line".into(),
            line_security: LineSecurity::AuthenticatedEncrypted,
            initiator: peer.clone(),
            responder: peer,
            nonce: [9; 32],
            expires_at_millis: 100,
            maximum_frames: 2,
        };
        vec![
            Request::BodyMemberCarrierReceiver {
                protocol: PROTOCOL,
                token: vec![token; 32],
            },
            Request::BodyMemberCarrierBegin {
                protocol: PROTOCOL,
                token: vec![token; 32],
                credential,
                advertisement: Box::new(ad),
                line: "fixture/line".into(),
                security: LineSecurity::AuthenticatedEncrypted,
                maximum_millis: 100,
                maximum_frames: 2,
            },
            Request::BodyMemberCarrierComplete {
                protocol: PROTOCOL,
                token: vec![token; 32],
                challenge: Box::new(challenge),
                signature: vec![0; 64],
            },
            Request::BodyMemberPresenceRenew {
                protocol: PROTOCOL,
                token: vec![token; 32],
                session: "fixture/session".into(),
                sequence: 1,
            },
            Request::BodyMemberCarrierInspect {
                protocol: PROTOCOL,
                token: vec![token; 32],
                session: "fixture/session".into(),
            },
            Request::BodyMemberCarrierRetire {
                protocol: PROTOCOL,
                token: vec![token; 32],
                session: "fixture/session".into(),
            },
        ]
    }
    #[test]
    fn member_carrier_control_requires_authenticated_local_caller_and_real_body_owner() {
        let mut runtime = DurableHostRuntime::new(
            "fixture/target".into(),
            "fixture/image".into(),
            StdHost::new(),
        );
        let original = runtime.truth().advertisement;
        for token in [8, 9] {
            for request in requests(token) {
                // Actual typed wire round-trip; neither malformed credentials
                // nor a caller's transport description can create an owner.
                let encoded = serde_json::to_vec(&request).unwrap();
                let request = serde_json::from_slice(&encoded).unwrap();
                assert!(
                    matches!(handle(request, &[9;32], &mut runtime), Response::Refused { code, .. }
                    if if token == 8 { code == "unauthorized" } else { code.contains("live installed owner") })
                );
            }
        }
        assert_eq!(runtime.truth().advertisement, original);
    }
}

#[cfg(unix)]
fn exchange(state: &Path, request: impl FnOnce(Vec<u8>) -> Request) -> Result<Response, String> {
    call(state, request(token(state)?))
}
#[cfg(not(unix))]
fn exchange(_: &Path, _: impl FnOnce(Vec<u8>) -> Request) -> Result<Response, String> {
    Err("current-member carrier requires local Unix owner control".into())
}
fn wrong(response: Response) -> String {
    match response {
        Response::Refused { code, .. } => code,
        _ => "wrong current-member carrier control response".into(),
    }
}
pub(crate) fn carrier_receiver(state: &Path) -> Result<conduit_body::FederationPeer, String> {
    match exchange(state, |token| Request::BodyMemberCarrierReceiver {
        protocol: PROTOCOL,
        token,
    })? {
        Response::BodyMemberCarrierReceiver {
            protocol: PROTOCOL,
            receiver,
        } => Ok(*receiver),
        other => Err(wrong(other)),
    }
}
pub(crate) fn begin_carrier(
    state: &Path,
    credential: MembershipCredential,
    advertisement: HostAdvertisement,
    line: LineId,
    maximum_millis: u32,
    maximum_frames: u32,
) -> Result<(FederationChallenge, [u8; 64]), String> {
    match exchange(state, |token| Request::BodyMemberCarrierBegin {
        protocol: PROTOCOL,
        token,
        credential,
        advertisement: Box::new(advertisement),
        line,
        security: LineSecurity::AuthenticatedEncrypted,
        maximum_millis,
        maximum_frames,
    })? {
        Response::BodyMemberCarrierChallenge {
            protocol: PROTOCOL,
            challenge,
            signature,
        } => Ok((
            *challenge,
            signature
                .try_into()
                .map_err(|_| "invalid owner carrier signature size")?,
        )),
        other => Err(wrong(other)),
    }
}
pub(crate) fn complete_carrier(
    state: &Path,
    challenge: FederationChallenge,
    signature: [u8; 64],
) -> Result<FederationInspection, String> {
    match exchange(state, |token| Request::BodyMemberCarrierComplete {
        protocol: PROTOCOL,
        token,
        challenge: Box::new(challenge),
        signature: signature.to_vec(),
    })? {
        Response::BodyMemberCarrierInspection {
            protocol: PROTOCOL,
            inspection,
        } => Ok(*inspection),
        other => Err(wrong(other)),
    }
}
pub(crate) fn retire_carrier(state: &Path, session: LinkBindingId) -> Result<(), String> {
    match exchange(state, |token| Request::BodyMemberCarrierRetire {
        protocol: PROTOCOL,
        token,
        session,
    })? {
        Response::BodyMemberCarrierRetired { protocol: PROTOCOL } => Ok(()),
        other => Err(wrong(other)),
    }
}

pub(crate) fn renew_carrier_presence(
    state: &Path,
    session: LinkBindingId,
    sequence: u64,
) -> Result<conduit_body::HostPresenceLease, String> {
    match exchange(state, |token| Request::BodyMemberPresenceRenew {
        protocol: PROTOCOL,
        token,
        session,
        sequence,
    })? {
        Response::BodyMemberPresenceRenewed {
            protocol: PROTOCOL,
            lease,
        } => Ok(*lease),
        other => Err(wrong(other)),
    }
}

pub(crate) fn carrier_mask_route(
    state: &Path,
    session: LinkBindingId,
    face_line: conduit_core::LineOffer,
    return_line: conduit_core::LineOffer,
) -> Result<conduit_face::RemoteOwnerMaskRouteSeal, String> {
    match exchange(state, |token| Request::BodyMemberCarrierMaskRoute {
        protocol: PROTOCOL,
        token,
        session,
        face_line: Box::new(face_line),
        return_line: Box::new(return_line),
    })? {
        Response::BodyNativeMaskRoute {
            protocol: PROTOCOL,
            route,
        } => Ok(*route),
        other => Err(wrong(other)),
    }
}
pub(crate) fn carrier_show(
    state: &Path,
    session: LinkBindingId,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
) -> Result<(), String> {
    match exchange(state, |token| Request::BodyMemberCarrierShow {
        protocol: PROTOCOL,
        token,
        session,
        request,
        show: Box::new(show),
    })? {
        Response::BodyNativeMaskShowAccepted { protocol: PROTOCOL } => Ok(()),
        other => Err(wrong(other)),
    }
}

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_attest_admission_receipt(
        &mut self,
        receipt: &conduit_body::PortableAdmissionReceipt,
        expected_owner: &conduit_body::FederationPeer,
    ) -> Result<conduit_body::SignedPortableAdmissionReceipt, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("receipt attestation requires the live installed owner".into());
        };
        owner.attest_current_member_receipt(receipt, expected_owner)
    }
}
