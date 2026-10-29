use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn offered(
    key: &SigningKey,
    body: &conduit_body::BodyId,
    nonce: [u8; 32],
    lifetime: u64,
) -> Vec<u8> {
    let boot = "boot/face-producer";
    let mut payload = [0; 156];
    payload[..32].copy_from_slice(&nonce);
    payload[32..64].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    payload[64..96].copy_from_slice(&Sha256::digest(boot.as_bytes()));
    payload[96..128].fill(9);
    payload[128..136].copy_from_slice(&7u64.to_le_bytes());
    payload[136..140].copy_from_slice(&1024u32.to_le_bytes());
    payload[140..148].copy_from_slice(&100u64.to_le_bytes());
    payload[148..156].copy_from_slice(&(100 + lifetime).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/export@1\0");
    hash.update(key.verifying_key().to_bytes());
    hash.update(payload);
    let mut signed = payload.to_vec();
    signed.extend_from_slice(&key.sign(&hash.finalize()).to_bytes());
    serde_json::to_vec(&serde_json::json!({"profile":"conduit.face/export@1","producer_boot":boot,"body_id":body,"offer":hex(&signed)})).unwrap()
}
#[test]
fn root_signature_nonce_body_and_finite_lifetime_are_all_required() {
    let key = SigningKey::from_bytes(&[17; 32]);
    let body: conduit_body::BodyId =
        serde_json::from_str(&format!("\"{}\"", "a".repeat(64))).unwrap();
    let nonce = [4; 32];
    let root = key.verifying_key().to_bytes();
    let reply = offered(&key, &body, nonce, 5000);
    let verified = verify(&root, &body, &nonce, &reply).unwrap();
    assert_eq!(
        (verified.bytes, verified.revision, verified.lifetime),
        (1024, 7, 5000)
    );
    assert!(verify(&root, &body, &[5; 32], &reply).is_err());
    assert!(verify(
        &SigningKey::from_bytes(&[18; 32]).verifying_key().to_bytes(),
        &body,
        &nonce,
        &reply
    )
    .is_err());
    assert!(verify(
        &root,
        &serde_json::from_str(&format!("\"{}\"", "b".repeat(64))).unwrap(),
        &nonce,
        &reply
    )
    .is_err());
    for lifetime in [0, 5001] {
        assert!(verify(&root, &body, &nonce, &offered(&key, &body, nonce, lifetime)).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    value["producer_boot"] = "boot/foreign".into();
    assert!(verify(&root, &body, &nonce, &serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn atomic_local_envelope_preserves_full_face_and_cannot_extend_expired_lease() {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join(format!(
        "conduit-face-publisher-{}-{}",
        std::process::id(),
        boot_ms().unwrap()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("current-face.json");
    let face = conduit_face::Face::new(
        7,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_face::FaceSubject {
            identity: "environment".into(),
            role: conduit_face::FaceRole::Info,
            name: "Fixture".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let deadline = boot_ms().unwrap() + 3000;
    publish(&path, &face, deadline).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(envelope["valid_until_boot_ms"], deadline);
    assert_eq!(
        envelope["boot_id"],
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap()
            .trim()
    );
    let decoded: conduit_face::Face = serde_json::from_value(envelope["face"].clone()).unwrap();
    assert_eq!(decoded, face);
    decoded.validate().unwrap();
    assert!(publish(&path, &face, 0).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
