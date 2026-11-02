//! Protected local control for current-member authentication. Descriptive replies
//! are not Line evidence or effect authority. The transport owner supplies the
//! locally authorized receiver/session context; remote packets cannot call this
//! entrance without the existing local control authentication.
use super::*;
use conduit_body::{FederationChallenge, FederationInspection};
use conduit_core::{HostAdvertisement, LineId, LineSecurity, LinkBindingId};
impl DurableHostRuntime {
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
