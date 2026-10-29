use super::*;
use crate::network_custody_client::tests::{committed_cursor, fixture, receipt_response, Fixture};
use ed25519_dalek::Signer;
fn token(f: &Fixture) -> VerifiedDurableSnapshot {
    VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap()
}
fn signed(f: &Fixture, t: &VerifiedDurableSnapshot) -> Vec<u8> {
    let signature = ed25519_dalek::SigningKey::from_bytes(&[22; 32])
        .sign(&t.signing_transcript(&f.credential).unwrap())
        .to_bytes();
    t.encode_ack(&f.credential, signature).unwrap()
}
fn ready(f: &Fixture) -> Acknowledgement {
    let t = token(f);
    let ack = signed(f, &t);
    match Acknowledgement::open(t, f.credential.clone(), ack).unwrap() {
        Start::Ready(owner) => owner,
        _ => panic!("new session must be ready"),
    }
}
#[test]
fn one_post_per_advance_exact_lost_reply_replay_and_durable_clean() {
    let f = fixture();
    let mut owner = ready(&f);
    let cursor = committed_cursor(&f, &owner.token, &owner.ack, 2);
    let clean = receipt_response(&cursor, 7);
    let ack = owner.ack.clone();
    for step in 0..3 {
        let mut calls = 0;
        let result = owner
            .advance(Instant::now() + Duration::from_secs(3), |sent, deadline| {
                calls += 1;
                assert_eq!(sent, ack);
                assert_eq!(reopen(&owner_path(&f, "ack.bin"), REQUEST).unwrap(), sent);
                assert!(deadline <= Instant::now() + Duration::from_secs(3));
                assert!(owner_path(&f, SESSION).exists());
                match step {
                    0 => Err("lost response".into()),
                    1 => Ok((202, vec![])),
                    _ => Ok((200, clean.clone())),
                }
            })
            .unwrap();
        assert_eq!(calls, 1);
        match result {
            Progress::Complete(receipt) => {
                assert_eq!(step, 2);
                assert_eq!(receipt.bytes(), &cursor);
            }
            Progress::Pending => assert!(step < 2),
        }
    }
    assert!(owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
            "terminal"
        ))
        .is_err());
    drop(owner);
    match Acknowledgement::open(token(&f), f.credential.clone(), ack).unwrap() {
        Start::Complete(receipt) => assert_eq!(receipt.bytes(), &cursor),
        _ => panic!("completion must reopen as independently verified proof"),
    }
}
fn owner_path(f: &Fixture, name: &str) -> PathBuf {
    token(f).directory.join(name)
}
#[test]
fn restart_is_explicitly_interrupted_without_renewing_any_allowance() {
    let f = fixture();
    let owner = ready(&f);
    let ack = owner.ack.clone();
    let marker = reopen(&owner.token.directory.join(SESSION), 88).unwrap();
    drop(owner);
    assert!(matches!(
        Acknowledgement::open(token(&f), f.credential.clone(), ack).unwrap(),
        Start::Interrupted
    ));
    assert_eq!(reopen(&owner_path(&f, SESSION), 88).unwrap(), marker);
}
#[test]
fn pending_receipt_cannot_claim_clean_and_retry_bound_is_finite() {
    let f = fixture();
    let mut owner = ready(&f);
    let cursor = committed_cursor(&f, &owner.token, &owner.ack, 1);
    assert!(matches!(
        owner
            .advance(Instant::now() + Duration::from_secs(3), |_, _| Ok((
                200,
                receipt_response(&cursor, 7)
            )))
            .unwrap(),
        Progress::Pending
    ));
    assert!(!owner.token.directory.join(COMPLETED).exists());
    for _ in 1..ATTEMPTS {
        assert!(matches!(
            owner
                .advance(Instant::now() + Duration::from_secs(3), |_, _| Ok((
                    503,
                    vec![]
                )))
                .unwrap(),
            Progress::Pending
        ));
    }
    assert!(owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
            "451 exhausted"
        ))
        .is_err());
    let ack = owner.ack.clone();
    drop(owner);
    assert!(matches!(
        Acknowledgement::open(token(&f), f.credential.clone(), ack).unwrap(),
        Start::Interrupted
    ));
}
#[test]
fn actual_deadline_caps_caller_budget_and_expiry_refuses_without_post() {
    let f = fixture();
    let mut owner = ready(&f);
    let deadline = owner.deadline();
    owner
        .advance(Instant::now() + Duration::from_secs(1000), |_, end| {
            assert_eq!(end, deadline);
            Ok((202, vec![]))
        })
        .unwrap();
    owner
        .advance(Instant::now() - Duration::from_secs(1), |_, _| {
            panic!("caller budget expired")
        })
        .unwrap();
    owner.deadline = Instant::now() - Duration::from_secs(1);
    assert!(owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
            "original window expired"
        ))
        .is_err());
}
#[test]
fn changed_ack_or_lost_evidence_refuses_before_post() {
    for name in ["ack.bin", "snapshot.bin"] {
        let f = fixture();
        let mut owner = ready(&f);
        let path = owner.token.directory.join(name);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(owner
            .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
                "corrupt custody"
            ))
            .is_err());
    }
    let f = fixture();
    let t = token(&f);
    let mut ack = signed(&f, &t);
    ack[52] ^= 1;
    assert!(Acknowledgement::open(t, f.credential.clone(), ack).is_err());
    assert!(!owner_path(&f, SESSION).exists());
}
#[test]
fn invalid_clean_is_terminal_and_completed_cache_is_reverified() {
    let f = fixture();
    let mut owner = ready(&f);
    let cursor = committed_cursor(&f, &owner.token, &owner.ack, 2);
    let mut response = receipt_response(&cursor, 7);
    response[0] ^= 1;
    assert!(owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| Ok((
            200,
            response.clone()
        )))
        .is_err());
    assert!(owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
            "refusal terminal"
        ))
        .is_err());
    assert!(!owner.token.directory.join(COMPLETED).exists());
    drop(owner);
    let f = fixture();
    let mut owner = ready(&f);
    let ack = owner.ack.clone();
    let cursor = committed_cursor(&f, &owner.token, &ack, 2);
    owner
        .advance(Instant::now() + Duration::from_secs(3), |_, _| {
            Ok((200, receipt_response(&cursor, 7)))
        })
        .unwrap();
    let path = owner.token.directory.join(COMPLETED);
    drop(owner);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(path, bytes).unwrap();
    assert!(Acknowledgement::open(token(&f), f.credential.clone(), ack).is_err());
}
#[test]
fn concurrent_owner_and_session_corruption_refuse_without_new_budget() {
    let f = fixture();
    let owner = ready(&f);
    let ack = owner.ack.clone();
    assert!(Acknowledgement::open(token(&f), f.credential.clone(), ack.clone()).is_err());
    let path = owner.token.directory.join(SESSION);
    drop(owner);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(path, bytes).unwrap();
    assert!(Acknowledgement::open(token(&f), f.credential.clone(), ack).is_err());
}
