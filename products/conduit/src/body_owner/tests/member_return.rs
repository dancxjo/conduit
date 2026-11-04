use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use conduit_body::{PartReturnChallenge, PartReturnProof, SpawnInvitationSecret};
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
