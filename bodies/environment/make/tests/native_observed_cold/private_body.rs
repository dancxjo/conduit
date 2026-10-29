//! Private actual-image preflight uses the same observation-layout profile as firmware.
/// Exact normal-boot preflight for physical installation. It neither restores
/// an owner nor changes flash; the caller keeps the private source file sealed.
#[test]
#[ignore = "requires private pinned Body snapshot and explicit proof output"]
fn private_pinned_body_has_no_network_reclaim_cursor() {
    preflight(false);
}
#[test]
#[ignore = "requires private pinned Body, original Pending receipt and explicit proof output"]
fn private_pinned_body_has_exact_clean_network_cursor() {
    preflight(true);
}
#[test]
#[ignore = "requires private pinned Body and independently retained committed Clean receipt"]
fn private_pinned_body_has_exact_committed_clean_network_cursor() {
    preflight_with_committed_receipt(true, true);
}
fn preflight(require_clean: bool) {
    preflight_with_committed_receipt(require_clean, false);
}
fn preflight_with_committed_receipt(require_clean: bool, committed: bool) {
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
    if require_clean {
        if committed {
            let bytes: [u8; 480] =
                std::fs::read(std::env::var("CONDUIT_PRIVATE_NETWORK_CLEAN_RECEIPT").unwrap())
                    .unwrap()
                    .try_into()
                    .unwrap();
            let expected_cursor =
                crate::native_restart::payload::network::Cursor::decode(bytes).unwrap();
            let root = ed25519_dalek::VerifyingKey::from_bytes(&expected).unwrap();
            expected_cursor.validate(&body, &root).unwrap();
            assert_eq!(expected_cursor.bytes()[8], 2);
            let cursor = resident
                .network
                .as_ref()
                .expect("authenticated Clean cursor");
            cursor.validate(&body, &root).unwrap();
            assert_eq!(cursor.bytes(), expected_cursor.bytes());
        } else {
            let pending =
                std::fs::read(std::env::var("CONDUIT_PRIVATE_NETWORK_PENDING_RECEIPT").unwrap())
                    .unwrap();
            assert_eq!(pending.len(), 480);
            assert_eq!(pending[8], 1);
            let cursor = resident
                .network
                .as_ref()
                .expect("authenticated Clean cursor");
            cursor
                .validate(
                    &body,
                    &ed25519_dalek::VerifyingKey::from_bytes(&expected).unwrap(),
                )
                .unwrap();
            assert_eq!(cursor.bytes()[8], 2);
            assert_eq!(
                u64::from_le_bytes(cursor.bytes()[16..24].try_into().unwrap()),
                u64::from_le_bytes(pending[16..24].try_into().unwrap())
                    .checked_add(1)
                    .unwrap()
            );
            assert_eq!(cursor.bytes()[24..408], pending[24..408]);
        }
    } else {
        assert!(
            resident.network.is_none(),
            "normal legacy install/fallback requires no existing Pending or Clean cursor"
        );
    }
    let proof = serde_json::json!({
        "scope": "read-only actual pinned Body checkpoint through production decoder",
        "actual_root_and_body_pin_match": true,
        "generation": source.generation(),
        "schema": resident.retained.schema,
        "network_cursor_absent": !require_clean,
        "exact_original_receipt_clean_cursor": require_clean && !committed,
        "exact_independently_retained_committed_clean_cursor": committed,
        "archives": resident.summary.count,
        "owner_restored_or_effects_started": false,
    });
    std::fs::write(proof_path, serde_json::to_vec_pretty(&proof).unwrap()).unwrap();
}
