use super::*;
fn begin(
    owner: &mut Owner,
    root: &Path,
) -> (String, conduit_body::PortableInvitation, HostAdvertisement) {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let authorization = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let advertisement = host("host/browser-test", "boot/browser/invited")
        .advertisement()
        .clone();
    let frame = owner
        .browser_begin(
            &authorization.window_id,
            &"line/invited".into(),
            In::InvitedAdvertise {
                protocol: PROTOCOL,
                advertisement: advertisement.clone(),
                claim: invitation.claim.clone(),
                verifying_key: BROWSER_KEY.to_vec(),
            },
            512,
        )
        .unwrap();
    assert!(matches!(frame, Out::SpawnChallenge { claim, .. } if claim == invitation.claim));
    (authorization.window_id, invitation, advertisement)
}
fn proof(invitation: &conduit_body::PortableInvitation, host: &HostAdvertisement) -> In {
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    In::SpawnProof {
        protocol: PROTOCOL,
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        nonce: invitation.claim.nonce.to_vec(),
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &host.host_id,
                &host.boot_id,
                host.offer_generation,
            ))
            .to_vec(),
    }
}
#[test]
fn invited_browser_gets_existing_active_line_grants_only_after_durable_admission() {
    let (mut owner, root, resident) = setup();
    let (window, invitation, advertisement) = begin(&mut owner, &root);
    let before = owner.session.evidence().membership.parts.len();
    let snapshot = owner
        .browser_complete(&root, &window, proof(&invitation, &advertisement))
        .unwrap();
    assert_eq!(owner.session.evidence().membership.parts.len(), before + 1);
    assert_eq!(
        snapshot.browser_advertisement.as_deref(),
        Some(&advertisement)
    );
    assert_eq!(
        snapshot
            .line_authorization
            .as_ref()
            .unwrap()
            .carrier_binding
            .as_str(),
        "line/invited"
    );
    assert!(owner
        .browser_complete(&root, &window, proof(&invitation, &advertisement))
        .is_err());
    // Reopen reads the durable consumed invitation/Part, not merely Active cache.
    owner.browser_cancel_window(&root, &window).unwrap();
    let retained = crate::durable_host::owner::state::load(&root)
        .unwrap()
        .unwrap();
    assert_eq!(retained.membership.parts.len(), before + 1);
    drop(owner);
    let mut reopened = Owner::open(
        host("host/owner-test", "boot/owner/second"),
        resident,
        Some(retained),
        "ignored",
    )
    .unwrap();
    reopened.restore_execution(&root).unwrap();
    assert!(reopened
        .admissions
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|r| r.credential.matches(&snapshot.credential)));
    let replay_window = reopened
        .browser_authorize_window("host/browser-other", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let replay_offer = host("host/browser-other", "boot/browser/replay")
        .advertisement()
        .clone();
    reopened
        .browser_begin(
            &replay_window.window_id,
            &"line/replay".into(),
            In::InvitedAdvertise {
                protocol: PROTOCOL,
                advertisement: replay_offer.clone(),
                claim: invitation.claim.clone(),
                verifying_key: BROWSER_KEY.to_vec(),
            },
            512,
        )
        .unwrap();
    let before_replay = reopened.session.evidence().membership.clone();
    assert!(reopened
        .browser_complete(
            &root,
            &replay_window.window_id,
            proof(&invitation, &replay_offer)
        )
        .is_err());
    assert_eq!(reopened.session.evidence().membership, before_replay);

    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn altered_nonce_boot_or_signature_retires_window_without_membership_or_grant() {
    for mode in 0..3 {
        let (mut owner, root, _) = setup();
        let (window, invitation, advertisement) = begin(&mut owner, &root);
        let before = owner.session.evidence().membership.clone();
        let mut frame = proof(&invitation, &advertisement);
        if let In::SpawnProof {
            boot_id,
            nonce,
            signature,
            ..
        } = &mut frame
        {
            match mode {
                0 => *boot_id = "boot/foreign".into(),
                1 => nonce[0] ^= 1,
                _ => signature[0] ^= 1,
            }
        }
        assert!(owner.browser_complete(&root, &window, frame).is_err());
        assert_eq!(owner.session.evidence().membership, before);
        assert!(matches!(
            owner.pending_browser.as_ref().unwrap().state,
            WindowState::Pending(_)
        ));
        assert!(
            owner
                .pending_browser
                .as_ref()
                .unwrap()
                .invited_proof_refused
        );
        owner.browser_abort(&window).unwrap();
        assert!(owner
            .browser_complete(&root, &window, proof(&invitation, &advertisement))
            .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn persistence_refusal_and_expired_window_never_publish_active_grant() {
    let (mut owner, root, _) = setup();
    let (window, invitation, advertisement) = begin(&mut owner, &root);
    let before = owner.session.evidence().membership.clone();
    let blocked = root.join("not-a-directory");
    std::fs::write(&blocked, b"blocked").unwrap();
    assert!(owner
        .browser_complete(&blocked, &window, proof(&invitation, &advertisement))
        .is_err());
    assert_eq!(owner.session.evidence().membership, before);
    assert!(matches!(
        owner.pending_browser.as_ref().unwrap().state,
        WindowState::Pending(_)
    ));
    owner.pending_browser.as_mut().unwrap().deadline = Instant::now() - Duration::from_millis(1);
    assert!(owner
        .browser_complete(&root, &window, proof(&invitation, &advertisement))
        .is_err());
    assert_eq!(owner.session.evidence().membership, before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn first_invited_frame_cannot_change_authorized_host_key_body_or_expiry() {
    for mode in 0..4 {
        let (mut owner, root, _) = setup();
        let invitation = owner.issue_invitation(&root, 60, None).unwrap();
        let window = owner
            .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
            .unwrap();
        let mut advertisement = host("host/browser-test", "boot/browser/invited")
            .advertisement()
            .clone();
        let mut claim = invitation.claim.clone();
        let mut verifying_key = BROWSER_KEY.to_vec();
        match mode {
            0 => advertisement.host_id = "host/foreign".into(),
            1 => verifying_key[0] ^= 1,
            2 => claim.body_id = serde_json::from_str("\"body/foreign\"").unwrap(),
            _ => claim.expires_at_millis = 0,
        }
        let before = owner.session.evidence().membership.clone();
        assert!(owner
            .browser_begin(
                &window.window_id,
                &"line/test".into(),
                In::InvitedAdvertise {
                    protocol: PROTOCOL,
                    advertisement,
                    claim,
                    verifying_key
                },
                512
            )
            .is_err());
        assert_eq!(owner.session.evidence().membership, before);
        assert!(matches!(
            owner.pending_browser.as_ref().unwrap().state,
            WindowState::Ready
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
