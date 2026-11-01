use super::*;
use conduit_body::{
    PortableInvitation, PortableMembershipAdmissionRequest, MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
};
fn establish(owner: &mut Owner, root: &Path) -> [u8; 32] {
    let body = owner.session.evidence().body_id.clone();
    crate::durable_host::owner::establish_root_signer(owner, root, body.as_str())
        .unwrap()
        .public_key
}
fn start(
    owner: &mut Owner,
    root: &Path,
    ttl: u64,
    id: &str,
    boot: &str,
) -> (String, PortableInvitation, HostAdvertisement) {
    let (invitation, authorization) = owner
        .browser_authorize_invitation_window(root, ttl * 1000)
        .unwrap();
    let advertisement = host(id, boot).advertisement().clone();
    let window = authorization.window_id;
    owner
        .browser_begin(
            &window,
            &"line/signed-browser".into(),
            In::InvitedAdvertise {
                protocol: PROTOCOL,
                advertisement: advertisement.clone(),
                claim: invitation.claim.clone(),
                verifying_key: BROWSER_KEY.to_vec(),
            },
            512,
        )
        .unwrap();
    (window, invitation, advertisement)
}
fn challenge(
    owner: &mut Owner,
    invitation: &PortableInvitation,
    ad: &HostAdvertisement,
    key: [u8; 32],
) -> String {
    let window = owner
        .browser_authorize_window(ad.host_id.as_str(), Some(key), 10_000)
        .unwrap();
    owner
        .browser_begin(
            &window.window_id,
            &"line/signed-browser".into(),
            In::InvitedAdvertise {
                protocol: PROTOCOL,
                advertisement: ad.clone(),
                claim: invitation.claim.clone(),
                verifying_key: key.to_vec(),
            },
            512,
        )
        .unwrap();
    window.window_id
}
fn request(
    invitation: &PortableInvitation,
    ad: &HostAdvertisement,
) -> PortableMembershipAdmissionRequest {
    let In::SpawnProof {
        invitation_id,
        body_id,
        host_id,
        boot_id,
        nonce,
        signature,
        ..
    } = super::invited_tests::proof(invitation, ad)
    else {
        unreachable!()
    };
    PortableMembershipAdmissionRequest {
        schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        invitation_id,
        body_id,
        host_id,
        boot_id,
        offer_generation: ad.offer_generation,
        nonce: nonce.try_into().unwrap(),
        signature: signature.try_into().unwrap(),
    }
}
#[test]
fn signed_receipt_recovers_after_owner_restart_and_invitation_expiry_without_reconsume() {
    let (mut owner, root, resident) = setup();
    let pin = establish(&mut owner, &root);
    let (window, invitation, ad) = start(
        &mut owner,
        &root,
        1,
        "host/browser-test",
        "boot/browser/signed",
    );
    let snapshot = owner
        .browser_complete(
            &root,
            &window,
            super::invited_tests::proof(&invitation, &ad),
        )
        .unwrap();
    let first = snapshot.signed_membership_receipt.unwrap();
    first.verify(&pin, &request(&invitation, &ad)).unwrap();
    assert!(!first.current_offers_available && !first.plan_created && !first.play_created);
    owner.browser_cancel_window(&root, &window).unwrap();
    let admitted_parts = owner.session.evidence().membership.parts.len();
    let count = owner.admissions.as_ref().unwrap().receipts.len();
    let retained = crate::durable_host::owner::state::load(&root)
        .unwrap()
        .unwrap();
    drop(owner);
    std::thread::sleep(Duration::from_millis(1100));
    assert!(
        crate::durable_host::current_time_millis().unwrap() >= invitation.claim.expires_at_millis
    );
    let mut reopened = Owner::open(
        host("host/owner-test", "boot/owner/new"),
        resident,
        Some(retained),
        "ignored",
    )
    .unwrap();
    reopened.restore_execution(&root).unwrap();
    // Canonical owner Boot recovery itself advances membership; custody retry
    // must leave that already resumed revision unchanged.
    let revision = reopened.session.evidence().membership.revision;
    let again = challenge(&mut reopened, &invitation, &ad, BROWSER_KEY);
    let crate::durable_host::owner::BrowserAdmissionCompletion::CustodyOnly { receipt: recovered } =
        reopened
            .browser_complete_outcome(&root, &again, super::invited_tests::proof(&invitation, &ad))
            .unwrap()
    else {
        panic!("recovery must not restore live authority")
    };
    let recovered = *recovered;
    assert!(matches!(
        reopened.pending_browser.as_ref().unwrap().state,
        WindowState::Ready
    ));
    assert!(reopened
        .browser_planning_offer(
            &again,
            &recovered.credential,
            &OfferDisclosureRequest {
                stage: OfferDisclosureStage::AdmittedMembership,
                capability_ids: vec![],
                resource_pool_ids: vec![],
            }
        )
        .is_err());
    assert_eq!(first, recovered);
    assert_ne!(
        recovered.producer_boot,
        reopened.host.advertisement().boot_id
    );
    assert_eq!(reopened.session.evidence().membership.revision, revision);
    assert_eq!(
        reopened.session.evidence().membership.parts.len(),
        admitted_parts
    );
    assert!(reopened
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .find(|part| part.part_id == recovered.credential.part_id)
        .unwrap()
        .current
        .is_none());
    assert_eq!(reopened.admissions.as_ref().unwrap().receipts.len(), count);
    recovered.verify(&pin, &request(&invitation, &ad)).unwrap();
    // Only a fresh canonical return challenge can attach the next Boot and
    // create active Line/Face authority after custody-only recovery.
    let returned_ad = host("host/browser-test", "boot/browser/returned")
        .advertisement()
        .clone();
    let Out::ReturnChallenge {
        challenge: return_challenge,
        ..
    } = reopened
        .browser_begin(
            &again,
            &"line/returned".into(),
            In::ReturnAdvertise {
                protocol: PROTOCOL,
                credential: recovered.credential.clone(),
                advertisement: returned_ad.clone(),
            },
            512,
        )
        .unwrap()
    else {
        panic!("expected canonical return")
    };
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let result = reopened
        .browser_complete_outcome(
            &root,
            &again,
            In::ReturnProof {
                protocol: PROTOCOL,
                admission_id: return_challenge.admission_id.clone(),
                body_id: return_challenge.body_id.clone(),
                part_id: return_challenge.part_id.clone(),
                host_id: return_challenge.host_id.clone(),
                boot_id: return_challenge.boot_id.clone(),
                nonce: return_challenge.nonce.to_vec(),
                signature: secret.sign(&return_challenge.signing_transcript()).to_vec(),
            },
        )
        .unwrap();
    let crate::durable_host::owner::BrowserAdmissionCompletion::Active { snapshot } = result else {
        panic!("return must admit real active carrier")
    };
    assert_eq!(snapshot.credential.boot_id, returned_ad.boot_id);
    assert!(snapshot.line_authorization.is_some());
    assert!(snapshot.signed_membership_receipt.is_none());
    reopened.browser_cancel_window(&root, &again).unwrap();
    let stale = challenge(&mut reopened, &invitation, &ad, BROWSER_KEY);
    assert!(reopened
        .browser_complete_outcome(&root, &stale, super::invited_tests::proof(&invitation, &ad))
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn occupied_receipt_slot_refuses_next_host_before_consuming_its_invitation() {
    let (mut owner, root, _) = setup();
    establish(&mut owner, &root);
    let (window, invitation, ad) = start(
        &mut owner,
        &root,
        60,
        "host/browser-test",
        "boot/browser/signed",
    );
    owner
        .browser_complete(
            &root,
            &window,
            super::invited_tests::proof(&invitation, &ad),
        )
        .unwrap();
    owner.browser_cancel_window(&root, &window).unwrap();
    let (next, invitation, ad) = start(
        &mut owner,
        &root,
        60,
        "host/another-browser",
        "boot/browser/other",
    );
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    assert!(owner
        .browser_complete(&root, &next, super::invited_tests::proof(&invitation, &ad))
        .err()
        .unwrap()
        .contains("slot is occupied"));
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn altered_recovery_key_offer_claim_or_proof_never_reissues_custody() {
    for mode in 0..4 {
        let (mut owner, root, _) = setup();
        establish(&mut owner, &root);
        let (window, mut invitation, mut ad) = start(
            &mut owner,
            &root,
            60,
            "host/browser-test",
            "boot/browser/signed",
        );
        owner
            .browser_complete(
                &root,
                &window,
                super::invited_tests::proof(&invitation, &ad),
            )
            .unwrap();
        owner.browser_cancel_window(&root, &window).unwrap();
        let before = owner.session.evidence().clone();
        let key = if mode == 0 {
            ed25519_dalek::SigningKey::from_bytes(&[8; 32])
                .verifying_key()
                .to_bytes()
        } else {
            BROWSER_KEY
        };
        if mode == 1 {
            ad.profile = "profile/changed-offer".into();
        }
        if mode == 2 {
            invitation.claim.expires_at_millis += 1;
        }
        let window = challenge(&mut owner, &invitation, &ad, key);
        let mut frame = super::invited_tests::proof(&invitation, &ad);
        if mode == 3 {
            if let In::SpawnProof { signature, .. } = &mut frame {
                signature[0] ^= 1;
            }
        }
        assert!(owner.browser_complete(&root, &window, frame).is_err());
        assert_eq!(owner.session.evidence(), &before);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn persistence_refusal_releases_no_receipt_and_valid_same_window_retry_commits_once() {
    let (mut owner, root, _) = setup();
    let pin = establish(&mut owner, &root);
    let (window, invitation, ad) = start(
        &mut owner,
        &root,
        60,
        "host/browser-test",
        "boot/browser/signed",
    );
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    std::fs::create_dir(root.join("body/owner-transaction.tmp")).unwrap();
    assert!(owner
        .browser_complete(
            &root,
            &window,
            super::invited_tests::proof(&invitation, &ad)
        )
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert!(!root.join("body/root-admission-receipt.json").exists());
    std::fs::remove_dir(root.join("body/owner-transaction.tmp")).unwrap();
    let result = owner
        .browser_complete(
            &root,
            &window,
            super::invited_tests::proof(&invitation, &ad),
        )
        .unwrap();
    result
        .signed_membership_receipt
        .unwrap()
        .verify(&pin, &request(&invitation, &ad))
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn interrupted_consume_journal_replays_original_outcome_before_any_signing() {
    let (mut owner, root, _) = setup();
    establish(&mut owner, &root);
    let (window, invitation, ad) = start(
        &mut owner,
        &root,
        60,
        "host/browser-test",
        "boot/browser/signed",
    );
    let paths = [
        "installation.json",
        "body/biography.json",
        "body/admission.json",
    ];
    let previous: Vec<_> = paths
        .iter()
        .map(|p| std::fs::read(root.join(p)).unwrap())
        .collect();
    let signed = owner
        .browser_complete(
            &root,
            &window,
            super::invited_tests::proof(&invitation, &ad),
        )
        .unwrap()
        .signed_membership_receipt
        .unwrap();
    let draft = crate::durable_host::root_signer::receipt::load(
        &root,
        owner.session.evidence().body_id.as_str(),
        "host/owner-test",
    )
    .unwrap()
    .unwrap();
    let journal = serde_json::json!({"schema":"conduit.body/owner-transaction@1", "biography":owner.session.evidence(),
        "installation":crate::durable_host::read_installation(&root.join("installation.json")).unwrap(),
        "root_signer":crate::durable_host::root_signer::selected(&root,owner.session.evidence().body_id.as_str(),"host/owner-test").unwrap(),
        "root_receipt":draft,"admissions":owner.admissions,"last_execution":null,"execution_custody":null,"source":null,"archives":[]});
    // Simulated cut before the transaction's materialized publication, preserving
    // its real canonical accepted manager/draft as the recoverable journal.
    for (path, bytes) in paths.iter().zip(previous) {
        std::fs::write(root.join(path), bytes).unwrap();
    }
    std::fs::remove_file(root.join("body/root-admission-receipt.json")).unwrap();
    crate::durable_host::write_json_atomic(&root.join("body/owner-transaction.json"), &journal)
        .unwrap();
    assert!(crate::durable_host::root_signer::receipt::sign_recovered(
        &root,
        &draft,
        owner.session.evidence(),
        owner.admissions.as_ref().unwrap()
    )
    .is_err());
    crate::durable_host::owner::state::recover(&root).unwrap();
    let again = crate::durable_host::root_signer::receipt::sign_recovered(
        &root,
        &draft,
        owner.session.evidence(),
        owner.admissions.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(again, signed);
    std::fs::remove_dir_all(root).unwrap();
}
