use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use conduit_body::{PartReturnChallenge, PartReturnProof, SpawnInvitationSecret};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn prove(challenge: &PartReturnChallenge, key: &SpawnInvitationSecret) -> PartReturnProof {
    PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    }
}
#[test]
fn member_return_commits_once_and_replays_only_the_exact_signed_outcome() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut offer) = admit_with_key(&mut owner, &root, "host/returning");
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    owner.member_leave(&root, &prior).unwrap();
    offer.boot_id = "boot/returning/2".into();
    offer.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    assert_eq!(
        owner.begin_member_return(&prior, &offer).unwrap(),
        challenge
    );
    let proof = prove(&challenge, &key);
    let before = owner.session.evidence().clone();
    let mut forged = proof.clone();
    forged.signature[0] ^= 1;
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &before);
    let returned = owner.complete_member_return(&root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert_eq!(returned.host_id, prior.host_id);
    assert_eq!(returned.boot_id, offer.boot_id);
    assert_ne!(returned.credential_id, prior.credential_id);
    let committed = owner.session.evidence().clone();
    assert_eq!(
        owner.complete_member_return(&root, &proof).unwrap(),
        returned
    );
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &committed);
    assert_eq!(state::load(&root).unwrap().unwrap(), committed);
    // A committed reply is not current admission after explicit withdrawal.
    owner.member_leave(&root, &returned).unwrap();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    // Process restart must not resurrect the ephemeral challenge.
    owner.pending_member_return = None;
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert!(owner.begin_member_return(&prior, &offer).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn member_return_persistence_failure_neither_attaches_nor_delivers_a_receipt() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, offer) = admit_with_key(&mut owner, &root, "host/return-fault");
    owner.member_leave(&root, &prior).unwrap();
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    let proof = prove(&challenge, &key);
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    // A regular file cannot be a retained-state root.
    let blocked = root.join("blocked-root");
    std::fs::write(&blocked, b"occupied").unwrap();
    assert!(owner.complete_member_return(&blocked, &proof).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    owner.complete_member_return(&root, &proof).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn member_return_bad_proofs_exhaust_the_same_finite_challenge() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, offer) = admit_with_key(&mut owner, &root, "host/return-attempts");
    owner.member_leave(&root, &prior).unwrap();
    let challenge = owner.begin_member_return(&prior, &offer).unwrap();
    let valid = prove(&challenge, &key);
    let mut forged = valid.clone();
    forged.signature[0] ^= 1;
    let before = owner.session.evidence().clone();
    for _ in 0..conduit_body::MAX_ADMISSION_ATTEMPTS {
        assert!(owner.complete_member_return(&root, &forged).is_err());
    }
    assert!(owner.complete_member_return(&root, &valid).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(
        owner.begin_member_return(&prior, &offer).unwrap(),
        challenge
    );
    std::fs::remove_dir_all(root).unwrap();
}

struct PendingReturnFixture {
    owner: Owner,
    root: PathBuf,
    prior: conduit_body::MembershipCredential,
    offer: conduit_core::HostAdvertisement,
    challenge: PartReturnChallenge,
    proof: PartReturnProof,
    now: Instant,
    at: u64,
}

fn pending_return_fixture(name: &str) -> PendingReturnFixture {
    let (mut owner, root) = owner_fixture();
    let (prior, key, offer) = admit_with_key(&mut owner, &root, name);
    owner.member_leave(&root, &prior).unwrap();
    let now = Instant::now();
    let at = crate::durable_host::current_time_millis().unwrap();
    let challenge = owner
        .begin_member_return_at(&prior, &offer, now, at)
        .unwrap();
    let proof = prove(&challenge, &key);
    PendingReturnFixture {
        owner,
        root,
        prior,
        offer,
        challenge,
        proof,
        now,
        at,
    }
}

#[test]
fn pending_return_replay_neither_extends_nor_ignores_either_clock_bound() {
    for wall_expires_first in [false, true] {
        let mut f = pending_return_fixture("host/return-clock-expiry");
        let before = f.owner.session.evidence().clone();
        let manager = f.owner.admissions.clone();
        let (expired_now, expired_at) = if wall_expires_first {
            (f.now, f.challenge.expires_at_millis)
        } else {
            (f.now + Duration::from_secs(10), f.at)
        };
        let fresh = f
            .owner
            .begin_member_return_at(&f.prior, &f.offer, expired_now, expired_at)
            .unwrap();
        assert_ne!(fresh.nonce, f.challenge.nonce);
        assert_eq!(fresh.issued_at_millis, expired_at);
        assert_eq!(fresh.expires_at_millis, expired_at + 10_000);
        let replay = f
            .owner
            .begin_member_return_at(
                &f.prior,
                &f.offer,
                expired_now + Duration::from_secs(1),
                expired_at + 1_000,
            )
            .unwrap();
        assert_eq!(replay, fresh);
        assert_eq!(f.owner.session.evidence(), &before);
        assert_eq!(f.owner.admissions, manager);
        // Even the newly issued challenge expires at its original deadline;
        // replay at one second did not move it to eleven seconds.
        let expired = f
            .owner
            .complete_member_return_at(
                &f.root,
                &f.proof,
                expired_now + Duration::from_secs(10),
                expired_at,
            )
            .unwrap_err();
        assert_eq!(expired, "member return challenge expired");
        assert_eq!(f.owner.session.evidence(), &before);
        std::fs::remove_dir_all(f.root).unwrap();
    }
}

#[test]
fn pending_return_clock_rollback_refuses_without_rotating_or_attaching() {
    let mut f = pending_return_fixture("host/return-clock-rollback");
    let before = f.owner.session.evidence().clone();
    let manager = f.owner.admissions.clone();
    assert_eq!(
        f.owner
            .begin_member_return_at(&f.prior, &f.offer, f.now, f.at - 1)
            .unwrap_err(),
        "member return challenge expired",
    );
    assert_eq!(
        f.owner
            .complete_member_return_at(&f.root, &f.proof, f.now, f.at - 1)
            .unwrap_err(),
        "member return challenge expired",
    );
    assert_eq!(f.owner.session.evidence(), &before);
    assert_eq!(f.owner.admissions, manager);
    assert_eq!(
        f.owner
            .begin_member_return_at(&f.prior, &f.offer, f.now, f.at)
            .unwrap(),
        f.challenge,
    );
    std::fs::remove_dir_all(f.root).unwrap();
}

#[test]
fn signed_return_and_committed_replay_refuse_at_each_exact_clock_boundary() {
    let mut f = pending_return_fixture("host/return-completion-clocks");
    for (now, at) in [
        (f.now + Duration::from_secs(10), f.at),
        (f.now, f.challenge.expires_at_millis),
    ] {
        assert_eq!(
            f.owner
                .complete_member_return_at(&f.root, &f.proof, now, at)
                .unwrap_err(),
            "member return challenge expired",
        );
    }
    let accepted = f
        .owner
        .complete_member_return_at(&f.root, &f.proof, f.now, f.at)
        .unwrap();
    let committed = f.owner.session.evidence().clone();
    for (now, at) in [
        (f.now + Duration::from_secs(10), f.at),
        (f.now, f.challenge.expires_at_millis),
        (f.now, f.at - 1),
    ] {
        assert!(f
            .owner
            .complete_member_return_at(&f.root, &f.proof, now, at)
            .is_err());
        assert_eq!(f.owner.session.evidence(), &committed);
    }
    assert_eq!(
        f.owner
            .complete_member_return_at(&f.root, &f.proof, f.now, f.at)
            .unwrap(),
        accepted,
    );
    std::fs::remove_dir_all(f.root).unwrap();
}

#[test]
fn lifecycle_policy_refusal_does_not_spend_a_signed_returns_proof_attempts() {
    let mut f = pending_return_fixture("host/return-policy");
    f.owner.plan(&source()).unwrap();
    let before = f.owner.session.evidence().clone();
    let realization = f.owner.session.realization().cloned();
    let manager = f.owner.admissions.clone();
    for _ in 0..=conduit_body::MAX_ADMISSION_ATTEMPTS {
        assert_eq!(
            f.owner
                .complete_member_return_at(&f.root, &f.proof, f.now, f.at)
                .unwrap_err(),
            "member return: NotLulled",
        );
        assert_eq!(f.owner.session.evidence(), &before);
        assert_eq!(f.owner.session.realization(), realization.as_ref());
        assert_eq!(f.owner.admissions, manager);
    }
    // Only this fixture's explicit lifecycle transition removes the policy
    // refusal. No admission, new identity or automatic Lull came from retries.
    f.owner.lull().unwrap();
    let credential = f
        .owner
        .complete_member_return_at(&f.root, &f.proof, f.now, f.at)
        .unwrap();
    assert_eq!(credential.part_id, f.prior.part_id);
    std::fs::remove_dir_all(f.root).unwrap();
}

#[test]
fn wrong_nonce_still_spends_the_original_finite_proof_budget() {
    let mut f = pending_return_fixture("host/return-wrong-nonce");
    let mut wrong = f.proof.clone();
    wrong.nonce[0] ^= 1;
    let before = f.owner.session.evidence().clone();
    for _ in 0..conduit_body::MAX_ADMISSION_ATTEMPTS {
        assert!(f
            .owner
            .complete_member_return_at(&f.root, &wrong, f.now, f.at)
            .is_err());
    }
    assert_eq!(
        f.owner
            .complete_member_return_at(&f.root, &f.proof, f.now, f.at)
            .unwrap_err(),
        "member return proof attempts exhausted",
    );
    assert_eq!(f.owner.session.evidence(), &before);
    std::fs::remove_dir_all(f.root).unwrap();
}
