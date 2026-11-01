use super::*;
use crate::network_custody_client::tests::{committed_cursor, fixture, receipt_response, Fixture};
use ed25519_dalek::Signer;
fn retained_token(f: &Fixture) -> VerifiedDurableSnapshot {
    VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap()
}
fn signed(f: &Fixture, token: &VerifiedDurableSnapshot) -> Vec<u8> {
    token
        .encode_ack(
            &f.credential,
            ed25519_dalek::SigningKey::from_bytes(&[22; 32])
                .sign(&token.signing_transcript(&f.credential).unwrap())
                .to_bytes(),
        )
        .unwrap()
}
fn completed(f: &Fixture) -> [u8; 480] {
    let token = retained_token(f);
    let ack = signed(f, &token);
    let cursor = committed_cursor(f, &token, &ack, 2);
    let mut owner = match Acknowledgement::open(token, f.credential.clone(), ack).unwrap() {
        Start::Ready(owner) => owner,
        _ => panic!("new session"),
    };
    assert!(matches!(
        owner
            .advance(Instant::now() + Duration::from_secs(3), |_, _| Ok((
                200,
                receipt_response(&cursor, 7)
            )))
            .unwrap(),
        Progress::Complete(_)
    ));
    cursor
}
fn files(token: &VerifiedDurableSnapshot) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(&token.directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}
#[test]
fn historical_clean_reopens_after_return_without_replacing_ack_or_files() {
    let mut f = fixture();
    let cursor = completed(&f);
    let token = retained_token(&f);
    let before = files(&token);
    f.credential.boot_id = "boot/returned-observer".into();
    f.credential.issued_at_millis += 1;
    let replacement = signed(&f, &token);
    assert_ne!(
        replacement,
        reopen(&token.directory.join("ack.bin"), REQUEST).unwrap()
    );
    assert!(token
        .verify_committed_receipt(&f.credential, &replacement, &receipt_response(&cursor, 7))
        .is_err());
    match reopen_completed(&token).unwrap() {
        Retained::Complete(receipt) => assert_eq!(receipt.bytes(), &cursor),
        _ => panic!("original completion must verify"),
    }
    assert_eq!(files(&token), before);
}
#[test]
fn incomplete_session_never_becomes_a_fresh_allowance() {
    let f = fixture();
    let token = retained_token(&f);
    let before = files(&token);
    assert!(matches!(
        reopen_completed(&token).unwrap(),
        Retained::Absent
    ));
    assert_eq!(files(&token), before);
    let ack = signed(&f, &token);
    drop(Acknowledgement::open(token, f.credential.clone(), ack).unwrap());
    let token = retained_token(&f);
    let before = files(&token);
    assert!(matches!(
        reopen_completed(&token).unwrap(),
        Retained::Interrupted
    ));
    assert_eq!(files(&token), before);
}

#[test]
fn staged_ack_without_a_reserved_window_preserves_exact_original_open() {
    let f = fixture();
    let token = retained_token(&f);
    let ack = signed(&f, &token);
    // Model the actual cut after ACK retention but before reserving a session.
    immutable(&token.directory.join("ack.bin"), &ack).unwrap();
    let before = files(&token);
    assert!(matches!(
        reopen_completed(&token).unwrap(),
        Retained::Absent
    ));
    assert_eq!(files(&token), before);
    assert!(matches!(
        Acknowledgement::open(token, f.credential.clone(), ack).unwrap(),
        Start::Ready(_)
    ));
}
#[test]
fn historical_reopen_revalidates_every_original_completion_file() {
    let f = fixture();
    let cursor = completed(&f);
    let token = retained_token(&f);
    let id = digest(&cursor)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for name in [
        "ack.bin".to_owned(),
        SESSION.into(),
        COMPLETED.into(),
        format!("receipt-{id}.bin"),
        format!("receipt-{id}.json"),
    ] {
        let path = token.directory.join(name);
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[0] ^= 1;
        std::fs::write(&path, &changed).unwrap();
        assert!(reopen_completed(&token).is_err(), "{}", path.display());
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        std::fs::write(path, original).unwrap();
    }
    assert!(matches!(
        reopen_completed(&token).unwrap(),
        Retained::Complete(_)
    ));
}

#[test]
fn interrupted_original_session_adopts_obtained_clean_without_new_allowance() {
    let f = fixture();
    let token = retained_token(&f);
    let ack = signed(&f, &token);
    let cursor = committed_cursor(&f, &token, &ack, 2);
    let owner = Acknowledgement::open(token, f.credential.clone(), ack).unwrap();
    let token = retained_token(&f);
    let before = files(&token);
    assert!(retain_completion(&token, &receipt_response(&cursor, 7)).is_err());
    assert_eq!(files(&token), before);
    drop(owner);
    let marker = reopen(&token.directory.join(SESSION), 88).unwrap();
    let original_ack = reopen(&token.directory.join("ack.bin"), REQUEST).unwrap();
    let response = receipt_response(&cursor, 7);
    assert_eq!(
        retain_completion(&token, &response).unwrap().bytes(),
        &cursor
    );
    assert_eq!(reopen(&token.directory.join(SESSION), 88).unwrap(), marker);
    assert_eq!(
        reopen(&token.directory.join("ack.bin"), REQUEST).unwrap(),
        original_ack
    );
    let before = files(&token);
    assert_eq!(
        retain_completion(&token, &response).unwrap().bytes(),
        &cursor
    );
    assert_eq!(files(&token), before);
}

#[test]
fn obtained_completion_requires_original_session_and_valid_clean_signature() {
    let f = fixture();
    let token = retained_token(&f);
    let ack = signed(&f, &token);
    let clean = committed_cursor(&f, &token, &ack, 2);
    let before = files(&token);
    assert!(retain_completion(&token, &receipt_response(&clean, 7)).is_err());
    assert_eq!(files(&token), before);
    drop(Acknowledgement::open(token, f.credential.clone(), ack.clone()).unwrap());
    let token = retained_token(&f);
    let before = files(&token);
    let pending = committed_cursor(&f, &token, &ack, 1);
    assert!(retain_completion(&token, &receipt_response(&pending, 7)).is_err());
    let mut corrupt = clean;
    corrupt[408] ^= 1;
    assert!(retain_completion(&token, &receipt_response(&corrupt, 7)).is_err());
    assert_eq!(files(&token), before);
}
