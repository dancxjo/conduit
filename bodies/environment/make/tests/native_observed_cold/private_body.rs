//! Private actual-image preflight uses the same observation-layout profile as firmware.
/// Exact normal-boot preflight for physical installation. It neither restores
/// an owner nor changes flash; the caller keeps the private source file sealed.
#[test]
#[ignore = "requires private pinned Body snapshot and explicit proof output"]
fn private_pinned_body_has_no_network_reclaim_cursor() {
    use sha2::{Digest, Sha256};
    let path = std::env::var("CONDUIT_PRIVATE_BODY").unwrap();
    let pin_path = std::env::var("CONDUIT_PRIVATE_ROOT_PIN").unwrap();
    let proof_path = std::env::var("CONDUIT_PRIVATE_PREFLIGHT_PROOF").unwrap();
    let raw = std::fs::read(path).unwrap();
    assert_eq!(raw.len(), 65536);
    let pin: serde_json::Value = serde_json::from_slice(&std::fs::read(pin_path).unwrap()).unwrap();
    let body: conduit_body::BodyId = serde_json::from_value(pin["body"].clone()).unwrap();
    let expected: Vec<u8> = serde_json::from_value(pin["root_key"].clone()).unwrap();
    let expected: [u8; 32] = expected.try_into().unwrap();
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let mut region = crate::native_evidence_region::test_empty();
    crate::native_body_checkpoint::Storage::program(&mut region, 0, &raw).unwrap();
    let mut journal = crate::native_body_checkpoint::Checkpoint::open(region).unwrap();
    let mut seed = [0; 32];
    let (resident, source) = journal
        .load_streamed_with_snapshot(&binding, &mut seed, |reader| {
            crate::native_restart::payload::decode_resident_boxed_with_prefix(
                |out| reader.read(out).map_err(|_| ()),
                &body,
                &expected,
                None,
            )
            .map_err(|reason| {
                eprintln!("private preflight decoder refusal: {reason}");
                crate::native_body_checkpoint::Refusal::Corrupt
            })
        })
        .unwrap()
        .unwrap();
    let actual = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    seed.fill(0);
    assert_eq!(
        actual, expected,
        "actual retained Root must match independently pinned Root"
    );
    assert!(
        resident.network.is_none(),
        "normal legacy install/fallback requires no existing Pending or Clean cursor"
    );
    let proof = serde_json::json!({
        "scope": "read-only actual pinned Body checkpoint through production decoder",
        "actual_root_and_body_pin_match": true,
        "generation": source.generation(),
        "schema": resident.retained.schema,
        "network_cursor_absent": true,
        "archives": resident.summary.count,
        "owner_restored_or_effects_started": false,
    });
    std::fs::write(proof_path, serde_json::to_vec_pretty(&proof).unwrap()).unwrap();
}
