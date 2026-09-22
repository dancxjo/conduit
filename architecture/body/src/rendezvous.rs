//! Finite transport-neutral reachability carried beside a spawn invitation.
//!
//! A candidate grants no membership authority. It only names an authenticated
//! Line attempt the provisioned Host may make before presenting its separate
//! invitation proof.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[cfg(feature = "authenticated-admission")]
use crate::{SpawnInvitation, SpawnInvitationClaim};

pub const RENDEZVOUS_DESCRIPTOR_PROTOCOL: u16 = 1;
pub const MAX_RENDEZVOUS_CANDIDATES: usize = 4;
pub const MAX_RENDEZVOUS_TEXT_BYTES: usize = 256;
pub const MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE: u8 = 3;
pub const MAX_RENDEZVOUS_ATTEMPT_MILLIS: u32 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RendezvousLineFamily {
    AuthenticatedTlsStream,
    AuthenticatedConduitLine,
    LocalLoopbackWebSocket,
    AttendedSerial,
    WebRtcDataChannel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousAuthentication {
    /// Stable identity of the expected body-side rendezvous service.
    pub server_identity: String,
    /// SHA-256 identity of reviewed transport authentication material. The
    /// material itself is carrier-specific and never a durable body key.
    pub transport_binding_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousCandidate {
    pub candidate_id: String,
    pub line_family: RendezvousLineFamily,
    /// A bounded endpoint or discovery reference interpreted only by the
    /// selected Line family.
    pub reachability: String,
    pub authentication: RendezvousAuthentication,
    pub expires_at_millis: u64,
    pub maximum_attempts: u8,
    pub attempt_timeout_millis: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnRendezvousDescriptor {
    pub protocol: u16,
    pub body_id: String,
    pub invitation_id: String,
    pub candidates: Vec<RendezvousCandidate>,
}

/// Transport-neutral finite candidates for attaching one already-running host.
///
/// Unlike [`SpawnRendezvousDescriptor`], this descriptor precedes a body
/// invitation and therefore carries no Body or invitation identity. Its
/// one-use secret authenticates only this rendezvous session; admission above
/// the resulting Line remains separate.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunningHostRendezvousDescriptor {
    pub schema: String,
    pub candidates: Vec<RendezvousCandidate>,
    session_secret: [u8; 32],
}

/// Exact self-joining authority and reachability embedded in one fresh spore.
///
/// The invitation secret is intentionally serialized for target provisioning,
/// but never exposed through `Debug`. It grants admission proof authority only;
/// the rendezvous descriptor independently bounds where that proof may be
/// presented.
#[cfg(feature = "authenticated-admission")]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnRendezvousProvision {
    pub claim: SpawnInvitationClaim,
    pub rendezvous: SpawnRendezvousDescriptor,
    invitation_secret: [u8; 32],
}

#[cfg(feature = "authenticated-admission")]
impl core::fmt::Debug for SpawnRendezvousProvision {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SpawnRendezvousProvision")
            .field("claim", &self.claim)
            .field("rendezvous", &self.rendezvous)
            .field("invitation_secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousDescriptorRefusal {
    WrongProtocol,
    InvalidIdentity,
    CandidateBound,
    DuplicateCandidate,
    InvalidReachability,
    MissingAuthentication,
    Expired,
    AttemptPolicy,
    InsecureRemoteWebSocket,
    WrongBody,
    WrongInvitation,
    WeakInvitationSecret,
}

#[cfg(feature = "authenticated-admission")]
impl SpawnRendezvousProvision {
    pub fn from_invitation(
        invitation: SpawnInvitation,
        rendezvous: SpawnRendezvousDescriptor,
        now_millis: u64,
    ) -> Result<Self, RendezvousDescriptorRefusal> {
        let claim = invitation.claim();
        let invitation_secret = invitation.secret.copy_for_target_provisioning();
        let provision = Self {
            claim,
            rendezvous,
            invitation_secret,
        };
        provision.validate(now_millis)?;
        Ok(provision)
    }

    pub fn validate(&self, now_millis: u64) -> Result<(), RendezvousDescriptorRefusal> {
        self.rendezvous.validate(now_millis)?;
        self.claim
            .inspect(now_millis)
            .map_err(|_| RendezvousDescriptorRefusal::Expired)?;
        if self.rendezvous.body_id != self.claim.body_id.as_str() {
            return Err(RendezvousDescriptorRefusal::WrongBody);
        }
        if self.rendezvous.invitation_id != self.claim.invitation_id.as_str() {
            return Err(RendezvousDescriptorRefusal::WrongInvitation);
        }
        if self.invitation_secret == [0; 32] {
            return Err(RendezvousDescriptorRefusal::WeakInvitationSecret);
        }
        Ok(())
    }

    /// Copy the admission capability into an exact target-provisioning buffer.
    /// The caller must zero the returned buffer immediately after transfer.
    pub fn copy_invitation_secret_for_target_provisioning(&self) -> [u8; 32] {
        self.invitation_secret
    }
}

impl SpawnRendezvousDescriptor {
    pub fn validate(&self, now_millis: u64) -> Result<(), RendezvousDescriptorRefusal> {
        if self.protocol != RENDEZVOUS_DESCRIPTOR_PROTOCOL {
            return Err(RendezvousDescriptorRefusal::WrongProtocol);
        }
        if !bounded_text(&self.body_id) || !bounded_text(&self.invitation_id) {
            return Err(RendezvousDescriptorRefusal::InvalidIdentity);
        }
        validate_candidates(&self.candidates, now_millis)
    }
}

pub(crate) fn validate_candidates(
    candidates: &[RendezvousCandidate],
    now_millis: u64,
) -> Result<(), RendezvousDescriptorRefusal> {
    crate::rendezvous_validation::validate_owned_candidates(candidates, now_millis)
}

impl RunningHostRendezvousDescriptor {
    pub fn new(
        candidates: Vec<RendezvousCandidate>,
        session_secret: [u8; 32],
        now_millis: u64,
    ) -> Result<Self, RendezvousDescriptorRefusal> {
        let descriptor = Self {
            schema: "conduit.host/rendezvous-descriptor@1".into(),
            candidates,
            session_secret,
        };
        descriptor.validate(now_millis)?;
        Ok(descriptor)
    }

    pub fn validate(&self, now_millis: u64) -> Result<(), RendezvousDescriptorRefusal> {
        if self.schema != "conduit.host/rendezvous-descriptor@1" {
            return Err(RendezvousDescriptorRefusal::WrongProtocol);
        }
        validate_candidates(&self.candidates, now_millis)?;
        if self.session_secret == [0; 32] {
            return Err(RendezvousDescriptorRefusal::WeakInvitationSecret);
        }
        Ok(())
    }

    /// Copy the one-use transport capability into an exact attempt buffer.
    /// The caller must zero the returned buffer when the attempt terminates.
    pub fn copy_session_secret_for_attempt(&self) -> [u8; 32] {
        self.session_secret
    }
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_RENDEZVOUS_TEXT_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "authenticated-admission")]
    use crate::{AdmissionManager, BodyId, SpawnInvitationClaim, SpawnInvitationSecret};
    use crate::{RendezvousAttemptDecision, RendezvousAttemptSchedule};
    #[cfg(feature = "authenticated-admission")]
    use alloc::format;
    use alloc::vec;

    fn candidate(id: &str, family: RendezvousLineFamily, endpoint: &str) -> RendezvousCandidate {
        RendezvousCandidate {
            candidate_id: id.into(),
            line_family: family,
            reachability: endpoint.into(),
            authentication: RendezvousAuthentication {
                server_identity: "body-rendezvous/key-7".into(),
                transport_binding_sha256: [7; 32],
            },
            expires_at_millis: 10_000,
            maximum_attempts: 2,
            attempt_timeout_millis: 2_000,
        }
    }

    #[test]
    fn multiple_finite_candidates_are_invitation_and_body_bound() {
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![
                candidate(
                    "candidate/tls",
                    RendezvousLineFamily::AuthenticatedTlsStream,
                    "tls://body.example:443/conduit",
                ),
                candidate(
                    "candidate/relay",
                    RendezvousLineFamily::AuthenticatedConduitLine,
                    "relay:reviewed/one",
                ),
            ],
        };
        assert_eq!(descriptor.validate(1_000), Ok(()));
        assert_eq!(descriptor.candidates.len(), 2);
    }

    #[test]
    fn running_host_descriptor_carries_ordered_candidates_without_body_authority() {
        let descriptor = RunningHostRendezvousDescriptor::new(
            vec![
                candidate(
                    "candidate/lan",
                    RendezvousLineFamily::AuthenticatedTlsStream,
                    "wss://host.example:7443/conduit",
                ),
                candidate(
                    "candidate/relay",
                    RendezvousLineFamily::AuthenticatedConduitLine,
                    "relay:operator/one",
                ),
                candidate(
                    "candidate/webrtc",
                    RendezvousLineFamily::WebRtcDataChannel,
                    "webrtc-bootstrap:operator/negotiation-7",
                ),
            ],
            [19; 32],
            1_000,
        )
        .unwrap();
        assert_eq!(descriptor.validate(1_000), Ok(()));
        assert_eq!(descriptor.candidates[0].candidate_id, "candidate/lan");
        assert_eq!(descriptor.candidates[1].candidate_id, "candidate/relay");
        assert_eq!(descriptor.candidates[2].candidate_id, "candidate/webrtc");
        assert_eq!(descriptor.copy_session_secret_for_attempt(), [19; 32]);
        let encoded = serde_json::to_value(&descriptor).unwrap();
        assert_eq!(
            encoded["candidates"][0]["line_family"],
            "authenticated-tls-stream"
        );
        assert!(encoded.get("body_id").is_none());
        assert!(encoded.get("invitation_id").is_none());
    }

    #[test]
    fn running_host_descriptor_refuses_weak_secret_and_unbounded_candidates() {
        assert!(matches!(
            RunningHostRendezvousDescriptor::new(
                vec![candidate(
                    "candidate/lan",
                    RendezvousLineFamily::AuthenticatedTlsStream,
                    "wss://host.example:7443/conduit",
                )],
                [0; 32],
                1_000,
            ),
            Err(RendezvousDescriptorRefusal::WeakInvitationSecret)
        ));
    }

    #[test]
    fn loopback_websocket_cannot_be_relabelled_as_remote_reachability() {
        let mut descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![candidate(
                "candidate/local",
                RendezvousLineFamily::LocalLoopbackWebSocket,
                "ws://127.0.0.1:4173/conduit",
            )],
        };
        assert_eq!(descriptor.validate(1_000), Ok(()));
        descriptor.candidates[0].reachability = "ws://192.0.2.7:4173/conduit".into();
        assert_eq!(
            descriptor.validate(1_000),
            Err(RendezvousDescriptorRefusal::InsecureRemoteWebSocket)
        );
    }

    #[test]
    fn stale_unbound_or_unbounded_candidates_refuse_before_line_work() {
        let mut descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![candidate(
                "candidate/tls",
                RendezvousLineFamily::AuthenticatedTlsStream,
                "tls://body.example:443/conduit",
            )],
        };
        assert_eq!(
            descriptor.validate(10_000),
            Err(RendezvousDescriptorRefusal::Expired)
        );
        descriptor.candidates[0].expires_at_millis = 11_000;
        descriptor.candidates[0]
            .authentication
            .transport_binding_sha256 = [0; 32];
        assert_eq!(
            descriptor.validate(10_000),
            Err(RendezvousDescriptorRefusal::MissingAuthentication)
        );
    }

    #[test]
    fn failed_first_candidate_falls_through_in_exact_finite_order() {
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![
                candidate(
                    "candidate/primary",
                    RendezvousLineFamily::AuthenticatedTlsStream,
                    "tls://primary.example:443/conduit",
                ),
                candidate(
                    "candidate/relay",
                    RendezvousLineFamily::AuthenticatedConduitLine,
                    "relay:reviewed/one",
                ),
            ],
        };
        let mut schedule = RendezvousAttemptSchedule::new(&descriptor, 1_000).unwrap();

        for expected_attempt in 1..=2 {
            let RendezvousAttemptDecision::Try(attempt) = schedule.next(1_000) else {
                panic!("primary attempt missing");
            };
            assert_eq!(attempt.candidate.candidate_id, "candidate/primary");
            assert_eq!(attempt.attempt, expected_attempt);
            assert_eq!(attempt.timeout_millis, 2_000);
        }
        for expected_attempt in 1..=2 {
            let RendezvousAttemptDecision::Try(attempt) = schedule.next(1_000) else {
                panic!("relay fallback attempt missing");
            };
            assert_eq!(attempt.candidate.candidate_id, "candidate/relay");
            assert_eq!(attempt.attempt, expected_attempt);
        }
        assert_eq!(schedule.next(1_000), RendezvousAttemptDecision::Exhausted);
        assert_eq!(schedule.next(1_000), RendezvousAttemptDecision::Exhausted);
    }

    #[test]
    fn candidate_expiring_between_attempts_is_skipped_without_extending_authority() {
        let mut primary = candidate(
            "candidate/primary",
            RendezvousLineFamily::AuthenticatedTlsStream,
            "tls://primary.example:443/conduit",
        );
        primary.expires_at_millis = 1_500;
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![
                primary,
                candidate(
                    "candidate/relay",
                    RendezvousLineFamily::AuthenticatedConduitLine,
                    "relay:reviewed/one",
                ),
            ],
        };
        let mut schedule = RendezvousAttemptSchedule::new(&descriptor, 1_000).unwrap();
        let RendezvousAttemptDecision::Try(first) = schedule.next(1_000) else {
            panic!("initial attempt missing");
        };
        assert_eq!(first.candidate.candidate_id, "candidate/primary");

        let RendezvousAttemptDecision::Try(fallback) = schedule.next(1_500) else {
            panic!("unexpired fallback missing");
        };
        assert_eq!(fallback.candidate.candidate_id, "candidate/relay");
        assert_eq!(fallback.attempt, 1);
    }

    #[cfg(feature = "authenticated-admission")]
    #[test]
    fn one_provision_binds_invitation_authority_to_exact_body_reachability() {
        let body_id = serde_json::from_str::<BodyId>("\"body/one\"").unwrap();
        let mut manager = AdmissionManager::new(body_id).unwrap();
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
                [12; 32],
                1_000,
                9_000,
            )
            .unwrap();
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: invitation.body_id.as_str().into(),
            invitation_id: invitation.invitation_id.as_str().into(),
            candidates: vec![candidate(
                "candidate/tls",
                RendezvousLineFamily::AuthenticatedTlsStream,
                "tls://body.example:443/conduit",
            )],
        };
        let provision =
            SpawnRendezvousProvision::from_invitation(invitation, descriptor, 2_000).unwrap();
        assert_eq!(provision.validate(2_000), Ok(()));
        assert_eq!(
            provision.copy_invitation_secret_for_target_provisioning(),
            [11; 32]
        );
        assert!(!format!("{provision:?}").contains(&"11".repeat(32)));
        assert_eq!(
            serde_json::from_slice::<SpawnRendezvousProvision>(
                &serde_json::to_vec(&provision).unwrap()
            )
            .unwrap(),
            provision
        );
    }

    #[cfg(feature = "authenticated-admission")]
    #[test]
    fn provision_refuses_relabelled_body_invitation_and_weak_secret() {
        let claim: SpawnInvitationClaim = serde_json::from_value(serde_json::json!({
            "invitation_id": "invitation/one",
            "body_id": "body/one",
            "nonce": vec![7; 32],
            "expires_at_millis": 9_000
        }))
        .unwrap();
        let mut provision = SpawnRendezvousProvision {
            claim,
            rendezvous: SpawnRendezvousDescriptor {
                protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
                body_id: "body/two".into(),
                invitation_id: "invitation/one".into(),
                candidates: vec![candidate(
                    "candidate/tls",
                    RendezvousLineFamily::AuthenticatedTlsStream,
                    "tls://body.example:443/conduit",
                )],
            },
            invitation_secret: [7; 32],
        };
        assert_eq!(
            provision.validate(2_000),
            Err(RendezvousDescriptorRefusal::WrongBody)
        );
        provision.rendezvous.body_id = "body/one".into();
        provision.rendezvous.invitation_id = "invitation/two".into();
        assert_eq!(
            provision.validate(2_000),
            Err(RendezvousDescriptorRefusal::WrongInvitation)
        );
        provision.rendezvous.invitation_id = "invitation/one".into();
        provision.invitation_secret = [0; 32];
        assert_eq!(
            provision.validate(2_000),
            Err(RendezvousDescriptorRefusal::WeakInvitationSecret)
        );
    }
}
