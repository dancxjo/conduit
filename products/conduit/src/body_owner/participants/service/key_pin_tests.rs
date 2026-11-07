//! Real canonical signed challenge fixtures; no browser/device execution claim.
use super::*;

fn proof(challenge: &conduit_body::AdmissionChallenge, secret: u8) -> In {
    let signer = SpawnInvitationSecret::from_csprng_bytes([secret; 32]).unwrap();
    In::AmbientProof {
        protocol: PROTOCOL,
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce.to_vec(),
        signature: signer.sign(&challenge.signing_transcript()).to_vec(),
    }
}
#[test]
fn key_pinned_window_learns_actual_identity_only_from_successful_signed_admission() {
    let (mut owner, root, _) = setup();
    let before = owner.session.evidence().membership.parts.len();
    assert!(owner.browser_authorize_window(None, None, 10_000).is_err());
    let authorization = owner
        .browser_authorize_window(None, Some(BROWSER_KEY), 10_000)
        .unwrap();
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    let challenge = advertise(&mut owner, &authorization.window_id);
    assert_eq!(challenge.host_id, HostId::from("host/browser-test"));
    assert_eq!(owner.session.evidence().membership.parts.len(), before);
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    let admitted = owner
        .browser_complete(&root, &authorization.window_id, proof(&challenge, 7))
        .unwrap();
    assert_eq!(admitted.credential.host_id, challenge.host_id);
    assert_eq!(admitted.credential.boot_id, challenge.boot_id);
    assert_eq!(
        owner.pending_browser.as_ref().unwrap().expected,
        Some(challenge.host_id)
    );
    assert_eq!(owner.session.evidence().membership.parts.len(), before + 1);
    assert_eq!(
        admitted
            .browser_advertisement
            .as_ref()
            .unwrap()
            .offer_generation,
        challenge.offer_generation
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn key_pinned_window_refuses_wrong_advertised_key_and_signature_key_swap() {
    let (mut owner, root, _) = setup();
    let before = owner.session.evidence().membership.clone();
    let authorization = owner
        .browser_authorize_window(None, Some(BROWSER_KEY), 10_000)
        .unwrap();
    let refusal = owner
        .browser_begin(
            &authorization.window_id,
            &LinkBindingId::from("line/test/browser"),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: host("host/browser-test", "boot/browser/first")
                    .advertisement()
                    .clone(),
                friendly_label: "Browser test".into(),
                verifying_key: vec![3; 32],
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap_err();
    assert!(refusal.contains("proof key differs"));
    assert!(matches!(
        owner.pending_browser.as_ref().unwrap().state,
        WindowState::Ready
    ));
    let challenge = advertise(&mut owner, &authorization.window_id);
    assert!(owner
        .browser_complete(&root, &authorization.window_id, proof(&challenge, 8))
        .is_err());
    assert_eq!(owner.session.evidence().membership, before);
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    owner
        .browser_complete(&root, &authorization.window_id, proof(&challenge, 7))
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn key_pinned_window_refuses_signed_proof_with_changed_advertised_host_or_boot() {
    let (mut owner, root, _) = setup();
    let before = owner.session.evidence().membership.clone();
    let authorization = owner
        .browser_authorize_window(None, Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorization.window_id);
    for change_host in [true, false] {
        let mut changed = challenge.clone();
        if change_host {
            changed.host_id = HostId::from("host/substituted");
        } else {
            changed.boot_id = BootId::from("boot/substituted");
        }
        assert!(owner
            .browser_complete(&root, &authorization.window_id, proof(&changed, 7))
            .is_err());
        assert_eq!(owner.session.evidence().membership, before);
        assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn key_pinned_window_is_not_a_return_identity_selector() {
    let (mut owner, root, _) = setup();
    let authorization = owner
        .browser_authorize_window(None, Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorization.window_id);
    let admitted = owner
        .browser_complete(&root, &authorization.window_id, proof(&challenge, 7))
        .unwrap();
    owner
        .browser_cancel_window(&root, &authorization.window_id)
        .unwrap();
    let authorization = owner
        .browser_authorize_window(None, Some(BROWSER_KEY), 10_000)
        .unwrap();
    let advertisement = host("host/browser-test", "boot/browser/returned")
        .advertisement()
        .clone();
    let frame = || In::ReturnAdvertise {
        protocol: PROTOCOL,
        credential: admitted.credential.clone(),
        advertisement: advertisement.clone(),
    };
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &LinkBindingId::from("line/returned"),
            frame(),
            512
        )
        .unwrap_err()
        .contains("known explicit Host"));
    owner
        .browser_cancel_window(&root, &authorization.window_id)
        .unwrap();
    let authorization = owner
        .browser_authorize_window(Some("host/browser-test"), None, 10_000)
        .unwrap();
    assert!(matches!(
        owner
            .browser_begin(
                &authorization.window_id,
                &LinkBindingId::from("line/returned"),
                frame(),
                512
            )
            .unwrap(),
        Out::ReturnChallenge { .. }
    ));
    std::fs::remove_dir_all(root).unwrap();
}
