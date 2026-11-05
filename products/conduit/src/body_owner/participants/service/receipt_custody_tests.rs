//! Browser completion uses the same durable retirement as native completion.
use super::*;

#[test]
fn signed_browser_returns_cross_hot_capacity_without_replacing_the_part() {
    let (mut owner, root, _) = setup();
    let secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let mut window = owner
        .browser_authorize_window(Some("host/browser-test"), Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &window.window_id);
    let mut current = owner
        .browser_complete(
            &root,
            &window.window_id,
            In::AmbientProof {
                protocol: PROTOCOL,
                admission_id: challenge.admission_id.clone(),
                body_id: challenge.body_id.clone(),
                host_id: challenge.host_id.clone(),
                boot_id: challenge.boot_id.clone(),
                nonce: challenge.nonce.to_vec(),
                signature: secret.sign(&challenge.signing_transcript()).to_vec(),
            },
        )
        .unwrap()
        .credential;
    let original = current.clone();
    for index in 1..=33 {
        owner
            .browser_cancel_window(&root, &window.window_id)
            .unwrap();
        window = owner
            .browser_authorize_window(Some("host/browser-test"), None, 10_000)
            .unwrap();
        let ad = host("host/browser-test", &format!("boot/browser/return/{index}"))
            .advertisement()
            .clone();
        let Out::ReturnChallenge { challenge, .. } = owner
            .browser_begin(
                &window.window_id,
                &format!("line/browser/return/{index}").into(),
                In::ReturnAdvertise {
                    protocol: PROTOCOL,
                    credential: current,
                    advertisement: ad,
                },
                512,
            )
            .unwrap()
        else {
            panic!("expected return challenge")
        };
        current = owner
            .browser_complete(
                &root,
                &window.window_id,
                In::ReturnProof {
                    protocol: PROTOCOL,
                    admission_id: challenge.admission_id.clone(),
                    body_id: challenge.body_id.clone(),
                    part_id: challenge.part_id.clone(),
                    host_id: challenge.host_id.clone(),
                    boot_id: challenge.boot_id.clone(),
                    nonce: challenge.nonce.to_vec(),
                    signature: secret.sign(&challenge.signing_transcript()).to_vec(),
                },
            )
            .unwrap()
            .credential;
        assert_eq!(current.part_id, original.part_id);
        assert_eq!(owner.session.evidence().membership.parts.len(), 2);
        let manager = owner.admissions.as_ref().unwrap();
        assert!(manager.receipts.len() <= conduit_body::MAX_ADMISSION_RECEIPTS);
        assert_eq!(manager.receipts[0].credential, original);
        assert_eq!(manager.storage_usage().continuity_keys.len, 1);
        assert_eq!(
            crate::durable_host::owner::state::admissions(&root, &current.body_id)
                .unwrap()
                .as_ref(),
            Some(manager)
        );
    }
    assert_eq!(
        owner.admissions.as_ref().unwrap().retired_receipt_count(),
        30
    );
    owner
        .browser_cancel_window(&root, &window.window_id)
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
