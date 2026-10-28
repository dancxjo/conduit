use super::*;
use ed25519_dalek::Signer;
struct Fixture {
    directory: PathBuf,
    scope: Scope,
    offer: [u8; 240],
    snapshot: Vec<u8>,
    credential: MembershipCredential,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn fixture() -> Fixture {
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
