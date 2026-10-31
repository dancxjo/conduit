//! CPU protocol proofs, not installed Host or physical-source admission.
use conduit_environment_host::signed_face_export::verify_snapshot;
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

fn body() -> conduit_body::BodyId {
    serde_json::from_str(&format!("\"{}\"", "a".repeat(64))).unwrap()
}
fn face() -> Face {
    Face::new(
        7,
        FaceBasis {
            body_id: Some(body()),
            wake_id: Some(serde_json::from_str("\"fixture/wake\"").unwrap()),
            source_document_id: Some("fixture/source".into()),
            checked_plot_id: Some("fixture/checked".into()),
            expanded_plot_id: Some("fixture/expanded".into()),
            plan_id: Some("fixture/plan".into()),
            active_play_id: Some("fixture/play".into()),
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "environment".into(),
            role: FaceRole::Info,
            name: "Room conditions".into(),
        }],
        vec![],
        vec![FaceProperty {
            subject: "environment".into(),
            name: "sequence".into(),
            value: FacePropertyValue::Count(4),
        }],
        vec![],
    )
    .unwrap()
}
fn offer(key: &SigningKey, document: &[u8], lifetime: u64) -> Vec<u8> {
    let mut payload = [0; 156];
    payload[..32].fill(4);
    payload[32..64].copy_from_slice(&Sha256::digest(body().as_str().as_bytes()));
    payload[64..96].copy_from_slice(&Sha256::digest(b"fixture/producer-boot"));
    payload[96..128].copy_from_slice(&Sha256::digest(document));
    payload[128..136].copy_from_slice(&7u64.to_le_bytes());
    payload[136..140].copy_from_slice(&(document.len() as u32).to_le_bytes());
    payload[140..148].copy_from_slice(&100u64.to_le_bytes());
    payload[148..156].copy_from_slice(&(100 + lifetime).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/export@1\0");
    hash.update(key.verifying_key().to_bytes());
    hash.update(payload);
    let mut signed = payload.to_vec();
    signed.extend_from_slice(&key.sign(&hash.finalize()).to_bytes());
    let hex: String = signed.iter().map(|b| format!("{b:02x}")).collect();
    serde_json::to_vec(&serde_json::json!({
        "profile": "conduit.face/export@1", "producer_boot": "fixture/producer-boot",
        "body_id": body(), "offer": hex
    }))
    .unwrap()
}

#[test]
fn exact_document_and_authenticated_producer_scope_survive_extraction() {
    let key = SigningKey::from_bytes(&[17; 32]);
    let original = face();
    let document = serde_json::to_vec(&original).unwrap();
    let verified = verify_snapshot(
        &key.verifying_key().to_bytes(),
        &body(),
        &[4; 32],
        &offer(&key, &document, 5000),
    )
    .unwrap();
    assert_eq!(verified.producer_boot().as_str(), "fixture/producer-boot");
    assert_eq!(verified.lifetime(), 5000);
    assert_eq!(verified.decode_document(&document).unwrap(), original);
    let mut changed = original;
    changed.properties[0].value = FacePropertyValue::Count(5);
    let substituted = serde_json::to_vec(&changed).unwrap();
    assert_eq!(substituted.len(), document.len());
    assert!(verified.decode_document(&substituted).is_err());
    assert!(verified
        .decode_document(&document[..document.len() - 1])
        .is_err());
}

#[test]
fn nonce_root_and_boot_cannot_be_substituted_and_lifetime_stays_finite() {
    let key = SigningKey::from_bytes(&[17; 32]);
    let document = serde_json::to_vec(&face()).unwrap();
    let encoded = offer(&key, &document, 5000);
    let root = key.verifying_key().to_bytes();
    assert!(verify_snapshot(&root, &body(), &[5; 32], &encoded).is_err());
    let foreign = SigningKey::from_bytes(&[18; 32]).verifying_key().to_bytes();
    assert!(verify_snapshot(&foreign, &body(), &[4; 32], &encoded).is_err());
    let mut changed: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    changed["producer_boot"] = "fixture/other-boot".into();
    assert!(verify_snapshot(
        &root,
        &body(),
        &[4; 32],
        &serde_json::to_vec(&changed).unwrap()
    )
    .is_err());
    for lifetime in [0, 5001] {
        assert!(
            verify_snapshot(&root, &body(), &[4; 32], &offer(&key, &document, lifetime)).is_err()
        );
    }
    assert!(verify_snapshot(&root, &body(), &[4; 32], &[b' '; 1409]).is_err());
}

#[test]
fn signed_but_incomplete_execution_basis_is_not_an_environmental_face() {
    let key = SigningKey::from_bytes(&[17; 32]);
    let mut incomplete = face();
    incomplete.basis.active_play_id = None;
    let document = serde_json::to_vec(&incomplete).unwrap();
    let verified = verify_snapshot(
        &key.verifying_key().to_bytes(),
        &body(),
        &[4; 32],
        &offer(&key, &document, 5000),
    )
    .unwrap();
    assert!(verified.decode_document(&document).is_err());
}
