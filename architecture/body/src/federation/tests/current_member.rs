//! Core custody tests using real spawn admission and synthetic federation frames.
//! These are neither network transport nor fresh-challenge issuance proof.
use super::*;
use crate::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, BodyMembership, CurrentMemberContinuity,
    CurrentMemberReceivingSession, SpawnAdmissionProof, SpawnInvitationSecret,
};
use alloc::vec;
use conduit_core::{HostAdvertisement, HostProfileId, PROTOCOL_VERSION};

struct Admitted {
    manager: AdmissionManager,
    membership: BodyMembership,
    advertisement: HostAdvertisement,
    part: PartId,
}
fn admitted() -> Admitted {
    let body = crate::BodyId::bound("fixture/body".into());
    let mut manager = AdmissionManager::new(body.clone()).unwrap();
    let mut membership = BodyMembership::new(body).unwrap();
    let a = peer("a", &SigningKey::from_bytes(&[1; 32]), 1);
    let advertisement = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: a.host_id.clone(),
        boot_id: a.boot_id.clone(),
        offer_generation: a.offer_generation,
        profile: HostProfileId::from("fixture/member"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([1; 32]).unwrap(),
            [8; 32],
            1,
            1000,
        )
        .unwrap();
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: a.host_id.clone(),
        boot_id: a.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &a.host_id,
            &a.boot_id,
            a.offer_generation,
        )),
    };
    let credential = manager
        .complete_spawn(
            &mut membership,
            &advertisement,
            &proof,
            2,
            AdmissionSigns {
                part_admitted: "fixture/part-sign".into(),
                host_attached: "fixture/host-sign".into(),
                candidate_admitted: "fixture/candidate-sign".into(),
            },
        )
        .unwrap();
    Admitted {
        manager,
        membership,
        advertisement,
        part: credential.part_id,
    }
}
fn witness(f: &Admitted) -> CurrentMemberContinuity {
    f.manager
        .current_member_continuity(&f.membership, &f.part, &f.advertisement)
        .unwrap()
}
fn bound_session(
    f: &Admitted,
    challenge: FederationChallenge,
    a: &SigningKey,
    b: &SigningKey,
) -> Result<CurrentMemberReceivingSession, FederationRefusal> {
    let transcript = challenge_transcript(&challenge);
    FederatedReceivingSession::establish_for_current_member(
        witness(f),
        &peer("b", b, 1),
        challenge,
        a.sign(&transcript).to_bytes(),
        b.sign(&transcript).to_bytes(),
        10,
    )
}
#[test]
fn stored_key_witness_requires_exact_body_part_and_current_observation() {
    let mut f = admitted();
    let exact = witness(&f);
    assert_eq!(exact.body_id(), &f.membership.body_id);
    assert_eq!(exact.part_id(), &f.part);
    assert_eq!(
        exact.peer().verifying_key,
        SigningKey::from_bytes(&[1; 32]).verifying_key().to_bytes()
    );
    let wrong = crate::BodyId::bound("fixture/other-body".into());
    f.membership.body_id = wrong;
    assert_eq!(
        f.manager
            .current_member_continuity(&f.membership, &f.part, &f.advertisement)
            .err(),
        Some(AdmissionRefusal::WrongBody)
    );
    let f = admitted();
    let empty_manager = AdmissionManager::new(f.membership.body_id.clone()).unwrap();
    assert_eq!(
        empty_manager
            .current_member_continuity(&f.membership, &f.part, &f.advertisement)
            .err(),
        Some(AdmissionRefusal::InvalidProof)
    );
    assert!(f
        .manager
        .current_member_continuity(
            &f.membership,
            &PartId::bound("fixture/unknown".into()),
            &f.advertisement
        )
        .is_err());
    for change in 0..5 {
        let mut f = admitted();
        match change {
            0 => f.membership.parts[0].current = None,
            1 => f.membership.parts[0].state = MembershipState::Revoked,
            2 => f.advertisement.host_id = "host/other".into(),
            3 => f.advertisement.boot_id = "boot/other".into(),
            _ => f.advertisement.offer_generation = OfferGeneration(2),
        }
        assert!(f
            .manager
            .current_member_continuity(&f.membership, &f.part, &f.advertisement)
            .is_err());
    }
}
#[test]
fn self_signed_key_substitution_and_receiver_redirection_refuse() {
    let f = admitted();
    let a = SigningKey::from_bytes(&[1; 32]);
    let b = SigningKey::from_bytes(&[2; 32]);
    let attacker = SigningKey::from_bytes(&[3; 32]);
    let mut substitution = challenge(peer("a", &a, 1), peer("b", &b, 1), "substitution");
    substitution.initiator.verifying_key = attacker.verifying_key().to_bytes();
    // Preserve the existing explicitly trusted-key constructor semantics. Its
    // caller's key authentication is essential; signatures alone accept this.
    let transcript = challenge_transcript(&substitution);
    assert!(FederatedReceivingSession::establish(
        substitution.clone(),
        attacker.sign(&transcript).to_bytes(),
        b.sign(&transcript).to_bytes(),
        10
    )
    .is_ok());
    assert!(matches!(
        bound_session(&f, substitution, &attacker, &b),
        Err(FederationRefusal::PeerNotCurrentMember)
    ));
    let mut redirected = challenge(peer("a", &a, 1), peer("b", &b, 1), "redirect");
    redirected.responder.verifying_key = attacker.verifying_key().to_bytes();
    assert!(matches!(
        bound_session(&f, redirected, &a, &b),
        Err(FederationRefusal::Redirected)
    ));
}
#[test]
fn scoped_session_rechecks_exact_part_and_body_before_effect_authority() {
    let f = admitted();
    let a = SigningKey::from_bytes(&[1; 32]);
    let b = SigningKey::from_bytes(&[2; 32]);
    let c = challenge(peer("a", &a, 1), peer("b", &b, 1), "bound");
    let mut session = bound_session(&f, c.clone(), &a, &b).unwrap();
    let (mut table, mut handle, claim) = capability(&c.responder, "base/b/effect", 8);
    let operation = frame(&c, 0, claim);
    let signature = a.sign(&operation_transcript(&operation)).to_bytes();
    for change in 0..5 {
        let mut membership = f.membership.clone();
        match change {
            0 => membership.body_id = crate::BodyId::bound("fixture/another-body".into()),
            1 => membership.parts[0].part_id = PartId::bound("fixture/replaced-part".into()),
            2 => membership.parts[0].state = MembershipState::Revoked,
            3 => membership.parts[0].current.as_mut().unwrap().boot_id = "boot/changed".into(),
            _ => {
                membership.parts[0]
                    .current
                    .as_mut()
                    .unwrap()
                    .offer_generation = OfferGeneration(2)
            }
        }
        assert!(!session.inspection(&membership).membership_current);
        assert_eq!(
            session.authorize(&operation, signature, 20, &membership, &mut table, &handle),
            Err(FederationRefusal::PeerNotCurrentMember)
        );
    }
    assert_eq!(session.inspection(&f.membership).next_sequence, 0);
    let lease = session
        .authorize(
            &operation,
            signature,
            20,
            &f.membership,
            &mut table,
            &handle,
        )
        .unwrap();
    table.complete(&mut handle, lease, 1).unwrap();
    assert_eq!(
        session.authorize(
            &operation,
            signature,
            20,
            &f.membership,
            &mut table,
            &handle
        ),
        Err(FederationRefusal::Replay)
    );
    session.revoke_credential();
    assert!(!session.inspection(&f.membership).credential_current);
}

#[path = "current_member/challenge_slot.rs"]
mod challenge_slot;
