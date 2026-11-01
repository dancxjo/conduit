use super::*;
use ed25519_dalek::Signer;
pub(super) struct Fixture {
    pub(super) directory: PathBuf,
    pub(super) scope: Scope,
    pub(super) offer: [u8; 240],
    pub(super) snapshot: Vec<u8>,
    pub(super) credential: MembershipCredential,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
pub(super) fn fixture() -> Fixture {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "conduit-network-custody-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
    let credential=serde_json::from_value::<MembershipCredential>(serde_json::json!({"credential_id":"01".repeat(32),"body_id":"02".repeat(32),"part_id":"03".repeat(32),"host_id":"04".repeat(32),"boot_id":"05".repeat(32),"issued_at_millis":123})).unwrap();
    let scope = Scope {
        body: credential.body_id.clone(),
        root: key.verifying_key().to_bytes(),
        producer_boot: "06".repeat(32).into(),
        epoch: 3,
        region: [7; 32],
        predecessor: [8; 32],
    };
    let snapshot = vec![17; 32768];
    let mut offer = [0; 240];
    offer[..32].copy_from_slice(&digest(scope.body.as_str().as_bytes()));
    offer[32..64].copy_from_slice(&scope.region);
    offer[64..72].copy_from_slice(&scope.epoch.to_le_bytes());
    offer[72..104].copy_from_slice(&digest(scope.producer_boot.as_str().as_bytes()));
    offer[104..108].copy_from_slice(&32768u32.to_le_bytes());
    offer[108] = 8;
    offer[112..144].copy_from_slice(&digest(&snapshot));
    offer[144..176].copy_from_slice(&scope.predecessor);
    let sig = key.sign(&offer_id(&scope.root, &offer));
    offer[176..].copy_from_slice(&sig.to_bytes());
    Fixture {
        directory,
        scope,
        offer,
        snapshot,
        credential,
    }
}
#[test]
fn durable_reopen_exact_request_and_immutable_retry() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let message = token.signing_transcript(&f.credential).unwrap();
    let member = ed25519_dalek::SigningKey::from_bytes(&[22; 32]);
    let request = token
        .encode_ack(&f.credential, member.sign(&message).to_bytes())
        .unwrap();
    assert_eq!(request.len(), 628);
    assert_eq!(&request[..8], b"NCACK1\0\0");
    assert_eq!(&request[8..40], &offer_id(&f.scope.root, &f.offer));
    assert_eq!(&request[40..48], &123u64.to_le_bytes());
    assert_eq!(request[48..52], [64; 4]);
    assert_eq!(&request[52..116], f.credential.part_id.as_str().as_bytes());
    assert!(request[116..180].iter().all(|b| *b == 0));
    let second =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    assert_eq!(message, second.signing_transcript(&f.credential).unwrap());
}
#[test]
fn corruption_after_retention_prevents_signing_and_overwrite() {
    for name in ["offer.bin", "snapshot.bin"] {
        let f = fixture();
        let token =
            VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
        let path = token.directory.join(name);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        assert!(token.signing_transcript(&f.credential).is_err());
        assert!(token.encode_ack(&f.credential, [0; 64]).is_err());
        assert!(
            VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
#[test]
fn invalid_scope_signature_extent_and_reserved_fields_never_retain() {
    let f = fixture();
    for at in [0, 32, 64, 72, 104, 108, 109, 112, 144, 176, 239] {
        let mut offer = f.offer;
        offer[at] ^= 1;
        assert!(
            VerifiedDurableSnapshot::retain(&f.directory, &offer, &f.snapshot, &f.scope).is_err()
        );
    }
    assert!(VerifiedDurableSnapshot::retain(
        &f.directory,
        &f.offer,
        &f.snapshot[..32767],
        &f.scope
    )
    .is_err());
    let mut scope = f.scope.clone();
    scope.root = [9; 32];
    assert!(VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &scope).is_err());
    assert_eq!(std::fs::read_dir(&f.directory).unwrap().count(), 0);
}
#[test]
fn interrupted_partial_file_and_symlink_refuse_without_replacing() {
    let f = fixture();
    let name = offer_id(&f.scope.root, &f.offer)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let dir = f.directory.join(format!("network-{name}"));
    std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    let partial = dir.join("offer.bin");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&partial)
        .unwrap();
    file.write_all(&f.offer[..100]).unwrap();
    file.sync_all().unwrap();
    assert!(
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).is_err()
    );
    assert_eq!(std::fs::read(&partial).unwrap().len(), 100);
    assert!(!dir.join("snapshot.bin").exists());
    std::fs::remove_file(&partial).unwrap();
    std::os::unix::fs::symlink("missing", &partial).unwrap();
    assert!(
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).is_err()
    );
    assert!(std::fs::symlink_metadata(partial)
        .unwrap()
        .file_type()
        .is_symlink());
}

// Compile the producer's exact canonical codec, including its independently
// implemented member transcript, against a minimal owner descriptor. No fixture
// owner is used to authenticate or mint custody in the host implementation.
pub(super) struct Draft {
    pub payload: [u8; 176],
    pub key: [u8; 32],
}
impl Draft {
    pub fn root(&self) -> &[u8; 32] {
        &self.key
    }
    pub fn transcript(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"conduit.environment/network-retirement-offer@1\0");
        h.update(self.key);
        h.update(self.payload);
        h.finalize().into()
    }
}
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_network_ack.rs"]
pub mod ack;
#[test]
fn member_transcript_and_encoding_match_actual_native_canonical_codec() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let transcript = token.signing_transcript(&f.credential).unwrap();
    let signature = ed25519_dalek::SigningKey::from_bytes(&[22; 32])
        .sign(&transcript)
        .to_bytes();
    let encoded = token.encode_ack(&f.credential, signature).unwrap();
    let native =
        ack::Request::for_credential(offer_id(&f.scope.root, &f.offer), &f.credential, signature)
            .unwrap();
    assert_eq!(encoded.as_slice(), native);
    let decoded = ack::Request::decode(&encoded).unwrap();
    assert_eq!(transcript, decoded.transcript(&f.scope.body));
}

#[path = "../../../../../targets/esp32/proof/tm1637/src/native_network_custody_wire.rs"]
pub(super) mod cursor;
pub(super) fn committed_cursor(
    f: &Fixture,
    token: &VerifiedDurableSnapshot,
    ack: &[u8],
    phase: u8,
) -> [u8; 480] {
    let mut bytes = [0; 480];
    bytes[..8].copy_from_slice(b"NCURSOR1");
    bytes[8] = phase;
    bytes[16..24].copy_from_slice(&(f.scope.epoch + u64::from(phase == 2)).to_le_bytes());
    bytes[24..168].copy_from_slice(&f.offer[..144]);
    bytes[168..200].copy_from_slice(&offer_id(&f.scope.root, &f.offer));
    bytes[200..232].copy_from_slice(&token.member_scope(&f.credential).unwrap());
    bytes[232..264].copy_from_slice(
        ed25519_dalek::SigningKey::from_bytes(&[22; 32])
            .verifying_key()
            .as_bytes(),
    );
    bytes[264..328].copy_from_slice(&ack[564..]);
    bytes[328..360].copy_from_slice(&token.signing_transcript(&f.credential).unwrap());
    bytes[360..392].copy_from_slice(&f.scope.predecessor);
    resign_cursor(&mut bytes);
    bytes
}
fn resign_cursor(bytes: &mut [u8; 480]) {
    let mut h = Sha256::new();
    h.update(b"conduit.environment/network-retirement-receipt@1\0");
    h.update(&bytes[..408]);
    let sig = ed25519_dalek::SigningKey::from_bytes(&[41; 32])
        .sign(&h.finalize())
        .to_bytes();
    bytes[408..472].copy_from_slice(&sig);
}
pub(super) fn receipt_response(cursor: &[u8; 480], generation: u64) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"profile":"network-retirement-committed@1","reclaimed":cursor[8]==2,"generation":generation,"receipt":cursor.iter().map(|b|format!("{b:02x}")).collect::<String>()})).unwrap()
}
#[test]
fn committed_receipt_agrees_with_native_cursor_and_rejects_every_changed_byte() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let sig = ed25519_dalek::SigningKey::from_bytes(&[22; 32])
        .sign(&token.signing_transcript(&f.credential).unwrap())
        .to_bytes();
    let ack = token.encode_ack(&f.credential, sig).unwrap();
    for phase in [1, 2] {
        let valid = committed_cursor(&f, &token, &ack, phase);
        cursor::Cursor::decode(valid)
            .validate(
                &f.scope.body,
                &VerifyingKey::from_bytes(&f.scope.root).unwrap(),
            )
            .unwrap();
        let checked = token
            .verify_committed_receipt(&f.credential, &ack, &receipt_response(&valid, 7))
            .unwrap();
        assert_eq!(checked.bytes(), &valid);
        // Generation is unsigned presentation only; it cannot alter the proof.
        assert_eq!(
            token
                .verify_committed_receipt(&f.credential, &ack, &receipt_response(&valid, 999))
                .unwrap()
                .bytes(),
            checked.bytes()
        );
    }
    for phase in [1, 2] {
        let valid = committed_cursor(&f, &token, &ack, phase);
        for at in 0..480 {
            for bit in 0..8 {
                let mut changed = valid;
                changed[at] ^= 1 << bit;
                assert!(
                    token
                        .verify_committed_receipt(
                            &f.credential,
                            &ack,
                            &receipt_response(&changed, 7)
                        )
                        .is_err(),
                    "cursor phase {phase} byte {at} bit {bit}"
                );
            }
        }
        // Even a valid Root signature cannot replace the actual phase/epoch
        // relation, and unsigned presentation cannot claim the opposite phase.
        let mut response: serde_json::Value =
            serde_json::from_slice(&receipt_response(&valid, 7)).unwrap();
        response["reclaimed"] = (phase != 2).into();
        assert!(token
            .verify_committed_receipt(&f.credential, &ack, &serde_json::to_vec(&response).unwrap())
            .is_err());
        let mut wrong_epoch = valid;
        wrong_epoch[16..24].copy_from_slice(&(f.scope.epoch + 2).to_le_bytes());
        resign_cursor(&mut wrong_epoch);
        assert!(token
            .verify_committed_receipt(&f.credential, &ack, &receipt_response(&wrong_epoch, 7))
            .is_err());
        let mut wrong_phase = valid;
        wrong_phase[8] = if phase == 1 { 2 } else { 1 };
        resign_cursor(&mut wrong_phase);
        assert!(token
            .verify_committed_receipt(&f.credential, &ack, &receipt_response(&wrong_phase, 7))
            .is_err());
    }
}
#[test]
fn root_signature_cannot_substitute_another_ack_offer_or_lost_durable_evidence() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let sig = ed25519_dalek::SigningKey::from_bytes(&[22; 32])
        .sign(&token.signing_transcript(&f.credential).unwrap())
        .to_bytes();
    let ack = token.encode_ack(&f.credential, sig).unwrap();
    let valid = committed_cursor(&f, &token, &ack, 1);
    for at in [24, 56, 88, 96, 128, 132, 136, 168, 200, 232, 264, 328, 360] {
        let mut changed = valid;
        changed[at] ^= 1;
        resign_cursor(&mut changed);
        assert!(
            token
                .verify_committed_receipt(&f.credential, &ack, &receipt_response(&changed, 7))
                .is_err(),
            "Root-signed mismatch {at}"
        );
    }
    let mut changed_ack = ack.clone();
    changed_ack[52] ^= 1;
    assert!(token
        .verify_committed_receipt(&f.credential, &changed_ack, &receipt_response(&valid, 7))
        .is_err());
    let mut response: serde_json::Value =
        serde_json::from_slice(&receipt_response(&valid, 7)).unwrap();
    response["reclaimed"] = true.into();
    assert!(token
        .verify_committed_receipt(&f.credential, &ack, &serde_json::to_vec(&response).unwrap())
        .is_err());
    std::fs::remove_file(token.directory.join("snapshot.bin")).unwrap();
    assert!(token
        .verify_committed_receipt(&f.credential, &ack, &receipt_response(&valid, 7))
        .is_err());
}
#[test]
fn attended_retry_posts_only_exact_reopened_ack_and_retains_signed_receipt() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let sig = ed25519_dalek::SigningKey::from_bytes(&[22; 32])
        .sign(&token.signing_transcript(&f.credential).unwrap())
        .to_bytes();
    let ack = token.encode_ack(&f.credential, sig).unwrap();
    let cursor = committed_cursor(&f, &token, &ack, 1);
    let response = receipt_response(&cursor, 7);
    let mut calls = 0;
    live::exchange(
        &token,
        &f.credential,
        &ack,
        |sent| {
            calls += 1;
            assert_eq!(sent, ack);
            assert_eq!(reopen(&token.directory.join("ack.bin"), 628).unwrap(), sent);
            match calls {
                1 => Err("lost reply".into()),
                2 => Ok((202, vec![])),
                _ => Ok((200, response.clone())),
            }
        },
        || {},
    )
    .unwrap();
    assert_eq!(calls, 3);
    let id = digest(&cursor)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        reopen(&token.directory.join(format!("receipt-{id}.bin")), 480).unwrap(),
        cursor
    );
    assert_eq!(
        reopen(
            &token.directory.join(format!("receipt-{id}.json")),
            response.len()
        )
        .unwrap(),
        response
    );
}
#[test]
fn attended_conflicting_ack_prevents_any_post_and_retry_bound_preserves_evidence() {
    let f = fixture();
    let token =
        VerifiedDurableSnapshot::retain(&f.directory, &f.offer, &f.snapshot, &f.scope).unwrap();
    let ack = token.encode_ack(&f.credential, [1; 64]).unwrap();
    immutable(&token.directory.join("ack.bin"), &[0; 628]).unwrap();
    assert!(live::exchange(
        &token,
        &f.credential,
        &ack,
        |_| panic!("must not post"),
        || {}
    )
    .is_err());
    std::fs::remove_file(token.directory.join("ack.bin")).unwrap();
    let mut calls = 0;
    assert!(live::exchange(
        &token,
        &f.credential,
        &ack,
        |_| {
            calls += 1;
            Ok((503, vec![]))
        },
        || {}
    )
    .is_err());
    assert_eq!(calls, 451);
    assert_eq!(reopen(&token.directory.join("ack.bin"), 628).unwrap(), ack);
    assert!(token.directory.join("snapshot.bin").exists());
}
#[test]
fn binary_http_preserves_exact_signed_bytes_and_content_type() {
    use std::net::TcpListener;
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = server.local_addr().unwrap().to_string();
    let sent = [31u8; 628];
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut bytes = Vec::new();
        let mut one = [0; 1];
        while !bytes.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut one).unwrap();
            bytes.push(one[0]);
        }
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("Content-Type: application/octet-stream\r\n"));
        assert!(text.starts_with("POST /network/custody/ack HTTP/1.1\r\n"));
        assert!(text.contains("Content-Length: 628\r\n"));
        let mut body = [0; 628];
        stream.read_exact(&mut body).unwrap();
        assert_eq!(body, sent);
        stream
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    assert_eq!(
        crate::body_join::http_binary(&peer, "/network/custody/ack", &sent)
            .unwrap()
            .0,
        202
    );
    thread.join().unwrap();
}
