use super::*;

fn frame(invitation: &conduit_body::PortableInvitation, ad: HostAdvertisement) -> In {
    In::InvitedAdvertise {
        protocol: PROTOCOL,
        advertisement: ad,
        claim: invitation.claim.clone(),
        verifying_key: BROWSER_KEY.to_vec(),
    }
}

#[test]
fn fresh_scope_is_exactly_issued_claim_and_freezes_first_finite_endpoint() {
    let (mut owner, root, _) = setup();
    let (invitation, authorization) = owner
        .browser_authorize_invitation_window(&root, 10_000)
        .unwrap();
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    assert!(owner
        .browser_authorize_invitation_window(&root, 10_000)
        .is_err());
    let ad = host("host/fresh-browser", "boot/fresh-browser")
        .advertisement()
        .clone();
    let mut foreign = invitation.clone();
    foreign.claim.nonce[0] ^= 1;
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/fresh".into(),
            frame(&foreign, ad.clone()),
            512
        )
        .is_err());
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/fresh".into(),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement: ad.clone(),
                friendly_label: "not invited".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512
        )
        .is_err());
    owner
        .browser_begin(
            &authorization.window_id,
            &"line/fresh".into(),
            frame(&invitation, ad.clone()),
            512,
        )
        .unwrap();
    let frozen = owner.pending_browser.as_ref().unwrap();
    assert_eq!(frozen.expected.as_ref(), Some(&ad.host_id));
    assert_eq!(frozen.new_key, Some(BROWSER_KEY));
    let WindowState::Pending(pending) = &frozen.state else {
        panic!("missing pending observation")
    };
    assert_eq!(pending.observation.advertisement, ad);
    let competing = host("host/competing-browser", "boot/competing")
        .advertisement()
        .clone();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/competing".into(),
            frame(&invitation, competing.clone()),
            512
        )
        .is_err());
    assert!(owner
        .browser_complete(
            &root,
            &authorization.window_id,
            super::invited_tests::proof(&invitation, &competing)
        )
        .is_err());
    assert!(owner
        .browser_complete(
            &root,
            &authorization.window_id,
            super::invited_tests::proof(&invitation, &ad)
        )
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scope_expiry_invalid_observation_and_failed_persistence_publish_no_fresh_binding() {
    let (mut owner, root, _) = setup();
    assert!(owner
        .browser_authorize_invitation_window(&root, 1001)
        .is_err());
    let before = owner.admissions.clone();
    std::fs::create_dir(root.join("body/owner-transaction.tmp")).unwrap();
    assert!(owner
        .browser_authorize_invitation_window(&root, 10_000)
        .is_err());
    assert!(owner.pending_browser.is_none());
    assert_eq!(owner.admissions, before);
    std::fs::remove_dir(root.join("body/owner-transaction.tmp")).unwrap();
    let (invitation, authorization) = owner
        .browser_authorize_invitation_window(&root, 10_000)
        .unwrap();
    let ad = host("host/fresh", "boot/fresh").advertisement().clone();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/fresh".into(),
            frame(&invitation, ad.clone()),
            u32::MAX
        )
        .is_err());
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    owner.pending_browser.as_mut().unwrap().deadline = Instant::now();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/fresh".into(),
            frame(&invitation, ad),
            512
        )
        .is_err());
    assert!(owner.pending_browser.as_ref().unwrap().expected.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn make_preparation_checks_selected_root_before_issue_and_exports_public_fields_only() {
    let (mut owner, root, _) = setup();
    let before = owner.admissions.clone();
    assert!(owner.browser_prepare_spore_window(&root, 10_000).is_err());
    assert_eq!(owner.admissions, before);
    assert!(owner.pending_browser.is_none());
    let body = owner.session.evidence().body_id.clone();
    let selected =
        crate::durable_host::owner::establish_root_signer(&mut owner, &root, body.as_str())
            .unwrap();
    let (invitation, authority, window) =
        owner.browser_prepare_spore_window(&root, 10_000).unwrap();
    assert_eq!(authority.public_key, selected.public_key);
    assert_eq!(authority.body_id, invitation.claim.body_id);
    assert_eq!(window.body_id, invitation.claim.body_id);
    let BrowserWindowScope::InvitationScope { claim, .. } =
        &owner.pending_browser.as_ref().unwrap().scope
    else {
        panic!("missing scope")
    };
    assert_eq!(claim, &invitation.claim);
    let encoded = serde_json::to_value(&authority).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 4);
    assert!(encoded.get("seed").is_none());
    assert!(encoded.get("private_record_sha256").is_none());
    owner
        .browser_cancel_window(&root, &window.window_id)
        .unwrap();
    std::fs::remove_file(root.join("body/root-signer-private.json")).unwrap();
    let before = owner.admissions.clone();
    assert!(owner.browser_prepare_spore_window(&root, 10_000).is_err());
    assert_eq!(owner.admissions, before);
    assert!(owner.pending_browser.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn transport_abort_retries_only_original_frozen_boot_generation_offer_and_deadline() {
    let (mut owner, root, _) = setup();
    let (invitation, authorization) = owner
        .browser_authorize_invitation_window(&root, 10_000)
        .unwrap();
    let ad = host("host/fresh-retry", "boot/fresh-retry")
        .advertisement()
        .clone();
    owner
        .browser_begin(
            &authorization.window_id,
            &"line/first".into(),
            frame(&invitation, ad.clone()),
            512,
        )
        .unwrap();
    let deadline = owner.pending_browser.as_ref().unwrap().deadline;
    owner.browser_abort(&authorization.window_id).unwrap();
    let mut changed = ad.clone();
    changed.boot_id = "boot/changed".into();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/changed".into(),
            frame(&invitation, changed),
            512
        )
        .is_err());
    let mut changed = ad.clone();
    changed.offer_generation.0 += 1;
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/changed".into(),
            frame(&invitation, changed),
            512
        )
        .is_err());
    let mut changed = ad.clone();
    changed.capabilities.pop();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/changed".into(),
            frame(&invitation, changed),
            512
        )
        .is_err());
    owner
        .browser_begin(
            &authorization.window_id,
            &"line/retry".into(),
            frame(&invitation, ad.clone()),
            512,
        )
        .unwrap();
    assert_eq!(owner.pending_browser.as_ref().unwrap().deadline, deadline);
    assert_eq!(
        crate::durable_host::root_signer::receipt::offered_digest(&ad).unwrap(),
        crate::durable_host::digest(&serde_json::to_vec(&ad).unwrap())
    );
    owner.browser_abort(&authorization.window_id).unwrap();
    owner.pending_browser.as_mut().unwrap().deadline = Instant::now();
    assert!(owner
        .browser_begin(
            &authorization.window_id,
            &"line/late".into(),
            frame(&invitation, ad),
            512
        )
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
