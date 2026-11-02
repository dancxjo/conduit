//! Synthetic signed challenge proof; no CSPRNG, transport or physical Line proof.
use super::*;
use crate::CurrentMemberChallengeSlot;

fn setup() -> (Admitted, FederationChallenge, [u8; 64], [u8; 64]) {
    let f = admitted();
    let a = SigningKey::from_bytes(&[1; 32]);
    let b = SigningKey::from_bytes(&[2; 32]);
    let challenge = challenge(peer("a", &a, 1), peer("b", &b, 1), "one-use");
    let transcript = challenge_transcript(&challenge);
    (
        f,
        challenge,
        a.sign(&transcript).to_bytes(),
        b.sign(&transcript).to_bytes(),
    )
}
#[test]
fn one_issuance_refuses_overwrite_and_cannot_be_rearmed_or_replayed() {
    let (f, challenge, a, b) = setup();
    let mut slot = CurrentMemberChallengeSlot::default();
    slot.issue(witness(&f), &challenge.responder, challenge.clone(), 1)
        .unwrap();
    assert_eq!(
        slot.issue(witness(&f), &challenge.responder, challenge.clone(), 1)
            .err(),
        Some(FederationRefusal::ChallengePending)
    );
    assert!(slot
        .complete(&f.membership, &challenge.responder, &challenge, a, b, 2)
        .is_ok());
    assert_eq!(
        slot.complete(&f.membership, &challenge.responder, &challenge, a, b, 2)
            .err(),
        Some(FederationRefusal::ChallengeConsumed)
    );
    assert_eq!(
        slot.issue(witness(&f), &challenge.responder, challenge.clone(), 3)
            .err(),
        Some(FederationRefusal::ChallengeConsumed)
    );
}
#[test]
fn matching_bad_signature_or_expired_attempt_is_consumed() {
    for expired in [false, true] {
        let (f, challenge, a, b) = setup();
        let mut slot = CurrentMemberChallengeSlot::default();
        slot.issue(witness(&f), &challenge.responder, challenge.clone(), 1)
            .unwrap();
        let result = slot.complete(
            &f.membership,
            &challenge.responder,
            &challenge,
            if expired { a } else { [0; 64] },
            b,
            if expired { 1001 } else { 2 },
        );
        assert_eq!(
            result.err(),
            Some(if expired {
                FederationRefusal::Expired
            } else {
                FederationRefusal::MutualAuthenticationFailed
            })
        );
        assert_eq!(
            slot.complete(&f.membership, &challenge.responder, &challenge, a, b, 2)
                .err(),
            Some(FederationRefusal::ChallengeConsumed)
        );
    }
}
#[test]
fn owner_context_and_current_member_are_rechecked_before_authentication() {
    for change in 0..6 {
        let (mut f, challenge, a, b) = setup();
        let mut slot = CurrentMemberChallengeSlot::default();
        slot.issue(witness(&f), &challenge.responder, challenge.clone(), 1)
            .unwrap();
        let mut responder = challenge.responder.clone();
        match change {
            0 => f.membership.body_id = crate::BodyId::bound("other/body".into()),
            1 => f.membership.parts[0].part_id = PartId::bound("other/part".into()),
            2 => f.membership.parts[0].current.as_mut().unwrap().boot_id = "other/boot".into(),
            3 => {
                f.membership.parts[0]
                    .current
                    .as_mut()
                    .unwrap()
                    .offer_generation = OfferGeneration(2)
            }
            4 => responder.boot_id = "other/responder-boot".into(),
            _ => responder.offer_generation = OfferGeneration(2),
        }
        assert!(slot
            .complete(&f.membership, &responder, &challenge, a, b, 2)
            .is_err());
        assert_eq!(
            slot.complete(&f.membership, &challenge.responder, &challenge, a, b, 2)
                .err(),
            Some(FederationRefusal::ChallengeConsumed)
        );
    }
}
#[test]
fn mismatched_received_context_cannot_replace_or_consume_the_pending_challenge() {
    let (f, challenge, a, b) = setup();
    let mut slot = CurrentMemberChallengeSlot::default();
    slot.issue(witness(&f), &challenge.responder, challenge.clone(), 1)
        .unwrap();
    let mut other = challenge.clone();
    other.nonce[0] ^= 1;
    assert_eq!(
        slot.complete(&f.membership, &challenge.responder, &other, a, b, 2)
            .err(),
        Some(FederationRefusal::WrongSession)
    );
    assert!(slot
        .complete(&f.membership, &challenge.responder, &challenge, a, b, 2)
        .is_ok());
}
